//! Executable scripts for tests, written without the test process ever holding them open for
//! writing. Tests run on many threads: one that forks while another writes a script hands the open
//! file to its child until that child execs, and running the script in that window fails with
//! ETXTBSY ("Text file busy"). That made tests fail now and then, mostly as timeouts. A separate
//! `sh` writes the file instead, so no thread of the test process has anything to hand on.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Writes `contents` to `path` and makes it executable (mode 0755).
pub fn write_executable(path: &Path, contents: &str) {
    let mut child = Command::new("/bin/sh")
        .args(["-c", r#"cat > "$1" && chmod 755 "$1""#, "sh"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .expect("start sh to write an executable");
    child.stdin.take().unwrap().write_all(contents.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success(), "writing {}", path.display());
}
