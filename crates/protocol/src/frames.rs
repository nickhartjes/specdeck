//! Signed binary frames on the reliable `stream` channel (design decision 7).
//!
//! Layout, big-endian: `kind` (1 byte), `seq` (8 bytes), payload, Ed25519 signature (64 bytes)
//! over every byte before it. Because the signature covers `kind` and `seq`, a frame cannot be
//! moved to another position or turned into another kind without detection.

use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
#[cfg(not(target_arch = "wasm32"))]
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};

use crate::timeline::TimelineEvent;

const KIND_OUTPUT: u8 = 1;
const KIND_RESIZE: u8 = 2;
const KIND_HISTORY: u8 = 3;
const KIND_EVENT: u8 = 4;
const HEADER: usize = 1 + 8;
const SIGNATURE: usize = 64;

/// Largest history batch after decompression, so a malicious frame cannot exhaust memory.
pub const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;

/// Largest encoded frame: the message size libdatachannel and browsers accept on a data channel.
/// Larger messages are refused when sent (`web-viewer` design decision 8).
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

/// One record of a session's terminal history. Output and resizes share one gap-free sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Record {
    Output { seq: u64, bytes: Vec<u8> },
    Resize { seq: u64, cols: u16, rows: u16 },
}

impl Record {
    pub fn seq(&self) -> u64 {
        match self {
            Record::Output { seq, .. } | Record::Resize { seq, .. } => *seq,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    /// A live record.
    Record(Record),
    /// Consecutive records sent at once, zstd-compressed, for late join and catch-up.
    History(Vec<Record>),
    /// A timeline event; `seq` is the event sequence, separate from the record sequence.
    Event { seq: u64, event: TimelineEvent },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("frame failed verification")]
    BadSignature,
    #[error("malformed frame")]
    Malformed,
}

/// The host's per-session signing key. Hosts run the app, so the WebAssembly build has none.
#[cfg(not(target_arch = "wasm32"))]
pub struct SessionKey(SigningKey);

#[cfg(not(target_arch = "wasm32"))]
impl SessionKey {
    pub fn generate() -> Self {
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret).expect("no random source");
        Self(SigningKey::from_bytes(&secret))
    }

    /// Sent to viewers in `Welcome`.
    pub fn public_key(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    pub fn encode(&self, frame: &Frame) -> Vec<u8> {
        let (kind, seq, payload) = match frame {
            Frame::Record(Record::Output { seq, bytes }) => (KIND_OUTPUT, *seq, bytes.clone()),
            Frame::Record(Record::Resize { seq, cols, rows }) => {
                (KIND_RESIZE, *seq, [cols.to_be_bytes(), rows.to_be_bytes()].concat())
            }
            Frame::History(records) => {
                let first = records.first().map_or(0, Record::seq);
                (KIND_HISTORY, first, encode_history(records))
            }
            Frame::Event { seq, event } => {
                (KIND_EVENT, *seq, serde_json::to_vec(event).expect("events always encode"))
            }
        };
        let mut bytes = Vec::with_capacity(HEADER + payload.len() + SIGNATURE);
        bytes.push(kind);
        bytes.extend_from_slice(&seq.to_be_bytes());
        bytes.extend_from_slice(&payload);
        let signature = self.0.sign(&bytes);
        bytes.extend_from_slice(&signature.to_bytes());
        bytes
    }
}

/// Checks and decodes frames from one session's host.
pub struct FrameVerifier(VerifyingKey);

impl FrameVerifier {
    pub fn new(public_key: &[u8; 32]) -> Result<Self, FrameError> {
        VerifyingKey::from_bytes(public_key).map(Self).map_err(|_| FrameError::Malformed)
    }

