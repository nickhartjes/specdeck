//! The whole text of code files the agent changes, while the host shares whole code files
//! (`files-documents-and-code` design decision 4). Whether a file's text may leave the host is
//! [`FileRule`]'s to decide, for file tools and shell commands alike; this module reads and decodes
//! the files that pass it.

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::documents::{self, DocumentText, MAX_DOCUMENT_BYTES};
use crate::file_rule::FileRule;
use crate::hooks::HookPayload;

/// Claude Code's tools that change a file named by `file_path`.
const FILE_TOOLS: [&str; 3] = ["Write", "Edit", "MultiEdit"];

/// Decides which code files may be sent whole, and reads them.
pub struct CodeFiles {
    rule: Arc<FileRule>,
}

impl CodeFiles {
    pub fn new(rule: Arc<FileRule>) -> Self {
        Self { rule }
    }

    /// Whether the text of `path` may be sent: a code file (documents are sent as documents),
    /// whose content [`FileRule`] shows.
    pub fn may_send(&self, path: &str) -> bool {
        !documents::is_document(path) && self.rule.shown(path).is_some()
    }

    /// The code file one of Claude Code's file tools just changed, read from disk, when the host
    /// shares whole code files and the file may be sent. Must run before the hook is answered,
    /// so the text is the one the change produced. The file read is the one the rule checked.
    pub fn from_file_tool(&self, payload: &HookPayload, whole_files: bool) -> Option<DocumentText> {
        if !whole_files {
            return None;
        }
        let text = |key: &str| payload.get(key).and_then(Value::as_str);
        if text("hook_event_name")? != "PostToolUse" || !FILE_TOOLS.contains(&text("tool_name")?) {
            return None;
        }
        let path = payload.get("tool_input")?.get("file_path")?.as_str()?;
        if documents::is_document(path) {
            return None;
        }
        let shown = self.rule.shown(path)?;
        let text = read(&shown.real)?;
        Some(DocumentText { path: self.rule.folder().absolute_path(path), ..text })
    }
}

/// The text of `path` on disk, a path the rule resolved: `None` when it cannot be read or is not
/// text, and a `DocumentText` without text when it is larger than [`MAX_DOCUMENT_BYTES`].
pub fn read(path: &Path) -> Option<DocumentText> {
    let size = std::fs::metadata(path).ok()?.len();
    let text =
        if size > MAX_DOCUMENT_BYTES { None } else { Some(decode(std::fs::read(path).ok()?)?) };
    Some(DocumentText { path: path.to_string_lossy().into_owned(), text })
}

