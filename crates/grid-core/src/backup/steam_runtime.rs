//! The Steam Linux Runtime ("sniper", umu's `steamrt3`) on a backup drive,
//! so a Windows game can start on a Linux computer with no network.
//!
//! On its first run `umu-run` downloads this runtime into
//! `<XDG_DATA_HOME>/umu/steamrt3` (about 800 MB installed). umu-launcher
//! (`umu/umu_runtime.py`, `setup_umu` / `_install_umu`) does NOT pin a
//! version: it reads
//! `https://repo.steampowered.com/steamrt3/images/latest-public-beta.txt`,
//! downloads `<that version>/SteamLinuxRuntime_sniper.tar.xz` and checks it
//! against the same folder's `SHA256SUMS`. A backup does exactly that.
//!
//! Placing it: umu extracts the archive and swaps its top folder
//! `SteamLinuxRuntime_sniper/` into `steamrt3`, then links `umu` to
//! `_v2-entry-point`. It then verifies the tree with
//! `pressure-vessel/bin/pv-verify` and only then writes its install marker
//! `.installed.ok`. GRID places the folder and the link the same way and
//! leaves the marker to umu: offline, umu's own check (`_check_offline_runtime`)
//! runs pv-verify on the tree and writes the marker when it passes, so a
//! broken copy is never marked as good.

use std::path::{Path, PathBuf};

use crate::backup::plan::{EmulatorKind, EmulatorPick, EmulatorSource};
use crate::launch::forge::{ForgeClient, ResolvedDownload};

/// The pack entry name, and the folder: `emulators/steam-runtime/<version>/linux-x86_64/`.
pub const RUNTIME_NAME: &str = "steam-runtime";
/// What the Downloads row and the dialog call it.
pub const RUNTIME_TITLE: &str = "Steam Linux Runtime";
/// The pack entry's `source_id`.
pub const RUNTIME_SOURCE_ID: &str = "valvesoftware/steamrt3";
/// umu's runtime variant: the folder under umu's data folder.
pub const VARIANT: &str = "steamrt3";
/// The archive umu downloads for the variant.
pub const ARCHIVE: &str = "SteamLinuxRuntime_sniper.tar.xz";
/// The archive's top folder, which becomes `steamrt3`.
pub const TOP_DIR: &str = "SteamLinuxRuntime_sniper";
/// Where umu downloads it from.
pub const DEFAULT_BASE: &str = "https://repo.steampowered.com";
/// umu's install marker, written by umu after pv-verify passes.
pub const INSTALL_MARKER: &str = ".installed.ok";

/// The pick a backup carries for the runtime: Linux only.
pub fn runtime_pick() -> EmulatorPick {
    EmulatorPick {
        name: RUNTIME_NAME.to_string(),
        kind: EmulatorKind::CompatTool,
        source: EmulatorSource::Catalog {
            source_id: RUNTIME_SOURCE_ID.to_string(),
        },
        platforms: Vec::new(),
    }
}

/// Whether a pack emulator entry is the runtime.
pub fn is_runtime_entry(source_id: &str) -> bool {
    source_id.trim().eq_ignore_ascii_case(RUNTIME_SOURCE_ID)
}

/// `<base>/steamrt3/images`.
pub fn images_url(base: &str) -> String {
    format!("{}/{VARIANT}/images", base.trim_end_matches('/'))
}

