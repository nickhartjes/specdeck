//! Where the app and a session process meet (design decision 3): a Unix socket in a folder only
//! this user can read on macOS and Linux, and a named pipe only this user can open on Windows.
//! Either way, a connection carries frames.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::frame::{Decoder, Frame, FrameError};

/// A connection, split so one task can read while another writes.
pub struct Connection {
    pub reader: FrameReader,
    pub writer: FrameWriter,
}

impl Connection {
    fn new<S: AsyncRead + AsyncWrite + Send + 'static>(stream: S) -> Self {
        let (reader, writer) = tokio::io::split(stream);
        Connection {
            reader: FrameReader { reader: Box::new(reader), decoder: Decoder::default() },
            writer: FrameWriter { writer: Box::new(writer) },
        }
    }
}

pub struct FrameReader {
    reader: Box<dyn AsyncRead + Unpin + Send>,
    decoder: Decoder,
}

impl FrameReader {
    /// The next frame; `None` when the other side closed the connection.
    pub async fn recv(&mut self) -> io::Result<Option<Frame>> {
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if let Some(frame) = self.decoder.next_frame().map_err(invalid)? {
                return Ok(Some(frame));
            }
            let read = self.reader.read(&mut buffer).await?;
            if read == 0 {
                return Ok(None);
            }
            self.decoder.push(&buffer[..read]);
        }
    }
}

pub struct FrameWriter {
    writer: Box<dyn AsyncWrite + Unpin + Send>,
}

impl FrameWriter {
    pub async fn send(&mut self, frame: &Frame) -> io::Result<()> {
        self.writer.write_all(&frame.encode().map_err(invalid)?).await?;
        self.writer.flush().await
    }
}

fn invalid(error: FrameError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(unix)]
pub use unix::{Listener, connect, make_private_dir};

#[cfg(unix)]
mod unix {
    use std::io;
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    use tokio::net::{UnixListener, UnixStream};

    use super::Connection;

    /// A socket listening at a path in a private folder. Removes its file when dropped.
    pub struct Listener {
        inner: UnixListener,
        path: PathBuf,
    }

    impl Listener {
        /// Listens at `path`. Refused unless its folder belongs to this user and no one else can
        /// read, write or enter it ([`make_private_dir`]); a stale socket left there is replaced.
        pub fn bind(path: &Path) -> io::Result<Listener> {
            let dir = path.parent().ok_or_else(|| io::Error::other("a socket needs a folder"))?;
            check_private(dir)?;
            if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_socket()) {
                std::fs::remove_file(path)?;
            }
            let inner = UnixListener::bind(path)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Listener { inner, path: path.to_owned() })
        }

        /// The next connection from this user; connections from other users are dropped.
        pub async fn accept(&self) -> io::Result<Connection> {
            loop {
                let (stream, _) = self.inner.accept().await?;
                let same_user = stream.peer_cred().is_ok_and(|peer| peer.uid() == own_uid());
                if same_user {
                    return Ok(Connection::new(stream));
                }
            }
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    pub async fn connect(path: &Path) -> io::Result<Connection> {
        Ok(Connection::new(UnixStream::connect(path).await?))
    }

    /// Creates `dir` if needed and makes it this user's alone (mode 0700).
    pub fn make_private_dir(dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
    }

    fn check_private(dir: &Path) -> io::Result<()> {
        let meta = std::fs::symlink_metadata(dir)?;
        if !meta.is_dir() || meta.uid() != own_uid() || meta.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the socket's folder is not private to this user",
            ));
        }
        Ok(())
    }

    fn own_uid() -> u32 {
        // SAFETY: geteuid has no preconditions and cannot fail.
        unsafe { libc::geteuid() }
    }
}

