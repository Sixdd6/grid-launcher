//! Selects the file to launch out of an extracted archive's contents. See
//! `docs/porting/03-library-install.md` §10 ("Launch-file selection") for
//! the Python behavior (`select_extracted_launch_file`) this mirrors.

use std::path::{Path, PathBuf};

/// Extensions (without the dot, lowercase) that identify an archive file.
/// Archive files are excluded from the selection pool unless every
/// candidate file is itself one of these (in which case the pool falls back
/// to the full file list).
const ARCHIVE_SUFFIXES: &[&str] = &["zip", "7z", "rar", "tar", "gz", "bz2", "xz"];

/// Extensions that identify a launchable ROM/disc image, in priority order
/// (earlier entries are preferred over later ones).
const PREFERRED_EXTENSIONS: &[&str] = &[
    "m3u", "cue", "chd", "iso", "xex", "bin", "pbp", "cso", "img", "ccd", "nrg", "mdf", "gdi",
    "rvz", "gcz", "wbfs", "gcm", "dol", "elf", "nes", "fds", "sfc", "smc", "gba", "gb", "gbc",
    "n64", "z64", "v64", "nds", "3ds", "cia", "xci", "nsp", "gen", "smd", "md", "32x", "sms", "gg",
    "pce", "sgx", "a26", "a52", "a78", "lnx", "ws", "wsc", "ngp", "ngc", "jag", "rom",
];

/// Directory names (case-folded) that mark a file as auxiliary rather than
/// game content, e.g. `docs/`, `__macosx/`.
const SUPPORT_DIRS: &[&str] = &[
    "__macosx",
    "glcache",
    "cache",
    "caches",
    "shadercache",
    "shaders",
    "docs",
    "doc",
    "manual",
    "manuals",
    "readme",
    "licenses",
    "license",
    "resources",
];

/// Extensions (without the dot, lowercase) that mark a file as auxiliary
/// rather than game content, e.g. `.txt`, `.png`.
const SUPPORT_EXTENSIONS: &[&str] = &[
    "txt", "nfo", "diz", "log", "json", "xml", "ini", "cfg", "conf", "url", "pdf", "html", "htm",
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "dll", "so", "dylib", "py", "lua",
    "js", "css", "db", "sqlite", "tmp", "cache", "sav", "srm", "state", "states", "cht", "slangp",
    "slang", "glsl", "vert", "frag",
];

/// One file discovered under the extraction root, identified by its path
/// relative to that root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelFile {
    pub rel: PathBuf,
}

/// Selects the file to launch under `root` (an extraction directory),
/// walking it recursively. `archive_stem` is the archive's file stem
/// (without extension), used to prefer a same-named file. Returns `None`
/// when `root` contains no files.
pub fn select_launch_file(root: &Path, archive_stem: &str) -> Option<PathBuf> {
    let files = walk(root);
    let index = rank(&files, archive_stem)?;
    Some(root.join(&files[index].rel))
}

/// Recursively collects every regular file under `root`, as paths relative
/// to `root`. Unreadable directories are skipped rather than causing an
/// error, matching a best-effort filesystem walk.
fn walk(root: &Path) -> Vec<RelFile> {
    let mut files = Vec::new();
    walk_into(root, root, &mut files);
    files
}

fn walk_into(root: &Path, dir: &Path, files: &mut Vec<RelFile>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        let path = entry.path();
        if file_type.is_dir() {
            walk_into(root, &path, files);
        } else if file_type.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                files.push(RelFile {
                    rel: rel.to_path_buf(),
                });
            }
        }
    }
}

/// The file's final extension, lowercased, without the leading dot. `None`
/// when the file has no extension.
fn suffix(rel: &Path) -> Option<String> {
    rel.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
}

/// Whether `rel`'s suffix is one of `ARCHIVE_SUFFIXES`.
fn is_archive(rel: &Path) -> bool {
    suffix(rel).is_some_and(|s| ARCHIVE_SUFFIXES.contains(&s.as_str()))
}

