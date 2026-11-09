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
        let process = Self::from_process();
        if cfg!(windows) {
            return process;
        }
        let Some(shell) = process.get("SHELL").filter(|shell| !shell.is_empty()) else {
            return process;
        };
        probe_login_shell(Path::new(&shell), timeout).unwrap_or(process)
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

/// Runs `shell` as an interactive login shell and reads the environment it ends up with.
fn probe_login_shell(shell: &Path, timeout: Duration) -> Option<Environment> {
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
    let mut child = command.spawn().ok()?;

    // Read on a separate thread so a large environment cannot fill the pipe and block the shell.
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output).map(|_| output)
    });

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
        }
    }
    parse_env_output(&reader.join().ok()?.ok()?)
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

    #[cfg(unix)]
    #[test]
    fn falls_back_when_the_shell_hangs() {
        let dir = tempfile::tempdir().unwrap();
        let shell = fake_shell(dir.path(), "sleep 10");
        let started = Instant::now();
        assert!(probe_login_shell(&shell, Duration::from_millis(300)).is_none());
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
            probe_login_shell(shell, PROBE_TIMEOUT)
        })
    }

    #[cfg(unix)]
    fn fake_shell(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-shell");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}