    /// Verifies before parsing anything.
    pub fn decode(&self, bytes: &[u8]) -> Result<Frame, FrameError> {
        if bytes.len() < HEADER + SIGNATURE {
            return Err(FrameError::Malformed);
        }
        let (signed, signature) = bytes.split_at(bytes.len() - SIGNATURE);
        let signature = Signature::from_slice(signature).map_err(|_| FrameError::Malformed)?;
        self.0.verify(signed, &signature).map_err(|_| FrameError::BadSignature)?;

        let kind = signed[0];
        let seq = u64::from_be_bytes(signed[1..HEADER].try_into().unwrap());
        let payload = &signed[HEADER..];
        Ok(match kind {
            KIND_OUTPUT => Frame::Record(Record::Output { seq, bytes: payload.to_vec() }),
            KIND_RESIZE => {
                let [c0, c1, r0, r1] = payload.try_into().map_err(|_| FrameError::Malformed)?;
                Frame::Record(Record::Resize {
                    seq,
                    cols: u16::from_be_bytes([c0, c1]),
                    rows: u16::from_be_bytes([r0, r1]),
                })
            }
            KIND_HISTORY => {
                let records = decode_history(payload)?;
                if records.first().map_or(0, Record::seq) != seq {
                    return Err(FrameError::Malformed);
                }
                Frame::History(records)
            }
            KIND_EVENT => Frame::Event {
                seq,
                event: serde_json::from_slice(payload).map_err(|_| FrameError::Malformed)?,
            },
            _ => return Err(FrameError::Malformed),
        })
    }
}

/// Records as postcard, compressed with zstd.
#[cfg(not(target_arch = "wasm32"))]
fn encode_history(records: &[Record]) -> Vec<u8> {
    let raw = postcard::to_allocvec(records).expect("records always encode");
    zstd::bulk::compress(&raw, 3).expect("compression into memory")
}

#[cfg(not(target_arch = "wasm32"))]
fn decompress(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    zstd::bulk::decompress(payload, MAX_HISTORY_BYTES).map_err(|_| FrameError::Malformed)
}

#[cfg(target_arch = "wasm32")]
fn decompress(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    decompress_pure(payload)
}

/// zstd decompression in pure Rust, for the WebAssembly build, which cannot use the C library.
/// Stops past `MAX_HISTORY_BYTES`, like the native path.
#[cfg(any(target_arch = "wasm32", test))]
fn decompress_pure(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    use std::io::Read;
    let mut decoder =
        ruzstd::decoding::StreamingDecoder::new(payload).map_err(|_| FrameError::Malformed)?;
    let mut raw = Vec::new();
    (&mut decoder)
        .take(MAX_HISTORY_BYTES as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| FrameError::Malformed)?;
    if raw.len() > MAX_HISTORY_BYTES {
        return Err(FrameError::Malformed);
    }
    Ok(raw)
}

fn decode_history(payload: &[u8]) -> Result<Vec<Record>, FrameError> {
    let raw = decompress(payload)?;
    parse_history(&raw)
}

fn parse_history(raw: &[u8]) -> Result<Vec<Record>, FrameError> {
    let records: Vec<Record> = match postcard::take_from_bytes(raw) {
        Ok((records, [])) => records,
        _ => return Err(FrameError::Malformed),
    };
    // A batch is consecutive records.
    if records.windows(2).any(|pair| pair[1].seq() != pair[0].seq() + 1) {
        return Err(FrameError::Malformed);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Entry;

    fn keys() -> (SessionKey, FrameVerifier) {
        let key = SessionKey::generate();
        let verifier = FrameVerifier::new(&key.public_key()).unwrap();
        (key, verifier)
    }

    fn output(seq: u64, text: &str) -> Frame {
        Frame::Record(Record::Output { seq, bytes: text.as_bytes().to_vec() })
    }

    #[test]
    fn every_kind_round_trips() {
        let (key, verifier) = keys();
        let frames = [
            output(0, "hello"),
            output(1, ""),
            Frame::Record(Record::Resize { seq: 2, cols: 120, rows: 40 }),
            Frame::History(vec![
                Record::Output { seq: 3, bytes: b"\x1b[2Jredraw".repeat(100) },
                Record::Resize { seq: 4, cols: 80, rows: 24 },
            ]),
            Frame::Event {
                seq: 0,
                event: TimelineEvent {
                    output_seq: -1,
                    at: 5,
                    entry: Entry::SessionStarted,
                    truncated: false,
                },
            },
        ];
        for frame in frames {
            assert_eq!(verifier.decode(&key.encode(&frame)), Ok(frame));
        }
    }

    #[test]
    fn signatures_match_rfc_8032() {
        // RFC 8032 section 7.1, test 1, so apps of different versions keep verifying each other's
        // frames.
        let unhex = |text: &str| -> Vec<u8> {
            (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                .collect()
        };
        let secret = unhex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let key = SessionKey(SigningKey::from_bytes(&secret.try_into().unwrap()));
        assert_eq!(
            key.public_key().to_vec(),
            unhex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a")
        );
        assert_eq!(
            key.0.sign(b"").to_bytes().to_vec(),
            unhex(
                "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
            )
        );
    }

    /// `encode_history` of the records in `history_from_older_apps_still_decodes`, from 1.1.0.
    const HISTORY_1_1_0: [u8; 30] = [
        40, 181, 47, 253, 32, 128, 173, 0, 0, 112, 2, 0, 0, 120, 104, 101, 108, 108, 111, 32, 1, 1,
        100, 32, 1, 0, 47, 105, 65, 2,
    ];

    #[test]
    fn history_from_older_apps_still_decodes() {
        // A history frame as 1.1.0 compressed it (zstd level 3), so a newer zstd still reads it.
        let records = vec![
            Record::Output { seq: 0, bytes: b"hello ".repeat(20) },
            Record::Resize { seq: 1, cols: 100, rows: 32 },
        ];
        assert_eq!(decode_history(&encode_history(&records)).unwrap(), records);
        assert_eq!(decode_history(&HISTORY_1_1_0).unwrap(), records);
    }

    #[test]
    fn the_web_viewers_decoder_reads_what_hosts_compress() {
        // The WebAssembly build decompresses with ruzstd; hosts compress with zstd.
        let pure = |payload: &[u8]| parse_history(&decompress_pure(payload).unwrap()).unwrap();
        let records = vec![
            Record::Output { seq: 0, bytes: b"hello ".repeat(20) },
            Record::Resize { seq: 1, cols: 100, rows: 32 },
        ];
        assert_eq!(pure(&HISTORY_1_1_0), records);
        assert_eq!(pure(&encode_history(&records)), records);
        let long: Vec<Record> = (0..200)
            .map(|seq| Record::Output {
                seq,
                bytes: format!("\x1b[32mline {seq}\x1b[0m\r\n").repeat(50).into_bytes(),
            })
            .collect();
        assert_eq!(pure(&encode_history(&long)), long);
        assert_eq!(decompress_pure(b"not zstd"), Err(FrameError::Malformed));
    }

    #[test]
    fn history_is_compressed() {
        let (key, _) = keys();
        let records =
            vec![Record::Output { seq: 0, bytes: b"\x1b[2K\rThinking...".repeat(10_000) }];
        assert!(key.encode(&Frame::History(records)).len() < 10_000);
    }

    #[test]
    fn a_full_batch_of_incompressible_output_fits_in_a_frame() {
        // The host reads history in batches of 192 KiB (`READ_BYTES` in specdeck-core's
        // sharing.rs); zstd adds a little to input it cannot compress.
        let (key, _) = keys();
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let noise: Vec<u8> = (0..192 * 1024)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        let records = noise
            .chunks(4096)
            .enumerate()
            .map(|(seq, bytes)| Record::Output { seq: seq as u64, bytes: bytes.to_vec() })
            .collect();
        assert!(key.encode(&Frame::History(records)).len() <= MAX_FRAME_BYTES);
    }

    #[test]
    fn tampered_payload_fails_verification() {
        let (key, verifier) = keys();
        let mut bytes = key.encode(&output(7, "rm -rf ~"));
        bytes[HEADER] ^= 1;
        assert_eq!(verifier.decode(&bytes), Err(FrameError::BadSignature));
    }

    #[test]
    fn moved_sequence_fails_verification() {
        let (key, verifier) = keys();
        let mut bytes = key.encode(&output(7, "x"));
        bytes[8] = 8;
        assert_eq!(verifier.decode(&bytes), Err(FrameError::BadSignature));
    }

    #[test]
    fn changed_kind_fails_verification() {
        let (key, verifier) = keys();
        let mut bytes = key.encode(&output(7, "abcd"));
        bytes[0] = KIND_RESIZE;
        assert_eq!(verifier.decode(&bytes), Err(FrameError::BadSignature));
    }

    #[test]
    fn frame_from_another_session_fails_verification() {
        let (key, _) = keys();
        let (_, other) = keys();
        assert_eq!(other.decode(&key.encode(&output(0, "x"))), Err(FrameError::BadSignature));
    }

    #[test]
    fn short_frame_is_malformed() {
        let (_, verifier) = keys();
        assert_eq!(verifier.decode(&[1; HEADER + SIGNATURE - 1]), Err(FrameError::Malformed));
    }

    #[test]
    fn signed_but_malformed_frames_are_rejected() {
        let (key, verifier) = keys();
        let sign = |kind: u8, seq: u64, payload: &[u8]| {
            let mut bytes = vec![kind];
            bytes.extend_from_slice(&seq.to_be_bytes());
            bytes.extend_from_slice(payload);
            let signature = key.0.sign(&bytes);
            bytes.extend_from_slice(&signature.to_bytes());
            bytes
        };
        assert_eq!(verifier.decode(&sign(9, 0, b"")), Err(FrameError::Malformed));
        assert_eq!(verifier.decode(&sign(KIND_RESIZE, 0, b"abc")), Err(FrameError::Malformed));
        assert_eq!(
            verifier.decode(&sign(KIND_HISTORY, 0, b"not zstd")),
            Err(FrameError::Malformed)
        );
        assert_eq!(verifier.decode(&sign(KIND_EVENT, 0, b"{}")), Err(FrameError::Malformed));
        // A batch whose records are not consecutive.
        let gap = encode_history(&[
            Record::Output { seq: 0, bytes: vec![] },
            Record::Output { seq: 2, bytes: vec![] },
        ]);
        assert_eq!(verifier.decode(&sign(KIND_HISTORY, 0, &gap)), Err(FrameError::Malformed));
        // A batch whose header sequence does not match its first record.
        let batch = encode_history(&[Record::Output { seq: 5, bytes: vec![] }]);
        assert_eq!(verifier.decode(&sign(KIND_HISTORY, 4, &batch)), Err(FrameError::Malformed));
    }
}
