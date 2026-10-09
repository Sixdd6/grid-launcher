//! Where the image cache lives, and which directory the webview may read.
//!
//! The cover cache directory is resolved here once. `run()` passes it to
//! `SessionManager::new`, and `setup` grants the asset protocol the directory
//! the session reports back (`asset_scope_dir`), so the cache and the scope
//! cannot name different places.

use grid_core::session::SessionManager;
use std::path::{Path, PathBuf};

/// The cover cache directory: `<override>/covers` when `GRID_LAUNCHER_DATA_DIR`
/// is set (the caller passes `grid_core::config::data_dir_override()`),
/// otherwise the `ProjectDirs` cache directory plus `covers`.
pub(crate) fn covers_dir(data_dir_override: Option<PathBuf>) -> PathBuf {
    data_dir_override
        .map(|d| d.join("covers"))
        .unwrap_or_else(|| {
            directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
                .expect("home directory must exist")
                .cache_dir()
                .join("covers")
        })
}

/// The directory the asset protocol must allow: the exact directory the
/// session caches images in.
pub(crate) fn asset_scope_dir(session: &SessionManager) -> &Path {
    session.cache().dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_core::secrets::MemoryStore;
    use std::sync::Arc;

    #[test]
    fn override_wins_and_gets_a_covers_suffix() {
        let dir = PathBuf::from("data-root");
        assert_eq!(covers_dir(Some(dir.clone())), dir.join("covers"));
    }

    #[test]
    fn without_override_uses_the_project_dirs_cache_dir() {
        let expected = directories::ProjectDirs::from("io.github", "Sixdd6", "grid-launcher")
            .expect("home directory must exist")
            .cache_dir()
            .join("covers");
        assert_eq!(covers_dir(None), expected);
    }

    #[test]
    fn asset_scope_is_the_directory_the_session_caches_in() {
        for dir in [
            covers_dir(None),
            covers_dir(Some(PathBuf::from("data-root"))),
        ] {
            let session = SessionManager::new(
                PathBuf::from("config.toml"),
                dir.clone(),
                Arc::new(MemoryStore::default()),
            );
            assert_eq!(asset_scope_dir(&session), dir.as_path());
        }
    }
}
