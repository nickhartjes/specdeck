//! What the status bar shows besides token usage (design decision 10a): Claude Code's status-line
//! data, relayed by a small script that also runs the host's own status line, and the git state of
//! the session folder.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use specdeck_protocol::timeline::{Entry, RateLimit};

use crate::environment::Environment;
use crate::git;

/// Environment variable with the host's own status-line command, run by the relay script.
pub const HOST_STATUS_LINE_ENV: &str = "SPECDECK_STATUSLINE_COMMAND";

/// Posts `$input` to SpecDeck with the hook token, passed to `curl` through a config on a file
/// descriptor so it never shows in the process list. Without `curl` it does nothing.
fn relay_post(status_url: &str) -> String {
    format!(
        r#"input=$(cat)
if command -v curl >/dev/null 2>&1; then
  printf '%s' "$input" | curl -s -m 1 -o /dev/null -X POST -K /dev/fd/3 \
    -H 'Content-Type: application/json' --data-binary @- '{status_url}' >/dev/null 2>&1 3<<EOF
header = "Authorization: Bearer $SPECDECK_HOOK_TOKEN"
EOF
fi
"#
    )
}

/// The relay script. It posts its input to SpecDeck, then runs the host's own status line with the
/// same input. Without `curl` only the host's status line runs.
fn relay_script(status_url: &str) -> String {
    format!(
        r#"#!/bin/sh
# SpecDeck status-line relay. Shows the host's own status line unchanged.
{post}if [ -n "${HOST_STATUS_LINE_ENV}" ]; then
  if command -v bash >/dev/null 2>&1; then
    printf '%s' "$input" | bash -c "${HOST_STATUS_LINE_ENV}"
  else
    printf '%s' "$input" | sh -c "${HOST_STATUS_LINE_ENV}"
  fi
fi
"#,
        post = relay_post(status_url),
        HOST_STATUS_LINE_ENV = HOST_STATUS_LINE_ENV,
    )
}

/// Writes the relay script to a user-only file in `dir`, deleted when the handle drops.
pub fn write_relay_script(
    dir: &Path,
    status_url: &str,
) -> std::io::Result<tempfile::NamedTempFile> {
    use std::io::Write;
    let mut file = tempfile::Builder::new().prefix("statusline-").suffix(".sh").tempfile_in(dir)?;
    file.write_all(relay_script(status_url).as_bytes())?;
    file.flush()?;
    Ok(file)
}

/// The `statusLine` setting that runs the relay, keeping the host's padding.
pub fn status_line_setting(script: &Path, host: Option<&Value>) -> Value {
    let quoted = script.to_string_lossy().replace('\'', r"'\''");
    let mut setting = json!({ "type": "command", "command": format!("sh '{quoted}'") });
    if let Some(padding) = host.and_then(|host| host.get("padding")) {
        setting["padding"] = padding.clone();
    }
    setting
}

/// The `statusLine` setting for Claude Code started through a start command (`agent-sandbox`
/// design decision 2): the relay inline, since a sandbox cannot read a script in the session's
/// data folder. The host's own status line cannot run there, so the line stays empty.
pub fn inline_status_line_setting(status_url: &str) -> Value {
    json!({ "type": "command", "command": relay_post(status_url) })
}

/// The status line the host set up for Claude Code, in the order Claude Code reads its settings:
/// the folder's local and shared project settings, then the user's.
pub fn host_status_line(folder: &Path, env: &Environment) -> Option<Value> {
    let user_dir =
        env.get("CLAUDE_CONFIG_DIR").filter(|dir| !dir.is_empty()).map(PathBuf::from).or_else(
            || {
                let home = env.get("HOME").or_else(|| env.get("USERPROFILE"))?;
                Some(PathBuf::from(home).join(".claude"))
            },
        );
    let files = [
        Some(folder.join(".claude").join("settings.local.json")),
        Some(folder.join(".claude").join("settings.json")),
        user_dir.map(|dir| dir.join("settings.json")),
    ];
    files.into_iter().flatten().find_map(|file| {
        let settings: Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
        let line = settings.get("statusLine")?;
        line.get("command").and_then(Value::as_str).filter(|c| !c.trim().is_empty())?;
        Some(line.clone())
    })
}

