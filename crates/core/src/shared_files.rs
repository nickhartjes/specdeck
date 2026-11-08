//! Files shared with everyone in a session (`shared-files` design decisions 5 and 7): what may be
//! shared, decided from the content; the session's store, held in memory with its limits; and
//! putting a shared file into the session folder for the agent.
//!
//! The store only ever holds bytes a person chose in a window, the host's or a viewer's. Nothing
//! here reads a path to share it.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

use specdeck_protocol::control::{FileKind, MAX_FILE_NAME_BYTES, SharedFileInfo};
use specdeck_protocol::files::{MAX_SHARED_FILE_BYTES, sha256};

use crate::session_folder::SessionFolder;

/// All shared files of a session together.
pub const MAX_SESSION_BYTES: u64 = 50 * 1024 * 1024;

/// One viewer's shared files together, so one viewer cannot fill the session.
pub const MAX_VIEWER_BYTES: u64 = 20 * 1024 * 1024;

/// Most files shared at once, which keeps the list sent to viewers small.
pub const MAX_FILES: usize = 200;

/// The folder in the session folder that shared files are put into.
pub const SHARED_FOLDER: &str = "shared";

/// Why a file is not shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Larger than [`MAX_SHARED_FILE_BYTES`].
    TooLarge,
    /// No room left in the session's or the viewer's share.
    Full,
    /// Not an image, Markdown or text, or empty.
    NotAllowedType,
    /// Removed, never shared, or not the asker's to remove.
    Gone,
    /// The host does not let viewers share files.
    Off,
}

impl Refusal {
    /// The refusal's fixed name, for diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Refusal::TooLarge => "too_large",
            Refusal::Full => "full",
            Refusal::NotAllowedType => "type",
            Refusal::Gone => "gone",
            Refusal::Off => "off",
        }
    }
}

/// What a file turned out to be, and the name it is shared under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub name: String,
    pub kind: FileKind,
    pub media_type: &'static str,
}

/// Decides whether `bytes` may be shared and under which name. Images are recognised by their
/// first bytes, whatever they are called, and get the extension of what they are. Markdown and
/// text must be valid UTF-8 without NUL bytes. SVG is refused, as is everything else.
pub fn check(name: &str, bytes: &[u8]) -> Result<Checked, Refusal> {
    if bytes.len() as u64 > MAX_SHARED_FILE_BYTES {
        return Err(Refusal::TooLarge);
    }
    if bytes.is_empty() {
        return Err(Refusal::NotAllowedType);
    }
    let name = clean_name(name);
    if let Some((media_type, extensions)) = image_type(bytes) {
        let name = with_extension(&name, extensions);
        return Ok(Checked { name, kind: FileKind::Image, media_type });
    }
    if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        return Err(Refusal::NotAllowedType);
    }
    let extension = extension(&name).map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("svg" | "svgz") => Err(Refusal::NotAllowedType),
        Some("md" | "markdown") => Ok(Checked {
            name: or_default(name, "md"),
            kind: FileKind::Markdown,
            media_type: "text/markdown",
        }),
        _ => Ok(Checked {
            name: or_default(name, "txt"),
            kind: FileKind::Text,
            media_type: "text/plain",
        }),
    }
}

/// The image type of `bytes` and its extensions, the first one preferred.
fn image_type(bytes: &[u8]) -> Option<(&'static str, &'static [&'static str])> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(("image/png", &["png"]))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("image/jpeg", &["jpg", "jpeg"]))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("image/gif", &["gif"]))
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", &["webp"]))
    } else {
        None
    }
}

/// `name` as a plain file name: the part after the last `/` or `\`, without control characters,
/// characters Windows does not allow in names, or leading dots and spaces, and at most
/// [`MAX_FILE_NAME_BYTES`] bytes. Empty when nothing is left.
pub fn clean_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let kept: String = last
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let trimmed = kept.trim_start_matches(['.', ' ']).trim_end();
    let mut name = trimmed.to_owned();
    if name.len() > MAX_FILE_NAME_BYTES {
        name.truncate(name.floor_char_boundary(MAX_FILE_NAME_BYTES));
    }
    name
}

fn extension(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_, extension)| extension).filter(|e| !e.is_empty())
}

fn or_default(name: String, extension: &str) -> String {
    if name.is_empty() { format!("file.{extension}") } else { name }
}

