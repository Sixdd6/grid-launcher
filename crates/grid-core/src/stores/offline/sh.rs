//! GOG's Linux installers: a MojoSetup shell script with a zip appended.
//! GRID opens the zip (the `zip` crate finds it past the script) and copies
//! `data/noarch/` into the install folder. The script is never run.
//!
//! Identity: `goggame-<id>.info` under `data/noarch/` (or its `game/`
//! folder); the name and version from `data/noarch/gameinfo` (line 1 the
//! name, line 2 the version). An installer with no goggame file but a GOG
//! product id in `scripts/config.lua` gets a minimal identity from it.

use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use crate::library::extract::is_absolute_entry_path;
use crate::library::queue::StoreProgressSink;
use crate::library::LibraryError;
use crate::stores::gog::goggame::{parse_info, GogGameInfo};

/// The folder of the zip GRID copies.
pub const NOARCH: &str = "data/noarch/";

/// The largest small file (goggame, gameinfo, config.lua) GRID reads.
const MAX_SMALL: u64 = 1024 * 1024;

/// What a Linux installer's zip says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShContents {
    pub info: Option<GogGameInfo>,
    /// `gameinfo` line 1, `""` when missing.
    pub name: String,
    /// `gameinfo` line 2, `""` when missing.
    pub version: String,
    /// The unpacked size of `data/noarch/`.
    pub unpacked_bytes: u64,
}

/// Opens `path` as a GOG Linux installer. `None` when it isn't one: not a
/// zip, or a zip with nothing under `data/noarch/`.
pub fn read(path: &Path) -> Option<ShContents> {
    let file = fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let mut contents = ShContents::default();
    let mut any = false;
    let mut infos: Vec<(String, GogGameInfo)> = Vec::new();
    let mut config_id = None;
    for index in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(index) else {
            continue;
        };
        let Ok(name) = entry.name().map(|name| name.replace('\\', "/")) else {
            continue;
        };
        if let Some(rel) = name.strip_prefix(NOARCH) {
            any = true;
            if !entry.is_dir() {
                contents.unpacked_bytes = contents.unpacked_bytes.saturating_add(entry.size());
            }
            if entry.is_dir() || entry.size() > MAX_SMALL {
                continue;
            }
            if rel == "gameinfo" {
                let text = read_small(&mut entry);
                let mut lines = text.lines().map(str::trim);
                contents.name = lines.next().unwrap_or_default().to_string();
                contents.version = lines.next().unwrap_or_default().to_string();
            } else if is_goggame_name(rel) {
                if let Some(info) = parse_info(read_small(&mut entry).as_bytes()) {
                    infos.push((rel.to_string(), info));
                }
            }
        } else if name == "scripts/config.lua" && entry.size() <= MAX_SMALL {
            config_id = gog_id_in_config(&read_small(&mut entry));
        }
    }
    if !any {
        return None;
    }
    contents.info = pick_info(infos).or_else(|| {
        config_id.map(|id| GogGameInfo {
            game_id: id.clone(),
            root_game_id: id,
            name: contents.name.clone(),
            ..Default::default()
        })
    });
    if let Some(info) = &mut contents.info {
        if info.name.is_empty() {
            info.name = contents.name.clone();
        }
    }
    Some(contents)
}

fn read_small(entry: &mut impl Read) -> String {
    let mut text = String::new();
    let _ = entry.take(MAX_SMALL).read_to_string(&mut text);
    text
}

/// Whether `rel` (under `data/noarch/`) is a goggame file GRID reads: at
/// the top, or in `game/`.
fn is_goggame_name(rel: &str) -> bool {
    let (dir, file) = match rel.rsplit_once('/') {
        Some((dir, file)) => (dir, file),
        None => ("", rel),
    };
    (dir.is_empty() || dir == "game") && file.starts_with("goggame-") && file.ends_with(".info")
}

/// The base game's file when there are several (an installer that bundles
/// DLC), else the only one. Top-level files first, then by name.
fn pick_info(mut infos: Vec<(String, GogGameInfo)>) -> Option<GogGameInfo> {
    infos.sort_by(|a, b| (a.0.contains('/'), &a.0).cmp(&(b.0.contains('/'), &b.0)));
    let base = infos.iter().position(|(_, info)| !info.is_dlc());
    match base {
        Some(index) => Some(infos.swap_remove(index).1),
        None => infos.into_iter().next().map(|(_, info)| info),
    }
}

/// A GOG product id (`gogid = "123"` or `id = "123"`, digits only) in a
/// MojoSetup `config.lua`.
pub fn gog_id_in_config(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key != "gogid" && key != "id" {
            continue;
        }
        let value = value
            .trim()
            .trim_end_matches(',')
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if !value.is_empty() && value.len() <= 20 && value.bytes().all(|b| b.is_ascii_digit()) {
            return Some(value.to_string());
        }
    }
    None
}

