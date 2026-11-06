//! `specdeck-session`: one hosted session, started by the app (`session-processes` design
//! decisions 1 and 2).
//!
//!   specdeck-session --id <n> --run-dir <dir> --endpoint <socket or pipe> --log-dir <dir>
//!
//! Debug builds also take `--grace-secs <s>` and `--loopback`, for tests.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use specdeck_core::peer::PeerConfig;
use specdeck_diag::Diagnostics;
use specdeck_session::server::{self, Options, START_TIMEOUT};

/// How long a session runs without the app before it ends itself (spec: A session without the
/// app ends itself).
const GRACE: Duration = Duration::from_secs(10 * 60);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value =
        |name: &str| args.iter().position(|arg| arg == name).and_then(|i| args.get(i + 1)).cloned();
    let (Some(id), Some(run_dir), Some(endpoint), Some(log_dir)) =
        (value("--id"), value("--run-dir"), value("--endpoint"), value("--log-dir"))
    else {
        eprintln!(
            "usage: specdeck-session --id <n> --run-dir <dir> --endpoint <e> --log-dir <dir>"
        );
        return ExitCode::FAILURE;
    };
    let Ok(id) = id.parse::<u32>() else {
        eprintln!("specdeck-session: --id must be a number");
        return ExitCode::FAILURE;
    };
    let mut grace = GRACE;
    let mut peer = PeerConfig::default();
    if cfg!(debug_assertions) {
        if let Some(seconds) = value("--grace-secs").and_then(|s| s.parse().ok()) {
            grace = Duration::from_secs(seconds);
        }
        if args.iter().any(|arg| arg == "--loopback") {
            peer.bind_address = Some("127.0.0.1".into());
        }
    }

    let (diagnostics, subscriber) = Diagnostics::for_session(PathBuf::from(log_dir), id);
    let _ = tracing::subscriber::set_global_default(subscriber);
    let options = Options {
        id,
        run_dir: PathBuf::from(run_dir),
        endpoint,
        grace,
        start_timeout: START_TIMEOUT,
        peer,
        diagnostics: Some(Arc::new(diagnostics)),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("specdeck-session: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(server::serve(options)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(session = id, kind = ?error.kind(), "session.serve_failed");
            eprintln!("specdeck-session: {error}");
            ExitCode::FAILURE
        }
    }
}
