//! Messages on the reliable `files` channel, which carries the bytes of shared files in both
//! directions (`shared-files` design decisions 1 and 2). Encoded with postcard, like `control`.
//!
//! What a file is, its size and its hash come in the signed timeline (`Entry::FileShared`), so
//! chunks are not signed: the receiver hashes the whole file and compares. Messages from viewers
//! are decoded strictly: unknown variants, trailing bytes and oversized chunks are rejected.

use serde::{Deserialize, Serialize};

use crate::control::{DecodeError, encode};

/// Most bytes of file content in one message.
pub const FILE_CHUNK_BYTES: usize = 64 * 1024;

/// Most bytes of one encoded message: a chunk and its framing.
pub const MAX_FILES_MESSAGE_BYTES: usize = FILE_CHUNK_BYTES + 64;

/// Largest shared file, in bytes.
pub const MAX_SHARED_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// A viewer sends at most this many upload bytes ahead of the host's last `Ack`.
pub const UPLOAD_WINDOW_BYTES: u64 = 512 * 1024;

/// From a viewer to the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilesRequest {
    /// Send this file's bytes from `from`. Replaces any fetch still running.
    Fetch { file: u32, from: u64 },
    /// Stop sending this file.
    Cancel { file: u32 },
    /// A chunk of an upload the host accepted with `Outcome::Ready`.
    Upload { upload: u32, offset: u64, bytes: Vec<u8> },
}

/// From the host to a viewer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilesReply {
    /// A chunk of a fetched file.
    Chunk { file: u32, offset: u64, bytes: Vec<u8> },
    /// The file was removed, or never existed.
    Gone { file: u32 },
    /// The host has `received` bytes of the upload; the viewer may send up to
    /// [`UPLOAD_WINDOW_BYTES`] beyond it.
    Ack { upload: u32, received: u64 },
    /// What became of the upload: `Received` when it is shared now, otherwise why not
    /// (`NotAllowedType`, `Full`, `Off`, or `Gone` when it timed out or did not match its hash).
    Finished { upload: u32, outcome: crate::control::Outcome },
}

pub fn encode_request(request: &FilesRequest) -> Vec<u8> {
    encode(request)
}

pub fn encode_reply(reply: &FilesReply) -> Vec<u8> {
    encode(reply)
}

/// Decodes a message from a viewer: strict and bounded.
pub fn decode_request(bytes: &[u8]) -> Result<FilesRequest, DecodeError> {
    if bytes.len() > MAX_FILES_MESSAGE_BYTES {
        return Err(DecodeError::TooLarge);
    }
    let request: FilesRequest = decode_exact(bytes)?;
    if let FilesRequest::Upload { bytes, .. } = &request
        && (bytes.is_empty() || bytes.len() > FILE_CHUNK_BYTES)
    {
        return Err(DecodeError::Malformed);
    }
    Ok(request)
}

/// Decodes a message from the host.
pub fn decode_reply(bytes: &[u8]) -> Result<FilesReply, DecodeError> {
    if bytes.len() > MAX_FILES_MESSAGE_BYTES {
        return Err(DecodeError::TooLarge);
    }
    let reply: FilesReply = decode_exact(bytes)?;
    if let FilesReply::Chunk { bytes, .. } = &reply
        && (bytes.is_empty() || bytes.len() > FILE_CHUNK_BYTES)
    {
        return Err(DecodeError::Malformed);
    }
    Ok(reply)
}

fn decode_exact<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    match postcard::take_from_bytes(bytes) {
        Ok((message, [])) => Ok(message),
        _ => Err(DecodeError::Malformed),
    }
}

/// The SHA-256 of `bytes`, as announced for a shared file.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes).into()
}

