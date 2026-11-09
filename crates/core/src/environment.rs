//! The environment an agent is started with.
//!
//! Apps started from the Finder, Dock or a desktop launcher do not inherit the `PATH` the user
//! set up in their shell profile, so on macOS and Linux the environment is read from the user's
//! login shell, falling back to the app's own environment (design decision 3).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::diag::{self, EnvOutcome, EnvSource, PathShape};

/// Printed before the environment so output from shell startup files can be skipped.
const MARKER: &str = "__SPECDECK_ENV__";

/// How long the login shell may take before falling back to the app's environment.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Environment variables for starting an agent. Serializable, so the app can pass the
/// environment it detected to a session process (`session-processes` design decision 2): as a
/// list of pairs, since JSON map keys must be strings and these are not always UTF-8.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Environment {
    #[serde(with = "pairs")]
    vars: BTreeMap<OsString, OsString>,
}

mod pairs {
    use std::collections::BTreeMap;
    use std::ffi::OsString;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        vars: &BTreeMap<OsString, OsString>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(vars)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<OsString, OsString>, D::Error> {
        let pairs = Vec::<(OsString, OsString)>::deserialize(deserializer)?;
        Ok(pairs.into_iter().collect())
    }
}

impl Environment {
    /// The app's own environment.
    pub fn from_process() -> Self {
        Self { vars: std::env::vars_os().collect() }
    }

    /// The user's login shell environment on macOS and Linux, or the app's own environment on
    /// Windows and when the shell cannot be read within `timeout`.
    pub fn detect(timeout: Duration) -> Self {
        Self::detect_from(Self::from_process(), timeout)
    }

    /// As [`detect`](Self::detect), with `process` as the app's own environment.
    fn detect_from(process: Self, timeout: Duration) -> Self {
        let shell = process.get("SHELL").filter(|shell| !shell.is_empty());
        let (env, source, outcome) = match shell {
            Some(shell) if !cfg!(windows) => match probe_login_shell(Path::new(&shell), timeout) {
                Ok(env) => (env, EnvSource::LoginShell, EnvOutcome::Ok),
                Err(outcome) => (process, EnvSource::Process, outcome),
            },
            _ => (process, EnvSource::Process, EnvOutcome::Skipped),
        };
        diag::environment_loaded(source, outcome, env.path_shape());
        env
    }

    /// Whether this environment has a `PATH` and how many entries it holds, for the log.
    pub fn path_shape(&self) -> PathShape {
        let exact = self.vars.contains_key(&OsString::from("PATH"));
        let path = self.get("PATH");
        PathShape {
            found: path.is_some(),
            case_only: path.is_some() && !exact,
            entries: path.map_or(0, |path| std::env::split_paths(&path).count()),
        }
    }

    /// The value of `name`. On Windows names are compared without case, as Windows does: its
    /// environment usually holds `Path`, which `get("PATH")` must find.
    pub fn get(&self, name: &str) -> Option<OsString> {
        if let Some(value) = self.vars.get(&OsString::from(name)) {
            return Some(value.clone());
        }
        if cfg!(windows) {
            return self
                .vars
                .iter()
                .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case(name))
                .map(|(_, value)| value.clone());
        }
        None
    }

    pub fn set(&mut self, name: impl Into<OsString>, value: impl Into<OsString>) {
        self.vars.insert(name.into(), value.into());
    }

    pub fn iter(&self) -> impl Iterator<Item = (&OsString, &OsString)> {
        self.vars.iter()
    }

    /// Finds an executable on this environment's `PATH`, as a shell would.
    pub fn find_executable(&self, name: &str) -> Option<PathBuf> {
        let path = self.get("PATH")?;
        let cwd = std::env::current_dir().ok()?;
        which::which_in(name, Some(path), cwd).ok()
    }
}

impl<K: Into<OsString>, V: Into<OsString>> FromIterator<(K, V)> for Environment {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self { vars: iter.into_iter().map(|(k, v)| (k.into(), v.into())).collect() }
    }
}

