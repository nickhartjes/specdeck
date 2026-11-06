//! The local protocol between the app and its session processes (`session-processes` design
//! decision 3): length-prefixed frames on a private socket, a JSON control frame for every
//! command and event, and a data frame for raw terminal records.
//!
//! Every connection starts with a handshake naming the protocol version, which follows the
//! release: the app fully controls sessions of its own and the previous release, and can only end
//! older ones.

mod frame;
mod handshake;
mod transport;

pub use frame::{Decoder, Frame, FrameError, MAX_FRAME};
pub use handshake::{Compat, Frozen, Protocol, compat};
#[cfg(unix)]
pub use transport::make_private_dir;
#[cfg(windows)]
pub use transport::pipe_name;
pub use transport::{Connection, FrameReader, FrameWriter, Listener, connect};
