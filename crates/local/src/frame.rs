//! Frames: a 4-byte big-endian length, then a kind byte and the payload. The length counts the
//! kind byte and the payload.

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// The largest frame either side accepts, kind byte and payload included. Terminal records are
/// read from the output log in chunks of at most 256 KiB, so this leaves ample room.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

const CONTROL: u8 = 1;
const DATA: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// A command or event, as JSON text.
    Control(Vec<u8>),
    /// Raw terminal records, encoded as for the UI.
    Data(Vec<u8>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    /// The frame is longer than [`MAX_FRAME`]; the connection cannot be read any further.
    TooLarge(usize),
    /// A frame without even a kind byte.
    Empty,
    UnknownKind(u8),
    /// A control frame that is not the JSON expected.
    Json(String),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::TooLarge(length) => {
                write!(f, "a frame of {length} bytes is larger than {MAX_FRAME}")
            }
            FrameError::Empty => write!(f, "an empty frame"),
            FrameError::UnknownKind(kind) => write!(f, "a frame of unknown kind {kind}"),
            FrameError::Json(error) => write!(f, "a control frame that cannot be read: {error}"),
        }
    }
}

impl std::error::Error for FrameError {}

impl Frame {
    /// A control frame holding `message` as JSON.
    pub fn control(message: &impl Serialize) -> Result<Frame, FrameError> {
        serde_json::to_vec(message).map(Frame::Control).map_err(|e| FrameError::Json(e.to_string()))
    }

    /// The message in a control frame; an error for a data frame or JSON of another shape.
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T, FrameError> {
        match self {
            Frame::Control(json) => {
                serde_json::from_slice(json).map_err(|e| FrameError::Json(e.to_string()))
            }
            Frame::Data(_) => Err(FrameError::Json("a data frame, not a control frame".into())),
        }
    }

    /// The frame as bytes to write; refused when it would be larger than [`MAX_FRAME`].
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        let (kind, payload) = match self {
            Frame::Control(payload) => (CONTROL, payload),
            Frame::Data(payload) => (DATA, payload),
        };
        let length = payload.len() + 1;
        if length > MAX_FRAME {
            return Err(FrameError::TooLarge(length));
        }
        let mut bytes = Vec::with_capacity(4 + length);
        bytes.extend_from_slice(&(length as u32).to_be_bytes());
        bytes.push(kind);
        bytes.extend_from_slice(payload);
        Ok(bytes)
    }
}

/// Turns bytes, as they arrive in reads of any size, back into frames.
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole frame, or `None` until more bytes arrive. An oversized frame is refused as
    /// soon as its length is known, without waiting for (or buffering) its payload.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, FrameError> {
        let Some(header) = self.buffer.first_chunk::<4>() else { return Ok(None) };
        let length = u32::from_be_bytes(*header) as usize;
        if length > MAX_FRAME {
            return Err(FrameError::TooLarge(length));
        }
        if length == 0 {
            return Err(FrameError::Empty);
        }
        if self.buffer.len() < 4 + length {
            return Ok(None);
        }
        let kind = self.buffer[4];
        let payload = self.buffer[5..4 + length].to_vec();
        self.buffer.drain(..4 + length);
        match kind {
            CONTROL => Ok(Some(Frame::Control(payload))),
            DATA => Ok(Some(Frame::Data(payload))),
            other => Err(FrameError::UnknownKind(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames() -> Vec<Frame> {
        vec![
            Frame::control(&serde_json::json!({"type": "input", "data": "ls\r"})).unwrap(),
            Frame::Data(vec![0, 1, 2, 255, 254]),
            Frame::Data(Vec::new()),
            Frame::control(&serde_json::json!({"type": "end"})).unwrap(),
        ]
    }

    fn stream() -> Vec<u8> {
        frames().iter().flat_map(|frame| frame.encode().unwrap()).collect()
    }

    fn decode_in_reads(bytes: &[u8], read: usize) -> Vec<Frame> {
        let mut decoder = Decoder::default();
        let mut decoded = Vec::new();
        for chunk in bytes.chunks(read) {
            decoder.push(chunk);
            while let Some(frame) = decoder.next_frame().unwrap() {
                decoded.push(frame);
            }
        }
        decoded
    }

    #[test]
    fn frames_come_back_whatever_the_reads_split() {
        let bytes = stream();
        for read in [1, 2, 3, 5, 7, 64, bytes.len()] {
            assert_eq!(decode_in_reads(&bytes, read), frames(), "reads of {read} bytes");
        }
    }

    #[test]
    fn an_incomplete_frame_waits_for_the_rest() {
        let bytes = Frame::Data(vec![9; 10]).encode().unwrap();
        let mut decoder = Decoder::default();
        decoder.push(&bytes[..bytes.len() - 1]);
        assert_eq!(decoder.next_frame(), Ok(None));
        decoder.push(&bytes[bytes.len() - 1..]);
        assert_eq!(decoder.next_frame(), Ok(Some(Frame::Data(vec![9; 10]))));
    }

    #[test]
    fn an_oversized_frame_is_refused_from_its_header() {
        let mut decoder = Decoder::default();
        decoder.push(&((MAX_FRAME + 1) as u32).to_be_bytes());
        assert_eq!(decoder.next_frame(), Err(FrameError::TooLarge(MAX_FRAME + 1)));
        assert_eq!(
            Frame::Data(vec![0; MAX_FRAME]).encode(),
            Err(FrameError::TooLarge(MAX_FRAME + 1)),
            "nor is one written"
        );
        assert!(Frame::Data(vec![0; MAX_FRAME - 1]).encode().is_ok());
    }

    #[test]
    fn malformed_frames_are_errors() {
        let mut empty = Decoder::default();
        empty.push(&0u32.to_be_bytes());
        assert_eq!(empty.next_frame(), Err(FrameError::Empty));

        let mut unknown = Decoder::default();
        unknown.push(&[0, 0, 0, 2, 9, 0]);
        assert_eq!(unknown.next_frame(), Err(FrameError::UnknownKind(9)));
    }

    #[test]
    fn control_frames_carry_json() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Resize {
            cols: u16,
            rows: u16,
        }
        let frame = Frame::control(&Resize { cols: 120, rows: 40 }).unwrap();
        assert_eq!(frame.parse::<Resize>().unwrap(), Resize { cols: 120, rows: 40 });
        assert!(Frame::Data(vec![1]).parse::<Resize>().is_err());
        assert!(Frame::Control(b"{\"cols\":1}".to_vec()).parse::<Resize>().is_err());
    }
}