/// The host's status-line command, from [`host_status_line`].
pub fn host_command(line: &Value) -> Option<&str> {
    line.get("command").and_then(Value::as_str)
}

/// Turns relayed status-line payloads into entries, only when what they show changed.
#[derive(Default)]
pub struct StatusLineReader {
    last: Option<Entry>,
}

impl StatusLineReader {
    pub fn on_payload(&mut self, payload: &Map<String, Value>) -> Option<Entry> {
        let entry = status_line_entry(payload);
        if self.last.as_ref() == Some(&entry) {
            return None;
        }
        self.last = Some(entry.clone());
        Some(entry)
    }
}

fn status_line_entry(payload: &Map<String, Value>) -> Entry {
    let payload = Value::Object(payload.clone());
    let at = |pointer: &str| payload.pointer(pointer);
    let text = |pointer: &str| at(pointer).and_then(Value::as_str).filter(|s| !s.is_empty());
    let number = |pointer: &str| at(pointer).and_then(Value::as_f64).filter(|n| *n >= 0.0);

    // "Opus 5.5 (1M context)": the window is shown separately.
    let model = text("/model/display_name").or_else(|| text("/model/id")).map(|name| {
        match name.find(" (") {
            Some(i) if name.ends_with(" context)") => name[..i].to_owned(),
            _ => name.to_owned(),
        }
    });
    let context_tokens = at("/context_window/current_usage").filter(|v| v.is_object()).map(|_| {
        ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
            .iter()
            .filter_map(|field| number(&format!("/context_window/current_usage/{field}")))
            .map(|n| n as u64)
            .sum()
    });
    let limit = |window: &str| {
        let used = number(&format!("/rate_limits/{window}/used_percentage"))?;
        Some(RateLimit {
            used_percent: used.round() as u32,
            resets_at: number(&format!("/rate_limits/{window}/resets_at")).map(|n| n as u64),
        })
    };
    Entry::StatusLine {
        model,
        context_window: number("/context_window/context_window_size").map(|n| n as u64),
        context_tokens,
        effort: text("/effort/level").map(str::to_owned),
        fast: at("/fast_mode").and_then(Value::as_bool).unwrap_or(false),
        five_hour: limit("five_hour"),
        seven_day: limit("seven_day"),
    }
}