/// Copies `data/noarch/` of the installer at `path` into `dest`, merging
/// over what is there (an update keeps saves and settings it doesn't
/// replace). Unix modes are kept (setuid and friends dropped); a symlink
/// that stays inside `dest` is made a symlink on unix and skipped
/// elsewhere. Progress goes to `sink`; a cancel stops between files.
pub fn unpack(
    path: &Path,
    dest: &Path,
    sink: &mut dyn StoreProgressSink,
) -> Result<(), LibraryError> {
    let error = |e: &dyn std::fmt::Display| LibraryError::Extract(format!("installer: {e}"));
    let file = fs::File::open(path).map_err(|e| error(&e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| error(&e))?;
    let mut total = 0u64;
    for index in 0..zip.len() {
        let Ok(entry) = zip.by_index(index) else {
            continue;
        };
        let in_noarch = entry
            .name()
            .is_ok_and(|name| name.replace('\\', "/").starts_with(NOARCH));
        if in_noarch && !entry.is_dir() {
            total = total.saturating_add(entry.size());
        }
    }
    fs::create_dir_all(dest)?;
    let mut done = 0u64;
    sink.progress(0, total);
    for index in 0..zip.len() {
        if sink.cancelled() {
            return Err(LibraryError::Cancelled);
        }
        let mut entry = zip.by_index(index).map_err(|e| error(&e))?;
        let raw = entry.name().map_err(|e| error(&e))?.into_owned();
        let name = raw.replace('\\', "/");
        let Some(rel) = name.strip_prefix(NOARCH) else {
            continue;
        };
        if rel.is_empty() {
            continue;
        }
        let rel =
            safe_relative(rel).ok_or_else(|| crate::library::extract::unsafe_path_error(&raw))?;
        let out = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mode = entry.unix_mode();
        if mode.is_some_and(|mode| mode & 0o170000 == 0o120000) {
            let mut target = String::new();
            entry
                .by_ref()
                .take(4096)
                .read_to_string(&mut target)
                .map_err(|e| error(&e))?;
            link_inside(dest, &rel, target.trim())?;
            continue;
        }
        if out.is_dir() {
            fs::remove_dir_all(&out)?;
        } else if fs::symlink_metadata(&out).is_ok() {
            fs::remove_file(&out)?;
        }
        let mut target = fs::File::create(&out)?;
        io::copy(&mut entry, &mut target).map_err(|e| error(&e))?;
        done = done.saturating_add(entry.size());
        sink.progress(done.min(total), total);
        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&out, fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

/// `rel` as a plain relative path: `None` when it is absolute or climbs
/// out with `..`.
fn safe_relative(rel: &str) -> Option<PathBuf> {
    if is_absolute_entry_path(rel) {
        return None;
    }
    let mut out = PathBuf::new();
    for part in rel.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            part if part.contains(':') && cfg!(windows) => return None,
            part => out.push(part),
        }
    }
    out.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then_some(out)
        .filter(|out| out.components().next().is_some())
}

/// Makes `dest/rel` a symlink to `target` when the link stays inside
/// `dest`; anything else is skipped.
fn link_inside(dest: &Path, rel: &Path, target: &str) -> Result<(), LibraryError> {
    if target.is_empty() || target.starts_with('/') || is_absolute_entry_path(target) {
        return Ok(());
    }
    // Depth of the link's folder below `dest`, minus every `..`.
    let mut depth = rel.components().count() as i64 - 1;
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => depth -= 1,
            _ => depth += 1,
        }
        if depth < 0 {
            return Ok(());
        }
    }
    let link = dest.join(rel);
    if fs::symlink_metadata(&link).is_ok() {
        fs::remove_file(&link)?;
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, &link)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_lua_ids() {
        assert_eq!(
            gog_id_in_config("Setup.Package {\n  id = \"1207658930\",\n}"),
            Some("1207658930".into())
        );
        assert_eq!(gog_id_in_config("gogid = '42'"), Some("42".into()));
        assert_eq!(gog_id_in_config("id = \"stardew_valley\","), None);
        assert_eq!(gog_id_in_config(""), None);
    }

    #[test]
    fn goggame_names_and_relative_paths() {
        assert!(is_goggame_name("goggame-1.info"));
        assert!(is_goggame_name("game/goggame-1.info"));
        assert!(!is_goggame_name("game/sub/goggame-1.info"));
        assert!(!is_goggame_name("goggame-1.txt"));
        assert_eq!(
            safe_relative("game/a.bin"),
            Some(PathBuf::from("game/a.bin"))
        );
        assert_eq!(safe_relative("../x"), None);
        assert_eq!(safe_relative("/etc/x"), None);
        assert_eq!(safe_relative("./"), None);
    }
}
