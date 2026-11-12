//! Native crashes on Windows (`diagnostics` spec, *Native crashes in diagnostics*): the debug
//! app, told to crash (`SPECDECK_CRASH`), ends with the exception's code, its log holds
//! `app.crashed` with that code, and the next start records the unexpected end.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

const APP: &str = env!("CARGO_BIN_EXE_specdeck-app");

/// A profile of its own per test, so its log folder starts empty.
fn profile(kind: &str) -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("crash-{kind}-{nanos}")
}

fn log_dir(profile: &str) -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA");
    Path::new(&local).join("nl.nickhartjes.specdeck").join("logs").join("profiles").join(profile)
}

/// The app's log lines so far.
fn lines(profile: &str) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(log_dir(profile)) else { return Vec::new() };
    let mut lines = Vec::new();
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with("specdeck.log.") {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            lines.extend(text.lines().filter_map(|line| serde_json::from_str(line).ok()));
        }
    }
    lines
}

fn start(profile: &str, crash: Option<&str>) -> Child {
    // Inherited by the app: Windows reports the crash without a dialog waiting for a click.
    // SAFETY: sets this process's error mode only.
    unsafe {
        use windows_sys::Win32::System::Diagnostics::Debug::{
            SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode,
        };
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
    let mut command = Command::new(APP);
    command.env("SPECDECK_PROFILE", profile).env_remove("SPECDECK_CRASH");
    if let Some(kind) = crash {
        command.env("SPECDECK_CRASH", kind);
    }
    command.spawn().expect("the app starts")
}

fn crashes_with(kind: &str, code: u32) {
    let profile = profile(kind);
    let mut app = start(&profile, Some(kind));
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        if let Some(status) = app.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "the app did not crash");
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(status.code(), Some(code as i32), "{status:?}");

    let crash = lines(&profile)
        .into_iter()
        .find(|line| line["event"] == "app.crashed")
        .unwrap_or_else(|| panic!("no app.crashed in {:?}", lines(&profile)));
    assert_eq!(crash["code"], format!("0x{code:08X}"));
    assert_eq!(crash["level"], "ERROR");
    let module = crash["module"].as_str().unwrap();
    assert!(!module.is_empty() && !module.contains('\\'), "{crash}");

    // The next start says the run before it ended unexpectedly.
    let mut again = start(&profile, None);
    let deadline = Instant::now() + Duration::from_secs(60);
    let unclean = loop {
        let found = lines(&profile).into_iter().find(|l| l["event"] == "app.ended_unexpectedly");
        if let Some(found) = found {
            break found;
        }
        assert!(Instant::now() < deadline, "no app.ended_unexpectedly");
        std::thread::sleep(Duration::from_millis(200));
    };
    let _ = again.kill();
    let _ = again.wait();
    assert_eq!(unclean["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn an_access_violation_is_recorded() {
    crashes_with("access_violation", 0xC000_0005);
}

#[test]
fn a_stack_overflow_is_recorded() {
    crashes_with("stack_overflow", 0xC000_00FD);
}