/// `name` with an extension from `extensions`: kept when it is one of them, otherwise the first
/// is added in place of the wrong one.
fn with_extension(name: &str, extensions: &[&str]) -> String {
    let current = extension(name).map(str::to_ascii_lowercase);
    if current.as_deref().is_some_and(|current| extensions.contains(&current)) {
        return name.to_owned();
    }
    let stem = match current {
        Some(_) => name.rsplit_once('.').map_or(name, |(stem, _)| stem),
        None => name,
    };
    let stem = if stem.is_empty() { "file" } else { stem };
    let mut stem = stem.to_owned();
    // Room for the extension within the name limit.
    let room = MAX_FILE_NAME_BYTES - extensions[0].len() - 1;
    if stem.len() > room {
        stem.truncate(stem.floor_char_boundary(room));
    }
    format!("{stem}.{}", extensions[0])
}

/// Who shared a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sharer {
    Host,
    Viewer(u32),
}

impl Sharer {
    /// The participant id: 0 for the host.
    pub fn id(self) -> u32 {
        match self {
            Sharer::Host => 0,
            Sharer::Viewer(id) => id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SharedFile {
    pub info: SharedFileInfo,
    pub sharer: Sharer,
    pub bytes: Arc<[u8]>,
}

/// Room held for an upload that has not arrived yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reservation {
    pub viewer: u32,
    pub name: String,
    pub size: u64,
    pub sha256: [u8; 32],
}

/// The session's shared files, in the order they were shared.
#[derive(Debug, Default)]
pub struct Store {
    files: Vec<SharedFile>,
    reserved: HashMap<u32, Reservation>,
    /// Ids are never reused, so a removed file's id never names another file.
    next_file: u32,
    next_upload: u32,
}

impl Store {
    pub fn files(&self) -> &[SharedFile] {
        &self.files
    }

    pub fn list(&self) -> Vec<SharedFileInfo> {
        self.files.iter().map(|file| file.info.clone()).collect()
    }

    pub fn get(&self, file: u32) -> Option<&SharedFile> {
        self.files.iter().find(|shared| shared.info.file == file)
    }

    pub fn reservation(&self, upload: u32) -> Option<&Reservation> {
        self.reserved.get(&upload)
    }

    /// Bytes held and reserved, in all and for `viewer`.
    fn used(&self, viewer: Option<u32>) -> u64 {
        let files =
            self.files.iter().filter(|f| viewer.is_none_or(|v| f.sharer == Sharer::Viewer(v)));
        let reserved = self.reserved.values().filter(|r| viewer.is_none_or(|v| r.viewer == v));
        files.map(|f| f.info.size).sum::<u64>() + reserved.map(|r| r.size).sum::<u64>()
    }

    /// Whether `sharer` may share `size` more bytes now.
    pub fn room(&self, sharer: Sharer, size: u64) -> Result<(), Refusal> {
        if size > MAX_SHARED_FILE_BYTES {
            return Err(Refusal::TooLarge);
        }
        if size == 0 {
            return Err(Refusal::NotAllowedType);
        }
        if self.files.len() + self.reserved.len() >= MAX_FILES
            || self.used(None) + size > MAX_SESSION_BYTES
        {
            return Err(Refusal::Full);
        }
        if let Sharer::Viewer(viewer) = sharer
            && self.used(Some(viewer)) + size > MAX_VIEWER_BYTES
        {
            return Err(Refusal::Full);
        }
        Ok(())
    }

    /// Holds room for a viewer's upload; the id it returns names the upload on `files`.
    pub fn reserve(
        &mut self,
        viewer: u32,
        name: String,
        size: u64,
        sha256: [u8; 32],
    ) -> Result<u32, Refusal> {
        self.room(Sharer::Viewer(viewer), size)?;
        self.next_upload += 1;
        let upload = self.next_upload;
        self.reserved.insert(upload, Reservation { viewer, name, size, sha256 });
        Ok(upload)
    }

    /// Frees an upload's room, whatever became of it.
    pub fn release(&mut self, upload: u32) -> Option<Reservation> {
        self.reserved.remove(&upload)
    }

    /// Frees the room of every upload of `viewer`, who disconnected.
    pub fn release_viewer(&mut self, viewer: u32) -> Vec<u32> {
        let uploads: Vec<u32> =
            self.reserved.iter().filter(|(_, r)| r.viewer == viewer).map(|(id, _)| *id).collect();
        for upload in &uploads {
            self.reserved.remove(upload);
        }
        uploads
    }

    /// Shares `bytes`, checked here, as `sharer`. A viewer's upload is released first, so its
    /// own reservation does not count against it.
    pub fn add(
        &mut self,
        sharer: Sharer,
        by_label: &str,
        name: &str,
        bytes: Vec<u8>,
        at_ms: u64,
    ) -> Result<SharedFileInfo, Refusal> {
        let checked = check(name, &bytes)?;
        self.room(sharer, bytes.len() as u64)?;
        self.next_file += 1;
        let info = SharedFileInfo {
            file: self.next_file,
            name: checked.name,
            kind: checked.kind,
            media_type: checked.media_type.into(),
            size: bytes.len() as u64,
            sha256: sha256(&bytes),
            by: sharer.id(),
            by_label: by_label.to_owned(),
            at_ms,
        };
        self.files.push(SharedFile { info: info.clone(), sharer, bytes: bytes.into() });
        Ok(info)
    }

    /// Removes `file` for the host (`asker` `None`) or for the viewer who shared it.
    pub fn remove(&mut self, file: u32, asker: Option<u32>) -> Result<SharedFileInfo, Refusal> {
        let at =
            self.files.iter().position(|shared| shared.info.file == file).ok_or(Refusal::Gone)?;
        if let Some(viewer) = asker
            && self.files[at].sharer != Sharer::Viewer(viewer)
        {
            return Err(Refusal::Gone);
        }
        Ok(self.files.remove(at).info)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PutError {
    #[error("the file is no longer shared")]
    Gone,
    #[error("the folder shared/ is outside the session folder")]
    OutsideFolder,
    #[error("shared/ in the session folder is not a folder")]
    NotAFolder,
    #[error("no free name in shared/")]
    NoFreeName,
    #[error("could not write the file: {0}")]
    Io(#[from] std::io::Error),
}

/// Writes `bytes` into `shared/` in the session folder under `name`, or the first free name with
/// a number, and returns the path relative to the folder. Never replaces a file, and never
/// follows a symlink at the name: the file is created new or not at all.
pub fn put_in_folder(
    folder: &SessionFolder,
    root: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<String, PutError> {
    let shared = root.join(SHARED_FOLDER);
    // Checked before creating it, so a link elsewhere is never followed to make a folder there.
    if !folder.contains(&shared.to_string_lossy()) {
        return Err(PutError::OutsideFolder);
    }
    std::fs::create_dir_all(&shared)?;
    if !shared.is_dir() {
        return Err(PutError::NotAFolder);
    }
    if !folder.contains(&shared.to_string_lossy()) {
        return Err(PutError::OutsideFolder);
    }
    let name = match clean_name(name) {
        name if name.is_empty() => "file".to_owned(),
        name => name,
    };
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, Some(extension)),
        _ => (name.as_str(), None),
    };
    for number in 1..=100 {
        let candidate = match (number, extension) {
            (1, _) => name.clone(),
            (n, Some(extension)) => format!("{stem} ({n}).{extension}"),
            (n, None) => format!("{stem} ({n})"),
        };
        match OpenOptions::new().write(true).create_new(true).open(shared.join(&candidate)) {
            Ok(mut file) => {
                file.write_all(bytes)?;
                return Ok(format!("{SHARED_FOLDER}/{candidate}"));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(PutError::NoFreeName)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];

    #[test]
    fn names_are_plain_names() {
        assert_eq!(clean_name("../../.ssh/config.md"), "config.md");
        assert_eq!(clean_name(r"C:\Users\nick\notes.txt"), "notes.txt");
        assert_eq!(clean_name("a\u{1b}[2Jb\n.log"), "a[2Jb.log");
        assert_eq!(clean_name(".env"), "env");
        assert_eq!(clean_name("..."), "");
        assert_eq!(clean_name("what?<>.md"), "what.md");
        let long = "é".repeat(200);
        let cut = clean_name(&long);
        assert!(cut.len() <= MAX_FILE_NAME_BYTES && cut.chars().all(|c| c == 'é'));
    }

    #[test]
    fn images_are_recognised_by_their_bytes() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0];
        let gif = *b"GIF89a\0\0";
        let webp = *b"RIFF\0\0\0\0WEBPVP8 ";
        for (bytes, name, media_type, expected) in [
            (PNG, "wireframe.png", "image/png", "wireframe.png"),
            (PNG, "wireframe.txt", "image/png", "wireframe.png"),
            (PNG, "wireframe", "image/png", "wireframe.png"),
            (&jpeg[..], "photo.JPEG", "image/jpeg", "photo.JPEG"),
            (&jpeg[..], "photo.gif", "image/jpeg", "photo.jpg"),
            (&gif[..], "a.gif", "image/gif", "a.gif"),
            (&webp[..], "b.webp", "image/webp", "b.webp"),
            (PNG, "", "image/png", "file.png"),
        ] {
            let checked = check(name, bytes).unwrap();
            assert_eq!(
                (checked.kind, checked.media_type, checked.name.as_str()),
                (FileKind::Image, media_type, expected)
            );
        }
    }

    #[test]
    fn markdown_and_text_must_be_text() {
        let md = check("acceptance-criteria.md", b"# Criteria").unwrap();
        assert_eq!((md.kind, md.media_type), (FileKind::Markdown, "text/markdown"));
        let log = check("ci-failure.log", b"FAILED").unwrap();
        assert_eq!((log.kind, log.name.as_str()), (FileKind::Text, "ci-failure.log"));
        assert_eq!(check("", b"x").unwrap().name, "file.txt");
        assert_eq!(check("notes.txt", b"MZ\0\x90"), Err(Refusal::NotAllowedType));
        assert_eq!(check("notes.txt", &[0xC3, 0x28]), Err(Refusal::NotAllowedType));
        assert_eq!(check("logo.svg", b"<svg onload=alert(1)/>"), Err(Refusal::NotAllowedType));
        assert_eq!(check("spec.pdf", b"%PDF-1.7\n\xE2\xE3\xCF\xD3"), Err(Refusal::NotAllowedType));
        assert_eq!(check("empty.txt", b""), Err(Refusal::NotAllowedType));
        let big = vec![b'x'; MAX_SHARED_FILE_BYTES as usize + 1];
        assert_eq!(check("big.log", &big), Err(Refusal::TooLarge));
    }

    #[test]
    fn the_store_keeps_its_limits() {
        let mut store = Store::default();
        let mb = |n: u64| vec![b'x'; (n * 1024 * 1024) as usize];
        // One viewer: 20 MB.
        store.add(Sharer::Viewer(1), "Mo", "a.log", mb(10), 0).unwrap();
        store.add(Sharer::Viewer(1), "Mo", "b.log", mb(9), 0).unwrap();
        assert_eq!(store.add(Sharer::Viewer(1), "Mo", "c.log", mb(2), 0), Err(Refusal::Full));
        // The session: 50 MB.
        store.add(Sharer::Viewer(2), "Sanne", "d.log", mb(10), 0).unwrap();
        store.add(Sharer::Host, "Nick", "e.log", mb(10), 0).unwrap();
        store.add(Sharer::Host, "Nick", "f.log", mb(9), 0).unwrap();
        // 48 MB are shared: 2 more fit, 3 do not.
        assert!(store.room(Sharer::Host, 2 * 1024 * 1024).is_ok());
        assert_eq!(store.room(Sharer::Host, 3 * 1024 * 1024), Err(Refusal::Full));
        // Removing makes room at once.
        store.remove(5, None).unwrap();
        assert!(store.room(Sharer::Host, 3 * 1024 * 1024).is_ok());
        assert_eq!(store.room(Sharer::Host, MAX_SHARED_FILE_BYTES + 1), Err(Refusal::TooLarge));
    }

    #[test]
    fn reservations_count_until_released() {
        let mut store = Store::default();
        let mb = |n: u64| n * 1024 * 1024;
        let upload = store.reserve(1, "a.log".into(), mb(9), [0; 32]).unwrap();
        store.reserve(1, "b.log".into(), mb(9), [0; 32]).unwrap();
        assert_eq!(store.reserve(1, "c.log".into(), mb(3), [0; 32]), Err(Refusal::Full));
        assert_eq!(store.release(upload).map(|r| r.viewer), Some(1));
        assert!(store.reserve(1, "c.log".into(), mb(3), [0; 32]).is_ok());
        assert_eq!(store.release_viewer(1).len(), 2);
        assert!(store.reserved.is_empty());
    }

    #[test]
    fn at_most_so_many_files() {
        let mut store = Store::default();
        for n in 0..MAX_FILES {
            store.add(Sharer::Host, "Nick", &format!("{n}.txt"), b"x".to_vec(), 0).unwrap();
        }
        assert_eq!(
            store.add(Sharer::Host, "Nick", "one-more.txt", b"x".to_vec(), 0),
            Err(Refusal::Full)
        );
    }

    #[test]
    fn removing_checks_who_asks_and_ids_are_never_reused() {
        let mut store = Store::default();
        let mine = store.add(Sharer::Viewer(1), "Mo", "a.txt", b"a".to_vec(), 0).unwrap().file;
        let theirs = store.add(Sharer::Viewer(2), "Sanne", "b.txt", b"b".to_vec(), 0).unwrap().file;
        assert_eq!(store.remove(theirs, Some(1)), Err(Refusal::Gone));
        assert_eq!(store.remove(mine, Some(1)).map(|f| f.file), Ok(mine));
        assert_eq!(store.remove(mine, Some(1)), Err(Refusal::Gone));
        assert_eq!(store.remove(theirs, None).map(|f| f.file), Ok(theirs));
        let next = store.add(Sharer::Host, "Nick", "c.txt", b"c".to_vec(), 0).unwrap().file;
        assert!(next > theirs);
        assert!(store.get(mine).is_none());
    }

    #[test]
    fn info_carries_the_hash_and_who() {
        let mut store = Store::default();
        let info = store.add(Sharer::Viewer(4), "Sanne", "../x.md", b"# x".to_vec(), 9).unwrap();
        assert_eq!(
            (info.name.as_str(), info.by, info.by_label.as_str(), info.at_ms),
            ("x.md", 4, "Sanne", 9)
        );
        assert_eq!(info.sha256, sha256(b"# x"));
        assert_eq!(store.list(), [info]);
    }

    fn session(root: &Path) -> SessionFolder {
        SessionFolder::new(root, None)
    }

    #[test]
    fn put_in_folder_writes_new_files_only() {
        let root = tempfile::tempdir().unwrap();
        let folder = session(root.path());
        assert_eq!(
            put_in_folder(&folder, root.path(), "wireframe.png", PNG).unwrap(),
            "shared/wireframe.png"
        );
        assert_eq!(std::fs::read(root.path().join("shared/wireframe.png")).unwrap(), PNG);
        assert_eq!(
            put_in_folder(&folder, root.path(), "wireframe.png", b"2").unwrap(),
            "shared/wireframe (2).png"
        );
        assert_eq!(
            put_in_folder(&folder, root.path(), "Makefile", b"a").unwrap(),
            "shared/Makefile"
        );
        assert_eq!(
            put_in_folder(&folder, root.path(), "Makefile", b"b").unwrap(),
            "shared/Makefile (2)"
        );
        assert_eq!(
            std::fs::read(root.path().join("shared/wireframe.png")).unwrap(),
            PNG,
            "never replaced"
        );
    }

    #[cfg(unix)]
    #[test]
    fn put_in_folder_never_follows_links_out() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let folder = session(root.path());
        std::os::unix::fs::symlink(elsewhere.path(), root.path().join("shared")).unwrap();
        assert!(matches!(
            put_in_folder(&folder, root.path(), "a.md", b"a"),
            Err(PutError::OutsideFolder)
        ));
        assert!(std::fs::read_dir(elsewhere.path()).unwrap().next().is_none());

        // A link planted at the target name is left alone, and the file goes to a free name.
        let root = tempfile::tempdir().unwrap();
        let folder = session(root.path());
        std::fs::create_dir(root.path().join("shared")).unwrap();
        let target = elsewhere.path().join("target");
        std::os::unix::fs::symlink(&target, root.path().join("shared/a.md")).unwrap();
        assert_eq!(put_in_folder(&folder, root.path(), "a.md", b"a").unwrap(), "shared/a (2).md");
        assert!(!target.exists());
    }

    #[test]
    fn put_in_folder_refuses_a_file_named_shared() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("shared"), b"x").unwrap();
        let result = put_in_folder(&session(root.path()), root.path(), "a.md", b"a");
        assert!(matches!(result, Err(PutError::Io(_) | PutError::NotAFolder)));
    }
}
