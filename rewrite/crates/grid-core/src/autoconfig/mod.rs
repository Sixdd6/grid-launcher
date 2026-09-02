//! Emulator autoconfiguration: the `ensure_*` writers that seed an
//! emulator's own settings files so a launched game finds its saves,
//! firmware, controller profile and RetroAchievements login where GRID
//! expects them.
//!
//! Ports `grid_launcher/emulator/*.py`'s `ensure_*` functions. See
//! `docs/porting/05-emulator-autoconfig.md` for the behavior contract; the
//! shared section-writer families live in [`writers`] and the path helpers
//! every module's candidate list is built from live in [`paths`].

pub mod paths;
pub mod writers;

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Every `ensure_*` writer's return value.
///
/// Spec deviation D8: Python returns a `str`, a `Path`, or a `dict`
/// depending on the module — a dynamic-typing artifact, not a behavior. One
/// struct carries all three shapes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnsureResult {
    /// True when this call wrote at least one file.
    pub changed: bool,
    /// The primary file the writer targeted. `None` when the writer bailed
    /// out (blank path, missing executable, unreadable or unwritable file).
    pub config_path: Option<PathBuf>,
    /// Secondary files a writer also owns. Documented keys, and only these:
    ///   dolphin -> "gfx_ini_path", "gcpad_ini_path"
    ///   rpcs3   -> "gui_config_path", "current_settings_path", "vfs_path"
    ///   cemu    -> "profile_path"
    ///   ppsspp  -> "ra_token_path"
    pub extras: BTreeMap<String, PathBuf>,
}

impl EnsureResult {
    /// The bail-out result: nothing written, no path to report.
    pub fn unchanged() -> Self {
        Self::default()
    }

    /// A result naming the primary config file the writer targeted.
    pub fn at(path: impl Into<PathBuf>, changed: bool) -> Self {
        Self {
            changed,
            config_path: Some(path.into()),
            extras: BTreeMap::new(),
        }
    }

    /// Record a secondary file this writer owns. Chainable.
    pub fn with_extra(mut self, key: &str, path: impl Into<PathBuf>) -> Self {
        self.extras.insert(key.to_string(), path.into());
        self
    }

    /// Fold another write's outcome in: `self.changed |= other`.
    pub fn merge_changed(&mut self, other: bool) {
        self.changed |= other;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_result_unchanged_is_all_default() {
        let result = EnsureResult::unchanged();
        assert!(!result.changed);
        assert_eq!(result.config_path, None);
        assert!(result.extras.is_empty());
    }

    #[test]
    fn ensure_result_at_records_path_and_extras() {
        let result =
            EnsureResult::at("/tmp/PCSX2.ini", true).with_extra("gfx_ini_path", "/tmp/GFX.ini");
        assert!(result.changed);
        assert_eq!(result.config_path, Some(PathBuf::from("/tmp/PCSX2.ini")));
        assert_eq!(
            result.extras.get("gfx_ini_path"),
            Some(&PathBuf::from("/tmp/GFX.ini"))
        );
    }

    /// The writers are consumed from sibling modules (one per emulator), so
    /// `desired!` must resolve through the crate root, not only inside
    /// `writers.rs` where `macro_rules!` puts it in textual scope.
    #[test]
    fn desired_macro_is_usable_from_another_module() {
        let want = crate::desired![("Key", "value"), ("Other", "2"),];
        assert_eq!(
            want,
            vec![
                ("Key".to_string(), "value".to_string()),
                ("Other".to_string(), "2".to_string()),
            ]
        );
        let empty: writers::Desired = crate::desired![];
        assert!(empty.is_empty());
    }

    #[test]
    fn ensure_result_merge_changed_is_sticky() {
        let mut result = EnsureResult::at("/tmp/x.ini", false);
        result.merge_changed(false);
        assert!(!result.changed);
        result.merge_changed(true);
        assert!(result.changed);
        result.merge_changed(false);
        assert!(result.changed, "merge_changed must never clear a set flag");
    }
}
