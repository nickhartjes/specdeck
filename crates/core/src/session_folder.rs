//! Which paths are inside the session folder, and how a path is shown to viewers
//! (`limit-viewer-data` design decision 1). Only files inside the folder are shared in full;
//! elsewhere viewers get the path alone, shortened to `~/…` in the home folder.

use std::path::{Component, Path, PathBuf};

use crate::environment::Environment;

#[derive(Clone, Debug)]
pub struct SessionFolder {
    /// The folder as the session was started in it.
    folder: PathBuf,
    /// The same folder with symlinks resolved, as the agent may report it (`/private/var` for
    /// `/var` on macOS); `None` when it cannot be resolved.
    resolved: Option<PathBuf>,
    /// The host's home folder and its resolved form, for showing paths as `~/…`.
    home: Option<(PathBuf, Option<PathBuf>)>,
}

impl SessionFolder {
    pub fn new(folder: impl Into<PathBuf>, home: Option<PathBuf>) -> Self {
        let folder = folder.into();
        let resolved = folder.canonicalize().ok();
        let home = home.filter(|home| home.is_absolute()).map(|home| {
            let resolved = home.canonicalize().ok();
            (home, resolved)
        });
        Self { folder, resolved, home }
    }

    /// The session folder, with the home folder from the agent's environment.
    pub fn from_environment(folder: &Path, env: &Environment) -> Self {
        let home = env.get("HOME").or_else(|| env.get("USERPROFILE")).filter(|dir| !dir.is_empty());
        Self::new(folder, home.map(PathBuf::from))
    }

    /// Whether `path` is inside the folder once symlinks are resolved. A path that does not exist
    /// counts by its nearest existing parent; one that cannot be resolved at all, by its text.
    pub fn contains(&self, path: &str) -> bool {
        let path = self.absolute(path);
        match (resolve(&path), &self.resolved) {
            (Some(path), Some(folder)) => below(&path, folder).is_some(),
            _ => below(&path, &self.folder).is_some(),
        }
    }

    /// `path` as viewers see it: relative inside the folder, `~/…` elsewhere in the home folder,
    /// otherwise unchanged.
    pub fn display(&self, path: &str) -> String {
        let full = self.absolute(path);
        if self.contains(path) {
            let resolved = resolve(&full);
            let inside = below(&full, &self.folder)
                .or_else(|| self.resolved.as_deref().and_then(|folder| below(&full, folder)))
                .or_else(|| below(resolved.as_deref()?, self.resolved.as_deref()?));
            if let Some(inside) = inside {
                return inside.to_string_lossy().into_owned();
            }
        }
        if let Some((home, resolved_home)) = &self.home {
            let in_home = below(&full, home).or_else(|| below(&full, resolved_home.as_deref()?));
            if let Some(rest) = in_home {
                return Path::new("~").join(rest).to_string_lossy().into_owned();
            }
        }
        path.to_owned()
    }

    fn absolute(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() { path.to_owned() } else { self.folder.join(path) }
    }
}

/// `path` below `folder`, by its text: only plain names after the folder, and at least one.
fn below<'a>(path: &'a Path, folder: &Path) -> Option<&'a Path> {
    let inside = path.strip_prefix(folder).ok()?;
    let plain = !inside.as_os_str().is_empty()
        && inside.components().all(|c| matches!(c, Component::Normal(_)));
    plain.then_some(inside)
}

