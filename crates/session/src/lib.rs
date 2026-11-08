//! A hosted session, run as a process of its own so it outlives the app (`session-processes`):
//! the agent's terminal, the output log, the hooks, sharing and the viewers' connections. The app
//! attaches to it over the local protocol (`specdeck-local`).

pub mod ask_notices;
pub mod messages;
pub mod runtime;
pub mod server;
