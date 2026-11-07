//! SpecDeck session engine: runs a coding agent for a host and connects hosts and viewers.

pub mod activity;
pub mod agent;
pub mod browser;
// The only module that may emit diagnostic events; a test in it holds that.
pub mod diag;
pub mod documents;
pub mod environment;
pub mod hooks;
pub mod ice;
pub mod launcher;
pub mod mirror;
pub mod output_log;
pub mod peer;
pub mod plans;
pub mod program_status;
pub mod session;
pub mod session_folder;
pub mod sharing;
pub mod shell_changes;
pub mod signalling_client;
pub mod status;
pub mod timeline;
pub mod usage;
pub mod viewer;

#[cfg(all(test, unix))]
mod test_support;

#[cfg(test)]
#[path = "../tests/support/capture.rs"]
mod capture;

// The OpenSSL that libdatachannel vendors uses these Windows libraries, but its build does not
// tell the linker; without them a program that links little else fails with unresolved symbols.
#[cfg(windows)]
#[link(name = "advapi32")]
#[link(name = "bcrypt")]
#[link(name = "crypt32")]
#[link(name = "user32")]
unsafe extern "C" {}