/// `rel`'s position in `PREFERRED_EXTENSIONS`, if its suffix is listed.
fn preferred_position(rel: &Path) -> Option<usize> {
    suffix(rel).and_then(|s| PREFERRED_EXTENSIONS.iter().position(|&p| p == s))
}

/// `rel`'s rank in the preferred-extension list, or
/// `PREFERRED_EXTENSIONS.len() + 10` when unlisted.
fn preferred_rank(rel: &Path) -> usize {
    preferred_position(rel).unwrap_or(PREFERRED_EXTENSIONS.len() + 10)
}

/// Whether any path segment of `rel`, excluding the file name itself and
/// case-folded, is one of `SUPPORT_DIRS`.
fn in_support_dir(rel: &Path) -> bool {
    rel.parent()
        .map(|parent| {
            parent.components().any(|component| {
                let segment = component.as_os_str().to_string_lossy().to_lowercase();
                SUPPORT_DIRS.contains(&segment.as_str())
            })
        })
        .unwrap_or(false)
}

/// Whether `rel`'s suffix is one of `SUPPORT_EXTENSIONS`.
fn has_support_extension(rel: &Path) -> bool {
    suffix(rel).is_some_and(|s| SUPPORT_EXTENSIONS.contains(&s.as_str()))
}

/// `in_support_dir` (+1) plus `has_support_extension` (+1): 0, 1, or 2.
fn penalty(rel: &Path) -> u8 {
    u8::from(in_support_dir(rel)) + u8::from(has_support_extension(rel))
}

/// Whether `rel`'s file stem case-folds equal to `archive_stem`.
fn stem_matches(rel: &Path, archive_stem: &str) -> bool {
    rel.file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase() == archive_stem.to_lowercase())
        .unwrap_or(false)
}

/// Number of components of the relative path.
fn depth(rel: &Path) -> usize {
    rel.components().count()
}

/// The full relative path, lowercased, for the deterministic final
/// tie-break.
fn casefolded_path(rel: &Path) -> String {
    rel.to_string_lossy().to_lowercase()
}

/// Ascending sort key: `(penalty, preferred_rank, stem_matches ? 0 : 1,
/// depth, casefolded_path)`.
type SortKey = (u8, usize, u8, usize, String);

fn sort_key(file: &RelFile, archive_stem: &str) -> SortKey {
    (
        penalty(&file.rel),
        preferred_rank(&file.rel),
        u8::from(!stem_matches(&file.rel, archive_stem)),
        depth(&file.rel),
        casefolded_path(&file.rel),
    )
}

/// Ranks `files` and returns the index of the selected one, or `None` when
/// `files` is empty. See the module doc for the algorithm this implements.
pub(crate) fn rank(files: &[RelFile], archive_stem: &str) -> Option<usize> {
    if files.is_empty() {
        return None;
    }

    let non_archive: Vec<usize> = (0..files.len())
        .filter(|&i| !is_archive(&files[i].rel))
        .collect();
    let pool: Vec<usize> = if non_archive.is_empty() {
        (0..files.len()).collect()
    } else {
        non_archive
    };

    let preferred: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&i| preferred_position(&files[i].rel).is_some())
        .collect();
    if !preferred.is_empty() {
        return best(preferred, files, archive_stem);
    }

    let zero_penalty: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&i| penalty(&files[i].rel) == 0)
        .collect();
    let narrowed = if zero_penalty.is_empty() {
        pool
    } else {
        zero_penalty
    };

    let stem_matching: Vec<usize> = narrowed
        .iter()
        .copied()
        .filter(|&i| stem_matches(&files[i].rel, archive_stem))
        .collect();
    let final_set = if stem_matching.is_empty() {
        narrowed
    } else {
        stem_matching
    };
    best(final_set, files, archive_stem)
}

