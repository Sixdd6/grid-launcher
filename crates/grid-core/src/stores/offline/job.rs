//! The offline installer's Downloads job: [`super::install_job`]'s runner.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::exec::{InstallerSpawner, Watch};
use super::{
    base_title_guess, decide, default_prefix, gog_row_for, installed_infos, needs_prefix,
    row_platform, run_into_with, Inspection, OfflineOpts, Verdict,
};
use crate::library::paths::sanitize_component;
use crate::library::queue::{StoreJobRunner, StoreProgressSink};
use crate::library::registry::InstalledGame;
use crate::library::LibraryError;
use crate::stores::gog::goggame::GogGameInfo;
use crate::stores::gog::meta::{GogDlcMeta, GogMeta, GogOrigin, GogPlatform};
use crate::stores::gog::row::gog_row;
use crate::stores::gog::GogJobCtx;

/// The folder under the library that GOG games go to by default
/// (`[stores.gog] install_dir = ""`).
pub const GOG_FOLDER: &str = "GOG";

pub(super) struct OfflineRunner {
    pub insp: Inspection,
    pub opts: OfflineOpts,
    pub ctx: GogJobCtx,
    pub wizard_allowed: Arc<dyn Fn() -> bool + Send + Sync>,
    pub spawner: Arc<dyn InstallerSpawner>,
    pub watch: Watch,
}

impl StoreJobRunner for OfflineRunner {
    /// Nothing to download: checks the installer files are still there.
    fn download(
        &self,
        _cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64, u64, f64),
    ) -> Result<(), LibraryError> {
        for file in self.insp.files() {
            if !file.is_file() {
                return Err(LibraryError::InstallerRefused(format!(
                    "The installer isn't at {} any more.",
                    file.display()
                )));
            }
        }
        progress(self.insp.bytes, self.insp.bytes, 0.0);
        Ok(())
    }

    fn finalize(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        let registry = &self.ctx.registry;
        // A DLC whose base went away since the dialog: the card's action
        // installs the base.
        if let Some(info) = self.insp.info.as_ref().filter(|info| info.is_dlc()) {
            if gog_row_for(registry, &info.root_game_id)?.is_none() {
                return Err(LibraryError::InstallerDlcBaseMissing {
                    base_title: base_title_guess(&info.name),
                    base_gog_id: info.root_game_id.clone(),
                });
            }
        }
        match decide(&self.insp, registry)? {
            refused @ Verdict::Refused { .. } => Err(LibraryError::InstallerRefused(
                super::refusal_message(&refused),
            )),
            Verdict::Install => self.install_fresh(sink),
            Verdict::Update { .. } => self.install_update(sink),
            Verdict::Dlc { .. } => self.install_dlc(sink),
        }
    }
}

impl OfflineRunner {
    fn compat(&self) -> String {
        let chosen = self.opts.compat_tool.trim();
        if chosen.is_empty() {
            self.ctx.default_compat_tool.trim().to_string()
        } else {
            chosen.to_string()
        }
    }

    fn platform(&self) -> GogPlatform {
        self.insp.kind.platform().unwrap_or_default()
    }

    fn run(
        &self,
        dest: &Path,
        prefix: Option<&Path>,
        sink: &mut dyn StoreProgressSink,
    ) -> Result<GogGameInfo, LibraryError> {
        run_into_with(
            &self.insp,
            dest,
            prefix,
            &self.compat(),
            (self.wizard_allowed)(),
            sink,
            self.spawner.as_ref(),
            self.watch,
        )
    }

    /// The prefix a run into `dest` uses: the row's own when it has one.
    fn prefix_for(&self, dest: &Path, row: Option<&InstalledGame>) -> Option<PathBuf> {
        if !needs_prefix(&self.insp.kind) {
            return None;
        }
        let own = row
            .map(|row| row.native_wineprefix.trim().to_string())
            .filter(|prefix| !prefix.is_empty());
        Some(
            own.map(PathBuf::from)
                .unwrap_or_else(|| default_prefix(dest)),
        )
    }

    fn install_fresh(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        let dest = self.opts.install_dir.clone().unwrap_or_else(|| {
            self.ctx
                .library
                .join(GOG_FOLDER)
                .join(sanitize_component(&self.insp.title(), "game"))
        });
        let fresh = is_empty_dir(&dest);
        let prefix = self.prefix_for(&dest, None);
        let info = match self.run(&dest, prefix.as_deref(), sink) {
            Ok(info) => info,
            Err(error) => {
                if fresh {
                    remove_install(&dest, prefix.as_deref());
                }
                return Err(error);
            }
        };
        if info.is_dlc() {
            return self.adopt_dlc(&info, &dest, prefix.as_deref());
        }
        let old = gog_row_for(&self.ctx.registry, &info.game_id)?;
        self.write_game_row(&info, &dest, prefix.as_deref(), old)
    }