/// Runs `shell` as an interactive login shell and reads the environment it ends up with; else
/// whether it took too long or failed.
fn probe_login_shell(shell: &Path, timeout: Duration) -> Result<Environment, EnvOutcome> {
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c", &format!("printf '\\0{MARKER}\\0'; env -0")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // In a session of its own the shell has no terminal. An interactive shell otherwise takes
    // over the terminal the app was started from, as in development, and never gives it back:
    // Ctrl+C there then reaches nobody.
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            if libc::setsid() == -1 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
        });
    }
    let failed = |_| EnvOutcome::Failed;
    let mut child = command.spawn().map_err(failed)?;

    // Read on a separate thread so a large environment cannot fill the pipe and block the shell.
    let mut stdout = child.stdout.take().ok_or(EnvOutcome::Failed)?;
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output).map(|_| output)
    });

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return Err(EnvOutcome::Failed),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(EnvOutcome::Timeout);
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
        }
    }
    let output = reader.join().map_err(|_| EnvOutcome::Failed)?.map_err(failed)?;
    parse_env_output(&output).ok_or(EnvOutcome::Failed)
}

/// Parses `\0MARKER\0` followed by `env -0` output, ignoring anything printed before the marker.
fn parse_env_output(output: &[u8]) -> Option<Environment> {
    let marker = format!("\0{MARKER}\0");
    let start = output.windows(marker.len()).position(|w| w == marker.as_bytes())? + marker.len();
    let vars = output[start..]
        .split(|&b| b == 0)
        .filter_map(|entry| {
            let entry = String::from_utf8_lossy(entry);
            let (name, value) = entry.split_once('=')?;
            (!name.is_empty()).then(|| (name.to_owned(), value.to_owned()))
        })
        .collect::<Environment>();
    vars.get("PATH").is_some().then_some(vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survives_a_trip_through_json() {
        let mut env: Environment =
            [("PATH", "/usr/bin:/bin"), ("LANG", "nl_NL.UTF-8")].into_iter().collect();
        env.set("EMPTY", "");
        // A value that is not UTF-8, as a Unix environment may hold.
        #[cfg(unix)]
        env.set("RAW", <OsString as std::os::unix::ffi::OsStringExt>::from_vec(vec![0x66, 0xff]));
        let json = serde_json::to_string(&env).unwrap();
        assert_eq!(serde_json::from_str::<Environment>(&json).unwrap(), env);
    }

    #[test]
    fn names_are_compared_without_case_only_on_windows() {
        let env: Environment = [("Path", r"C:\Windows")].into_iter().collect();
        assert_eq!(env.get("Path").unwrap(), r"C:\Windows");
        assert_eq!(env.get("PATH").is_some(), cfg!(windows));
    }

    #[cfg(windows)]
    #[test]
    fn programs_are_found_through_windows_path() {
        // The test process's own environment, as the app has it: Windows spells it `Path`.
        let env = Environment::from_process();
        assert!(env.find_executable("cmd").is_some());
    }

    #[test]
    fn parses_env_after_marker_and_skips_startup_noise() {
        let output = b"Welcome!\n\0__SPECDECK_ENV__\0PATH=/opt/bin:/usr/bin\0EMPTY=\0WITH_EQ=a=b\0";
        let env = parse_env_output(output).unwrap();
        assert_eq!(env.get("PATH").unwrap(), "/opt/bin:/usr/bin");
        assert_eq!(env.get("EMPTY").unwrap(), "");
        assert_eq!(env.get("WITH_EQ").unwrap(), "a=b");
    }

    #[test]
    fn rejects_output_without_marker_or_path() {
        assert!(parse_env_output(b"PATH=/usr/bin\0").is_none());
        assert!(parse_env_output(b"\0__SPECDECK_ENV__\0HOME=/home/a\0").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn reads_the_login_shell_environment() {
        let dir = tempfile::tempdir().unwrap();
        let shell = fake_shell(dir.path(), "export PATH=/from/profile:$PATH; eval \"$4\"");
        let env = probe_fake_shell(&shell).unwrap();
        assert!(env.get("PATH").unwrap().to_string_lossy().starts_with("/from/profile:"));
    }

    #[cfg(unix)]
    #[test]
    fn the_login_shell_runs_in_a_session_of_its_own() {
        // A session leader leads its own process group; without one it would share the test's.
        let dir = tempfile::tempdir().unwrap();
        let shell = fake_shell(
            dir.path(),
            "export PATH=/usr/bin:/bin LEADS=$(ps -o pgid= -p $$ | tr -d ' ')-$$; eval \"$4\"",
        );
        let env = probe_fake_shell(&shell).unwrap();
        let leads = env.get("LEADS").unwrap().into_string().unwrap();
        let (group, pid) = leads.split_once('-').unwrap();
        assert_eq!(group, pid);
    }

    /// The `environment.loaded` event of detecting with `process`; never a name or a value.
    fn loaded(process: Environment, timeout: Duration) -> crate::capture::Captured {
        let (capture, _guard) = crate::capture::Capture::start();
        Environment::detect_from(process, timeout);
        let events = capture.named("environment.loaded");
        assert_eq!(events.len(), 1);
        let event = events[0].clone();
        let keys: Vec<&str> = event.fields.keys().map(String::as_str).collect();
        assert_eq!(keys, ["has_path", "outcome", "path_case_only", "path_entries", "source"]);
        assert!(event.fields.values().all(|value| !value.contains("arker")), "{event:?}");
        event
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_that_hangs_is_recorded_as_a_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let shell = fake_shell(dir.path(), "sleep 10");
        let process: Environment =
            [("SHELL", shell.to_str().unwrap()), ("PATH", "/usr/bin:/bin:/Marker-Path")]
                .into_iter()
                .collect();
        let event = loaded(process, Duration::from_millis(300));
        assert_eq!(event.field("source"), Some("process"));
        assert_eq!(event.field("outcome"), Some("timeout"));
        assert_eq!(event.field("has_path"), Some("true"));
        assert_eq!(event.field("path_entries"), Some("3"));
    }

    #[test]
    fn an_environment_without_path_is_recorded() {
        let process: Environment = [("HOME", "/Users/Marker-User")].into_iter().collect();
        let event = loaded(process, PROBE_TIMEOUT);
        assert_eq!(event.field("outcome"), Some("skipped"));
        assert_eq!(event.field("has_path"), Some("false"));
        assert_eq!(event.field("path_entries"), Some("0"));
        assert_eq!(event.level.as_str(), "WARN");
    }

    #[cfg(windows)]
    #[test]
    fn windows_path_is_found_only_without_case() {
        let process: Environment = [("Path", r"C:\Windows;C:\Marker")].into_iter().collect();
        let event = loaded(process, PROBE_TIMEOUT);
        assert_eq!(event.field("has_path"), Some("true"));
        assert_eq!(event.field("path_case_only"), Some("true"));
        assert_eq!(event.field("path_entries"), Some("2"));
    }

    #[cfg(unix)]
    #[test]
    fn falls_back_when_the_shell_hangs() {
        let dir = tempfile::tempdir().unwrap();
        let shell = fake_shell(dir.path(), "sleep 10");
        let started = Instant::now();
        assert_eq!(probe_login_shell(&shell, Duration::from_millis(300)), Err(EnvOutcome::Timeout));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    /// Probes `shell`, trying again a few times. Linux refuses to run a file while any process
    /// has it open for writing ("text file busy"), and a process another test starts at the
    /// moment `fake_shell` writes holds that handle until it execs. The app never writes the
    /// shell it probes.
    #[cfg(unix)]
    fn probe_fake_shell(shell: &Path) -> Option<Environment> {
        (0..5).find_map(|attempt| {
            if attempt > 0 {
                thread::sleep(Duration::from_millis(50));
            }
            probe_login_shell(shell, PROBE_TIMEOUT).ok()
        })
    }

    #[cfg(unix)]
    fn fake_shell(dir: &Path, body: &str) -> PathBuf {
        crate::test_support::fake_executable(dir, "fake-shell", body)
    }
}