/// The git state of `folder` as a `Git` entry; no branch when it is not in a repository or git is
/// not installed. `-uno` keeps it cheap in large repositories, as the host's own status line does.
pub fn git_entry(folder: &Path, env: &Environment) -> Entry {
    let mut branch = None;
    let (mut dirty, mut ahead, mut behind) = (false, 0, 0);
    let mut worktree = None;
    let folder_name = folder.file_name().map_or_else(
        || folder.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let Some(git) = env.find_executable("git") else {
        return Entry::Git { folder: folder_name, branch, dirty, ahead, behind, worktree };
    };
    // `status` may run a clean filter to compare a file, so the folder's filters are switched off.
    // A submodule's own settings are not read here, so `status` does not look into submodules.
    let no_filters = git::no_filters(&git, env, folder);
    let run = |args: &[&str]| {
        let mut command = git::command(&git, env, folder);
        command.args(&no_filters).args(args);
        let output = command.output().ok().filter(|output| output.status.success())?;
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    };

    if let Some(status) = run(&[
        "--no-optional-locks",
        "status",
        "--porcelain=v2",
        "--branch",
        "-uno",
        "--ignore-submodules=all",
    ]) {
        for line in status.lines() {
            if let Some(head) = line.strip_prefix("# branch.head ") {
                branch = Some(if head == "(detached)" { "detached".into() } else { head.into() });
            } else if let Some(counts) = line.strip_prefix("# branch.ab ") {
                let mut parts = counts.split_whitespace();
                ahead =
                    parts.next().and_then(|n| n.trim_start_matches('+').parse().ok()).unwrap_or(0);
                behind =
                    parts.next().and_then(|n| n.trim_start_matches('-').parse().ok()).unwrap_or(0);
            } else if !line.starts_with('#') && !line.is_empty() {
                dirty = true;
                break;
            }
        }
    }
    if branch.is_some()
        && let Some(dirs) = run(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
        ])
    {
        let lines: Vec<&str> = dirs.lines().collect();
        if let [git_dir, common_dir, top] = lines[..]
            && git_dir != common_dir
        {
            worktree = Path::new(top).file_name().map(|name| name.to_string_lossy().into_owned());
        }
    }
    Entry::Git { folder: folder_name, branch, dirty, ahead, behind, worktree }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn reads_what_the_status_line_gets() {
        let payload = object(json!({
            "model": { "id": "claude-opus-5-5[1m]", "display_name": "Opus 5.5 (1M context)" },
            "effort": { "level": "medium" },
            "fast_mode": false,
            "context_window": {
                "context_window_size": 1_000_000,
                "current_usage": { "input_tokens": 2, "cache_creation_input_tokens": 300, "cache_read_input_tokens": 177_000, "output_tokens": 90 }
            },
            "rate_limits": {
                "five_hour": { "used_percentage": 23.4, "resets_at": 1_793_500_000 },
                "seven_day": { "used_percentage": 40.6 }
            }
        }));
        assert_eq!(
            status_line_entry(&payload),
            Entry::StatusLine {
                model: Some("Opus 5.5".into()),
                context_window: Some(1_000_000),
                context_tokens: Some(177_302),
                effort: Some("medium".into()),
                fast: false,
                five_hour: Some(RateLimit { used_percent: 23, resets_at: Some(1_793_500_000) }),
                seven_day: Some(RateLimit { used_percent: 41, resets_at: None }),
            }
        );
    }

    #[test]
    fn leaves_out_what_was_not_reported() {
        let payload = object(
            json!({ "model": { "id": "claude-sonnet-5-5" }, "rate_limits": { "five_hour": { "used_percentage": "x" } } }),
        );
        assert_eq!(
            status_line_entry(&payload),
            Entry::StatusLine {
                model: Some("claude-sonnet-5-5".into()),
                context_window: None,
                context_tokens: None,
                effort: None,
                fast: false,
                five_hour: None,
                seven_day: None,
            }
        );
    }

    #[test]
    fn emits_only_changes() {
        let mut reader = StatusLineReader::default();
        let payload = object(json!({ "model": { "display_name": "Sonnet 5.5" } }));
        assert!(reader.on_payload(&payload).is_some());
        assert!(reader.on_payload(&payload).is_none());
        let changed =
            object(json!({ "model": { "display_name": "Sonnet 5.5" }, "fast_mode": true }));
        assert!(reader.on_payload(&changed).is_some());
    }

    #[test]
    fn finds_the_hosts_status_line_in_settings_order() {
        let home = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let env: Environment = [("HOME", home.path())].into_iter().collect();
        assert_eq!(host_status_line(folder.path(), &env), None);

        std::fs::create_dir_all(home.path().join(".claude")).unwrap();
        std::fs::write(
            home.path().join(".claude/settings.json"),
            r#"{"statusLine":{"type":"command","command":"user-line","padding":0}}"#,
        )
        .unwrap();
        let line = host_status_line(folder.path(), &env).unwrap();
        assert_eq!(host_command(&line), Some("user-line"));

        std::fs::create_dir_all(folder.path().join(".claude")).unwrap();
        std::fs::write(
            folder.path().join(".claude/settings.local.json"),
            r#"{"statusLine":{"type":"command","command":"local-line"}}"#,
        )
        .unwrap();
        assert_eq!(
            host_command(&host_status_line(folder.path(), &env).unwrap()),
            Some("local-line")
        );

        let setting = status_line_setting(Path::new("/data/it's here.sh"), Some(&line));
        assert_eq!(setting["command"], r"sh '/data/it'\''s here.sh'");
        assert_eq!(setting["padding"], 0);
    }

    /// A one-request server that records the headers and body the relay sends to `/status`.
    #[cfg(unix)]
    fn status_server() -> (String, std::thread::JoinHandle<(String, String)>) {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/status", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut headers = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(n) = line.to_ascii_lowercase().strip_prefix("content-length: ") {
                    length = n.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
                headers.push_str(&line);
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            (&stream).write_all(b"HTTP/1.1 204 No Content\r\n\r\n").unwrap();
            (headers, String::from_utf8(body).unwrap())
        });
        (url, server)
    }

    #[cfg(unix)]
    #[test]
    fn relay_posts_its_input_and_still_shows_the_hosts_line() {
        use std::io::Write;

        let (url, server) = status_server();
        let dir = tempfile::tempdir().unwrap();
        let script = write_relay_script(dir.path(), &url).unwrap();
        let mut child = Command::new("sh")
            .arg(script.path())
            .env("SPECDECK_HOOK_TOKEN", "secret")
            .env(HOST_STATUS_LINE_ENV, r#"read -r line; echo "host line got $line""#)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(br#"{"model":{"id":"m"}}"#).unwrap();
        let output = child.wait_with_output().unwrap();

        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "host line got {\"model\":{\"id\":\"m\"}}\n"
        );
        let (headers, body) = server.join().unwrap();
        assert!(headers.contains("Authorization: Bearer secret"), "{headers}");
        assert_eq!(body, r#"{"model":{"id":"m"}}"#);
    }

    /// Claude Code runs a status-line command through a shell; inline, the relay needs no file.
    #[cfg(unix)]
    #[test]
    fn the_inline_relay_posts_its_input_with_the_token_from_the_environment() {
        use std::io::Write;

        let (url, server) = status_server();
        let setting = inline_status_line_setting(&url);
        assert_eq!(setting["type"], "command");
        let command = setting["command"].as_str().unwrap();
        assert!(!command.contains("secret"));
        let mut child = Command::new("sh")
            .args(["-c", command])
            .env("SPECDECK_HOOK_TOKEN", "secret")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(br#"{"model":{"id":"m"}}"#).unwrap();
        let output = child.wait_with_output().unwrap();

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
        let (headers, body) = server.join().unwrap();
        assert!(headers.contains("Authorization: Bearer secret"), "{headers}");
        assert_eq!(body, r#"{"model":{"id":"m"}}"#);
    }

    #[cfg(unix)]
    #[test]
    fn git_state_of_the_folder() {
        let env = Environment::from_process();
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(git_entry(dir.path(), &env), Entry::Git { branch: None, .. }));

        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(args)
                .current_dir(dir.path())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join("a.txt"), "one").unwrap();
        git(&["add", "a.txt"]);
        git(&["commit", "-q", "-m", "first"]);
        assert_eq!(
            git_entry(dir.path(), &env),
            Entry::Git {
                folder: dir.path().file_name().unwrap().to_string_lossy().into(),
                branch: Some("main".into()),
                dirty: false,
                ahead: 0,
                behind: 0,
                worktree: None
            }
        );

        std::fs::write(dir.path().join("a.txt"), "two").unwrap();
        assert!(matches!(git_entry(dir.path(), &env), Entry::Git { dirty: true, .. }));

        // Ahead of an upstream (a local branch will do).
        git(&["branch", "base"]);
        git(&["commit", "-q", "-am", "second"]);
        git(&["branch", "-q", "--set-upstream-to=base"]);
        assert!(matches!(
            git_entry(dir.path(), &env),
            Entry::Git { dirty: false, ahead: 1, behind: 0, .. }
        ));

        let linked = dir.path().join("wt-feature");
        git(&["worktree", "add", "-q", "-b", "feature", linked.to_str().unwrap()]);
        assert_eq!(
            git_entry(&linked, &env),
            Entry::Git {
                folder: "wt-feature".into(),
                branch: Some("feature".into()),
                dirty: false,
                ahead: 0,
                behind: 0,
                worktree: Some("wt-feature".into())
            }
        );
    }

    /// A test repository whose settings would leave `marker` when SpecDeck's git ran them.
    #[cfg(unix)]
    pub(crate) fn repository_with_a_trap(dir: &Path, marker: &Path) -> PathBuf {
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(args)
                .current_dir(dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("a.txt"), "one").unwrap();
        git(&["add", "a.txt"]);
        git(&["commit", "-q", "-m", "first"]);
        let trap = dir.join(".git/trap.sh");
        std::fs::write(&trap, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&trap, std::fs::Permissions::from_mode(0o755)).unwrap();
        trap
    }

    #[cfg(unix)]
    #[test]
    fn the_folder_s_file_system_monitor_does_not_run() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let trap = repository_with_a_trap(dir.path(), &marker);
        let status = Command::new("git")
            .args(["config", "core.fsmonitor", &trap.to_string_lossy()])
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(status.success());
        // Touched since the index was written, so git asks the monitor.
        std::fs::write(dir.path().join("a.txt"), "two").unwrap();
        let entry = git_entry(dir.path(), &Environment::from_process());
        assert!(
            matches!(entry, Entry::Git { branch: Some(ref b), dirty: true, .. } if b == "main")
        );
        assert!(!marker.exists(), "the monitor ran");
    }

    /// A submodule's own settings name a clean filter; SpecDeck's `git status` does not look into
    /// the submodule, so the filter does not run.
    #[cfg(unix)]
    #[test]
    fn a_submodule_s_filter_does_not_run() {
        let base = tempfile::tempdir().unwrap();
        let git = |dir: &Path, args: &[&str]| {
            let status = Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(["-c", "protocol.file.allow=always"])
                .args(args)
                .current_dir(dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        let lib = base.path().join("lib");
        std::fs::create_dir(&lib).unwrap();
        git(&lib, &["init", "-q"]);
        std::fs::write(lib.join("a.txt"), "a").unwrap();
        git(&lib, &["add", "."]);
        git(&lib, &["commit", "-q", "-m", "lib"]);
        let project = base.path().join("project");
        std::fs::create_dir(&project).unwrap();
        git(&project, &["init", "-q", "-b", "main"]);
        git(&project, &["submodule", "add", "-q", &lib.to_string_lossy(), "vendor/lib"]);
        git(&project, &["commit", "-q", "-m", "submodule"]);

        let marker = base.path().join("ran");
        let trap = base.path().join("trap.sh");
        std::fs::write(&trap, format!("#!/bin/sh\ncat >/dev/null\ntouch '{}'\n", marker.display()))
            .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&trap, std::fs::Permissions::from_mode(0o755)).unwrap();
        let sub = project.join("vendor/lib");
        git(&sub, &["config", "filter.trap.clean", &trap.to_string_lossy()]);
        std::fs::write(sub.join(".gitattributes"), "*.txt filter=trap\n").unwrap();
        // Same size, newer time: git compares the content, through the filter.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(sub.join("a.txt"), "b").unwrap();

        let entry = git_entry(&project, &Environment::from_process());
        assert!(matches!(entry, Entry::Git { branch: Some(ref b), .. } if b == "main"));
        assert!(!marker.exists(), "the submodule's clean filter ran");
    }

    /// git for Windows takes the null device `NUL` as the hooks path, and runs no monitor.
    #[cfg(windows)]
    #[test]
    fn the_folder_s_file_system_monitor_does_not_run_on_windows() {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(args)
                .current_dir(dir.path())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join("a.txt"), "one").unwrap();
        git(&["add", "a.txt"]);
        git(&["commit", "-q", "-m", "first"]);
        let marker = dir.path().join("ran");
        let trap = dir.path().join(".git").join("trap.cmd");
        std::fs::write(&trap, format!("@echo off\r\ntype nul > \"{}\"\r\n", marker.display()))
            .unwrap();
        git(&["config", "core.fsmonitor", &trap.to_string_lossy()]);
        std::fs::write(dir.path().join("a.txt"), "two").unwrap();
        let entry = git_entry(dir.path(), &Environment::from_process());
        assert!(
            matches!(entry, Entry::Git { branch: Some(ref b), dirty: true, .. } if b == "main")
        );
        assert!(!marker.exists(), "the monitor ran");
    }
}