    fn install_update(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        let id = self
            .insp
            .info
            .as_ref()
            .map(|info| info.game_id.clone())
            .unwrap_or_default();
        let old = gog_row_for(&self.ctx.registry, &id)?
            .ok_or_else(|| LibraryError::Registry("not installed".to_string()))?;
        let dest = PathBuf::from(old.extracted_dir.trim());
        let prefix = self.prefix_for(&dest, Some(&old));
        let info = self.run(&dest, prefix.as_deref(), sink)?;
        self.write_game_row(&info, &dest, prefix.as_deref(), Some(old))
    }

    /// A DLC installer runs in the base game's folder (its `/DIR`) and
    /// Wine prefix (spike 5).
    fn install_dlc(&self, sink: &mut dyn StoreProgressSink) -> Result<String, LibraryError> {
        let dlc = self.insp.info.clone().unwrap_or_default();
        let base = gog_row_for(&self.ctx.registry, &dlc.root_game_id)?.ok_or_else(|| {
            LibraryError::InstallerDlcBaseMissing {
                base_title: base_title_guess(&dlc.name),
                base_gog_id: dlc.root_game_id.clone(),
            }
        })?;
        let dest = PathBuf::from(base.extracted_dir.trim());
        let prefix = self.prefix_for(&dest, Some(&base));
        let installed = self.run(&dest, prefix.as_deref(), sink)?;
        let info = if installed.is_dlc() { installed } else { dlc };
        self.record_dlc(base, &info, &dest)
    }

    /// A fresh install whose goggame file says DLC (its identity was only
    /// known after the run): its files join the base game's folder.
    fn adopt_dlc(
        &self,
        info: &GogGameInfo,
        dest: &Path,
        prefix: Option<&Path>,
    ) -> Result<String, LibraryError> {
        let base = gog_row_for(&self.ctx.registry, &info.root_game_id)?;
        let Some(base) = base else {
            remove_install(dest, prefix);
            return Err(LibraryError::InstallerDlcBaseMissing {
                base_title: base_title_guess(&info.name),
                base_gog_id: info.root_game_id.clone(),
            });
        };
        let installed = row_platform(&base);
        if installed != self.platform() {
            remove_install(dest, prefix);
            return Err(LibraryError::InstallerRefused(super::refusal_message(
                &super::other_os_dlc_refusal(&base.title, installed, self.platform()),
            )));
        }
        let base_dir = PathBuf::from(base.extracted_dir.trim());
        // The run's own prefix stays out of the base game's folder (the base
        // keeps its prefix).
        if let Some(prefix) = prefix.filter(|prefix| prefix.starts_with(dest)) {
            let _ = fs::remove_dir_all(prefix);
        }
        crate::library::specials::copy_tree_merge(dest, &base_dir)?;
        remove_install(dest, prefix);
        self.record_dlc(base, info, &base_dir)
    }