/// The SHA-256 a `SHA256SUMS` file lists for `archive`, lower-case hex, as
/// umu reads it: the first field of the line ending with the name. Pure.
pub fn parse_sha256sums(text: &str, archive: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| line.ends_with(archive))
        .and_then(|line| line.split_whitespace().next())
        .filter(|hex| hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

/// A version string fit for a folder name and a URL segment: the
/// `latest-public-beta.txt` body is like `3.0.20250210.116596`.
fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

async fn get_text(forge: &ForgeClient, url: &str) -> Result<String, String> {
    let response = forge.get(url, false).await.map_err(|e| e.0)?;
    response.text().await.map_err(|e| e.to_string())
}

/// Resolves the runtime umu would download now: the version
/// `latest-public-beta.txt` names, its archive URL, the SHA-256 the
/// version's `SHA256SUMS` lists (as `sha256:<hex>`), and the size the
/// server reports (`0` when it does not say). Never sends a credential: the
/// forge client has none.
pub async fn resolve(forge: &ForgeClient, base: &str) -> Result<ResolvedDownload, String> {
    let images = images_url(base);
    let version = get_text(forge, &format!("{images}/latest-public-beta.txt"))
        .await?
        .trim()
        .to_string();
    if !valid_version(&version) {
        return Err("The Steam Linux Runtime version could not be read".to_string());
    }
    let folder = format!("{images}/{version}");
    let sums = get_text(forge, &format!("{folder}/SHA256SUMS")).await?;
    let sha = parse_sha256sums(&sums, ARCHIVE)
        .ok_or_else(|| "The Steam Linux Runtime has no published checksum".to_string())?;
    let url = format!("{folder}/{ARCHIVE}");
    // The size from the archive's headers; the body is never read.
    let size = match forge.get(&url, false).await {
        Ok(response) => response
            .content_length()
            .and_then(|len| i64::try_from(len).ok())
            .unwrap_or(0),
        Err(_) => 0,
    };
    Ok(ResolvedDownload {
        provider: "direct".to_string(),
        owner: String::new(),
        repo: String::new(),
        release_tag: version,
        asset_name: ARCHIVE.to_string(),
        download_url: url,
        size,
        digest: format!("sha256:{sha}"),
    })
}

/// umu's data folder (`UMU_LOCAL` in `umu/umu_consts.py`):
/// `$UMU_FOLDERS_PATH/umu` when set, else `$XDG_DATA_HOME/umu`, else
/// `<home>/.local/share/umu`. Pure: `env` reads a variable. (umu inside a
/// Flatpak uses `HOST_XDG_DATA_HOME`; GRID does not run umu that way.)
pub fn umu_local(env: &dyn Fn(&str) -> Option<String>, home: &Path) -> PathBuf {
    let set = |name: &str| env(name).filter(|value| !value.trim().is_empty());
    if let Some(folders) = set("UMU_FOLDERS_PATH") {
        return PathBuf::from(folders).join("umu");
    }
    match set("XDG_DATA_HOME") {
        Some(data) => PathBuf::from(data).join("umu"),
        None => home.join(".local").join("share").join("umu"),
    }
}

/// [`umu_local`] for this process. `None` with no home folder.
pub fn umu_local_here() -> Option<PathBuf> {
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    Some(umu_local(&|name| std::env::var(name).ok(), &home))
}

/// Whether `<umu_local>/steamrt3` holds a runtime umu can use without a
/// download: its marker, or the `pressure-vessel` folder an unpacked
/// runtime has (umu verifies and marks that itself). An empty `steamrt3`,
/// which `umu-run` creates before it downloads, does not count.
pub fn runtime_present(umu_local: &Path) -> bool {
    let dir = umu_local.join(VARIANT);
    dir.join(INSTALL_MARKER).is_file() || dir.join("pressure-vessel").is_dir()
}

/// Where a drive copy of the archive waits before it is placed.
pub fn download_dir(umu_local: &Path) -> PathBuf {
    umu_local.join(".grid-steamrt3-download")
}

/// Places the runtime from `archive` (a `SteamLinuxRuntime_sniper.tar.xz`)
/// the way umu does: extract it next to `steamrt3`, move its
/// `SteamLinuxRuntime_sniper/` folder to `steamrt3`, link `umu` to
/// `_v2-entry-point`. A usable runtime already there is kept (returns
/// `false`); an unusable leftover (no marker, no `pressure-vessel`) is
/// replaced. umu writes its own install marker after it verifies the tree.
pub fn place_runtime(
    archive: &Path,
    umu_local: &Path,
    progress: crate::library::extract::ExtractProgress,
) -> Result<bool, crate::library::LibraryError> {
    use crate::library::LibraryError;
    if runtime_present(umu_local) {
        return Ok(false);
    }
    std::fs::create_dir_all(umu_local)?;
    let tmp = umu_local.join(".grid-steamrt3.tmp");
    let placed = (|| -> Result<(), LibraryError> {
        crate::library::extract::extract_archive(archive, &tmp, progress)?;
        let top = tmp.join(TOP_DIR);
        if !top.join("_v2-entry-point").exists() {
            return Err(LibraryError::Extract(format!(
                "The Steam Linux Runtime archive has no {TOP_DIR}/_v2-entry-point"
            )));
        }
        let dest = umu_local.join(VARIANT);
        match std::fs::symlink_metadata(&dest) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&dest)?,
            Ok(_) => std::fs::remove_file(&dest)?,
            Err(_) => {}
        }
        std::fs::rename(&top, &dest)?;
        #[cfg(unix)]
        {
            let link = dest.join("umu");
            if std::fs::symlink_metadata(&link).is_err() {
                std::os::unix::fs::symlink("_v2-entry-point", &link)?;
            }
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    placed.map(|()| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256sums_line_for_the_archive() {
        let hex = "a".repeat(64);
        let text = format!(
            "{} *SteamLinuxRuntime_sniper.tar.xz.zsync\n{hex} *SteamLinuxRuntime_sniper.tar.xz\n",
            "b".repeat(64)
        );
        assert_eq!(parse_sha256sums(&text, ARCHIVE), Some(hex));
        assert_eq!(parse_sha256sums("nothing here", ARCHIVE), None);
        assert_eq!(
            parse_sha256sums("xyz SteamLinuxRuntime_sniper.tar.xz", ARCHIVE),
            None
        );
    }

    #[test]
    fn versions_are_plain_segments() {
        assert!(valid_version("3.0.20250210.116596"));
        assert!(!valid_version(""));
        assert!(!valid_version("../etc"));
        assert!(!valid_version("1 2"));
    }

    #[test]
    fn umu_local_follows_umu() {
        let home = Path::new("/home/u");
        let none = |_: &str| None;
        assert_eq!(
            umu_local(&none, home),
            PathBuf::from("/home/u/.local/share/umu")
        );
        let xdg = |name: &str| (name == "XDG_DATA_HOME").then(|| "/data".to_string());
        assert_eq!(umu_local(&xdg, home), PathBuf::from("/data/umu"));
        let folders = |name: &str| match name {
            "UMU_FOLDERS_PATH" => Some("/umu-root".to_string()),
            "XDG_DATA_HOME" => Some("/data".to_string()),
            _ => None,
        };
        assert_eq!(umu_local(&folders, home), PathBuf::from("/umu-root/umu"));
    }

    #[test]
    fn an_empty_steamrt3_is_not_a_runtime() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!runtime_present(dir.path()));
        std::fs::create_dir_all(dir.path().join(VARIANT)).unwrap();
        assert!(!runtime_present(dir.path()));
        std::fs::create_dir_all(dir.path().join(VARIANT).join("pressure-vessel")).unwrap();
        assert!(runtime_present(dir.path()));
    }
}