/// `bytes` as text, or `None` when they are not: invalid UTF-8 or a NUL byte. Unlike documents,
/// not decoded lossily, which would show viewers something that is not in the file.
pub fn decode(bytes: Vec<u8>) -> Option<String> {
    if bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;
    use serde_json::json;
    use std::process::Command;

    fn post(tool: &str, path: &Path) -> HookPayload {
        json!({
            "hook_event_name": "PostToolUse", "prompt_id": "p", "tool_use_id": "t",
            "tool_name": tool, "tool_input": { "file_path": path }
        })
        .as_object()
        .unwrap()
        .clone()
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(dir).output().unwrap().status;
        assert!(status.success(), "git {args:?}");
    }

    fn code_files(dir: &Path) -> CodeFiles {
        CodeFiles::new(Arc::new(FileRule::new(dir, &Environment::from_process())))
    }

    fn path(dir: &Path, name: &str) -> String {
        dir.join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn an_ordinary_code_file_may_be_sent() {
        let dir = tempfile::tempdir().unwrap();
        let files = code_files(dir.path());
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        // Outside a git repository nothing is ignored.
        assert!(files.may_send(&path(dir.path(), "src/main.rs")));
        assert!(files.may_send("src/main.rs"), "relative to the session folder");
        // Documents have their own rule.
        assert!(!files.may_send(&path(dir.path(), "README.md")));
    }

    #[test]
    fn files_outside_the_session_folder_are_not_sent() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("project");
        std::fs::create_dir(&project).unwrap();
        std::fs::write(home.path().join("secret.rs"), "const KEY: &str = \"x\";").unwrap();
        assert!(!code_files(&project).may_send(&path(home.path(), "secret.rs")));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_pointing_outside_is_not_sent() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("project");
        let other = home.path().join("other");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&other).unwrap();
        std::fs::write(other.join("app.toml"), "token = 1").unwrap();
        std::os::unix::fs::symlink(other.join("app.toml"), project.join("app.toml")).unwrap();
        std::os::unix::fs::symlink(&other, project.join("linked")).unwrap();
        let files = code_files(&project);
        assert!(!files.may_send(&path(&project, "app.toml")));
        assert!(!files.may_send(&path(&project, "linked/app.toml")));
    }

    #[test]
    fn private_files_are_not_sent() {
        let dir = tempfile::tempdir().unwrap();
        let files = code_files(dir.path());
        for name in [".env", ".env.local", "server.pem", "id_ed25519", ".npmrc"] {
            std::fs::write(dir.path().join(name), "secret").unwrap();
            assert!(!files.may_send(&path(dir.path(), name)), "{name}");
        }
    }

    #[test]
    fn ignored_files_are_not_sent() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        std::fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
        std::fs::create_dir(dir.path().join("build")).unwrap();
        std::fs::write(dir.path().join("build/x.ts"), "export {}").unwrap();
        std::fs::write(dir.path().join("main.ts"), "export {}").unwrap();
        let files = code_files(dir.path());
        assert!(!files.may_send(&path(dir.path(), "build/x.ts")));
        assert!(files.may_send(&path(dir.path(), "main.ts")));
    }

    #[test]
    fn only_text_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let text = path(dir.path(), "a.rs");
        std::fs::write(&text, "fn a() {}\n").unwrap();
        assert_eq!(read(Path::new(&text)).unwrap().text.as_deref(), Some("fn a() {}\n"));
        let nul = path(dir.path(), "b.bin");
        std::fs::write(&nul, b"ab\0cd").unwrap();
        assert_eq!(read(Path::new(&nul)), None);
        let latin1 = path(dir.path(), "c.txt");
        std::fs::write(&latin1, b"caf\xe9").unwrap();
        assert_eq!(read(Path::new(&latin1)), None);
        assert_eq!(read(Path::new(&path(dir.path(), "gone.rs"))), None);
    }

    #[test]
    fn large_files_come_without_text() {
        let dir = tempfile::tempdir().unwrap();
        let big = path(dir.path(), "big.rs");
        std::fs::write(&big, "x".repeat(MAX_DOCUMENT_BYTES as usize + 1)).unwrap();
        assert_eq!(read(Path::new(&big)).unwrap().text, None);
        std::fs::write(&big, "x".repeat(MAX_DOCUMENT_BYTES as usize)).unwrap();
        assert!(read(Path::new(&big)).unwrap().text.is_some());
    }

    #[test]
    fn file_tools_give_the_text_only_while_whole_files_are_shared() {
        let dir = tempfile::tempdir().unwrap();
        let files = code_files(dir.path());
        let source = dir.path().join("server.ts");
        std::fs::write(&source, "export {}").unwrap();
        for tool in ["Write", "Edit", "MultiEdit"] {
            assert_eq!(
                files.from_file_tool(&post(tool, &source), true),
                Some(DocumentText {
                    path: source.to_string_lossy().into(),
                    text: Some("export {}".into())
                })
            );
        }
        assert_eq!(files.from_file_tool(&post("Edit", &source), false), None);
        assert_eq!(files.from_file_tool(&post("Read", &source), true), None);
        let mut pre = post("Edit", &source);
        pre.insert("hook_event_name".into(), json!("PreToolUse"));
        assert_eq!(files.from_file_tool(&pre, true), None);
        let env = dir.path().join(".env");
        std::fs::write(&env, "TOKEN=1").unwrap();
        assert_eq!(files.from_file_tool(&post("Edit", &env), true), None);
        assert_eq!(files.from_file_tool(&post("Write", &dir.path().join("gone.rs")), true), None);
    }
}