    fn installer_paths(&self) -> Vec<String> {
        self.insp
            .files()
            .iter()
            .map(|file| {
                std::path::absolute(file)
                    .unwrap_or_else(|_| file.clone())
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    fn write_game_row(
        &self,
        info: &GogGameInfo,
        dest: &Path,
        prefix: Option<&Path>,
        old: Option<InstalledGame>,
    ) -> Result<String, LibraryError> {
        let mut meta = old
            .as_ref()
            .map(|row| GogMeta::from_json(&row.store_meta))
            .unwrap_or_default();
        meta.origin = GogOrigin::Offline;
        meta.platform = self.platform();
        meta.version = self.insp.version.clone();
        meta.build_id = info.build_id.clone();
        if !info.language.trim().is_empty() {
            meta.language = info.language.clone();
        }
        meta.installer = self.installer_paths();
        meta.installer_bytes = self.insp.bytes;
        // DLC the installer bundled with the game.
        for (_, other) in installed_infos(dest) {
            if other.is_dlc()
                && other.root_game_id == info.game_id
                && !meta.dlc.iter().any(|dlc| dlc.id == other.game_id)
            {
                meta.dlc.push(GogDlcMeta {
                    id: other.game_id.clone(),
                    title: other.name.clone(),
                    installer: Vec::new(),
                });
            }
        }
        let mut row = gog_row(info, dest, &meta, None, prefix);
        if meta.platform == GogPlatform::Linux {
            linux_launch(&mut row, dest);
        }
        if meta.platform == GogPlatform::Windows {
            row.native_compat_tool = self.opts.compat_tool.trim().to_string();
        }
        row.filesize_bytes = i64::try_from(game_bytes(dest)).unwrap_or(i64::MAX);
        let row = match old {
            Some(old) => carry_over(old, row, meta.exe_user_pinned),
            None => row,
        };
        self.ctx.registry.upsert_store(&row)?;
        Ok(String::new())
    }

    fn record_dlc(
        &self,
        mut base: InstalledGame,
        dlc: &GogGameInfo,
        base_dir: &Path,
    ) -> Result<String, LibraryError> {
        let mut meta = GogMeta::from_json(&base.store_meta);
        let entry = GogDlcMeta {
            id: dlc.game_id.clone(),
            title: dlc.name.clone(),
            installer: self.installer_paths(),
        };
        match meta.dlc.iter_mut().find(|known| known.id == entry.id) {
            Some(known) => *known = entry,
            None => meta.dlc.push(entry),
        }
        base.store_meta = meta.to_json();
        base.filesize_bytes = i64::try_from(game_bytes(base_dir)).unwrap_or(i64::MAX);
        self.ctx.registry.upsert_store(&base)?;
        Ok(String::new())
    }
}

/// A Linux game launches through GOG's `start.sh` (it sets up the game's
/// support libraries); without one, the play task's executable under
/// `game/`.
pub(crate) fn linux_launch(row: &mut InstalledGame, dest: &Path) {
    let start = dest.join("start.sh");
    if start.is_file() {
        row.native_executable_path = start.to_string_lossy().into_owned();
        row.native_launch_parameters = String::new();
        row.native_working_dir = String::new();
        return;
    }
    let exe = Path::new(&row.native_executable_path);
    if !row.native_executable_path.is_empty() && !exe.is_file() {
        if let Ok(rel) = exe.strip_prefix(dest) {
            let alt = dest.join("game").join(rel);
            if alt.is_file() {
                row.native_executable_path = alt.to_string_lossy().into_owned();
            }
        }
    }
}

/// The new row's install fields over the old row (its store details,
/// art, compat tool and play history stay); a user-pinned executable
/// stays too.
fn carry_over(old: InstalledGame, new: InstalledGame, exe_pinned: bool) -> InstalledGame {
    let mut row = old;
    row.extracted_dir = new.extracted_dir;
    row.native_game_dir = new.native_game_dir;
    if !exe_pinned {
        row.native_executable_path = new.native_executable_path;
        row.native_launch_parameters = new.native_launch_parameters;
        row.native_working_dir = new.native_working_dir;
    }
    row.native_launch_source = new.native_launch_source;
    row.native_wineprefix = new.native_wineprefix;
    if !new.native_compat_tool.is_empty() {
        row.native_compat_tool = new.native_compat_tool;
    }
    row.platform_slug = new.platform_slug;
    row.revision = new.revision;
    row.languages = new.languages;
    row.installed_at = new.installed_at;
    row.store_meta = new.store_meta;
    row.filesize_bytes = new.filesize_bytes;
    if row.title.trim().is_empty() {
        row.title = new.title;
    }
    row
}

fn is_empty_dir(dir: &Path) -> bool {
    match fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => true,
    }
}

/// Removes a failed fresh install: its folder and its Wine prefix.
pub(crate) fn remove_install(dest: &Path, prefix: Option<&Path>) {
    let _ = fs::remove_dir_all(dest);
    if let Some(prefix) = prefix {
        let _ = fs::remove_dir_all(prefix);
    }
}

/// The game's bytes: the files under `dir` without its Wine prefix.
pub(crate) fn game_bytes(dir: &Path) -> u64 {
    dir_bytes(dir).saturating_sub(dir_bytes(&dir.join(crate::backup::store::STORE_PREFIX_DIR)))
}

/// Total bytes of the files under `dir`.
pub(crate) fn dir_bytes(dir: &Path) -> u64 {
    fn walk(dir: &Path, sum: &mut u64) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => walk(&entry.path(), sum),
                Ok(kind) if kind.is_file() => {
                    *sum = sum.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
                }
                _ => {}
            }
        }
    }
    let mut sum = 0;
    walk(dir, &mut sum);
    sum
}
