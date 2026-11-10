//! The whole text of documents the agent changes, so participants can read them as documents and
//! not only as diffs (`viewer-views` design decision 2). Only Markdown files count, and only the
//! ones the agent itself created or changed.

use std::path::Path;

use serde_json::Value;

use crate::file_rule::FileRule;
use crate::hooks::HookPayload;

/// Larger documents are announced without their text; their diffs are still sent.
pub const MAX_DOCUMENT_BYTES: u64 = 200 * 1024;

/// Claude Code's tools that change a file named by `file_path`.
const FILE_TOOLS: [&str; 3] = ["Write", "Edit", "MultiEdit"];

/// A changed document's text as it was right after the change; `None` when it is too large.
/// `path` is absolute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentText {
    pub path: String,
    pub text: Option<String>,
}

/// Whether participants can read `path` as a document.
pub fn is_document(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

/// The text of `path` on disk, a path the rule resolved; `None` when it cannot be read (it was
/// deleted, for example).
pub fn read(path: &Path) -> Option<DocumentText> {
    let size = std::fs::metadata(path).ok()?.len();
    let text = if size > MAX_DOCUMENT_BYTES {
        None
    } else {
        Some(String::from_utf8_lossy(&std::fs::read(path).ok()?).into_owned())
    };
    Some(DocumentText { path: path.to_string_lossy().into_owned(), text })
}

/// The document one of Claude Code's file tools just changed, read from disk. Must run before the
/// hook is answered, so the text is the one the change produced. Only documents whose content
/// [`FileRule`] shows are read, and the file read is the one it checked: otherwise viewers learn
/// only that the file changed. The text's path is the agent's, made absolute.
pub fn from_file_tool(payload: &HookPayload, rule: &FileRule) -> Option<DocumentText> {
    let text = |key: &str| payload.get(key).and_then(Value::as_str);
    if text("hook_event_name")? != "PostToolUse" || !FILE_TOOLS.contains(&text("tool_name")?) {
        return None;
    }
    let path = payload.get("tool_input")?.get("file_path")?.as_str()?;
    if !is_document(path) {
        return None;
    }
    let document = read(&rule.shown(path)?.real)?;
    Some(DocumentText { path: rule.folder().absolute_path(path), ..document })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;
    use crate::session_folder::SessionFolder;
    use serde_json::json;

    fn post(tool: &str, path: &Path) -> HookPayload {
        json!({
            "hook_event_name": "PostToolUse", "prompt_id": "p", "tool_use_id": "t",
            "tool_name": tool, "tool_input": { "file_path": path }
        })
        .as_object()
        .unwrap()
        .clone()
    }

    #[test]
    fn markdown_files_are_documents() {
        assert!(is_document("openspec/changes/x/proposal.md"));
        assert!(is_document("/repo/README.MD"));
        assert!(is_document("notes.markdown"));
        assert!(!is_document("src/server.ts"));
        assert!(!is_document("Makefile"));
    }

    fn session(dir: &Path) -> FileRule {
        FileRule::from(SessionFolder::new(dir, None))
    }

    fn with_git(dir: &Path) -> FileRule {
        FileRule::new(dir, &Environment::from_process())
    }

    fn payload(tool: &str, path: &str) -> HookPayload {
        json!({
            "hook_event_name": "PostToolUse", "prompt_id": "p", "tool_use_id": "t",
            "tool_name": tool, "tool_input": { "file_path": path }
        })
        .as_object()
        .unwrap()
        .clone()
    }

    #[test]
    fn a_relative_path_is_read_in_the_session_folder() {
        // A folder below the tests' working directory, as a session process's working directory
        // can hold files with the same relative path as the agent reports.
        let working = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let relative = working.path().file_name().unwrap().to_string_lossy().into_owned();
        std::fs::create_dir(working.path().join("secret")).unwrap();
        std::fs::write(working.path().join("secret/notes.md"), "# Private").unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let named = format!("{relative}/secret/notes.md");
        assert!(Path::new(&named).exists(), "below the working directory");
        assert_eq!(from_file_tool(&payload("Write", &named), &session(elsewhere.path())), None);

        let project = tempfile::tempdir().unwrap();
        std::fs::create_dir(project.path().join("docs")).unwrap();
        std::fs::write(project.path().join("docs/plan.md"), "# Plan").unwrap();
        assert_eq!(
            from_file_tool(&payload("Edit", "docs/plan.md"), &session(project.path())),
            Some(DocumentText {
                path: project.path().join("docs/plan.md").to_string_lossy().into(),
                text: Some("# Plan".into())
            })
        );
    }

    #[test]
    fn private_documents_are_not_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("credentials.md"), "token").unwrap();
        let path = dir.path().join("credentials.md");
        assert_eq!(from_file_tool(&post("Write", &path), &session(dir.path())), None);
    }

    #[cfg(unix)]
    #[test]
    fn ignored_documents_are_not_read_tracked_ones_are() {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(args)
                .current_dir(dir.path())
                .output()
                .unwrap()
                .status;
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        std::fs::write(dir.path().join(".gitignore"), "*.local.md\n").unwrap();
        std::fs::write(dir.path().join("README.md"), "# Readme").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "first"]);
        std::fs::write(dir.path().join("notes.local.md"), "# Mine").unwrap();
        let rule = with_git(dir.path());
        assert_eq!(from_file_tool(&post("Write", &dir.path().join("notes.local.md")), &rule), None);
        let readme = from_file_tool(&post("Edit", &dir.path().join("README.md")), &rule);
        assert_eq!(readme.and_then(|doc| doc.text).as_deref(), Some("# Readme"));
    }

    #[test]
    fn reads_the_text_an_edit_left_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("design.md");
        std::fs::write(&path, "# Design\n\nNew section.\n").unwrap();
        for tool in ["Write", "Edit", "MultiEdit"] {
            assert_eq!(
                from_file_tool(&post(tool, &path), &session(dir.path())),
                Some(DocumentText {
                    path: path.to_string_lossy().into(),
                    text: Some("# Design\n\nNew section.\n".into())
                })
            );
        }
    }

    #[test]
    fn source_files_reads_and_failures_give_no_document() {
        let dir = tempfile::tempdir().unwrap();
        let folder = session(dir.path());
        let source = dir.path().join("server.ts");
        std::fs::write(&source, "export {}").unwrap();
        assert_eq!(from_file_tool(&post("Edit", &source), &folder), None);
        let doc = dir.path().join("a.md");
        std::fs::write(&doc, "# A").unwrap();
        assert_eq!(from_file_tool(&post("Read", &doc), &folder), None);
        let mut pre = post("Edit", &doc);
        pre.insert("hook_event_name".into(), json!("PreToolUse"));
        assert_eq!(from_file_tool(&pre, &folder), None);
        // A failed or deleted file: nothing to read.
        assert_eq!(from_file_tool(&post("Write", &dir.path().join("gone.md")), &folder), None);
    }

    #[test]
    fn documents_outside_the_session_folder_are_not_read() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let memory = home.path().join("memory.md");
        std::fs::write(&memory, "# Private").unwrap();
        assert_eq!(from_file_tool(&post("Write", &memory), &session(&project)), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_document_through_a_symlink_pointing_outside_is_not_read() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("project");
        let notes = home.path().join("notes");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&notes).unwrap();
        std::fs::write(notes.join("plan.md"), "# Private").unwrap();
        std::os::unix::fs::symlink(&notes, project.join("notes")).unwrap();
        let through_link = project.join("notes/plan.md");
        assert_eq!(from_file_tool(&post("Edit", &through_link), &session(&project)), None);
    }

    #[test]
    fn large_documents_come_without_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.md");
        std::fs::write(&path, "x".repeat(MAX_DOCUMENT_BYTES as usize + 1)).unwrap();
        assert_eq!(read(&path).unwrap().text, None);
        std::fs::write(&path, "x".repeat(MAX_DOCUMENT_BYTES as usize)).unwrap();
        assert!(read(&path).unwrap().text.is_some());
    }
}