/// Sorts `indices` by `sort_key` and returns the first (lowest-key) index.
fn best(mut indices: Vec<usize>, files: &[RelFile], archive_stem: &str) -> Option<usize> {
    indices.sort_by_key(|&i| sort_key(&files[i], archive_stem));
    indices.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(path: &str) -> RelFile {
        RelFile {
            rel: PathBuf::from(path),
        }
    }

    // --- rank: preferred extension ordering -------------------------------

    #[test]
    fn chd_beats_iso_beats_bin() {
        let files = vec![rel("game.bin"), rel("game.iso"), rel("game.chd")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("game.chd"));
    }

    #[test]
    fn iso_beats_bin_when_chd_absent() {
        let files = vec![rel("game.bin"), rel("game.iso")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("game.iso"));
    }

    // --- rank: support directory penalty -----------------------------------

    #[test]
    fn file_in_docs_dir_loses_to_same_extension_at_root() {
        let files = vec![rel("docs/game.iso"), rel("other.iso")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("other.iso"));
    }

    #[test]
    fn support_dir_match_is_case_folded() {
        let files = vec![rel("DOCS/game.iso"), rel("other.iso")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("other.iso"));
    }

    // --- rank: support extension never beats a preferred extension --------

    #[test]
    fn support_extension_never_beats_a_preferred_extension() {
        // Stem matches the .txt file exactly, but a preferred-extension
        // file exists elsewhere in the pool (even with a non-matching
        // stem) and must win: any preferred-extension file narrows
        // selection to the preferred subset before stem matching applies.
        let files = vec![rel("game.txt"), rel("other.bin")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("other.bin"));
    }

    // --- rank: stem match breaks ties within the narrowed pool -------------

    #[test]
    fn stem_match_breaks_ties_among_non_preferred_files() {
        let files = vec![rel("readme.dat"), rel("game.dat")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("game.dat"));
    }

    #[test]
    fn stem_match_is_case_folded() {
        let files = vec![rel("other.dat"), rel("GAME.dat")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("GAME.dat"));
    }

    // --- rank: shallower wins ------------------------------------------------

    #[test]
    fn shallower_path_wins() {
        let files = vec![rel("sub/dir/game.chd"), rel("game.chd")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("game.chd"));
    }

    #[test]
    fn shallower_path_wins_among_non_preferred_files() {
        let files = vec![rel("sub/dir/game.dat"), rel("game.dat")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("game.dat"));
    }

    // --- rank: deterministic tie-break by casefolded path -------------------

    #[test]
    fn ties_break_by_casefolded_path() {
        let files = vec![rel("zeta.chd"), rel("alpha.chd")];
        let index = rank(&files, "no-match").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("alpha.chd"));
    }

    #[test]
    fn tie_break_is_case_folded() {
        let files = vec![rel("Bravo.chd"), rel("alpha.chd")];
        let index = rank(&files, "no-match").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("alpha.chd"));
    }

    // --- rank: archives excluded from pool unless pool would be empty -------

    #[test]
    fn archive_suffix_excluded_when_a_real_rom_exists() {
        let files = vec![rel("game.zip"), rel("game.bin")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("game.bin"));
    }

    #[test]
    fn archive_suffix_used_when_pool_would_otherwise_be_empty() {
        let files = vec![rel("b.zip"), rel("a.zip")];
        let index = rank(&files, "game").unwrap();
        assert_eq!(files[index].rel, PathBuf::from("a.zip"));
    }

    // --- rank: empty input ----------------------------------------------------

    #[test]
    fn empty_file_list_yields_none() {
        assert_eq!(rank(&[], "game"), None);
    }

    // --- select_launch_file: filesystem walker ------------------------------

    #[test]
    fn select_launch_file_returns_none_for_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(select_launch_file(dir.path(), "game"), None);
    }

    #[test]
    fn select_launch_file_finds_nested_preferred_file_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("sub").join("deeper");
        std::fs::create_dir_all(&nested).unwrap();
        let target = nested.join("game.chd");
        std::fs::write(&target, b"data").unwrap();

        assert_eq!(select_launch_file(dir.path(), "game"), Some(target));
    }

    #[test]
    fn select_launch_file_prefers_root_file_over_docs_subdir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs").join("game.iso"), b"data").unwrap();
        let expected = dir.path().join("other.iso");
        std::fs::write(&expected, b"data").unwrap();

        assert_eq!(select_launch_file(dir.path(), "game"), Some(expected));
    }
}
