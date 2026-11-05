//! SpecDeck wire protocol shared by hosts and viewers. Contains no I/O.

mod agent;
pub mod control;
pub mod frames;
pub mod invite;
pub mod service;
pub mod signal_auth;
pub mod signalling;
pub mod terminal_stream;
pub mod timeline;
pub mod viewer_state;

pub use agent::{AgentId, AgentStatus};