/// A file arriving in chunks, in order, checked against what was announced.
#[derive(Debug)]
pub struct Assembly {
    size: u64,
    sha256: [u8; 32],
    bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Assembled {
    /// More chunks to come.
    Partial,
    /// All bytes arrived and match the hash.
    Done(Vec<u8>),
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AssemblyError {
    #[error("chunk out of order")]
    OutOfOrder,
    #[error("more bytes than announced")]
    TooLong,
    #[error("content does not match its hash")]
    BadHash,
}

impl Assembly {
    pub fn new(size: u64, sha256: [u8; 32]) -> Self {
        // Reserve what was announced, but never more than a shared file may be.
        let capacity = size.min(MAX_SHARED_FILE_BYTES) as usize;
        Self { size, sha256, bytes: Vec::with_capacity(capacity) }
    }

    pub fn received(&self) -> u64 {
        self.bytes.len() as u64
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    /// Adds the chunk at `offset`; `Done` once the last byte arrived and the hash matched.
    pub fn add(&mut self, offset: u64, chunk: &[u8]) -> Result<Assembled, AssemblyError> {
        if offset != self.received() {
            return Err(AssemblyError::OutOfOrder);
        }
        if self.received() + chunk.len() as u64 > self.size {
            return Err(AssemblyError::TooLong);
        }
        self.bytes.extend_from_slice(chunk);
        if self.received() < self.size {
            return Ok(Assembled::Partial);
        }
        if sha256(&self.bytes) != self.sha256 {
            return Err(AssemblyError::BadHash);
        }
        Ok(Assembled::Done(std::mem::take(&mut self.bytes)))
    }
}

/// A viewer fetching one file, with one retry when it does not match its hash.
#[derive(Debug)]
pub struct Fetching {
    file: u32,
    assembly: Assembly,
    retried: bool,
}

/// What a chunk did to a fetch.
#[derive(Debug, PartialEq, Eq)]
pub enum FetchStep {
    /// `received` of the file's bytes are here.
    Partial { received: u64 },
    /// All of it, checked.
    Done(Vec<u8>),
    /// It did not match; ask for it again with this.
    Retry(FilesRequest),
    /// It did not match twice, or the host no longer has it.
    Failed,
}

impl Fetching {
    /// Starts fetching `file`, announced with `size` and `sha256`; returns the request to send.
    pub fn start(file: u32, size: u64, sha256: [u8; 32]) -> (Self, FilesRequest) {
        let fetching = Self { file, assembly: Assembly::new(size, sha256), retried: false };
        (fetching, FilesRequest::Fetch { file, from: 0 })
    }

    pub fn file(&self) -> u32 {
        self.file
    }

    pub fn size(&self) -> u64 {
        self.assembly.size()
    }

    /// The request that continues where this fetch is, after reconnecting.
    pub fn resume(&self) -> FilesRequest {
        FilesRequest::Fetch { file: self.file, from: self.assembly.received() }
    }

    /// Applies a reply about this file; `None` for replies about anything else.
    pub fn on_reply(&mut self, reply: &FilesReply) -> Option<FetchStep> {
        match reply {
            FilesReply::Chunk { file, offset, bytes } if *file == self.file => {
                Some(match self.assembly.add(*offset, bytes) {
                    Ok(Assembled::Partial) => {
                        FetchStep::Partial { received: self.assembly.received() }
                    }
                    Ok(Assembled::Done(bytes)) => FetchStep::Done(bytes),
                    Err(_) if !self.retried => {
                        self.retried = true;
                        self.assembly = Assembly::new(self.assembly.size, self.assembly.sha256);
                        FetchStep::Retry(FilesRequest::Fetch { file: self.file, from: 0 })
                    }
                    Err(_) => FetchStep::Failed,
                })
            }
            FilesReply::Gone { file } if *file == self.file => Some(FetchStep::Failed),
            _ => None,
        }
    }
}

/// A viewer sending one file the host accepted, within the upload window.
#[derive(Debug)]
pub struct Uploading {
    upload: u32,
    bytes: Vec<u8>,
    sent: u64,
    acked: u64,
}

impl Uploading {
    pub fn new(upload: u32, bytes: Vec<u8>) -> Self {
        Self { upload, bytes, sent: 0, acked: 0 }
    }

    pub fn upload(&self) -> u32 {
        self.upload
    }

    pub fn size(&self) -> u64 {
        self.bytes.len() as u64
    }

    /// Bytes the host confirmed.
    pub fn acked(&self) -> u64 {
        self.acked
    }

    /// The chunks that may be sent now: as far as the window beyond the last `Ack` allows.
    pub fn next_chunks(&mut self) -> Vec<FilesRequest> {
        let mut chunks = Vec::new();
        while self.sent < self.size() && self.sent < self.acked + UPLOAD_WINDOW_BYTES {
            let start = self.sent as usize;
            let end = (start + FILE_CHUNK_BYTES).min(self.bytes.len());
            chunks.push(FilesRequest::Upload {
                upload: self.upload,
                offset: self.sent,
                bytes: self.bytes[start..end].to_vec(),
            });
            self.sent = end as u64;
        }
        chunks
    }

    /// Applies an `Ack` for this upload; returns whether it was one.
    pub fn on_ack(&mut self, reply: &FilesReply) -> bool {
        match reply {
            FilesReply::Ack { upload, received } if *upload == self.upload => {
                self.acked = (*received).min(self.size());
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        for request in [
            FilesRequest::Fetch { file: 3, from: 65536 },
            FilesRequest::Cancel { file: 3 },
            FilesRequest::Upload { upload: 1, offset: 0, bytes: vec![1; FILE_CHUNK_BYTES] },
        ] {
            assert_eq!(decode_request(&encode_request(&request)), Ok(request));
        }
        for reply in [
            FilesReply::Chunk { file: 3, offset: 0, bytes: vec![2; FILE_CHUNK_BYTES] },
            FilesReply::Gone { file: 3 },
            FilesReply::Ack { upload: 1, received: 1024 },
            FilesReply::Finished { upload: 1, outcome: crate::control::Outcome::NotAllowedType },
        ] {
            assert_eq!(decode_reply(&encode_reply(&reply)), Ok(reply));
        }
    }

    #[test]
    fn oversized_chunks_are_rejected() {
        let upload =
            FilesRequest::Upload { upload: 1, offset: 0, bytes: vec![1; FILE_CHUNK_BYTES + 1] };
        assert_eq!(decode_request(&encode_request(&upload)), Err(DecodeError::Malformed));
        let chunk = FilesReply::Chunk { file: 1, offset: 0, bytes: vec![1; FILE_CHUNK_BYTES + 1] };
        assert_eq!(decode_reply(&encode_reply(&chunk)), Err(DecodeError::Malformed));
        assert_eq!(
            decode_request(&vec![2; MAX_FILES_MESSAGE_BYTES + 1]),
            Err(DecodeError::TooLarge)
        );
        let empty = FilesRequest::Upload { upload: 1, offset: 0, bytes: vec![] };
        assert_eq!(decode_request(&encode_request(&empty)), Err(DecodeError::Malformed));
    }

    #[test]
    fn unknown_variants_and_trailing_bytes_are_rejected() {
        assert_eq!(decode_request(&[3, 0]), Err(DecodeError::Malformed));
        assert_eq!(decode_reply(&[3, 0]), Err(DecodeError::Malformed));
        let mut bytes = encode_request(&FilesRequest::Cancel { file: 1 });
        bytes.push(0);
        assert_eq!(decode_request(&bytes), Err(DecodeError::Malformed));
    }

    #[test]
    fn assembly_checks_order_size_and_hash() {
        let content = b"hello, world".to_vec();
        let mut assembly = Assembly::new(12, sha256(&content));
        assert_eq!(assembly.add(0, &content[..5]), Ok(Assembled::Partial));
        assert_eq!(assembly.add(0, &content[..5]), Err(AssemblyError::OutOfOrder));
        assert_eq!(assembly.add(5, &content[5..]), Ok(Assembled::Done(content.clone())));

        let mut long = Assembly::new(4, sha256(b"abcd"));
        assert_eq!(long.add(0, b"abcde"), Err(AssemblyError::TooLong));

        let mut tampered = Assembly::new(4, sha256(b"abcd"));
        assert_eq!(tampered.add(0, b"abce"), Err(AssemblyError::BadHash));
    }

    #[test]
    fn a_fetch_retries_once_after_a_bad_hash() {
        let content = vec![7u8; FILE_CHUNK_BYTES + 10];
        let (mut fetch, first) = Fetching::start(4, content.len() as u64, sha256(&content));
        assert_eq!(first, FilesRequest::Fetch { file: 4, from: 0 });
        let chunk = |offset: usize, bytes: &[u8]| FilesReply::Chunk {
            file: 4,
            offset: offset as u64,
            bytes: bytes.to_vec(),
        };
        assert_eq!(
            fetch.on_reply(&chunk(0, &content[..FILE_CHUNK_BYTES])),
            Some(FetchStep::Partial { received: FILE_CHUNK_BYTES as u64 })
        );
        assert_eq!(fetch.resume(), FilesRequest::Fetch { file: 4, from: FILE_CHUNK_BYTES as u64 });
        // The last chunk was altered on the way: fetched again from the start, once.
        let tampered = vec![8u8; 10];
        assert_eq!(
            fetch.on_reply(&chunk(FILE_CHUNK_BYTES, &tampered)),
            Some(FetchStep::Retry(FilesRequest::Fetch { file: 4, from: 0 }))
        );
        assert!(matches!(
            fetch.on_reply(&chunk(0, &content[..FILE_CHUNK_BYTES])),
            Some(FetchStep::Partial { .. })
        ));
        assert_eq!(
            fetch.on_reply(&chunk(FILE_CHUNK_BYTES, &content[FILE_CHUNK_BYTES..])),
            Some(FetchStep::Done(content.clone()))
        );

        let (mut twice, _) = Fetching::start(5, 3, sha256(b"abc"));
        let bad = FilesReply::Chunk { file: 5, offset: 0, bytes: b"abd".to_vec() };
        assert!(matches!(twice.on_reply(&bad), Some(FetchStep::Retry(_))));
        assert_eq!(twice.on_reply(&bad), Some(FetchStep::Failed));
        assert_eq!(twice.on_reply(&FilesReply::Gone { file: 6 }), None);
        assert_eq!(twice.on_reply(&FilesReply::Gone { file: 5 }), Some(FetchStep::Failed));
    }

    #[test]
    fn an_upload_stays_within_its_window() {
        let size = UPLOAD_WINDOW_BYTES as usize + FILE_CHUNK_BYTES * 2;
        let mut upload = Uploading::new(9, vec![1; size]);
        let first = upload.next_chunks();
        assert_eq!(first.len(), UPLOAD_WINDOW_BYTES as usize / FILE_CHUNK_BYTES);
        assert!(upload.next_chunks().is_empty(), "nothing more until the host acknowledges");
        assert!(!upload.on_ack(&FilesReply::Ack { upload: 8, received: 1 }));
        assert!(
            upload.on_ack(&FilesReply::Ack { upload: 9, received: FILE_CHUNK_BYTES as u64 * 2 })
        );
        let rest = upload.next_chunks();
        assert_eq!(rest.len(), 2);
        let FilesRequest::Upload { offset, .. } = &rest[1] else { panic!() };
        assert_eq!(*offset as usize, size - FILE_CHUNK_BYTES);
    }
}