#[cfg(windows)]
pub use windows::{Listener, connect, pipe_name};

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::io;
    use std::ptr;

    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_PIPE_BUSY, HANDLE, HLOCAL, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    use super::Connection;

    /// The pipe name for `tag` (a session's number), unique to this user: other users' pipes of
    /// the same session number are named differently.
    pub fn pipe_name(tag: &str) -> io::Result<String> {
        let sid = user_sid()?;
        let hash = sid.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
        Ok(format!(r"\\.\pipe\specdeck-{hash:016x}-{tag}"))
    }

    /// A named pipe only this user can open, refusing other machines. Creating it fails when a
    /// pipe of that name already exists, so another process cannot listen in its place.
    pub struct Listener {
        name: String,
        next: NamedPipeServer,
        security: Security,
    }

    impl Listener {
        pub fn bind(name: &str) -> io::Result<Listener> {
            let security = Security::user_only()?;
            let next = create(name, &security, true)?;
            Ok(Listener { name: name.to_owned(), next, security })
        }

        /// The next connection. Only this user can open the pipe, so every connection is theirs.
        pub async fn accept(&mut self) -> io::Result<Connection> {
            self.next.connect().await?;
            let fresh = create(&self.name, &self.security, false)?;
            let connected = std::mem::replace(&mut self.next, fresh);
            Ok(Connection::new(connected))
        }
    }

    fn create(name: &str, security: &Security, first: bool) -> io::Result<NamedPipeServer> {
        let mut options = ServerOptions::new();
        options.first_pipe_instance(first).reject_remote_clients(true);
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security.descriptor,
            bInheritHandle: 0,
        };
        // SAFETY: the attributes point at a security descriptor that lives as long as `security`,
        // and Windows only reads them while the pipe is created.
        unsafe {
            options
                .create_with_security_attributes_raw(name, (&raw mut attributes).cast::<c_void>())
        }
    }

    pub async fn connect(name: &str) -> io::Result<Connection> {
        loop {
            match ClientOptions::new().open(name) {
                Ok(client) => return Ok(Connection::new(client)),
                // Every instance is taken for a moment; the listener makes a new one.
                Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// A security descriptor that lets only the current user open the pipe.
    struct Security {
        descriptor: PSECURITY_DESCRIPTOR,
    }

    // SAFETY: the descriptor is allocated once, never changed, and freed when dropped.
    unsafe impl Send for Security {}
    unsafe impl Sync for Security {}

    impl Security {
        fn user_only() -> io::Result<Security> {
            // Protected DACL with one entry: full access for the current user.
            let sddl: Vec<u16> =
                format!("D:P(A;;GA;;;{})", user_sid()?).encode_utf16().chain([0]).collect();
            let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
            // SAFETY: `sddl` is NUL-terminated; on success Windows allocates the descriptor,
            // which `Drop` frees.
            let converted = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    ptr::null_mut(),
                )
            };
            if converted == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Security { descriptor })
        }
    }

    impl Drop for Security {
        fn drop(&mut self) {
            // SAFETY: allocated by ConvertStringSecurityDescriptorToSecurityDescriptorW.
            unsafe { LocalFree(self.descriptor as HLOCAL) };
        }
    }

    /// The current user's SID, as `S-1-5-21-…`.
    fn user_sid() -> io::Result<String> {
        // SAFETY: the token handle is closed before returning; the buffer is u64-aligned and
        // large enough, as GetTokenInformation reported; the string Windows allocates is freed.
        unsafe {
            let mut token: HANDLE = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut length = 0u32;
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut length);
            let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
            let read = GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                length,
                &mut length,
            );
            CloseHandle(token);
            if read == 0 {
                return Err(io::Error::last_os_error());
            }
            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            let mut wide: *mut u16 = ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut wide) == 0 {
                return Err(io::Error::last_os_error());
            }
            let length = (0..).take_while(|&i| *wide.add(i) != 0).count();
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(wide, length));
            LocalFree(wide as HLOCAL);
            Ok(sid)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_frame_goes_both_ways() {
        #[cfg(unix)]
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        let (endpoint, listener) = {
            let run = dir.path().join("run");
            make_private_dir(&run).unwrap();
            let path = run.join("1.sock");
            let listener = Listener::bind(&path).unwrap();
            (path, listener)
        };
        #[cfg(windows)]
        let (endpoint, mut listener) = {
            let name = pipe_name(&format!("test-{}", std::process::id())).unwrap();
            let listener = Listener::bind(&name).unwrap();
            (name, listener)
        };

        let server = tokio::spawn(async move {
            let mut connection = listener.accept().await.unwrap();
            let frame = connection.reader.recv().await.unwrap().unwrap();
            connection.writer.send(&Frame::Data(vec![1, 2, 3])).await.unwrap();
            frame
        });
        let mut client = connect(&endpoint).await.unwrap();
        let hello = Frame::control(&serde_json::json!({"type": "hello"})).unwrap();
        client.writer.send(&hello).await.unwrap();
        assert_eq!(client.reader.recv().await.unwrap(), Some(Frame::Data(vec![1, 2, 3])));
        assert_eq!(server.await.unwrap(), hello);
        // The listener is gone with the task: the other side sees the connection close.
        assert_eq!(client.reader.recv().await.unwrap(), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_socket_in_a_folder_others_can_read_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let open = dir.path().join("open");
        std::fs::create_dir(&open).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o755)).unwrap();
        let refused = Listener::bind(&open.join("1.sock")).err().unwrap();
        assert_eq!(refused.kind(), std::io::ErrorKind::PermissionDenied);
        assert!(!open.join("1.sock").exists(), "nothing is created");

        make_private_dir(&open).unwrap();
        assert!(Listener::bind(&open.join("1.sock")).is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_stale_socket_is_replaced_and_removed_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let run = dir.path().join("run");
        make_private_dir(&run).unwrap();
        let path = run.join("1.sock");
        // A socket left by a session that crashed.
        std::mem::forget(Listener::bind(&path).unwrap());
        let listener = Listener::bind(&path).unwrap();
        drop(listener);
        assert!(!path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_second_listener_on_the_same_pipe_is_refused() {
        let name = pipe_name(&format!("twice-{}", std::process::id())).unwrap();
        let _first = Listener::bind(&name).unwrap();
        assert!(Listener::bind(&name).is_err());
        assert!(name.starts_with(r"\\.\pipe\specdeck-"));
    }
}