/// `path` with symlinks resolved, through its nearest existing parent for a path that does not
/// exist (yet, or any more). `None` when no parent exists, or a missing part is `..`.
fn resolve(path: &Path) -> Option<PathBuf> {
    let mut existing = path;
    let mut rest = Vec::new();
    loop {
        if let Ok(resolved) = existing.canonicalize() {
            return Some(rest.iter().rev().fold(resolved, |path, name| path.join(name)));
        }
        // `file_name` is `None` for a path ending in `..`, which cannot be resolved by its text.
        rest.push(existing.file_name()?);
        existing = existing.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(path: &Path, home: Option<&Path>) -> SessionFolder {
        SessionFolder::new(path, home.map(Path::to_owned))
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn inside_and_outside() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("project");
        std::fs::create_dir_all(project.join("src")).unwrap();
        std::fs::write(project.join("src/main.rs"), "").unwrap();
        let session = folder(&project, Some(home.path()));

        assert!(session.contains(&text(&project.join("src/main.rs"))));
        assert_eq!(
            session.display(&text(&project.join("src/main.rs"))),
            Path::new("src").join("main.rs").to_string_lossy()
        );
        assert!(session.contains("src/main.rs"));

        let notes = home.path().join(".claude/notes.md");
        assert!(!session.contains(&text(&notes)));
        assert_eq!(
            session.display(&text(&notes)),
            Path::new("~").join(".claude").join("notes.md").to_string_lossy()
        );
    }

    #[test]
    fn outside_the_home_folder_is_unchanged() {
        let project = tempfile::tempdir().unwrap();
        let session = folder(project.path(), Some(Path::new("/home/someone")));
        assert!(!session.contains("/etc/hosts"));
        assert_eq!(session.display("/etc/hosts"), "/etc/hosts");
    }

    #[test]
    fn without_a_home_folder_outside_paths_are_unchanged() {
        let project = tempfile::tempdir().unwrap();
        let session = folder(project.path(), None);
        let outside = text(&std::env::temp_dir().join("elsewhere.md"));
        assert_eq!(session.display(&outside), outside);
    }

    #[test]
    fn a_file_that_does_not_exist_yet_counts_by_its_folder() {
        let project = tempfile::tempdir().unwrap();
        let session = folder(project.path(), None);
        let new = project.path().join("docs/new/plan.md");
        assert!(session.contains(&text(&new)));
        assert_eq!(
            session.display(&text(&new)),
            Path::new("docs").join("new").join("plan.md").to_string_lossy()
        );
    }

    #[test]
    fn dot_dot_leaves_the_folder() {
        let parent = tempfile::tempdir().unwrap();
        let project = parent.path().join("app");
        std::fs::create_dir(&project).unwrap();
        let session = folder(&project, None);
        let escape = text(&project.join("..").join(".env"));
        assert!(!session.contains(&escape));
        assert!(!session.contains("../.env"));
        assert_eq!(session.display("../.env"), "../.env");
    }

    #[test]
    fn a_folder_that_cannot_be_resolved_is_compared_by_text() {
        let session = SessionFolder::new("/work", None);
        assert!(session.contains("/work/src/a.rs"));
        assert_eq!(
            session.display("/work/src/a.rs"),
            Path::new("src").join("a.rs").to_string_lossy()
        );
        assert!(!session.contains("/elsewhere/a.rs"));
        assert!(!session.contains("/work"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_session_folder() {
        let real = tempfile::tempdir().unwrap();
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("project");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        let session = folder(&link, None);
        let resolved = real.path().canonicalize().unwrap().join("docs/a.md");
        assert!(session.contains(&text(&resolved)));
        assert_eq!(session.display(&text(&resolved)), "docs/a.md");
        assert_eq!(session.display(&text(&link.join("b.md"))), "b.md");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_inside_pointing_outside_is_outside() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("project");
        let secrets = home.path().join("secrets");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&secrets).unwrap();
        std::fs::write(secrets.join("key.md"), "").unwrap();
        std::os::unix::fs::symlink(&secrets, project.join("linked")).unwrap();
        let session = folder(&project, Some(home.path()));
        let through_link = text(&project.join("linked/key.md"));
        assert!(!session.contains(&through_link));
        assert_eq!(session.display(&through_link), "~/project/linked/key.md");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn var_and_private_var_are_the_same_folder() {
        let project = tempfile::tempdir_in("/var/tmp").unwrap();
        let session = folder(project.path(), None);
        let private = format!("/private{}", text(&project.path().join("a.md")));
        assert!(session.contains(&private));
        assert_eq!(session.display(&private), "a.md");
    }
}
