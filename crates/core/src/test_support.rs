//! Helpers shared by unit tests.

use std::path::{Path, PathBuf};

#[path = "../tests/support/executable.rs"]
mod executable;
pub use executable::write_executable;

/// Writes an executable shell script named `name` into `dir`.
pub fn fake_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    write_executable(&path, &format!("#!/bin/sh\n{body}\n"));
    path
}
