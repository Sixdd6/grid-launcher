//! Save/state-path readers for PCSX2, DuckStation, Dolphin and RPCS3: the
//! `*_directory_settings` / `*_data_root_candidates` / `*_save_path_overrides`
//! / `*_state_path_overrides` family that turns an emulator's own config
//! files into concrete cloud-sync directories.
//!
//! Ports `grid_launcher/emulator/pcsx2.py`, `duckstation.py`, `dolphin.py`
//! and `rpcs3.py`'s reader functions (as opposed to the `ensure_*` writers,
//! which live in the sibling `pcsx2`/`duckstation`/`dolphin`/`rpcs3`
//! modules). See `docs/porting/05-emulator-autoconfig.md`'s per-emulator
//! sections and the `*_directory_settings` result-shape table (lines
//! 167-187) for the behavior contract.
//!
//! Nothing in this crate calls these readers yet — milestone 6 (cloud
//! saves) is their only consumer. Every reader returns a FULLY POPULATED
//! struct: an unresolvable value is an empty string, never absent (doc 05
//! invariant 6). Every path list is deduped CASE-INSENSITIVELY via
//! [`unique_paths`] (doc 05 invariant 7).
//!
//! The Python reference takes a raw `launch_template: str` plus a
//! `split_launch_template_args` callback and re-splits it inside every
//! reader; this port instead takes the ALREADY-SPLIT arguments as
//! [`Args`] — the caller is responsible for producing an empty slice where
//! the reference's splitter would have raised (doc 05 invariant 9,
//! `rpcs3.py:40`'s `except ValueError: return []`).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::{cemu, duckstation, paths, redream, xemu};

/// The launch template already split into arguments, or an empty slice
/// where the reference's splitter would have raised (`rpcs3.py:40`).
pub type Args<'a> = &'a [String];

// ---------------------------------------------------------------------
// Result shapes (doc 05 lines 167-187) — every field is a plain, always
// populated `String` (or `bool`); an unresolvable value is `""`, never
// absent (doc 05 invariant 6).
// ---------------------------------------------------------------------

/// `pcsx2_directory_settings`'s return shape (`pcsx2.py:537-581`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pcsx2Settings {
    pub config_path: String,
    pub data_root: String,
    pub memory_cards: String,
    pub savestates: String,
    pub slot1_filename: String,
    pub slot2_filename: String,
}

/// `duckstation_memory_card_settings`'s return shape (`duckstation.py:144-195`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DuckstationSettings {
    pub config_path: String,
    pub directory: String,
    pub card1_type: String,
    pub card2_type: String,
    pub use_playlist_title: bool,
}

/// `dolphin_directory_settings`'s return shape (`dolphin.py:427-487`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DolphinSettings {
    pub config_path: String,
    pub user_root: String,
    pub gc_root: String,
    pub wii_root: String,
    pub state_saves: String,
    pub memcard_a_path: String,
    pub memcard_b_path: String,
    pub gci_folder_a_path: String,
    pub gci_folder_b_path: String,
    pub gci_folder_a_override: String,
    pub gci_folder_b_override: String,
}

/// `rpcs3_directory_settings`'s return shape (`rpcs3.py:660-706`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rpcs3Settings {
    pub config_path: String,
    pub persistent_settings_path: String,
    pub data_root: String,
    pub dev_hdd0: String,
    pub current_user: String,
}

// ---------------------------------------------------------------------
// Shared regexes and small text helpers.
// ---------------------------------------------------------------------

/// `^\[(.+?)\]\s*$` applied to the trimmed line — the INI section-header
/// pattern every reader in this file shares (`pcsx2.py:517`,
/// `duckstation.py:169`, matching `writers::SECTION_RE`, which is private to
/// its own module).
static SECTION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[(.+?)\]\s*$").unwrap());

/// `$VAR` / `${VAR}` — the POSIX subset of `os.path.expandvars` this crate
/// needs. An unset variable is left as literal text, matching Python's
/// `expandvars` (it never raises or blanks an unresolved reference).
static ENV_VAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)").unwrap()
});

/// `\s+#` — RPCS3's "unquoted trailing comment" splitter (`rpcs3.py:78`).
static COMMENT_SPLIT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+#").unwrap());

/// `Path.resolve(strict=False)` port — see `paths::resolve_best_effort`'s
/// doc comment for the full contract (relative-to-absolute, lexical `..`/`.`
/// collapse through nonexistent segments, root clamping, symlink resolution
/// for whatever prefix exists). Used throughout this module via the
/// re-export below rather than a local copy — this file and `rpcs3.rs`
/// used to each carry their own, subtly different (and, in this file's
/// case, `..`-blind) duplicate; both now share the one implementation.
use paths::resolve_best_effort;

/// `os.path.expandvars` (POSIX subset): replace `$VAR`/`${VAR}` references
/// with the named environment variable's value, leaving an unset
/// reference's literal text untouched.
fn expand_vars(text: &str) -> String {
    ENV_VAR_RE
        .replace_all(text, |caps: &regex::Captures| {
            let name = caps.get(1).or_else(|| caps.get(2)).unwrap().as_str();
            std::env::var(name).unwrap_or_else(|_| caps[0].to_string())
        })
        .into_owned()
}

/// `_clean_path_value` as PCSX2 and Dolphin define it (`pcsx2.py:493-494`,
/// `dolphin.py:409-410`): trim, then strip every leading/trailing `"`, then
/// every leading/trailing `'`. No comment handling — that is RPCS3-only
/// (see [`clean_yaml_value`]).
fn clean_ini_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

/// RPCS3's `_clean_path_value` (`rpcs3.py:75-79`): trim; when the value does
/// NOT start with a quote character, strip an unquoted trailing `\s+#...`
/// comment; then strip every leading/trailing `"` and `'`, same as
/// [`clean_ini_value`].
fn clean_yaml_value(value: &str) -> String {
    let trimmed = value.trim();
    let mut cleaned = trimmed.to_string();
    if !trimmed.starts_with(['"', '\'']) {
        if let Some(m) = COMMENT_SPLIT_RE.find(trimmed) {
            cleaned = trimmed[..m.start()].trim().to_string();
        }
    }
    cleaned
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

/// The environment variable named `var`, `Some` only when set and non-blank
/// after trimming — the `os.environ.get(var, "").strip()` idiom repeated at
/// every PCSX2/Dolphin/RPCS3 candidate site.
fn env_trimmed(var: &str) -> Option<String> {
    std::env::var(var)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// A minimal case-insensitive-section, case-insensitive-key INI reader
/// shared by PCSX2's `[Folders]`/`[MemoryCards]` probe (`pcsx2.py:508-529`
/// `_parse_ini_sections`) and Dolphin's `[Core]`/`[General]` probe
/// (`dolphin.py:458-465`'s `configparser` read, whose default option
/// lowercasing this reproduces). Blank lines and `#`/`;` comments are
/// skipped; a line with no `=` outside any section is ignored.
fn parse_ini_sections(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current = String::new();

    for raw_line in text.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with(';') {
            continue;
        }
        if let Some(caps) = SECTION_RE.captures(stripped) {
            current = caps[1].trim().to_lowercase();
            sections.entry(current.clone()).or_default();
            continue;
        }
        if current.is_empty() {
            continue;
        }
        let Some(eq_index) = raw_line.find('=') else {
            continue;
        };
        let key = raw_line[..eq_index].trim().to_lowercase();
        let value = raw_line[eq_index + 1..].trim().to_string();
        sections
            .entry(current.clone())
            .or_default()
            .insert(key, value);
    }

    sections
}

// ---------------------------------------------------------------------
// Shared helpers named in the task interface.
// ---------------------------------------------------------------------

/// `_consume_arg_value` (`xemu.py:36-56`): starting at `args[index]`, rejoin
/// tokens with a single space until one ends with the SAME quote character
/// the first token opened with, then strip that quoting — tolerating a
/// splitter that produced fragments (doc 05 invariant 9). `None` when
/// `index` is out of bounds or the token there is blank once trimmed. Unlike
/// the Python reference, this reports only the value, not the "how many
/// tokens did that consume" index the Python tuple also carries — no caller
/// in this file needs both; the ones that DO need the index (Xemu, Xenia,
/// MAME, Pico-8's own `_consume_arg_value` ports, added in Task 9) call
/// [`consume_arg_value_indexed`] instead, which this function is defined in
/// terms of.
///
/// Still `pub(crate)` and exercised only by its own unit test today — every
/// `-u`/`--user`/`--user-id` probe elsewhere in this module reads its value
/// with a plain next-token check instead, matching the Python reference
/// precisely (`dolphin.py:53-67`, `rpcs3.py:46-72`, neither of which calls
/// `_consume_arg_value`).
#[allow(dead_code)]
pub(crate) fn consume_arg_value(args: Args, index: usize) -> Option<String> {
    let (value, _) = consume_arg_value_indexed(args, index);
    (!value.is_empty()).then_some(value)
}

/// Deduplicate a candidate list case-insensitively, keeping the first
/// occurrence of each path and dropping any path whose text is empty —
/// `_unique_paths`, duplicated in `pcsx2.py:383-392`, `dolphin.py:15-24` and
/// `rpcs3.py:16-25`. Built on [`paths::dedupe_casefold`], which shares the
/// same casefold-key logic but has no empty-path filter of its own (the
/// writer candidate lists it serves never produce one).
pub(crate) fn unique_paths(candidates: Vec<PathBuf>) -> Vec<PathBuf> {
    paths::dedupe_casefold(
        candidates
            .into_iter()
            .filter(|p| !p.as_os_str().is_empty())
            .collect(),
    )
}

/// Resolve a config value against a base root: expand `$VAR`/`${VAR}`
/// references, expand a leading `~`, join onto `base` when the result is
/// relative, then canonicalize best-effort. The common tail of PCSX2's and
/// Dolphin's `_resolve_setting_path` (`pcsx2.py:497-505`,
/// `dolphin.py:413-424`) once the caller has already picked a non-blank,
/// quote-cleaned value — RPCS3's `$(EmulatorDir)` substitution runs a
/// different pipeline and is NOT built on this helper (see
/// `resolve_rpcs3_path`).
pub(crate) fn resolve_against(base: &Path, value: &str) -> PathBuf {
    let expanded = paths::expand_user(&expand_vars(value));
    let candidate = if expanded.is_absolute() {
        expanded
    } else {
        base.join(expanded)
    };
    resolve_best_effort(&candidate)
}

/// The PCSX2/Dolphin "use the configured value, else the default, else
/// blank" resolution rule: clean `raw_value`; if that is blank, fall back to
/// `default_value`; if THAT is also blank, return `""` without touching the
/// filesystem (Dolphin's memcard/GCI fields have no default —
/// `dolphin.py:413-419`); otherwise resolve the result against `base` via
/// [`resolve_against`]. PCSX2 always supplies a non-blank default
/// (`"memcards"`/`"sstates"`), so this never hits the blank branch for it.
fn resolve_setting_or_default(base: &Path, raw_value: &str, default_value: &str) -> String {
    let cleaned = clean_ini_value(raw_value);
    let value = if cleaned.is_empty() {
        default_value.to_string()
    } else {
        cleaned
    };
    if value.is_empty() {
        String::new()
    } else {
        resolve_against(base, &value).to_string_lossy().to_string()
    }
}

/// Windows Shell `SHGetKnownFolderPath(FOLDERID_Documents)`
/// (`pcsx2.py:10-49`) is not ported — this crate has no Windows Shell API
/// bindings. Always `None`, matching the Python reference's own behavior on
/// every platform this crate is tested on (`pcsx2.py:17-18`: `None` when
/// `sys.platform != "win32"`); every caller already falls through to the
/// next candidate.
fn windows_documents_folder() -> Option<PathBuf> {
    None
}

// =======================================================================
// PCSX2 — pcsx2.py:420-621
// =======================================================================

/// `-portable`, case-insensitively, anywhere in the launch args
/// (`pcsx2.py:415-418`).
fn pcsx2_portable_flag_present(args: Args) -> bool {
    args.iter()
        .any(|arg| arg.trim().to_lowercase() == "-portable")
}

/// `_portable_data_root` (`pcsx2.py:409-433`): `None` unless `-portable` is
/// in `args` or `portable.ini`/`portable.txt` exists next to the
/// executable. When `portable.txt` holds text, that text is a subdirectory
/// suffix under `emulator_dir` (best-effort read: an I/O error is treated
/// as no suffix, `pcsx2.py:427-430`); otherwise the portable root is
/// `emulator_dir` itself. Always canonicalized best-effort.
fn pcsx2_portable_data_root(emulator_dir: &Path, args: Args) -> Option<PathBuf> {
    let portable_ini = emulator_dir.join("portable.ini");
    let portable_txt = emulator_dir.join("portable.txt");
    if !pcsx2_portable_flag_present(args) && !portable_ini.exists() && !portable_txt.exists() {
        return None;
    }

    let portable_suffix = if portable_txt.is_file() {
        std::fs::read_to_string(&portable_txt)
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        String::new()
    };

    let portable_root = if portable_suffix.is_empty() {
        emulator_dir.to_path_buf()
    } else {
        emulator_dir.join(portable_suffix)
    };
    Some(resolve_best_effort(&portable_root))
}

/// `pcsx2_data_root_candidates` (`pcsx2.py:436-480`): portable roots, then
/// user (Documents/XDG) roots, then the plain emulator directory —
/// deduplicated case-insensitively. Only the portable and plain-emulator
/// roots are canonicalized; the user-root entries are joined but left
/// unresolved, exactly as the Python reference leaves them
/// (`pcsx2.py:455-478` never calls `.resolve()` on a `user_roots` entry).
pub fn pcsx2_data_root_candidates(path: &str, args: Args) -> Vec<PathBuf> {
    let mut portable_roots: Vec<PathBuf> = Vec::new();
    let mut fallback_roots: Vec<PathBuf> = Vec::new();

    let trimmed = path.trim();
    if !trimmed.is_empty() {
        let expanded = paths::expand_user(trimmed);
        let emulator_dir = if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if !emulator_dir.as_os_str().is_empty() {
            if let Some(portable_root) = pcsx2_portable_data_root(&emulator_dir, args) {
                portable_roots.push(portable_root);
            }
            fallback_roots.push(resolve_best_effort(&emulator_dir));
        }
    }

    let mut user_roots: Vec<PathBuf> = Vec::new();
    if let Some(docs) = windows_documents_folder() {
        user_roots.push(docs.join("PCSX2"));
    }
    for var in ["OneDrive", "USERPROFILE", "HOME"] {
        if let Some(base) = env_trimmed(var) {
            user_roots.push(paths::expand_user(&base).join("Documents").join("PCSX2"));
        }
    }

    let home = paths::home_dir().unwrap_or_default();
    user_roots.push(home.join("Documents").join("PCSX2"));
    user_roots.push(home.join(".config").join("PCSX2"));
    user_roots.push(
        home.join("Library")
            .join("Application Support")
            .join("PCSX2"),
    );

    if let Some(xdg) = env_trimmed("XDG_CONFIG_HOME") {
        user_roots.push(paths::expand_user(&xdg).join("PCSX2"));
    }

    let mut combined = portable_roots;
    combined.extend(user_roots);
    combined.extend(fallback_roots);
    unique_paths(combined)
}

/// `<root>/inis/PCSX2.ini` for every [`pcsx2_data_root_candidates`] entry,
/// deduplicated (`pcsx2_settings_path_candidates`, `pcsx2.py:483-490`).
fn pcsx2_settings_path_candidates(path: &str, args: Args) -> Vec<PathBuf> {
    unique_paths(
        pcsx2_data_root_candidates(path, args)
            .into_iter()
            .map(|root| root.join("inis").join("PCSX2.ini"))
            .collect(),
    )
}

/// `pcsx2_directory_settings` (`pcsx2.py:532-581`): walk the data roots and
/// their `inis/PCSX2.ini` candidates in lockstep; the first candidate that
/// exists as a file wins. `[Folders] MemoryCards`/`Savestates` resolve
/// against that root, defaulting to `memcards`/`sstates`;
/// `[MemoryCards] Slot1_Filename`/`Slot2_Filename` override the
/// `Mcd001.ps2`/`Mcd002.ps2` defaults only when non-blank once cleaned. With
/// no matching candidate but at least one data root, `data_root` is the
/// first candidate root and `memory_cards`/`savestates` still resolve to
/// their defaults under it; with no data roots at all every field stays
/// blank (its `Default`), except the two slot filenames, which are always
/// populated (doc 05 invariant 6).
pub fn pcsx2_directory_settings(path: &str, args: Args) -> Pcsx2Settings {
    let mut settings = Pcsx2Settings {
        slot1_filename: "Mcd001.ps2".to_string(),
        slot2_filename: "Mcd002.ps2".to_string(),
        ..Default::default()
    };

    let data_roots = pcsx2_data_root_candidates(path, args);
    let settings_candidates = pcsx2_settings_path_candidates(path, args);

    for (root, candidate) in data_roots.iter().zip(settings_candidates.iter()) {
        if !candidate.is_file() {
            continue;
        }
        let Ok(raw_content) = std::fs::read_to_string(candidate) else {
            continue;
        };

        let sections = parse_ini_sections(&raw_content);
        let empty = HashMap::new();
        let folders = sections.get("folders").unwrap_or(&empty);
        let memory_cards = sections.get("memorycards").unwrap_or(&empty);

        settings.config_path = candidate.to_string_lossy().to_string();
        settings.data_root = root.to_string_lossy().to_string();
        settings.memory_cards = resolve_setting_or_default(
            root,
            folders.get("memorycards").map(String::as_str).unwrap_or(""),
            "memcards",
        );
        settings.savestates = resolve_setting_or_default(
            root,
            folders.get("savestates").map(String::as_str).unwrap_or(""),
            "sstates",
        );

        let slot1 = clean_ini_value(
            memory_cards
                .get("slot1_filename")
                .map(String::as_str)
                .unwrap_or(""),
        );
        if !slot1.is_empty() {
            settings.slot1_filename = slot1;
        }
        let slot2 = clean_ini_value(
            memory_cards
                .get("slot2_filename")
                .map(String::as_str)
                .unwrap_or(""),
        );
        if !slot2.is_empty() {
            settings.slot2_filename = slot2;
        }
        return settings;
    }

    if let Some(default_root) = data_roots.first() {
        settings.data_root = default_root.to_string_lossy().to_string();
        settings.memory_cards = resolve_against(default_root, "memcards")
            .to_string_lossy()
            .to_string();
        settings.savestates = resolve_against(default_root, "sstates")
            .to_string_lossy()
            .to_string();
    }

    settings
}

/// `pcsx2_save_path_overrides` (`pcsx2.py:584-609`): the two memory-card
/// slot files, then the containing directory — FILES FIRST. `[]` when
/// [`pcsx2_directory_settings`] resolved no `memory_cards` directory.
pub fn pcsx2_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = pcsx2_directory_settings(path, args);
    let memory_cards = settings.memory_cards.trim();
    if memory_cards.is_empty() {
        return Vec::new();
    }

    let root = paths::expand_user(memory_cards);
    unique_paths(vec![
        root.join(&settings.slot1_filename),
        root.join(&settings.slot2_filename),
        root,
    ])
}

/// `pcsx2_state_path_overrides` (`pcsx2.py:612-621`): the single
/// `savestates` directory, or `[]` when it did not resolve.
pub fn pcsx2_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = pcsx2_directory_settings(path, args);
    let savestates = settings.savestates.trim();
    if savestates.is_empty() {
        Vec::new()
    } else {
        vec![PathBuf::from(savestates)]
    }
}

// =======================================================================
// DuckStation — duckstation.py:10, 144-195
// =======================================================================

/// `1`/`true`/`yes`/`on`, case-insensitively (`_duckstation_config_bool`,
/// `duckstation.py:50-51`).
fn duckstation_bool(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// `duckstation_memory_card_settings` (`duckstation.py:144-195`): walk
/// [`duckstation::config_path_candidates`] — shared verbatim with the
/// writer, `duckstation.py:10` — and stop at the first candidate with at
/// least one parsed `[MemoryCards]` line (an empty-valued or unrecognized
/// key still counts as parsed, matching `duckstation::memory_card_settings`'s
/// own doc comment on the same rule). Defaults when nothing parses:
/// `directory = "memcards"`, `card1_type = "PerGameTitle"`,
/// `card2_type = "None"`, `use_playlist_title = true`.
pub fn duckstation_memory_card_settings(path: &str) -> DuckstationSettings {
    let mut settings = DuckstationSettings {
        config_path: String::new(),
        directory: "memcards".to_string(),
        card1_type: "PerGameTitle".to_string(),
        card2_type: "None".to_string(),
        use_playlist_title: true,
    };

    for candidate in duckstation::config_path_candidates(path) {
        if !candidate.is_file() {
            continue;
        }
        let Ok(raw_content) = std::fs::read_to_string(&candidate) else {
            continue;
        };

        let mut current_section = String::new();
        let mut parsed_any = false;

        for raw_line in raw_content.lines() {
            let stripped = raw_line.trim();
            if stripped.is_empty() || stripped.starts_with('#') || stripped.starts_with(';') {
                continue;
            }
            if let Some(caps) = SECTION_RE.captures(stripped) {
                current_section = caps[1].trim().to_lowercase();
                continue;
            }
            if current_section != "memorycards" {
                continue;
            }
            let Some(eq_index) = raw_line.find('=') else {
                continue;
            };
            parsed_any = true;
            let key = raw_line[..eq_index].trim();
            let value = raw_line[eq_index + 1..].trim();
            if value.is_empty() {
                continue;
            }
            match key {
                "Directory" => settings.directory = value.to_string(),
                "Card1Type" => settings.card1_type = value.to_string(),
                "Card2Type" => settings.card2_type = value.to_string(),
                "UsePlaylistTitle" => settings.use_playlist_title = duckstation_bool(value),
                _ => {}
            }
        }

        if parsed_any {
            settings.config_path = candidate.to_string_lossy().to_string();
            break;
        }
    }

    settings
}

// =======================================================================
// Dolphin — dolphin.py:53-157, 435-560
// =======================================================================

const DOLPHIN_REGION_NAMES: [&str; 5] = ["USA", "JPN", "JAP", "EUR", "DEV"];
const DOLPHIN_MEMCARD_SIZE_SUFFIXES: [&str; 7] =
    ["", ".59", ".123", ".251", ".507", ".1019", ".2043"];
const DOLPHIN_WII_TITLE_GROUPS: [&str; 6] = [
    "00010000", "00010001", "00010002", "00010004", "00010005", "00010008",
];

/// `_launch_user_root` (`dolphin.py:41-67`): `-u`/`--user` (with the value
/// as the following token) or `--user=`/`--user=VALUE`, first match wins.
/// The value is quote-stripped; a blank result after cleaning is treated as
/// no match and the scan continues.
fn dolphin_launch_user_root(args: Args) -> Option<PathBuf> {
    let mut index = 0;
    while index < args.len() {
        let raw_arg = &args[index];
        let normalized = raw_arg.trim();
        if normalized.is_empty() {
            index += 1;
            continue;
        }
        let lowered = normalized.to_lowercase();

        if (lowered == "-u" || lowered == "--user") && index + 1 < args.len() {
            let next_arg = &args[index + 1];
            if !next_arg.trim().is_empty() {
                let cleaned = clean_ini_value(next_arg);
                if !cleaned.is_empty() {
                    return Some(resolve_best_effort(&paths::expand_user(&cleaned)));
                }
            }
            index += 1;
            continue;
        }

        if lowered.starts_with("--user=") {
            // `"--user=".len() == 7`, and every byte in that literal prefix
            // is ASCII, so slicing the ORIGINAL (mixed-case) text at byte 7
            // is safe and yields the same value Python's
            // `normalized_arg.split("=", 1)` would (`dolphin.py:61-63`).
            let value = &normalized[7..];
            let cleaned = clean_ini_value(value);
            if !cleaned.is_empty() {
                return Some(resolve_best_effort(&paths::expand_user(&cleaned)));
            }
        }

        index += 1;
    }
    None
}

/// Windows registry `HKCU\Software\Dolphin Emulator` (`dolphin.py:70-97`) is
/// not ported — no registry access from this crate. Always `None`, matching
/// the Python reference on every non-Windows platform (`dolphin.py:71-72`);
/// every caller already falls through to the next candidate.
fn dolphin_registry_user_root(_emulator_dir: &Path) -> Option<PathBuf> {
    None
}

/// `dolphin_user_root_candidates` (`dolphin.py:100-146`): launch `-u`, then
/// `<exe_dir>/User` when `portable.txt` exists, then the (unported, always
/// `None`) registry root, then — WINDOWS ONLY (`dolphin.py:125`'s explicit
/// `sys.platform == "win32"` gate, unlike PCSX2's ungated env-var checks) —
/// OneDrive/USERPROFILE Documents and `%APPDATA%`, then
/// `~/.dolphin-emu`, `~/Library/Application Support/Dolphin`, the Flatpak
/// data dir, then `<exe_dir>/User` again as a fallback. EVERY candidate is
/// expanded and canonicalized best-effort in one final pass
/// (`dolphin.py:146`), unlike PCSX2's selective resolution.
pub fn dolphin_user_root_candidates(path: &str, args: Args) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    let trimmed = path.trim();
    let emulator_dir = if trimmed.is_empty() {
        None
    } else {
        let expanded = paths::expand_user(trimmed);
        Some(if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        })
    };

    if let Some(root) = dolphin_launch_user_root(args) {
        candidates.push(root);
    }

    if let Some(dir) = &emulator_dir {
        if dir.join("portable.txt").exists() {
            candidates.push(dir.join("User"));
        }
        if let Some(registry_root) = dolphin_registry_user_root(dir) {
            candidates.push(registry_root);
        }
    }

    #[cfg(windows)]
    {
        for var in ["OneDrive", "USERPROFILE"] {
            if let Some(base) = env_trimmed(var) {
                candidates.push(
                    paths::expand_user(&base)
                        .join("Documents")
                        .join("Dolphin Emulator"),
                );
            }
        }
        if let Some(appdata) = env_trimmed("APPDATA") {
            candidates.push(paths::expand_user(&appdata).join("Dolphin Emulator"));
        }
    }

    let home = paths::home_dir().unwrap_or_default();
    candidates.push(home.join(".dolphin-emu"));
    candidates.push(
        home.join("Library")
            .join("Application Support")
            .join("Dolphin"),
    );
    candidates.push(
        home.join(".var")
            .join("app")
            .join("org.DolphinEmu.dolphin-emu")
            .join("data")
            .join("dolphin-emu"),
    );

    if let Some(dir) = &emulator_dir {
        candidates.push(dir.join("User"));
    }

    let resolved: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|c| !c.as_os_str().is_empty())
        .map(|c| resolve_best_effort(&paths::expand_user(&c.to_string_lossy())))
        .collect();
    unique_paths(resolved)
}

/// `<root>/Config/Dolphin.ini` for every [`dolphin_user_root_candidates`]
/// entry, deduplicated (`dolphin_settings_path_candidates`,
/// `dolphin.py:149-156`).
fn dolphin_settings_path_candidates(path: &str, args: Args) -> Vec<PathBuf> {
    unique_paths(
        dolphin_user_root_candidates(path, args)
            .into_iter()
            .map(|root| root.join("Config").join("Dolphin.ini"))
            .collect(),
    )
}

/// `dolphin_directory_settings` (`dolphin.py:427-487`): walk the user roots
/// and their `Config/Dolphin.ini` candidates in lockstep; the first
/// candidate that parses as an existing file wins. `[General] NANDRootPath`
/// resolves `wii_root`, defaulting to `<root>/Wii`; the four
/// `[Core] Memcard{A,B}Path`/`GCIFolder{A,B}Path{,Override}` fields resolve
/// with NO default, so an unconfigured one stays blank
/// ([`resolve_setting_or_default`]'s `""`/`""` branch). With no matching
/// candidate, `user_root`/`gc_root`/`wii_root`/`state_saves` still populate
/// from the first EXISTING root directory (or the first candidate root when
/// none exist), and every `Core`-derived field stays blank.
pub fn dolphin_directory_settings(path: &str, args: Args) -> DolphinSettings {
    let mut settings = DolphinSettings::default();

    let user_roots = dolphin_user_root_candidates(path, args);
    let settings_candidates = dolphin_settings_path_candidates(path, args);

    let mut first_existing_root: Option<PathBuf> = None;

    for (root, candidate) in user_roots.iter().zip(settings_candidates.iter()) {
        if first_existing_root.is_none() && root.is_dir() {
            first_existing_root = Some(root.clone());
        }
        if !candidate.is_file() {
            continue;
        }
        let Ok(raw_content) = std::fs::read_to_string(candidate) else {
            continue;
        };

        let sections = parse_ini_sections(&raw_content);
        let empty = HashMap::new();
        let core = sections.get("core").unwrap_or(&empty);
        let general = sections.get("general").unwrap_or(&empty);

        settings.config_path = candidate.to_string_lossy().to_string();
        settings.user_root = root.to_string_lossy().to_string();
        settings.gc_root = resolve_best_effort(&root.join("GC"))
            .to_string_lossy()
            .to_string();
        settings.wii_root = resolve_setting_or_default(
            root,
            general
                .get("nandrootpath")
                .map(String::as_str)
                .unwrap_or(""),
            "Wii",
        );
        settings.state_saves = resolve_best_effort(&root.join("StateSaves"))
            .to_string_lossy()
            .to_string();
        settings.memcard_a_path = resolve_setting_or_default(
            root,
            core.get("memcardapath").map(String::as_str).unwrap_or(""),
            "",
        );
        settings.memcard_b_path = resolve_setting_or_default(
            root,
            core.get("memcardbpath").map(String::as_str).unwrap_or(""),
            "",
        );
        settings.gci_folder_a_path = resolve_setting_or_default(
            root,
            core.get("gcifolderapath").map(String::as_str).unwrap_or(""),
            "",
        );
        settings.gci_folder_b_path = resolve_setting_or_default(
            root,
            core.get("gcifolderbpath").map(String::as_str).unwrap_or(""),
            "",
        );
        settings.gci_folder_a_override = resolve_setting_or_default(
            root,
            core.get("gcifolderapathoverride")
                .map(String::as_str)
                .unwrap_or(""),
            "",
        );
        settings.gci_folder_b_override = resolve_setting_or_default(
            root,
            core.get("gcifolderbpathoverride")
                .map(String::as_str)
                .unwrap_or(""),
            "",
        );
        return settings;
    }

    let default_root = first_existing_root.or_else(|| user_roots.first().cloned());
    if let Some(root) = default_root {
        if !root.as_os_str().is_empty() {
            settings.user_root = root.to_string_lossy().to_string();
            settings.gc_root = resolve_best_effort(&root.join("GC"))
                .to_string_lossy()
                .to_string();
            settings.wii_root = resolve_best_effort(&root.join("Wii"))
                .to_string_lossy()
                .to_string();
            settings.state_saves = resolve_best_effort(&root.join("StateSaves"))
                .to_string_lossy()
                .to_string();
        }
    }

    settings
}

/// Every `MemoryCard{letter}.{region}{suffix}.raw` permutation under
/// `gc_root` — 5 regions x 7 size suffixes (`_default_memcard_paths`,
/// `dolphin.py:490-496`).
fn dolphin_default_memcard_paths(gc_root: &Path, slot_letter: char) -> Vec<PathBuf> {
    let mut paths =
        Vec::with_capacity(DOLPHIN_REGION_NAMES.len() * DOLPHIN_MEMCARD_SIZE_SUFFIXES.len());
    for region in DOLPHIN_REGION_NAMES {
        for suffix in DOLPHIN_MEMCARD_SIZE_SUFFIXES {
            paths.push(gc_root.join(format!("MemoryCard{slot_letter}.{region}{suffix}.raw")));
        }
    }
    paths
}

/// `<gc_root>/<region>/Card {letter}` for every region (`_default_gci_paths`,
/// `dolphin.py:499-500`).
fn dolphin_default_gci_paths(gc_root: &Path, slot_letter: char) -> Vec<PathBuf> {
    DOLPHIN_REGION_NAMES
        .iter()
        .map(|region| gc_root.join(region).join(format!("Card {slot_letter}")))
        .collect()
}

/// `_configured_gci_paths` (`dolphin.py:503-508`): the configured path
/// itself, then that same path's sibling regions. When the configured
/// path's OWN directory name is already a region name (case-insensitively),
/// its PARENT is the base for the siblings — otherwise the configured path
/// itself is the base.
fn dolphin_configured_gci_paths(configured_path: &Path) -> Vec<PathBuf> {
    let name_upper = configured_path
        .file_name()
        .map(|n| n.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    let is_region = DOLPHIN_REGION_NAMES.contains(&name_upper.as_str());
    let base_path = if is_region {
        configured_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| configured_path.to_path_buf())
    } else {
        configured_path.to_path_buf()
    };

    let mut candidates = vec![configured_path.to_path_buf()];
    for region in DOLPHIN_REGION_NAMES {
        candidates.push(base_path.join(region));
    }
    unique_paths(candidates)
}

/// `dolphin_save_path_overrides` (`dolphin.py:511-554`): `[]` when
/// [`dolphin_directory_settings`] resolved no `user_root`. Otherwise, in
/// order: the configured `MemcardAPath`/`MemcardBPath` (when set) followed
/// by ALL 35 default permutations for that slot letter — the defaults are
/// unconditional, not gated on a configured path being present; the
/// `GCIFolder{A,B}PathOverride` values when set; the configured
/// `GCIFolder{A,B}Path` expanded to sibling regions (when set) followed by
/// the 5 default `Card {letter}` paths — again unconditional; then
/// `<wii_root>/title` and its six title-group children.
pub fn dolphin_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = dolphin_directory_settings(path, args);
    if settings.user_root.trim().is_empty() {
        return Vec::new();
    }

    let gc_root = if !settings.gc_root.is_empty() {
        resolve_best_effort(&paths::expand_user(&settings.gc_root))
    } else {
        resolve_best_effort(&paths::expand_user(&settings.user_root).join("GC"))
    };
    let wii_root = if !settings.wii_root.is_empty() {
        resolve_best_effort(&paths::expand_user(&settings.wii_root))
    } else {
        resolve_best_effort(&paths::expand_user(&settings.user_root).join("Wii"))
    };

    let mut raw_paths: Vec<PathBuf> = Vec::new();

    for (configured, slot_letter) in [
        (&settings.memcard_a_path, 'A'),
        (&settings.memcard_b_path, 'B'),
    ] {
        if !configured.trim().is_empty() {
            raw_paths.push(resolve_best_effort(&paths::expand_user(configured.trim())));
        }
        raw_paths.extend(dolphin_default_memcard_paths(&gc_root, slot_letter));
    }

    for configured in [
        &settings.gci_folder_a_override,
        &settings.gci_folder_b_override,
    ] {
        if !configured.trim().is_empty() {
            raw_paths.push(resolve_best_effort(&paths::expand_user(configured.trim())));
        }
    }

    for (configured, slot_letter) in [
        (&settings.gci_folder_a_path, 'A'),
        (&settings.gci_folder_b_path, 'B'),
    ] {
        if !configured.trim().is_empty() {
            let resolved_configured = resolve_best_effort(&paths::expand_user(configured.trim()));
            raw_paths.extend(dolphin_configured_gci_paths(&resolved_configured));
        }
        raw_paths.extend(dolphin_default_gci_paths(&gc_root, slot_letter));
    }

    raw_paths.push(wii_root.join("title"));
    for group in DOLPHIN_WII_TITLE_GROUPS {
        raw_paths.push(wii_root.join("title").join(group));
    }

    unique_paths(raw_paths)
}

/// `dolphin_state_path_overrides` (`dolphin.py:557-566`): the single
/// `state_saves` directory, or `[]` when it did not resolve.
pub fn dolphin_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = dolphin_directory_settings(path, args);
    let state_saves = settings.state_saves.trim();
    if state_saves.is_empty() {
        Vec::new()
    } else {
        vec![PathBuf::from(state_saves)]
    }
}

// =======================================================================
// RPCS3 — rpcs3.py:13-110, 285-305, 469-531, 605-745
// =======================================================================

/// Exactly 8 ASCII digits and not `00000000` (`_is_valid_user_id`,
/// `rpcs3.py:13,28-29`).
fn rpcs3_is_valid_user_id(value: &str) -> bool {
    value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()) && value != "00000000"
}

/// `_launch_user_id` (`rpcs3.py:46-72`): `--user-id <id>` (next token) or
/// `--user-id=<id>`, first VALID 8-digit id wins; an invalid candidate does
/// not stop the scan.
fn rpcs3_launch_user_id(args: Args) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        let raw_arg = &args[index];
        let normalized = raw_arg.trim();
        if normalized.is_empty() {
            index += 1;
            continue;
        }
        let lowered = normalized.to_lowercase();

        if lowered == "--user-id" && index + 1 < args.len() {
            let candidate = args[index + 1].trim();
            if rpcs3_is_valid_user_id(candidate) {
                return Some(candidate.to_string());
            }
            index += 1;
            continue;
        }

        if lowered.starts_with("--user-id=") {
            let candidate = normalized[normalized.find('=').unwrap() + 1..].trim();
            if rpcs3_is_valid_user_id(candidate) {
                return Some(candidate.to_string());
            }
        }

        index += 1;
    }
    None
}

/// `_yaml_scalar_value` (`rpcs3.py:82-91`): find `key: value` (optionally
/// quoted key, case-insensitive, anywhere on its own line), and treat
/// `""`, `{}`, `[]`, `|`, `>` as unset. Cleaned via [`clean_yaml_value`]
/// (comment-stripping and quote-stripping) before being returned.
fn rpcs3_yaml_scalar_value(raw_content: &str, key: &str) -> String {
    let pattern = format!(
        r#"(?im)^\s*["']?{}["']?\s*:\s*(.+?)\s*$"#,
        regex::escape(key)
    );
    let Ok(re) = Regex::new(&pattern) else {
        return String::new();
    };
    let Some(caps) = re.captures(raw_content) else {
        return String::new();
    };
    let raw_value = caps[1].trim();
    if matches!(raw_value, "" | "{}" | "[]" | "|" | ">") {
        return String::new();
    }
    clean_yaml_value(raw_value)
}

/// `_resolve_rpcs3_path` (`rpcs3.py:94-110`): clean `raw_value` (RPCS3's
/// comment-aware [`clean_yaml_value`]); fall back to `default_value` when
/// blank; with STILL nothing, return `base_root` itself, canonicalized —
/// unlike [`resolve_against`], which never fills in a base-root fallback.
/// Otherwise substitute `$(EmulatorDir)` with `base_root`, forward-slashed
/// and trailing-slash-terminated, THEN run `$VAR`/`~` expansion, THEN join
/// onto `base_root` if still relative, then canonicalize best-effort.
fn resolve_rpcs3_path(base_root: &Path, raw_value: &str, default_value: &str) -> PathBuf {
    let cleaned = clean_yaml_value(raw_value);
    let value = if cleaned.is_empty() {
        default_value.to_string()
    } else {
        cleaned
    };
    if value.is_empty() {
        return resolve_best_effort(base_root);
    }

    let resolved_base = resolve_best_effort(base_root);
    let mut emulator_dir_value = resolved_base.to_string_lossy().replace('\\', "/");
    if !emulator_dir_value.ends_with('/') {
        emulator_dir_value.push('/');
    }

    let substituted = value.replace("$(EmulatorDir)", &emulator_dir_value);
    let expanded = paths::expand_user(&expand_vars(&substituted));
    let candidate = if expanded.is_absolute() {
        expanded
    } else {
        base_root.join(expanded)
    };
    resolve_best_effort(&candidate)
}

/// `<data_root>/config/vfs.yml` then `<data_root>/vfs.yml`
/// (`_vfs_path_candidates_for_root`, `rpcs3.py:633-634`).
fn rpcs3_vfs_path_candidates(data_root: &Path) -> Vec<PathBuf> {
    unique_paths(vec![
        data_root.join("config").join("vfs.yml"),
        data_root.join("vfs.yml"),
    ])
}

/// `_persistent_active_user` (`rpcs3.py:641-657`): read
/// `<data_root>/GuiConfigs/persistent_settings.dat`'s `[Users] active_user`.
/// Returns `("", "")` when no such file exists; `(user, path)` when the
/// parsed id is a valid 8-digit id; `("", path)` when the file exists but
/// the id is missing or invalid.
fn rpcs3_persistent_active_user(data_root: &Path) -> (String, String) {
    let candidate = data_root.join("GuiConfigs").join("persistent_settings.dat");
    if !candidate.is_file() {
        return (String::new(), String::new());
    }
    let Ok(raw_content) = std::fs::read_to_string(&candidate) else {
        return (String::new(), String::new());
    };
    let sections = parse_ini_sections(&raw_content);
    let active_user = sections
        .get("users")
        .and_then(|section| section.get("active_user"))
        .map(|v| v.trim().to_string())
        .unwrap_or_default();

    if rpcs3_is_valid_user_id(&active_user) {
        (active_user, candidate.to_string_lossy().to_string())
    } else {
        (String::new(), candidate.to_string_lossy().to_string())
    }
}

/// `rpcs3_data_root_candidates` (`rpcs3.py:605-630`): `<exe_dir>/portable`
/// (when it exists, canonicalized) then `<exe_dir>` (canonicalized); then
/// `$RPCS3_CONFIG_DIR`, expanded and canonicalized, inserted at index 1 (or
/// 0 when the list above was empty — `rpcs3.py:619-621`); then
/// `$XDG_CONFIG_HOME/rpcs3` and `~/Library/Application Support/rpcs3`,
/// pushed WITHOUT canonicalizing (`rpcs3.py:625-627` never calls `.resolve()`
/// on either). Deduplicated case-insensitively. `args` exists only for
/// signature parity with the other `*_data_root_candidates` readers in this
/// file — the Python reference takes no launch-argument parameters at all
/// (`rpcs3.py:605`).
pub fn rpcs3_data_root_candidates(path: &str, _args: Args) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    let trimmed = path.trim();
    if !trimmed.is_empty() {
        let expanded = paths::expand_user(trimmed);
        let emulator_dir = if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if !emulator_dir.as_os_str().is_empty() {
            let portable_dir = emulator_dir.join("portable");
            if portable_dir.is_dir() {
                candidates.push(resolve_best_effort(&portable_dir));
            }
            candidates.push(resolve_best_effort(&emulator_dir));
        }
    }

    if let Some(config_env) = env_trimmed("RPCS3_CONFIG_DIR") {
        let resolved = resolve_best_effort(&paths::expand_user(&config_env));
        let insert_at = if candidates.is_empty() { 0 } else { 1 };
        candidates.insert(insert_at, resolved);
    }

    let home = paths::home_dir().unwrap_or_default();
    candidates.push(paths::xdg_config_home().join("rpcs3"));
    candidates.push(
        home.join("Library")
            .join("Application Support")
            .join("rpcs3"),
    );

    unique_paths(candidates)
}

/// `rpcs3_data_root` (`rpcs3.py:285-304`): `None` for a blank path, or when
/// the path neither exists as a directory/file nor has an existing parent
/// directory. Otherwise `<emulator_dir>/portable` when that is a directory,
/// else `emulator_dir` itself — both canonicalized.
pub fn rpcs3_data_root(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let expanded = paths::expand_user(trimmed);
    let emulator_dir = if expanded.is_dir() {
        expanded
    } else if expanded.is_file() {
        expanded.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        let parent = expanded.parent().map(Path::to_path_buf).unwrap_or_default();
        if !parent.is_dir() {
            return None;
        }
        parent
    };

    let portable_dir = emulator_dir.join("portable");
    if portable_dir.is_dir() {
        Some(resolve_best_effort(&portable_dir))
    } else {
        Some(resolve_best_effort(&emulator_dir))
    }
}

/// `rpcs3_directory_settings` (`rpcs3.py:660-706`): only the FIRST data-root
/// candidate is ever inspected — the Python reference's `for data_root in
/// ...: ...; return settings` loop returns unconditionally on its first
/// iteration, so a second or later candidate is never tried. `current_user`
/// is the launch `--user-id` when present, else the persisted
/// `[Users] active_user`, else `"00000001"`. `dev_hdd0` reads `vfs.yml`'s
/// `/dev_hdd0/` scalar (default `$(EmulatorDir)dev_hdd0/`) against whatever
/// `$(EmulatorDir)` resolves to (default: the data root itself). With no
/// data-root candidates at all, every field stays blank except
/// `current_user`, which still falls back to the launch id or
/// `"00000001"`.
pub fn rpcs3_directory_settings(path: &str, args: Args) -> Rpcs3Settings {
    let launch_user = rpcs3_launch_user_id(args);
    let mut settings = Rpcs3Settings {
        current_user: launch_user
            .clone()
            .unwrap_or_else(|| "00000001".to_string()),
        ..Default::default()
    };

    let Some(data_root) = rpcs3_data_root_candidates(path, args).into_iter().next() else {
        return settings;
    };

    settings.data_root = data_root.to_string_lossy().to_string();

    let (persistent_user, persistent_settings_path) = rpcs3_persistent_active_user(&data_root);
    if !persistent_settings_path.is_empty() {
        settings.persistent_settings_path = persistent_settings_path;
    }
    if launch_user.is_none() && !persistent_user.is_empty() {
        settings.current_user = persistent_user;
    }

    let mut emulator_root = resolve_best_effort(&data_root);
    let mut dev_hdd0_root = emulator_root.join("dev_hdd0");

    for candidate in rpcs3_vfs_path_candidates(&data_root) {
        if !candidate.is_file() {
            continue;
        }
        let Ok(raw_content) = std::fs::read_to_string(&candidate) else {
            continue;
        };

        settings.config_path = candidate.to_string_lossy().to_string();
        let raw_emulator_root = rpcs3_yaml_scalar_value(&raw_content, "$(EmulatorDir)");
        emulator_root = resolve_rpcs3_path(&data_root, &raw_emulator_root, "");
        let raw_dev_hdd0 = rpcs3_yaml_scalar_value(&raw_content, "/dev_hdd0/");
        dev_hdd0_root =
            resolve_rpcs3_path(&emulator_root, &raw_dev_hdd0, "$(EmulatorDir)dev_hdd0/");
        break;
    }

    settings.dev_hdd0 = dev_hdd0_root.to_string_lossy().to_string();
    settings
}

/// `rpcs3_save_path_overrides` (`rpcs3.py:709-744`): `[]` when
/// [`rpcs3_directory_settings`] resolved no `dev_hdd0`. Otherwise the
/// current user's `<dev_hdd0>/home/<user>/savedata` first (only when
/// `current_user` is a valid 8-digit id), then every EXISTING valid
/// 8-digit user directory under `<dev_hdd0>/home` in NAME order, then
/// `<dev_hdd0>/home/00000001/savedata` as a guaranteed tail entry — all
/// resolved and deduplicated case-insensitively.
pub fn rpcs3_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = rpcs3_directory_settings(path, args);
    let dev_hdd0 = settings.dev_hdd0.trim();
    if dev_hdd0.is_empty() {
        return Vec::new();
    }

    let home_root = paths::expand_user(dev_hdd0).join("home");
    let mut raw_paths: Vec<PathBuf> = Vec::new();

    if rpcs3_is_valid_user_id(&settings.current_user) {
        raw_paths.push(home_root.join(&settings.current_user).join("savedata"));
    }

    if home_root.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&home_root) {
            let mut names: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.path())
                .collect();
            names.sort_by_key(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
            for child in names {
                let name = child
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if rpcs3_is_valid_user_id(&name) {
                    raw_paths.push(child.join("savedata"));
                }
            }
        }
    }

    raw_paths.push(home_root.join("00000001").join("savedata"));

    let resolved: Vec<PathBuf> = raw_paths.iter().map(|p| resolve_best_effort(p)).collect();
    unique_paths(resolved)
}

/// Shared body of [`ps3_vfs_dev_hdd0_path`]/[`ps3_vfs_games_path`]: search
/// every data root's `vfs.yml` candidates for `yaml_key`'s scalar; the
/// first FILE candidate found for a given data root ends that data root's
/// inner search (success or not) and moves on to the next data root. `None`
/// when no `vfs.yml` supplied a value AND `ps3_library` is blank.
fn ps3_vfs_scalar_path(
    path: &str,
    args: Args,
    ps3_library: &str,
    yaml_key: &str,
    library_suffix: &str,
) -> Option<PathBuf> {
    let path_text = path.trim();
    let library_text = ps3_library.trim();

    for data_root in rpcs3_data_root_candidates(path_text, args) {
        for vfs_candidate in rpcs3_vfs_path_candidates(&data_root) {
            if !vfs_candidate.is_file() {
                continue;
            }
            let Ok(raw_content) = std::fs::read_to_string(&vfs_candidate) else {
                continue;
            };
            let raw_emulator_root = rpcs3_yaml_scalar_value(&raw_content, "$(EmulatorDir)");
            let emulator_root = resolve_rpcs3_path(&data_root, &raw_emulator_root, "");
            let raw_value = rpcs3_yaml_scalar_value(&raw_content, yaml_key);
            if !raw_value.is_empty() {
                return Some(resolve_rpcs3_path(&emulator_root, &raw_value, ""));
            }
            break;
        }
    }

    if library_text.is_empty() {
        return None;
    }
    let library_path = resolve_best_effort(&paths::expand_user(library_text));
    Some(library_path.join(".vfs").join(library_suffix))
}

/// `ps3_vfs_dev_hdd0_path` (`rpcs3.py:471-500`): the resolved
/// `/dev_hdd0/` VFS scalar when a readable `vfs.yml` supplies one; else
/// `<ps3_library>/.vfs/dev_hdd0`; else `None` when the library path is
/// also blank.
pub fn ps3_vfs_dev_hdd0_path(path: &str, args: Args, ps3_library: &str) -> Option<PathBuf> {
    ps3_vfs_scalar_path(path, args, ps3_library, "/dev_hdd0/", "dev_hdd0")
}

/// `ps3_vfs_games_path` (`rpcs3.py:503-530`): the resolved `/games/` VFS
/// scalar when a readable `vfs.yml` supplies one; else
/// `<ps3_library>/.vfs/games`; else `None` when the library path is also
/// blank.
pub fn ps3_vfs_games_path(path: &str, args: Args, ps3_library: &str) -> Option<PathBuf> {
    ps3_vfs_scalar_path(path, args, ps3_library, "/games/", "games")
}

// =======================================================================
// Shared helpers for part 2 (Azahar, Eden, Cemu, Xemu, Xenia, Redream,
// FBNeo, MAME, Pico-8, Vita3K, Flycast VMU).
// =======================================================================

/// `_bool_value` as azahar.py:31-41, eden.py and xenia.py:147-157 each
/// define it verbatim: case-insensitive `1`/`true`/`yes`/`on` -> `true`,
/// `0`/`false`/`no`/`off` -> `false`; a blank value or anything else ->
/// `default`.
fn bool_value(value: &str, default: bool) -> bool {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return default;
    }
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

/// [`resolve_setting_or_default`], but cleaning `raw_value` via
/// [`clean_yaml_value`] (comment-aware) rather than [`clean_ini_value`) —
/// the pipeline Xemu's and Xenia's own `_resolve_setting_path`/
/// `_clean_path_value` use (xemu.py:59-74, xenia.py:140-168), unlike every
/// other reader in this file's plain quote-stripping.
fn resolve_setting_or_default_commented(
    base: &Path,
    raw_value: &str,
    default_value: &str,
) -> String {
    let cleaned = clean_yaml_value(raw_value);
    let value = if cleaned.is_empty() {
        default_value.to_string()
    } else {
        cleaned
    };
    if value.is_empty() {
        String::new()
    } else {
        resolve_against(base, &value).to_string_lossy().to_string()
    }
}

/// `_consume_arg_value` (`xemu.py:36-56`, `xenia.py:117-137`, `mame.py:34-54`,
/// `pico8.py:38-58`) reporting BOTH the cleaned value and the last argument
/// index consumed, matching the Python tuple return exactly — unlike
/// [`consume_arg_value`], whose callers never needed the index. `start_index`
/// itself for an out-of-bounds or blank starting token; the index of the
/// closing-quote token (or `args.len()` when no closing quote was ever
/// found) after a multi-token quoted rejoin; `start_index` for a single
/// self-contained token.
fn consume_arg_value_indexed(args: Args, start_index: usize) -> (String, usize) {
    let Some(raw_token) = args.get(start_index) else {
        return (String::new(), start_index);
    };
    let token = raw_token.trim();
    if token.is_empty() {
        return (String::new(), start_index);
    }

    let first = token.chars().next().unwrap();
    let quote = (first == '"' || first == '\'').then_some(first);

    if let Some(quote) = quote {
        if token.chars().count() == 1 || !token.ends_with(quote) {
            let mut parts = vec![token.to_string()];
            let mut index = start_index + 1;
            while index < args.len() {
                parts.push(args[index].clone());
                if args[index].trim().ends_with(quote) {
                    break;
                }
                index += 1;
            }
            return (clean_ini_value(&parts.join(" ")), index);
        }
    }

    (clean_ini_value(token), start_index)
}

/// A directory's entries (files and directories alike), sorted by `key_fn`
/// applied to each entry's own path — the shared engine behind
/// [`sorted_dir_entries`] and [`sorted_dir_entries_casefold`]. An
/// unreadable directory yields an empty list.
fn sorted_dir_entries_by(dir: &Path, key_fn: impl Fn(&Path) -> String) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort_by_key(|p| key_fn(p));
    paths
}

/// `sorted(dir.iterdir(), key=lambda item: item.name)` — plain (non-folded)
/// name order, as azahar.py's and eden.py's title/save-root walks use.
fn sorted_dir_entries(dir: &Path) -> Vec<PathBuf> {
    sorted_dir_entries_by(dir, |p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    })
}

/// `sorted(dir.iterdir(), key=lambda item: item.name.casefold())` — as
/// xenia.py's content-root walk uses (xenia.py:451, xenia.py:456).
fn sorted_dir_entries_casefold(dir: &Path) -> Vec<PathBuf> {
    sorted_dir_entries_by(dir, |p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    })
}

// =======================================================================
// Azahar / Eden — azahar.py:10, 219-425; eden.py:294-472
// =======================================================================

const SDMC_TITLE_GROUPS: [&str; 5] = ["00040000", "00040002", "0004000e", "0004008c", "00048004"];
const NAND_TITLE_GROUPS: [&str; 2] = ["00040010", "00040030"];
/// `_ZERO_ID` (azahar.py:12): 32 ASCII zeros.
const ZERO_ID_32: &str = "00000000000000000000000000000000";

/// `azahar_directory_settings`/`eden_directory_settings`'s return shape.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AzaharSettings {
    pub config_path: String,
    pub user_root: String,
    pub nand_root: String,
    pub sdmc_root: String,
    pub states_root: String,
    pub use_custom_storage: bool,
    pub use_virtual_sd: bool,
}

/// Same field set as [`AzaharSettings`] — Eden's own return shape, kept as a
/// distinct nominal type per the task interface even though the two structs
/// are structurally identical today.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EdenSettings {
    pub config_path: String,
    pub user_root: String,
    pub nand_root: String,
    pub sdmc_root: String,
    pub states_root: String,
    pub use_custom_storage: bool,
    pub use_virtual_sd: bool,
}

/// The shared body of [`azahar_directory_settings`] and
/// [`eden_directory_settings`] (azahar.py:272-330, eden.py:394-452): the two
/// functions are byte-for-byte identical past their own user-root/settings
/// candidate lists, differing only in the `[Data Storage]` INI they read
/// (already selected by the caller via `settings_candidates`).
struct NintendoStorageSettings {
    config_path: String,
    user_root: String,
    nand_root: String,
    sdmc_root: String,
    states_root: String,
    use_custom_storage: bool,
    use_virtual_sd: bool,
}

impl Default for NintendoStorageSettings {
    fn default() -> Self {
        Self {
            config_path: String::new(),
            user_root: String::new(),
            nand_root: String::new(),
            sdmc_root: String::new(),
            states_root: String::new(),
            use_custom_storage: false,
            use_virtual_sd: true,
        }
    }
}

fn resolve_nintendo_storage_settings(
    user_roots: &[PathBuf],
    settings_candidates: &[PathBuf],
) -> NintendoStorageSettings {
    let mut settings = NintendoStorageSettings::default();
    let mut first_existing_root: Option<PathBuf> = None;

    for (root, candidate) in user_roots.iter().zip(settings_candidates.iter()) {
        if first_existing_root.is_none() && root.is_dir() {
            first_existing_root = Some(resolve_best_effort(root));
        }
        if !candidate.is_file() {
            continue;
        }
        let Ok(raw_content) = std::fs::read_to_string(candidate) else {
            continue;
        };

        let sections = parse_ini_sections(&raw_content);
        let empty = HashMap::new();
        let storage = sections.get("data storage").unwrap_or(&empty);
        let use_custom_storage = bool_value(
            storage
                .get("use_custom_storage")
                .map(String::as_str)
                .unwrap_or("false"),
            false,
        );
        let use_virtual_sd = bool_value(
            storage
                .get("use_virtual_sd")
                .map(String::as_str)
                .unwrap_or("true"),
            true,
        );

        settings.config_path = resolve_best_effort(candidate).to_string_lossy().to_string();
        settings.user_root = resolve_best_effort(root).to_string_lossy().to_string();
        settings.states_root = resolve_best_effort(&root.join("states"))
            .to_string_lossy()
            .to_string();
        settings.use_custom_storage = use_custom_storage;
        settings.use_virtual_sd = use_virtual_sd;

        if use_custom_storage {
            settings.nand_root = resolve_setting_or_default(
                root,
                storage
                    .get("nand_directory")
                    .map(String::as_str)
                    .unwrap_or(""),
                "nand",
            );
            settings.sdmc_root = resolve_setting_or_default(
                root,
                storage
                    .get("sdmc_directory")
                    .map(String::as_str)
                    .unwrap_or(""),
                "sdmc",
            );
        } else {
            settings.nand_root = resolve_best_effort(&root.join("nand"))
                .to_string_lossy()
                .to_string();
            settings.sdmc_root = resolve_best_effort(&root.join("sdmc"))
                .to_string_lossy()
                .to_string();
        }
        return settings;
    }

    let default_root = first_existing_root.or_else(|| user_roots.first().cloned());
    if let Some(root) = default_root {
        if !root.as_os_str().is_empty() {
            settings.user_root = resolve_best_effort(&root).to_string_lossy().to_string();
            settings.nand_root = resolve_best_effort(&root.join("nand"))
                .to_string_lossy()
                .to_string();
            settings.sdmc_root = resolve_best_effort(&root.join("sdmc"))
                .to_string_lossy()
                .to_string();
            settings.states_root = resolve_best_effort(&root.join("states"))
                .to_string_lossy()
                .to_string();
        }
    }
    settings
}

/// Existing `<sdmc>/Nintendo 3DS/<sysid>/<storeid>/title/<group>`
/// directories for every [`SDMC_TITLE_GROUPS`] entry under every existing
/// system/storage id pair, sorted by name (azahar.py:333-354). Falls back to
/// the all-zero 32-character id path when nothing exists.
fn azahar_existing_sdmc_title_roots(sdmc_root: &Path) -> Vec<PathBuf> {
    let container_root = sdmc_root.join("Nintendo 3DS");
    let mut discovered = Vec::new();

    if container_root.is_dir() {
        for system_dir in sorted_dir_entries(&container_root) {
            if !system_dir.is_dir() {
                continue;
            }
            for storage_dir in sorted_dir_entries(&system_dir) {
                if !storage_dir.is_dir() {
                    continue;
                }
                let title_root = storage_dir.join("title");
                for group in SDMC_TITLE_GROUPS {
                    let candidate = resolve_best_effort(&title_root.join(group));
                    if candidate.is_dir() {
                        discovered.push(candidate);
                    }
                }
            }
        }
    }

    if !discovered.is_empty() {
        return unique_paths(discovered);
    }
    let default_title_root = container_root
        .join(ZERO_ID_32)
        .join(ZERO_ID_32)
        .join("title");
    unique_paths(
        SDMC_TITLE_GROUPS
            .iter()
            .map(|group| resolve_best_effort(&default_title_root.join(group)))
            .collect(),
    )
}

/// Existing `<nand>/title/<group>` and `<nand>/<child>/title/<group>`
/// directories for every [`NAND_TITLE_GROUPS`] entry (azahar.py:357-384).
/// Falls back to `<nand>/<0*32>/title/<group>` then `<nand>/title/<group>`
/// when nothing exists.
fn azahar_existing_nand_title_roots(nand_root: &Path) -> Vec<PathBuf> {
    let mut discovered = Vec::new();
    let mut title_containers: Vec<PathBuf> = Vec::new();

    let direct_title_root = nand_root.join("title");
    if direct_title_root.is_dir() {
        title_containers.push(direct_title_root);
    }
    if nand_root.is_dir() {
        for child in sorted_dir_entries(nand_root) {
            if !child.is_dir() {
                continue;
            }
            let title_root = child.join("title");
            if title_root.is_dir() {
                title_containers.push(title_root);
            }
        }
    }

    for title_root in &title_containers {
        for group in NAND_TITLE_GROUPS {
            let candidate = resolve_best_effort(&title_root.join(group));
            if candidate.is_dir() {
                discovered.push(candidate);
            }
        }
    }

    if !discovered.is_empty() {
        return unique_paths(discovered);
    }
    let mut fallbacks: Vec<PathBuf> = NAND_TITLE_GROUPS
        .iter()
        .map(|group| resolve_best_effort(&nand_root.join(ZERO_ID_32).join("title").join(group)))
        .collect();
    fallbacks.extend(
        NAND_TITLE_GROUPS
            .iter()
            .map(|group| resolve_best_effort(&nand_root.join("title").join(group))),
    );
    unique_paths(fallbacks)
}

/// `azahar_user_root_candidates` (azahar.py:219-259). `args` is accepted for
/// signature uniformity only — the Python reference deletes its
/// `launch_template`/splitter parameters unused (azahar.py:224).
fn azahar_user_root_candidates(path: &str) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut emulator_dir = PathBuf::new();

    let trimmed = path.trim();
    if !trimmed.is_empty() {
        let expanded = paths::expand_user(trimmed);
        emulator_dir = if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if !emulator_dir.as_os_str().is_empty() {
            let portable_root = resolve_best_effort(&emulator_dir.join("user"));
            if portable_root.is_dir() {
                candidates.push(portable_root);
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(appdata) = env_trimmed("APPDATA") {
            candidates.push(resolve_best_effort(
                &paths::expand_user(&appdata).join("Azahar"),
            ));
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = env_trimmed("XDG_DATA_HOME") {
            candidates.push(resolve_best_effort(
                &paths::expand_user(&xdg).join("Azahar"),
            ));
        }
        let home = paths::home_dir().unwrap_or_default();
        candidates.push(home.join(".local").join("share").join("Azahar"));
        candidates.push(
            home.join("Library")
                .join("Application Support")
                .join("Azahar"),
        );
        candidates.push(
            home.join(".var")
                .join("app")
                .join("org.azahar_emu.Azahar")
                .join("data")
                .join("Azahar"),
        );
    }

    if !emulator_dir.as_os_str().is_empty() {
        candidates.push(resolve_best_effort(&emulator_dir.join("user")));
    }

    let resolved: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|c| !c.as_os_str().is_empty())
        .map(|c| resolve_best_effort(&paths::expand_user(&c.to_string_lossy())))
        .collect();
    unique_paths(resolved)
}

/// `azahar_directory_settings` (azahar.py:272-330).
pub fn azahar_directory_settings(path: &str, _args: Args) -> AzaharSettings {
    let user_roots = azahar_user_root_candidates(path);
    let settings_candidates = unique_paths(
        user_roots
            .iter()
            .map(|root| root.join("config").join("qt-config.ini"))
            .collect(),
    );
    let settings = resolve_nintendo_storage_settings(&user_roots, &settings_candidates);
    AzaharSettings {
        config_path: settings.config_path,
        user_root: settings.user_root,
        nand_root: settings.nand_root,
        sdmc_root: settings.sdmc_root,
        states_root: settings.states_root,
        use_custom_storage: settings.use_custom_storage,
        use_virtual_sd: settings.use_virtual_sd,
    }
}

/// `azahar_save_path_overrides` (azahar.py:387-412): the SDMC title roots
/// (skipped when `use_virtual_sd` is false), then the NAND title roots.
pub fn azahar_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = azahar_directory_settings(path, args);
    let mut raw_paths: Vec<PathBuf> = Vec::new();

    if settings.use_virtual_sd && !settings.sdmc_root.trim().is_empty() {
        let sdmc_root = resolve_best_effort(&paths::expand_user(settings.sdmc_root.trim()));
        raw_paths.extend(azahar_existing_sdmc_title_roots(&sdmc_root));
    }
    if !settings.nand_root.trim().is_empty() {
        let nand_root = resolve_best_effort(&paths::expand_user(settings.nand_root.trim()));
        raw_paths.extend(azahar_existing_nand_title_roots(&nand_root));
    }

    unique_paths(raw_paths)
}

/// `azahar_state_path_overrides` (azahar.py:415-424): the single
/// `<user_root>/states` directory, or `[]` when it did not resolve.
pub fn azahar_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = azahar_directory_settings(path, args);
    let states_root = settings.states_root.trim();
    if states_root.is_empty() {
        Vec::new()
    } else {
        vec![PathBuf::from(states_root)]
    }
}

/// `_app_name_candidates` (eden.py:294-313): the executable stem in three
/// casings (as given, lowercased, Python `str.title()`-cased), then `Eden`,
/// `eden`, `yuzu`, `Yuzu`, `suyu`, `Suyu` — deduplicated case-insensitively,
/// keeping the first spelling.
fn eden_app_name_candidates(path: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();

    let trimmed = path.trim();
    if !trimmed.is_empty() {
        let stem = Path::new(trimmed)
            .file_stem()
            .map(|s| s.to_string_lossy().trim().to_string())
            .unwrap_or_default();
        if !stem.is_empty() {
            names.push(stem.clone());
            names.push(stem.to_lowercase());
            names.push(python_title_case(&stem));
        }
    }
    for extra in ["Eden", "eden", "yuzu", "Yuzu", "suyu", "Suyu"] {
        names.push(extra.to_string());
    }

    let mut seen = HashSet::new();
    names
        .into_iter()
        .filter(|name| {
            let key = name.to_lowercase();
            !key.is_empty() && seen.insert(key)
        })
        .collect()
}

/// Python `str.title()`: a cased character following an uncased one is
/// upper-cased, a cased character following a cased one is lower-cased —
/// non-alphabetic characters (digits, `-`, `_`, …) count as "uncased"
/// boundaries and are copied through unchanged.
fn python_title_case(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut previous_is_alpha = false;
    for ch in text.chars() {
        if ch.is_alphabetic() {
            if previous_is_alpha {
                result.extend(ch.to_lowercase());
            } else {
                result.extend(ch.to_uppercase());
            }
            previous_is_alpha = true;
        } else {
            result.push(ch);
            previous_is_alpha = false;
        }
    }
    result
}

/// `eden_user_root_candidates` (eden.py:316-359). `args` is accepted for
/// signature uniformity only, matching [`azahar_user_root_candidates`].
fn eden_user_root_candidates(path: &str) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut emulator_dir = PathBuf::new();

    let trimmed = path.trim();
    if !trimmed.is_empty() {
        let expanded = paths::expand_user(trimmed);
        emulator_dir = if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if !emulator_dir.as_os_str().is_empty() {
            let portable_root = resolve_best_effort(&emulator_dir.join("user"));
            if portable_root.is_dir() {
                candidates.push(portable_root);
            }
        }
    }

    let app_names = eden_app_name_candidates(path);
    #[cfg(windows)]
    {
        if let Some(appdata) = env_trimmed("APPDATA") {
            let base = paths::expand_user(&appdata);
            for name in &app_names {
                candidates.push(resolve_best_effort(&base.join(name)));
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = env_trimmed("XDG_DATA_HOME") {
            let base = paths::expand_user(&xdg);
            for name in &app_names {
                candidates.push(resolve_best_effort(&base.join(name)));
            }
        }
        let home = paths::home_dir().unwrap_or_default();
        for name in &app_names {
            candidates.push(home.join(".local").join("share").join(name));
            candidates.push(home.join("Library").join("Application Support").join(name));
        }
    }

    if !emulator_dir.as_os_str().is_empty() {
        candidates.push(resolve_best_effort(&emulator_dir.join("user")));
    }

    let resolved: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|c| !c.as_os_str().is_empty())
        .map(|c| resolve_best_effort(&paths::expand_user(&c.to_string_lossy())))
        .collect();
    unique_paths(resolved)
}

/// `eden_directory_settings` (eden.py:394-452).
pub fn eden_directory_settings(path: &str, _args: Args) -> EdenSettings {
    let user_roots = eden_user_root_candidates(path);
    let settings_candidates = unique_paths(
        user_roots
            .iter()
            .map(|root| root.join("config").join("qt-config.ini"))
            .collect(),
    );
    let settings = resolve_nintendo_storage_settings(&user_roots, &settings_candidates);
    EdenSettings {
        config_path: settings.config_path,
        user_root: settings.user_root,
        nand_root: settings.nand_root,
        sdmc_root: settings.sdmc_root,
        states_root: settings.states_root,
        use_custom_storage: settings.use_custom_storage,
        use_virtual_sd: settings.use_virtual_sd,
    }
}

/// `eden_keys_path` (eden.py:372-380): `<emulator_dir>/user/keys/prod.keys`
/// when it exists as a file, else `None`. `None` for a blank path too.
pub fn eden_keys_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let expanded = paths::expand_user(trimmed);
    let emulator_dir = if expanded.is_dir() {
        expanded
    } else {
        expanded.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let keys_path = emulator_dir.join("user").join("keys").join("prod.keys");
    if keys_path.is_file() {
        Some(resolve_best_effort(&keys_path))
    } else {
        None
    }
}

/// `eden_has_firmware` (eden.py:383-391): true when
/// `<emulator_dir>/user/nand/system/Contents/registered` is a non-empty
/// directory.
pub fn eden_has_firmware(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }
    let expanded = paths::expand_user(trimmed);
    let emulator_dir = if expanded.is_dir() {
        expanded
    } else {
        expanded.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let firmware_dir = emulator_dir
        .join("user")
        .join("nand")
        .join("system")
        .join("Contents")
        .join("registered");
    if !firmware_dir.is_dir() {
        return false;
    }
    std::fs::read_dir(&firmware_dir)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// Existing `<nand>/user/save/0000000000000000/<user-dir>` directories that
/// contain at least one subdirectory, sorted by name (eden.py:455-469).
/// Falls back to the parent save root itself when none qualify.
fn eden_existing_user_save_roots(nand_root: &Path) -> Vec<PathBuf> {
    let save_root = nand_root.join("user").join("save").join("0000000000000000");
    let mut discovered = Vec::new();

    if save_root.is_dir() {
        for user_root in sorted_dir_entries(&save_root) {
            if !user_root.is_dir() {
                continue;
            }
            let has_child_dir = std::fs::read_dir(&user_root)
                .map(|entries| entries.flatten().any(|e| e.path().is_dir()))
                .unwrap_or(false);
            if has_child_dir {
                discovered.push(resolve_best_effort(&user_root));
            }
        }
    }

    if !discovered.is_empty() {
        return unique_paths(discovered);
    }
    unique_paths(vec![resolve_best_effort(&save_root)])
}

/// `eden_save_path_overrides` (eden.py:472-492). Eden has no state-path
/// override function.
pub fn eden_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = eden_directory_settings(path, args);
    let nand_root = settings.nand_root.trim();
    if nand_root.is_empty() {
        return Vec::new();
    }
    let nand_root = resolve_best_effort(&paths::expand_user(nand_root));
    unique_paths(eden_existing_user_save_roots(&nand_root))
}

// =======================================================================
// Cemu — cemu.py:252-270, 361-443
// =======================================================================

/// `cemu_directory_settings`'s return shape (cemu.py:361-384).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CemuSettings {
    pub config_path: String,
    pub mlc_path: String,
}

/// The first `<mlc_path>...</mlc_path>` element's inner text in `raw_content`
/// (D11-style regex substring scan, matching this crate's no-XML-crate
/// deviation already established for the Cemu writer — see `cemu.rs`'s
/// module doc comment). `None` when absent or blank once trimmed.
static CEMU_MLC_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<mlc_path>([^<]*)</mlc_path>").unwrap());

fn cemu_mlc_path_from_xml(raw_content: &str) -> Option<String> {
    let caps = CEMU_MLC_PATH_RE.captures(raw_content)?;
    let value = caps[1].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// `cemu_directory_settings` (cemu.py:361-384): the first
/// [`cemu::settings_path_candidates`] entry that exists as a file AND yields
/// a non-blank `<mlc_path>` wins. Neither `config_path` nor `mlc_path` is
/// filesystem-resolved — `config_path` is the candidate's own (unresolved)
/// text, `mlc_path` is the raw, trimmed XML text (cemu.py:380-381 never
/// calls `.resolve()` on either).
pub fn cemu_directory_settings(path: &str, _args: Args) -> CemuSettings {
    let mut settings = CemuSettings::default();
    for candidate in cemu::settings_path_candidates(path) {
        if !candidate.is_file() {
            continue;
        }
        let Ok(raw_content) = std::fs::read_to_string(&candidate) else {
            continue;
        };
        if let Some(mlc_path) = cemu_mlc_path_from_xml(&raw_content) {
            settings.config_path = candidate.to_string_lossy().to_string();
            settings.mlc_path = mlc_path;
            return settings;
        }
    }
    settings
}

/// `_save_root_from_mlc_path` (cemu.py:387-395): a purely textual transform,
/// never touching the filesystem. Already ending in `usr/save` (either slash
/// style, case-insensitively, after collapsing repeated slashes) -> the
/// ORIGINAL text with trailing `/`/`\` trimmed; else `<mlc_path>/usr/save`
/// (plain `Path` join, not resolved). `""` for a blank (after quote-trim)
/// input.
fn cemu_save_root_from_mlc_path(raw_path: &str) -> String {
    let mlc_path = raw_path.trim().trim_matches('"').trim_matches('\'');
    if mlc_path.is_empty() {
        return String::new();
    }

    static SLASH_RUN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\\/]+").unwrap());
    let normalized = SLASH_RUN_RE
        .replace_all(mlc_path, "/")
        .trim_end_matches('/')
        .to_lowercase();

    if normalized.ends_with("/usr/save") {
        mlc_path.trim_end_matches(['\\', '/']).to_string()
    } else {
        PathBuf::from(mlc_path)
            .join("usr")
            .join("save")
            .to_string_lossy()
            .to_string()
    }
}

/// `cemu_save_path_overrides` (cemu.py:398-443): MLC paths from
/// `-m`/`--mlc <value>` or `--mlc=`/`-m=<value>` launch arguments (in
/// argument order — NOT a manual index-advancing scan; every token is
/// checked, matching the Python reference's plain `for index, raw_arg in
/// enumerate(args)` loop), then [`cemu_directory_settings`]'s own
/// `mlc_path` when non-blank. Each is converted via
/// [`cemu_save_root_from_mlc_path`] and deduplicated. Cemu has no
/// state-path override function.
pub fn cemu_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let mut raw_mlc_paths: Vec<String> = Vec::new();

    for (index, raw_arg) in args.iter().enumerate() {
        let normalized = raw_arg.trim();
        if normalized.is_empty() {
            continue;
        }
        let lowered = normalized.to_lowercase();

        if (lowered == "-m" || lowered == "--mlc") && index + 1 < args.len() {
            let next_arg = args[index + 1].trim();
            if !next_arg.is_empty() {
                raw_mlc_paths.push(next_arg.to_string());
            }
            continue;
        }

        if lowered.starts_with("--mlc=") || lowered.starts_with("-m=") {
            if let Some(eq_pos) = normalized.find('=') {
                let value = normalized[eq_pos + 1..].trim();
                if !value.is_empty() {
                    raw_mlc_paths.push(value.to_string());
                }
            }
        }
    }

    let settings = cemu_directory_settings(path, args);
    if !settings.mlc_path.trim().is_empty() {
        raw_mlc_paths.push(settings.mlc_path.trim().to_string());
    }

    let resolved: Vec<PathBuf> = raw_mlc_paths
        .iter()
        .map(|raw| PathBuf::from(cemu_save_root_from_mlc_path(raw)))
        .collect();
    unique_paths(resolved)
}

// =======================================================================
// Xemu — xemu.py:36-137, 340-460
// =======================================================================

/// `xemu_directory_settings`'s return shape (xemu.py:392-437).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XemuSettings {
    pub config_path: String,
    pub base_path: String,
    pub hdd_path: String,
    pub eeprom_path: String,
}

const XEMU_CONFIG_PATH_FLAGS: [&str; 4] = [
    "-config_path",
    "--config_path",
    "-config-path",
    "--config-path",
];

/// `_launch_config_path` (xemu.py:81-117): the first `-config_path`/
/// `--config_path`/`-config-path`/`--config-path` (space- or `=`-separated)
/// launch argument with a non-blank value wins. A value with a file
/// extension (`Path.suffix` truthy) is treated as the config file itself;
/// otherwise `xemu.toml` is appended as if the value named a directory.
fn xemu_launch_config_path(args: Args) -> Option<PathBuf> {
    fn resolve_candidate(value: &str) -> PathBuf {
        let expanded = paths::expand_user(&expand_vars(value));
        if expanded.extension().is_some() {
            resolve_best_effort(&expanded)
        } else {
            resolve_best_effort(&expanded.join("xemu.toml"))
        }
    }

    let mut index = 0;
    while index < args.len() {
        let raw_arg = &args[index];
        index += 1;
        let normalized = raw_arg.trim();
        if normalized.is_empty() {
            continue;
        }
        let lowered = normalized.to_lowercase();

        if XEMU_CONFIG_PATH_FLAGS.contains(&lowered.as_str()) && index < args.len() {
            let (value, consumed_index) = consume_arg_value_indexed(args, index);
            index = consumed_index + 1;
            if !value.is_empty() {
                return Some(resolve_candidate(&value));
            }
            continue;
        }

        for flag in XEMU_CONFIG_PATH_FLAGS {
            let prefix = format!("{flag}=");
            if lowered.starts_with(&prefix) {
                if let Some(eq_pos) = normalized.find('=') {
                    let value = clean_ini_value(&normalized[eq_pos + 1..]);
                    if !value.is_empty() {
                        return Some(resolve_candidate(&value));
                    }
                }
            }
        }
    }
    None
}

/// `xemu_base_path_candidates` (xemu.py:137-158): a launch config-path
/// override's parent directory first, then the emulator directory when it
/// already holds `xemu.toml`, `xbox_hdd.qcow2` or `eeprom.bin`, then
/// [`xemu::default_base_root`].
pub fn xemu_base_path_candidates(path: &str, args: Args) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(config_override) = xemu_launch_config_path(args) {
        let parent = config_override
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        candidates.push(resolve_best_effort(&parent));
    }

    let trimmed = path.trim();
    if !trimmed.is_empty() {
        let expanded = paths::expand_user(trimmed);
        let emulator_dir = if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if !emulator_dir.as_os_str().is_empty() {
            let has_marker = emulator_dir.join("xemu.toml").exists()
                || emulator_dir.join("xbox_hdd.qcow2").exists()
                || emulator_dir.join("eeprom.bin").exists();
            if has_marker {
                candidates.push(resolve_best_effort(&emulator_dir));
            }
        }
    }

    candidates.push(xemu::default_base_root());
    unique_paths(candidates)
}

/// `xemu_config_path_candidates` (xemu.py:161-181): when a launch override
/// is present, it comes first, followed by every base-path candidate's own
/// `xemu.toml` EXCEPT the one whose resolved root equals the override's
/// resolved parent (already listed). With no override, every base path's
/// `xemu.toml`.
pub fn xemu_config_path_candidates(path: &str, args: Args) -> Vec<PathBuf> {
    if let Some(config_override) = xemu_launch_config_path(args) {
        let resolved_override = resolve_best_effort(&config_override);
        let override_parent = config_override
            .parent()
            .map(resolve_best_effort)
            .unwrap_or_default();

        let mut list = vec![resolved_override];
        for root in xemu_base_path_candidates(path, args) {
            if resolve_best_effort(&root) != override_parent {
                list.push(resolve_best_effort(&root.join("xemu.toml")));
            }
        }
        return unique_paths(list);
    }

    unique_paths(
        xemu_base_path_candidates(path, args)
            .into_iter()
            .map(|root| root.join("xemu.toml"))
            .collect(),
    )
}

/// `_parse_inline_table` (xemu.py:338-348): `raw_value` must be a `{ ... }`
/// span once trimmed; each `key = value` pair inside (quoted or comma
/// -terminated) is captured, key lowercased and trimmed, value trimmed but
/// NOT quote-stripped.
fn xemu_parse_inline_table(raw_value: &str) -> HashMap<String, String> {
    let stripped = raw_value.trim();
    if !stripped.starts_with('{') || !stripped.ends_with('}') {
        return HashMap::new();
    }
    let body = &stripped[1..stripped.len() - 1];

    static INLINE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"([A-Za-z0-9_.]+)\s*=\s*("[^"]*"|'[^']*'|[^,]+)"#).unwrap());
    let mut values = HashMap::new();
    for caps in INLINE_RE.captures_iter(body) {
        let key = caps[1].trim().to_lowercase();
        let value = caps[2].trim().to_string();
        values.insert(key, value);
    }
    values
}

/// `_parse_toml_sections` (xemu.py:351-389): dotted keys flatten into
/// synthetic `<section>.<prefix>` sections; a literal `files = { ... }`
/// value additionally expands into a `<section>.files` pseudo-section via
/// [`xemu_parse_inline_table`].
fn xemu_parse_toml_sections(raw_content: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for raw_line in raw_content.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        if let Some(caps) = SECTION_RE.captures(stripped) {
            current_section = caps[1].trim().to_lowercase();
            sections.entry(current_section.clone()).or_default();
            continue;
        }
        let Some(eq_index) = raw_line.find('=') else {
            continue;
        };
        let key = raw_line[..eq_index].trim().to_lowercase();
        let value = raw_line[eq_index + 1..].trim().to_string();

        let (target_section, target_key) = if let Some(dot_pos) = key.rfind('.') {
            let prefix = &key[..dot_pos];
            let suffix = &key[dot_pos + 1..];
            let target_section = if current_section.is_empty() {
                prefix.to_string()
            } else {
                format!("{current_section}.{prefix}")
                    .trim_matches('.')
                    .to_string()
            };
            (target_section, suffix.to_string())
        } else {
            (current_section.clone(), key.clone())
        };

        sections
            .entry(target_section)
            .or_default()
            .insert(target_key, value.clone());

        if key == "files" {
            let inline_values = xemu_parse_inline_table(&value);
            if !inline_values.is_empty() {
                let file_section_key = if current_section.is_empty() {
                    "files".to_string()
                } else {
                    format!("{current_section}.files")
                        .trim_matches('.')
                        .to_string()
                };
                let file_section = sections.entry(file_section_key).or_default();
                for (inline_key, inline_value) in inline_values {
                    let stored_key = inline_key
                        .rsplit('.')
                        .next()
                        .unwrap_or(&inline_key)
                        .to_string();
                    file_section.insert(stored_key, inline_value);
                }
            }
        }
    }
    sections
}

/// `xemu_directory_settings` (xemu.py:392-437): walk base paths and their
/// config-path candidates in lockstep; the first config file that exists
/// wins (its `[sys.files] hdd_path`/`eeprom_path`, resolved via
/// [`resolve_setting_or_default_commented`], override the
/// `<base>/xbox_hdd.qcow2` / `<base>/eeprom.bin` defaults already set for
/// that pair); failing that, the first candidate whose base directory
/// already exists; failing THAT, the first base path's own defaults.
pub fn xemu_directory_settings(path: &str, args: Args) -> XemuSettings {
    let base_paths = xemu_base_path_candidates(path, args);
    let config_paths = xemu_config_path_candidates(path, args);
    let mut defaults = XemuSettings::default();

    for (base_root, config_path) in base_paths.iter().zip(config_paths.iter()) {
        let mut settings = XemuSettings {
            base_path: resolve_best_effort(base_root).to_string_lossy().to_string(),
            config_path: resolve_best_effort(config_path)
                .to_string_lossy()
                .to_string(),
            hdd_path: resolve_best_effort(&base_root.join("xbox_hdd.qcow2"))
                .to_string_lossy()
                .to_string(),
            eeprom_path: resolve_best_effort(&base_root.join("eeprom.bin"))
                .to_string_lossy()
                .to_string(),
        };

        if config_path.is_file() {
            if let Ok(raw_content) = std::fs::read_to_string(config_path) {
                if !raw_content.is_empty() {
                    let sections = xemu_parse_toml_sections(&raw_content);
                    let empty = HashMap::new();
                    let file_settings = sections.get("sys.files").unwrap_or(&empty);
                    settings.hdd_path = resolve_setting_or_default_commented(
                        base_root,
                        file_settings
                            .get("hdd_path")
                            .map(String::as_str)
                            .unwrap_or(""),
                        "xbox_hdd.qcow2",
                    );
                    settings.eeprom_path = resolve_setting_or_default_commented(
                        base_root,
                        file_settings
                            .get("eeprom_path")
                            .map(String::as_str)
                            .unwrap_or(""),
                        "eeprom.bin",
                    );
                }
            }
            return settings;
        }

        if base_root.is_dir() {
            return settings;
        }
    }

    if let Some(base_root) = base_paths.first() {
        defaults.base_path = resolve_best_effort(base_root).to_string_lossy().to_string();
        defaults.config_path = resolve_best_effort(&base_root.join("xemu.toml"))
            .to_string_lossy()
            .to_string();
        defaults.hdd_path = resolve_best_effort(&base_root.join("xbox_hdd.qcow2"))
            .to_string_lossy()
            .to_string();
        defaults.eeprom_path = resolve_best_effort(&base_root.join("eeprom.bin"))
            .to_string_lossy()
            .to_string();
    }
    defaults
}

/// `xemu_save_path_overrides` (xemu.py:440-459): `[hdd_path, eeprom_path]`,
/// deduplicated. There is no state-path override.
pub fn xemu_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = xemu_directory_settings(path, args);
    let mut raw = Vec::new();
    for value in [&settings.hdd_path, &settings.eeprom_path] {
        if !value.trim().is_empty() {
            raw.push(PathBuf::from(value.trim()));
        }
    }
    unique_paths(raw)
}

// =======================================================================
// Xenia — xenia.py:91-489
// =======================================================================

/// `xenia_directory_settings`'s return shape (xenia.py:346-428).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XeniaSettings {
    pub variant: String,
    pub config_path: String,
    pub storage_root: String,
    pub content_root: String,
    pub cache_root: String,
    pub portable: bool,
}

static XENIA_HEX8_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-fA-F]{8}$").unwrap());
static XENIA_HEX16_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9a-fA-F]{16}$").unwrap());

/// `_emulator_dir` (xenia.py:171-177): dir-or-parent, empty `PathBuf` for a
/// blank path.
fn xenia_emulator_dir(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return PathBuf::new();
    }
    let expanded = paths::expand_user(trimmed);
    if expanded.is_dir() {
        expanded
    } else {
        expanded.parent().map(Path::to_path_buf).unwrap_or_default()
    }
}

/// `_is_canary_variant` (xenia.py:180-182): `xenia_canary`, `xenia-canary`
/// or bare `canary`, matched case-insensitively as a substring anywhere in
/// the (untrimmed-of-slashes) path text.
fn xenia_is_canary_variant(path: &str) -> bool {
    let normalized = path.trim().to_lowercase();
    ["xenia_canary", "xenia-canary", "canary"]
        .iter()
        .any(|token| normalized.contains(token))
}

/// `_is_edge_variant` (xenia.py:185-187): `xenia_edge` or `xenia-edge`.
fn xenia_is_edge_variant(path: &str) -> bool {
    let normalized = path.trim().to_lowercase();
    ["xenia_edge", "xenia-edge"]
        .iter()
        .any(|token| normalized.contains(token))
}

/// `_resolve_launch_path` (xenia.py:190-198): comment-aware clean via
/// [`clean_yaml_value`]; `""` for a blank result (no default fallback,
/// unlike [`resolve_setting_or_default_commented`]); joined onto
/// `base_root` only when `base_root` itself is non-blank.
fn xenia_resolve_launch_path(base_root: &Path, raw_value: &str) -> String {
    let value = clean_yaml_value(raw_value);
    if value.is_empty() {
        return String::new();
    }
    let expanded = paths::expand_user(&expand_vars(&value));
    let candidate = if expanded.is_absolute() {
        expanded
    } else if !base_root.as_os_str().is_empty() {
        base_root.join(expanded)
    } else {
        expanded
    };
    resolve_best_effort(&candidate)
        .to_string_lossy()
        .to_string()
}

/// Which override field a matched xenia path flag targets — see
/// [`xenia_launch_overrides`].
enum XeniaFlagMatch {
    NotMatched,
    MatchedNoValue,
    MatchedWithValue(String),
}

/// Try one xenia path-flag alias group (e.g. `-storage_root`/
/// `--storage-root`/…) against the current token: an exact match consumes a
/// following value via [`consume_arg_value_indexed`] (advancing `*index`
/// exactly as the Python tuple return does); an `=`-suffixed match reads the
/// value after the first `=` in the ORIGINAL (not lowercased) token text.
fn xenia_try_path_flag(
    names: &[&str],
    args: Args,
    lowered: &str,
    normalized: &str,
    index: &mut usize,
    emulator_dir: &Path,
) -> XeniaFlagMatch {
    if names.contains(&lowered) && *index < args.len() {
        let (value, consumed_index) = consume_arg_value_indexed(args, *index);
        *index = consumed_index + 1;
        if !value.is_empty() {
            return XeniaFlagMatch::MatchedWithValue(xenia_resolve_launch_path(
                emulator_dir,
                &value,
            ));
        }
        return XeniaFlagMatch::MatchedNoValue;
    }

    for name in names {
        let prefix = format!("{name}=");
        if lowered.starts_with(&prefix) {
            if let Some(eq_pos) = normalized.find('=') {
                let raw_value = &normalized[eq_pos + 1..];
                if !raw_value.trim().is_empty() {
                    return XeniaFlagMatch::MatchedWithValue(xenia_resolve_launch_path(
                        emulator_dir,
                        raw_value,
                    ));
                }
            }
            return XeniaFlagMatch::MatchedNoValue;
        }
    }
    XeniaFlagMatch::NotMatched
}

#[derive(Debug, Clone, Default)]
struct XeniaLaunchOverrides {
    config_path: String,
    storage_root: String,
    content_root: String,
    cache_root: String,
    portable: Option<bool>,
}

const XENIA_PORTABLE_BOOL_TOKENS: [&str; 8] = ["0", "1", "true", "false", "yes", "no", "on", "off"];

/// `_launch_path_overrides` (xenia.py:201-276): the four path-flag groups
/// (checked in `config_path`, `storage_root`, `content_root`, `cache_root`
/// order, matching the Python dict's insertion order) then, only when no
/// path flag matched the token, the `-portable`/`--portable` flag —
/// optionally followed by a recognized boolean token, consumed only when
/// present.
fn xenia_launch_overrides(path: &str, args: Args) -> XeniaLaunchOverrides {
    let emulator_dir = xenia_emulator_dir(path);
    let mut overrides = XeniaLaunchOverrides::default();

    let flag_groups: [(&str, &[&str]); 4] = [
        ("config_path", &["-config", "--config"]),
        (
            "storage_root",
            &[
                "-storage_root",
                "--storage_root",
                "-storage-root",
                "--storage-root",
            ],
        ),
        (
            "content_root",
            &[
                "-content_root",
                "--content_root",
                "-content-root",
                "--content-root",
            ],
        ),
        (
            "cache_root",
            &["-cache_root", "--cache_root", "-cache-root", "--cache-root"],
        ),
    ];
    const PORTABLE_OPTIONS: [&str; 2] = ["-portable", "--portable"];

    let mut index = 0;
    while index < args.len() {
        let raw_arg = &args[index];
        index += 1;
        let normalized = raw_arg.trim();
        if normalized.is_empty() {
            continue;
        }
        let lowered = normalized.to_lowercase();

        let mut matched_option = false;
        for (field, names) in flag_groups {
            match xenia_try_path_flag(names, args, &lowered, normalized, &mut index, &emulator_dir)
            {
                XeniaFlagMatch::NotMatched => continue,
                XeniaFlagMatch::MatchedNoValue => {
                    matched_option = true;
                    break;
                }
                XeniaFlagMatch::MatchedWithValue(resolved) => {
                    match field {
                        "config_path" => overrides.config_path = resolved,
                        "storage_root" => overrides.storage_root = resolved,
                        "content_root" => overrides.content_root = resolved,
                        "cache_root" => overrides.cache_root = resolved,
                        _ => unreachable!(),
                    }
                    matched_option = true;
                    break;
                }
            }
        }
        if matched_option {
            continue;
        }

        if PORTABLE_OPTIONS.contains(&lowered.as_str()) {
            let mut portable_value = true;
            if index < args.len() {
                let next_value = args[index].trim();
                let next_lowered = next_value.to_lowercase();
                if XENIA_PORTABLE_BOOL_TOKENS.contains(&next_lowered.as_str()) {
                    portable_value = bool_value(next_value, true);
                    index += 1;
                }
            }
            overrides.portable = Some(portable_value);
            continue;
        }

        for name in PORTABLE_OPTIONS {
            let prefix = format!("{name}=");
            if lowered.starts_with(&prefix) {
                if let Some(eq_pos) = normalized.find('=') {
                    let raw_value = &normalized[eq_pos + 1..];
                    overrides.portable = Some(bool_value(raw_value, true));
                }
                break;
            }
        }
    }
    overrides
}

/// `_default_user_storage_root` (xenia.py:279-289).
fn xenia_default_user_storage_root() -> PathBuf {
    let home = paths::home_dir().unwrap_or_default();
    if cfg!(target_os = "windows") {
        return resolve_best_effort(&home.join("Documents").join("Xenia"));
    }
    if cfg!(target_os = "macos") {
        return resolve_best_effort(
            &home
                .join("Library")
                .join("Application Support")
                .join("Xenia"),
        );
    }
    if let Some(xdg) = env_trimmed("XDG_DATA_HOME") {
        return resolve_best_effort(&paths::expand_user(&xdg).join("Xenia"));
    }
    resolve_best_effort(&home.join(".local").join("share").join("Xenia"))
}

/// `_config_name_candidates` (xenia.py:292-319): edge names win over
/// canary names when BOTH `is_edge` and `is_canary` are true — a genuine
/// Python quirk (variant selection itself prefers canary; config-name
/// selection prefers edge) preserved here rather than "fixed".
fn xenia_config_name_candidates(is_canary: bool, is_edge: bool) -> Vec<String> {
    let mut names = vec![
        "xenia.config.toml".to_string(),
        "xenia-config.toml".to_string(),
    ];
    if is_edge {
        let mut prefixed: Vec<String> = [
            "xenia-edge.config.toml",
            "xenia-edge-config.toml",
            "xenia_edge.config.toml",
            "xenia_edge-config.toml",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        prefixed.append(&mut names);
        names = prefixed;
    } else if is_canary {
        let mut prefixed: Vec<String> = [
            "xenia-canary.config.toml",
            "xenia-canary-config.toml",
            "xenia_canary.config.toml",
            "xenia_canary-config.toml",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        prefixed.append(&mut names);
        names = prefixed;
    }

    let mut seen = HashSet::new();
    names
        .into_iter()
        .filter(|name| {
            let key = name.to_lowercase();
            !key.is_empty() && seen.insert(key)
        })
        .collect()
}

/// `_parse_toml_sections` (xenia.py:322-343): a plain (non-flattening)
/// section/key-value TOML reader — unlike Xemu's, no dotted-key or inline
/// -table handling.
fn xenia_parse_toml_sections(raw_content: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current_section = String::new();

    for raw_line in raw_content.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        if let Some(caps) = SECTION_RE.captures(stripped) {
            current_section = caps[1].trim().to_lowercase();
            sections.entry(current_section.clone()).or_default();
            continue;
        }
        let Some(eq_index) = raw_line.find('=') else {
            continue;
        };
        let key = raw_line[..eq_index].trim().to_lowercase();
        let value = raw_line[eq_index + 1..].trim().to_string();
        sections
            .entry(current_section.clone())
            .or_default()
            .insert(key, value);
    }
    sections
}

/// `xenia_directory_settings` (xenia.py:346-428).
pub fn xenia_directory_settings(path: &str, args: Args) -> XeniaSettings {
    let emulator_dir = xenia_emulator_dir(path);
    let is_canary = xenia_is_canary_variant(path);
    let is_edge = xenia_is_edge_variant(path);
    let launch_overrides = xenia_launch_overrides(path, args);

    let portable_file_exists =
        !emulator_dir.as_os_str().is_empty() && emulator_dir.join("portable.txt").exists();
    let default_portable = is_canary && cfg!(target_os = "windows");
    let portable_mode =
        portable_file_exists || launch_overrides.portable.unwrap_or(default_portable);

    let storage_root: PathBuf = if !launch_overrides.storage_root.trim().is_empty() {
        resolve_best_effort(&paths::expand_user(launch_overrides.storage_root.trim()))
    } else if portable_mode && !emulator_dir.as_os_str().is_empty() {
        resolve_best_effort(&emulator_dir)
    } else {
        xenia_default_user_storage_root()
    };

    let variant = if is_canary {
        "canary"
    } else if is_edge {
        "edge"
    } else {
        "master"
    };
    let cache_dir_name = if is_canary || is_edge {
        "cache_host"
    } else {
        "cache"
    };

    let mut settings = XeniaSettings {
        variant: variant.to_string(),
        config_path: String::new(),
        storage_root: resolve_best_effort(&storage_root)
            .to_string_lossy()
            .to_string(),
        content_root: resolve_best_effort(&storage_root.join("content"))
            .to_string_lossy()
            .to_string(),
        cache_root: resolve_best_effort(&storage_root.join(cache_dir_name))
            .to_string_lossy()
            .to_string(),
        portable: portable_mode,
    };

    if !launch_overrides.content_root.trim().is_empty() {
        settings.content_root = resolve_setting_or_default_commented(
            &storage_root,
            &launch_overrides.content_root,
            "content",
        );
    }
    if !launch_overrides.cache_root.trim().is_empty() {
        settings.cache_root = resolve_setting_or_default_commented(
            &storage_root,
            &launch_overrides.cache_root,
            cache_dir_name,
        );
    }

    let mut config_candidates: Vec<PathBuf> = Vec::new();
    if !launch_overrides.config_path.trim().is_empty() {
        config_candidates.push(resolve_best_effort(&paths::expand_user(
            launch_overrides.config_path.trim(),
        )));
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if !storage_root.as_os_str().is_empty() {
        roots.push(storage_root.clone());
    }
    if !emulator_dir.as_os_str().is_empty() {
        roots.push(emulator_dir.clone());
    }
    for root in unique_paths(roots) {
        for name in xenia_config_name_candidates(is_canary, is_edge) {
            config_candidates.push(resolve_best_effort(&root.join(name)));
        }
    }

    for candidate in unique_paths(config_candidates) {
        if !candidate.is_file() {
            continue;
        }
        settings.config_path = candidate.to_string_lossy().to_string();
        let Ok(raw_content) = std::fs::read_to_string(&candidate) else {
            return settings;
        };

        let sections = xenia_parse_toml_sections(&raw_content);
        let empty = HashMap::new();
        let storage = sections.get("storage").unwrap_or(&empty);

        if launch_overrides.content_root.trim().is_empty() {
            settings.content_root = resolve_setting_or_default_commented(
                &storage_root,
                storage
                    .get("content_root")
                    .map(String::as_str)
                    .unwrap_or(""),
                "content",
            );
        }
        if launch_overrides.cache_root.trim().is_empty() {
            settings.cache_root = resolve_setting_or_default_commented(
                &storage_root,
                storage.get("cache_root").map(String::as_str).unwrap_or(""),
                cache_dir_name,
            );
        }
        return settings;
    }

    settings
}

/// The existing subset of `00000001`, `Headers/00000001`, `profile` under a
/// title directory (xenia.py:431-443).
fn xenia_save_roots_for_title_dir(title_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for relative in [
        PathBuf::from("00000001"),
        PathBuf::from("Headers").join("00000001"),
        PathBuf::from("profile"),
    ] {
        let candidate = title_dir.join(&relative);
        if candidate.is_dir() {
            candidates.push(resolve_best_effort(&candidate));
        }
    }
    unique_paths(candidates)
}

/// `_existing_xenia_save_roots` (xenia.py:446-464): first-level 16-hex
/// entries are XUIDs whose 8-hex children are titles; first-level 8-hex
/// entries are titles directly. Both levels walked in casefolded name
/// order.
fn xenia_existing_save_roots(content_root: &Path) -> Vec<PathBuf> {
    let mut discovered = Vec::new();
    if !content_root.is_dir() {
        return discovered;
    }

    for first_level in sorted_dir_entries_casefold(content_root) {
        if !first_level.is_dir() {
            continue;
        }
        let name = first_level
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if XENIA_HEX16_RE.is_match(&name) {
            for title_dir in sorted_dir_entries_casefold(&first_level) {
                let title_name = title_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if title_dir.is_dir() && XENIA_HEX8_RE.is_match(&title_name) {
                    discovered.extend(xenia_save_roots_for_title_dir(&title_dir));
                }
            }
            continue;
        }

        if XENIA_HEX8_RE.is_match(&name) {
            discovered.extend(xenia_save_roots_for_title_dir(&first_level));
        }
    }
    unique_paths(discovered)
}

/// `xenia_save_path_overrides` (xenia.py:467-486).
pub fn xenia_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = xenia_directory_settings(path, args);
    if settings.content_root.trim().is_empty() {
        return Vec::new();
    }
    let content_root = resolve_best_effort(&paths::expand_user(settings.content_root.trim()));
    unique_paths(xenia_existing_save_roots(&content_root))
}

/// `xenia_state_path_overrides` (xenia.py:489-495): ALWAYS `[]`.
pub fn xenia_state_path_overrides(_path: &str, _args: Args) -> Vec<PathBuf> {
    Vec::new()
}

// =======================================================================
// Redream — redream.py:30-152
// =======================================================================

/// `redream_directory_settings`'s return shape (redream.py:81-107).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedreamSettings {
    pub config_path: String,
    pub data_root: String,
    pub portable: bool,
}

/// `redream_directory_settings` (redream.py:81-107), built on
/// [`redream::data_root_candidates`] (the writer's own portable-marker/
/// platform-default candidate list, redream.py:57-77 — shared verbatim
/// rather than duplicated). `portable` is true exactly when a chosen data
/// root resolves to the same directory as the emulator directory itself,
/// regardless of WHY that root was selected.
pub fn redream_directory_settings(path: &str, _args: Args) -> RedreamSettings {
    let trimmed = path.trim();
    let emulator_dir = if trimmed.is_empty() {
        PathBuf::new()
    } else {
        let expanded = paths::expand_user(trimmed);
        if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        }
    };

    let data_roots = redream::data_root_candidates(path);
    let mut defaults = RedreamSettings::default();
    let mut fallback_set = false;

    for data_root in &data_roots {
        let resolved_root = resolve_best_effort(data_root);
        let settings = RedreamSettings {
            config_path: resolve_best_effort(&data_root.join("redream.cfg"))
                .to_string_lossy()
                .to_string(),
            data_root: resolved_root.to_string_lossy().to_string(),
            portable: !emulator_dir.as_os_str().is_empty()
                && resolved_root == resolve_best_effort(&emulator_dir),
        };

        if data_root.join("redream.cfg").exists() || data_root.is_dir() {
            return settings;
        }

        if !fallback_set {
            defaults = settings;
            fallback_set = true;
        }
    }
    defaults
}

/// `redream_save_path_overrides` (redream.py:110-129): the existing subset
/// of `vmu0.bin`..`vmu3.bin` FILE paths in the data root.
pub fn redream_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = redream_directory_settings(path, args);
    let data_root_text = settings.data_root.trim();
    if data_root_text.is_empty() {
        return Vec::new();
    }
    let data_root = paths::expand_user(data_root_text);
    if !data_root.is_dir() {
        return Vec::new();
    }

    let mut vmu_paths = Vec::new();
    for slot in 0..4 {
        let candidate = data_root.join(format!("vmu{slot}.bin"));
        if candidate.is_file() {
            vmu_paths.push(candidate);
        }
    }
    unique_paths(vmu_paths)
}

/// `redream_state_path_overrides` (redream.py:132-151): `<data_root>/states`
/// when it exists, then the data root itself.
pub fn redream_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = redream_directory_settings(path, args);
    let data_root_text = settings.data_root.trim();
    if data_root_text.is_empty() {
        return Vec::new();
    }
    let data_root = paths::expand_user(data_root_text);
    if !data_root.is_dir() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let states_root = data_root.join("states");
    if states_root.is_dir() {
        candidates.push(resolve_best_effort(&states_root));
    }
    candidates.push(resolve_best_effort(&data_root));
    unique_paths(candidates)
}

// =======================================================================
// FBNeo — fbneo.py:35-154
// =======================================================================

/// `fbneo_directory_settings`'s return shape (fbneo.py:80-129).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FbneoSettings {
    pub config_path: String,
    pub base_path: String,
    pub eeprom_path: String,
    pub memcard_path: String,
    pub hiscore_path: String,
    pub hdd_path: String,
    pub state_path: String,
}

/// `_config_path_candidates` (fbneo.py:35-51): `<emulator_dir>/config/
/// <stem>.ini` (only when a non-blank stem is derivable), then
/// `config/fbneo.ini`, then `config/FinalBurn Neo.ini`. `stem` is the
/// EXPANDED (not dir-or-parent) path's own file stem when it has an
/// extension, else the resolved emulator directory's own name.
fn fbneo_config_path_candidates(path: &str) -> Vec<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let expanded = paths::expand_user(trimmed);
    let emulator_dir = if expanded.is_dir() {
        expanded.clone()
    } else {
        expanded.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let stem = if expanded.extension().is_some() {
        expanded
            .file_stem()
            .map(|s| s.to_string_lossy().trim().to_string())
            .unwrap_or_default()
    } else {
        emulator_dir
            .file_name()
            .map(|s| s.to_string_lossy().trim().to_string())
            .unwrap_or_default()
    };

    let mut candidates = Vec::new();
    if !emulator_dir.as_os_str().is_empty() {
        if !stem.is_empty() {
            candidates.push(resolve_best_effort(
                &emulator_dir.join("config").join(format!("{stem}.ini")),
            ));
        }
        candidates.push(resolve_best_effort(
            &emulator_dir.join("config").join("fbneo.ini"),
        ));
        candidates.push(resolve_best_effort(
            &emulator_dir.join("config").join("FinalBurn Neo.ini"),
        ));
    }
    unique_paths(candidates)
}

/// `_read_fbneo_config`/`_read_mame_ini_settings`-style whitespace-separated
/// `key value` reader, shared shape between FBNeo (`//`/`#`/`;` comments,
/// key case PRESERVED — fbneo.py:54-77) and MAME (`#`/`;` comments, key
/// LOWERCASED — mame.py:146-169). `lowercase_keys` selects which.
fn read_whitespace_kv_config(
    path: &Path,
    comment_prefixes: &[&str],
    lowercase_keys: bool,
) -> HashMap<String, String> {
    let mut result = HashMap::new();
    if !path.is_file() {
        return result;
    }
    let Ok(raw_content) = std::fs::read_to_string(path) else {
        return result;
    };

    for raw_line in raw_content.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty() || comment_prefixes.iter().any(|p| stripped.starts_with(p)) {
            continue;
        }
        let mut parts = stripped.splitn(2, char::is_whitespace);
        let raw_key = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();
        if raw_key.is_empty() {
            continue;
        }
        let key = if lowercase_keys {
            raw_key.to_lowercase()
        } else {
            raw_key.to_string()
        };
        result.insert(key, value.to_string());
    }
    result
}

/// `fbneo_directory_settings` (fbneo.py:80-129). A blank path uses the
/// process's current working directory as the base (fbneo.py:89).
pub fn fbneo_directory_settings(path: &str, _args: Args) -> FbneoSettings {
    let trimmed = path.trim();
    let emulator_dir = if trimmed.is_empty() {
        resolve_best_effort(&std::env::current_dir().unwrap_or_default())
    } else {
        let expanded = paths::expand_user(trimmed);
        let dir = if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        resolve_best_effort(&dir)
    };

    let mut settings = FbneoSettings {
        config_path: resolve_best_effort(&emulator_dir.join("config").join("fbneo.ini"))
            .to_string_lossy()
            .to_string(),
        base_path: emulator_dir.to_string_lossy().to_string(),
        eeprom_path: resolve_best_effort(&emulator_dir.join("config").join("games"))
            .to_string_lossy()
            .to_string(),
        memcard_path: resolve_best_effort(&emulator_dir.join("config").join("memcards"))
            .to_string_lossy()
            .to_string(),
        hiscore_path: resolve_best_effort(&emulator_dir.join("support").join("hiscores"))
            .to_string_lossy()
            .to_string(),
        hdd_path: resolve_best_effort(&emulator_dir.join("support").join("hdd"))
            .to_string_lossy()
            .to_string(),
        state_path: resolve_best_effort(&emulator_dir.join("savestates"))
            .to_string_lossy()
            .to_string(),
    };

    let config_candidates = fbneo_config_path_candidates(path);
    let selected_config = config_candidates
        .iter()
        .find(|c| c.is_file())
        .or_else(|| config_candidates.first());

    if let Some(candidate) = selected_config {
        settings.config_path = resolve_best_effort(candidate).to_string_lossy().to_string();
    }

    let config_values = selected_config
        .map(|c| read_whitespace_kv_config(c, &["//", "#", ";"], false))
        .unwrap_or_default();

    settings.eeprom_path = resolve_setting_or_default(
        &emulator_dir,
        config_values
            .get("szAppEEPROMPath")
            .map(String::as_str)
            .unwrap_or(""),
        "config/games",
    );
    settings.hiscore_path = resolve_setting_or_default(
        &emulator_dir,
        config_values
            .get("szAppHiscorePath")
            .map(String::as_str)
            .unwrap_or(""),
        "support/hiscores",
    );
    settings.hdd_path = resolve_setting_or_default(
        &emulator_dir,
        config_values
            .get("szAppHDDPath")
            .map(String::as_str)
            .unwrap_or(""),
        "support/hdd",
    );

    settings
}

/// `fbneo_save_path_overrides` (fbneo.py:132-151): eeprom, memcard, hiscore,
/// hdd, in that order.
pub fn fbneo_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = fbneo_directory_settings(path, args);
    let mut raw = Vec::new();
    for value in [
        &settings.eeprom_path,
        &settings.memcard_path,
        &settings.hiscore_path,
        &settings.hdd_path,
    ] {
        if !value.trim().is_empty() {
            raw.push(resolve_best_effort(&paths::expand_user(value.trim())));
        }
    }
    unique_paths(raw)
}

/// `fbneo_state_path_overrides` (fbneo.py:154-163): the single state path.
pub fn fbneo_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = fbneo_directory_settings(path, args);
    let value = settings.state_path.trim();
    if value.is_empty() {
        Vec::new()
    } else {
        vec![resolve_best_effort(&paths::expand_user(value))]
    }
}

// =======================================================================
// MAME — mame.py:57-239
// =======================================================================

/// `mame_directory_settings`'s return shape (mame.py:172-214).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MameSettings {
    pub ini_path: String,
    pub base_path: String,
    pub cfg_directory: String,
    pub nvram_directory: String,
    pub memcard_directory: String,
    pub diff_directory: String,
    pub state_directory: String,
}

const MAME_SUPPORTED_OPTIONS: [&str; 6] = [
    "inipath",
    "cfg_directory",
    "nvram_directory",
    "state_directory",
    "diff_directory",
    "memcard_directory",
];

/// `_launch_path_overrides` (mame.py:68-112): every `-`-prefixed token's
/// leading dashes are stripped and internal `-` normalized to `_`
/// (case-insensitively) before matching [`MAME_SUPPORTED_OPTIONS`]. An `=`
/// form reads its value directly; a space form consumes the FOLLOWING token
/// only when it exists and does NOT itself start with `-` (a dash-prefixed
/// following token is treated as its own, separate flag, leaving this
/// option valueless). A later occurrence of the same option overwrites an
/// earlier one (plain last-write-wins, not first).
fn mame_launch_overrides(args: Args) -> HashMap<String, String> {
    let mut overrides = HashMap::new();
    if args.is_empty() {
        return overrides;
    }

    let mut index = 0;
    while index < args.len() {
        let raw_arg = &args[index];
        index += 1;
        let normalized = raw_arg.trim();
        if normalized.is_empty() || !normalized.starts_with('-') {
            continue;
        }

        let stripped = normalized.trim_start_matches('-');
        let (option_text, mut value) = if let Some(eq_pos) = stripped.find('=') {
            (
                stripped[..eq_pos].to_string(),
                clean_ini_value(&stripped[eq_pos + 1..]),
            )
        } else {
            (stripped.to_string(), String::new())
        };

        if value.is_empty() && !stripped.contains('=') && index < args.len() {
            let next_token = args[index].trim();
            if !next_token.is_empty() && !next_token.starts_with('-') {
                let (v, consumed_index) = consume_arg_value_indexed(args, index);
                value = v;
                index = consumed_index + 1;
            }
        }

        let option_name = option_text.replace('-', "_").to_lowercase();
        if MAME_SUPPORTED_OPTIONS.contains(&option_name.as_str()) && !value.is_empty() {
            overrides.insert(option_name, value);
        }
    }
    overrides
}

/// `_ini_path_candidates` (mame.py:115-143): `-inipath`'s semicolon
/// -separated directories when present, else `<base>`, `<base>/ini`,
/// `<base>/ini/presets` — each yielding `<dir>/mame.ini`.
fn mame_ini_path_candidates(base_root: &Path, args: Args) -> Vec<PathBuf> {
    let overrides = mame_launch_overrides(args);
    let raw_inipath = overrides.get("inipath").cloned().unwrap_or_default();

    let mut directories = Vec::new();
    if !raw_inipath.is_empty() {
        for part in raw_inipath.split(';') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            let expanded = paths::expand_user(&expand_vars(trimmed));
            let candidate = if expanded.is_absolute() {
                expanded
            } else {
                base_root.join(expanded)
            };
            directories.push(resolve_best_effort(&candidate));
        }
    } else {
        directories.push(resolve_best_effort(base_root));
        directories.push(resolve_best_effort(&base_root.join("ini")));
        directories.push(resolve_best_effort(&base_root.join("ini").join("presets")));
    }

    unique_paths(
        directories
            .into_iter()
            .map(|d| d.join("mame.ini"))
            .collect(),
    )
}

/// `mame_directory_settings` (mame.py:172-214). A blank path uses the
/// process's current working directory as the base.
pub fn mame_directory_settings(path: &str, args: Args) -> MameSettings {
    let trimmed = path.trim();
    let base_root = if trimmed.is_empty() {
        resolve_best_effort(&std::env::current_dir().unwrap_or_default())
    } else {
        let expanded = paths::expand_user(trimmed);
        let dir = if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        resolve_best_effort(&dir)
    };

    let mut settings = MameSettings {
        ini_path: resolve_best_effort(&base_root.join("mame.ini"))
            .to_string_lossy()
            .to_string(),
        base_path: base_root.to_string_lossy().to_string(),
        cfg_directory: resolve_best_effort(&base_root.join("cfg"))
            .to_string_lossy()
            .to_string(),
        nvram_directory: resolve_best_effort(&base_root.join("nvram"))
            .to_string_lossy()
            .to_string(),
        memcard_directory: resolve_best_effort(&base_root.join("memcard"))
            .to_string_lossy()
            .to_string(),
        diff_directory: resolve_best_effort(&base_root.join("diff"))
            .to_string_lossy()
            .to_string(),
        state_directory: resolve_best_effort(&base_root.join("sta"))
            .to_string_lossy()
            .to_string(),
    };

    let ini_candidates = mame_ini_path_candidates(&base_root, args);
    let selected_ini = ini_candidates
        .iter()
        .find(|c| c.is_file())
        .or_else(|| ini_candidates.first());
    let ini_settings = selected_ini
        .map(|c| read_whitespace_kv_config(c, &["#", ";"], true))
        .unwrap_or_default();
    let launch_overrides = mame_launch_overrides(args);

    if let Some(candidate) = selected_ini {
        settings.ini_path = resolve_best_effort(candidate).to_string_lossy().to_string();
    }

    for (option_name, default_name) in [
        ("cfg_directory", "cfg"),
        ("nvram_directory", "nvram"),
        ("memcard_directory", "memcard"),
        ("diff_directory", "diff"),
        ("state_directory", "sta"),
    ] {
        let raw_value = launch_overrides
            .get(option_name)
            .cloned()
            .unwrap_or_else(|| ini_settings.get(option_name).cloned().unwrap_or_default());
        let resolved = resolve_setting_or_default(&base_root, &raw_value, default_name);
        match option_name {
            "cfg_directory" => settings.cfg_directory = resolved,
            "nvram_directory" => settings.nvram_directory = resolved,
            "memcard_directory" => settings.memcard_directory = resolved,
            "diff_directory" => settings.diff_directory = resolved,
            "state_directory" => settings.state_directory = resolved,
            _ => unreachable!(),
        }
    }

    settings
}

/// `mame_save_path_overrides` (mame.py:217-236): `nvram_directory`,
/// `memcard_directory`, `diff_directory`, in that order.
pub fn mame_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = mame_directory_settings(path, args);
    let mut raw = Vec::new();
    for value in [
        &settings.nvram_directory,
        &settings.memcard_directory,
        &settings.diff_directory,
    ] {
        if !value.trim().is_empty() {
            raw.push(resolve_best_effort(&paths::expand_user(value.trim())));
        }
    }
    unique_paths(raw)
}

/// `mame_state_path_overrides` (mame.py:239-248): the single
/// `state_directory`.
pub fn mame_state_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = mame_directory_settings(path, args);
    let value = settings.state_directory.trim();
    if value.is_empty() {
        Vec::new()
    } else {
        vec![resolve_best_effort(&paths::expand_user(value))]
    }
}

// =======================================================================
// Pico-8 — pico8.py:61-228
// =======================================================================

/// `pico8_directory_settings`'s return shape (pico8.py:174-216).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pico8Settings {
    pub config_path: String,
    pub user_root: String,
    pub carts_root: String,
    pub cdata_root: String,
    pub cstore_root: String,
    pub backup_root: String,
    pub desktop_path: String,
}

/// `_launch_home_root` (pico8.py:61-91): `-home`/`--home <value>` or
/// `-home=`/`--home=<value>`, first non-blank value wins.
fn pico8_launch_home_root(args: Args) -> Option<PathBuf> {
    let mut index = 0;
    while index < args.len() {
        let raw_arg = &args[index];
        index += 1;
        let normalized = raw_arg.trim();
        if normalized.is_empty() {
            continue;
        }
        let lowered = normalized.to_lowercase();

        if (lowered == "-home" || lowered == "--home") && index < args.len() {
            let (value, consumed_index) = consume_arg_value_indexed(args, index);
            index = consumed_index + 1;
            if !value.is_empty() {
                return Some(resolve_best_effort(&paths::expand_user(&expand_vars(
                    &value,
                ))));
            }
            continue;
        }

        for prefix in ["-home=", "--home="] {
            if lowered.starts_with(prefix) {
                if let Some(eq_pos) = normalized.find('=') {
                    let value = clean_ini_value(&normalized[eq_pos + 1..]);
                    if !value.is_empty() {
                        return Some(resolve_best_effort(&paths::expand_user(&expand_vars(
                            &value,
                        ))));
                    }
                }
            }
        }
    }
    None
}

/// `pico8_user_root_candidates` (pico8.py:128-161): a launch `-home`
/// argument first; then `<emulator_dir>`, `<emulator_dir>/pico-8`,
/// `<emulator_dir>/userdata` — each kept only when it already contains
/// `config.txt`, `cdata` or `cstore`; then the platform default(s).
fn pico8_user_root_candidates(path: &str, args: Args) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home_root) = pico8_launch_home_root(args) {
        candidates.push(home_root);
    }

    let trimmed = path.trim();
    if !trimmed.is_empty() {
        let expanded = paths::expand_user(trimmed);
        let emulator_dir = if expanded.is_dir() {
            expanded
        } else {
            expanded.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if !emulator_dir.as_os_str().is_empty() {
            for local_candidate in [
                emulator_dir.clone(),
                emulator_dir.join("pico-8"),
                emulator_dir.join("userdata"),
            ] {
                let has_marker = local_candidate.join("config.txt").exists()
                    || local_candidate.join("cdata").exists()
                    || local_candidate.join("cstore").exists();
                if has_marker {
                    candidates.push(resolve_best_effort(&local_candidate));
                }
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(appdata) = env_trimmed("APPDATA") {
            candidates.push(resolve_best_effort(
                &paths::expand_user(&appdata).join("pico-8"),
            ));
        }
    }
    #[cfg(not(windows))]
    {
        let home = paths::home_dir().unwrap_or_default();
        if cfg!(target_os = "macos") {
            candidates.push(resolve_best_effort(
                &home
                    .join("Library")
                    .join("Application Support")
                    .join("pico-8"),
            ));
        } else {
            candidates.push(resolve_best_effort(
                &home.join(".lexaloffle").join("pico-8"),
            ));
            candidates.push(resolve_best_effort(&paths::xdg_data_home().join("pico-8")));
        }
    }

    let resolved: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|c| !c.as_os_str().is_empty())
        .map(|c| resolve_best_effort(&paths::expand_user(&c.to_string_lossy())))
        .collect();
    unique_paths(resolved)
}

/// `_parse_config_values` (pico8.py:109-125): whitespace-separated
/// `key value` with `#`, `;`, `--` comments.
fn pico8_parse_config_values(raw_content: &str) -> HashMap<String, String> {
    static PICO8_KV_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^([A-Za-z0-9_]+)\s+(.+?)\s*$").unwrap());
    let mut values = HashMap::new();
    for raw_line in raw_content.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty()
            || stripped.starts_with('#')
            || stripped.starts_with(';')
            || stripped.starts_with("--")
        {
            continue;
        }
        if let Some(caps) = PICO8_KV_RE.captures(stripped) {
            let key = caps[1].trim().to_lowercase();
            let value = caps[2].trim().to_string();
            values.insert(key, value);
        }
    }
    values
}

/// `pico8_directory_settings` (pico8.py:174-216). **Trap**: unlike every
/// other reader's zip-loop in this file, EVERY field is overwritten on
/// EVERY iteration before the match check — so when no `(root, candidate)`
/// pair ever matches, the returned defaults reflect the LAST candidate
/// examined, not the first (pico8.py:192-214's `defaults[...] = ...` lines
/// run unconditionally at the top of the loop body, before either `return`
/// site).
pub fn pico8_directory_settings(path: &str, args: Args) -> Pico8Settings {
    let user_roots = pico8_user_root_candidates(path, args);
    let settings_candidates = unique_paths(
        user_roots
            .iter()
            .map(|root| root.join("config.txt"))
            .collect(),
    );

    let mut defaults = Pico8Settings::default();

    for (root, candidate) in user_roots.iter().zip(settings_candidates.iter()) {
        defaults.user_root = resolve_best_effort(root).to_string_lossy().to_string();
        defaults.config_path = resolve_best_effort(candidate).to_string_lossy().to_string();
        defaults.carts_root = resolve_best_effort(&root.join("carts"))
            .to_string_lossy()
            .to_string();
        defaults.desktop_path = resolve_best_effort(&root.join("desktop"))
            .to_string_lossy()
            .to_string();
        defaults.cdata_root = resolve_best_effort(&root.join("cdata"))
            .to_string_lossy()
            .to_string();
        defaults.cstore_root = resolve_best_effort(&root.join("cstore"))
            .to_string_lossy()
            .to_string();
        defaults.backup_root = resolve_best_effort(&root.join("backup"))
            .to_string_lossy()
            .to_string();

        if candidate.is_file() {
            if let Ok(raw_content) = std::fs::read_to_string(candidate) {
                if !raw_content.is_empty() {
                    let config_values = pico8_parse_config_values(&raw_content);
                    defaults.carts_root = resolve_setting_or_default(
                        root,
                        config_values
                            .get("root_path")
                            .map(String::as_str)
                            .unwrap_or(""),
                        "carts",
                    );
                    defaults.desktop_path = resolve_setting_or_default(
                        root,
                        config_values
                            .get("desktop")
                            .map(String::as_str)
                            .unwrap_or(""),
                        "desktop",
                    );
                }
            }
            return defaults;
        }

        if root.is_dir() {
            return defaults;
        }
    }

    defaults
}

/// `pico8_save_path_overrides` (pico8.py:219-238): `cdata_root` then
/// `cstore_root`. There is no state-path override.
pub fn pico8_save_path_overrides(path: &str, args: Args) -> Vec<PathBuf> {
    let settings = pico8_directory_settings(path, args);
    let mut raw = Vec::new();
    for value in [&settings.cdata_root, &settings.cstore_root] {
        if !value.trim().is_empty() {
            raw.push(PathBuf::from(value.trim()));
        }
    }
    unique_paths(raw)
}

// =======================================================================
// Vita3K — vita3k.py:9-95
// =======================================================================

static VITA3K_USER_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{2}$").unwrap());

enum Vita3kHost {
    Linux,
    Windows,
    Macos,
    Other,
}

fn vita3k_host() -> Vita3kHost {
    if cfg!(target_os = "linux") {
        Vita3kHost::Linux
    } else if cfg!(target_os = "windows") {
        Vita3kHost::Windows
    } else if cfg!(target_os = "macos") {
        Vita3kHost::Macos
    } else {
        Vita3kHost::Other
    }
}

/// `vita3k_pref_path` (vita3k.py:9-55). Strict priority:
/// `<emulator_dir>/portable/` when it is a directory; then the `pref-path:`
/// scalar in `<emulator_dir>/config.yml` (ONE matching layer of quotes
/// stripped, `~` expanded, the file read lossily via `errors="replace"`
/// semantics — [`String::from_utf8_lossy`]); then the platform default.
/// **None of these three results are filesystem-canonicalized** — vita3k.py
/// never calls `.resolve()` anywhere in this function.
pub fn vita3k_pref_path(path: &str) -> Option<PathBuf> {
    vita3k_pref_path_for_host(path, vita3k_host())
}

fn vita3k_pref_path_for_host(path: &str, host: Vita3kHost) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let emulator_dir = paths::expand_user(trimmed)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();

    let portable_dir = emulator_dir.join("portable");
    if portable_dir.is_dir() {
        return Some(portable_dir);
    }

    let config_path = emulator_dir.join("config.yml");
    if config_path.is_file() {
        if let Ok(bytes) = std::fs::read(&config_path) {
            let content = String::from_utf8_lossy(&bytes);
            for line in content.lines() {
                let stripped = line.trim();
                let Some(rest) = stripped.strip_prefix("pref-path:") else {
                    continue;
                };
                let mut raw_value = rest.trim().to_string();
                let chars: Vec<char> = raw_value.chars().collect();
                if chars.len() >= 2 {
                    let first = chars[0];
                    let last = chars[chars.len() - 1];
                    if (first == '"' || first == '\'') && first == last {
                        raw_value = chars[1..chars.len() - 1].iter().collect();
                    }
                }
                if !raw_value.is_empty() {
                    return Some(paths::expand_user(&raw_value));
                }
            }
        }
    }

    match host {
        Vita3kHost::Linux => Some(
            paths::home_dir()
                .unwrap_or_default()
                .join(".local")
                .join("share")
                .join("Vita3K")
                .join("Vita3K"),
        ),
        Vita3kHost::Windows => Some(
            paths::home_dir()
                .unwrap_or_default()
                .join("AppData")
                .join("Roaming")
                .join("Vita3K")
                .join("Vita3K"),
        ),
        Vita3kHost::Macos => Some(
            paths::home_dir()
                .unwrap_or_default()
                .join("Library")
                .join("Application Support")
                .join("Vita3K")
                .join("Vita3K"),
        ),
        Vita3kHost::Other => None,
    }
}

/// `vita3k_save_path_overrides` (vita3k.py:61-99). `args` is accepted only
/// for signature uniformity and unused, matching the Python reference
/// exactly (vita3k.py:63-64's own comment).
pub fn vita3k_save_path_overrides(path: &str, _args: Args) -> Vec<PathBuf> {
    let Some(pref_path) = vita3k_pref_path(path) else {
        return Vec::new();
    };

    let user_root = pref_path.join("ux0").join("user");
    let mut found_ids: Vec<String> = Vec::new();
    if user_root.is_dir() {
        for child in sorted_dir_entries(&user_root) {
            if !child.is_dir() {
                continue;
            }
            let name = child
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if VITA3K_USER_ID_RE.is_match(&name) {
                found_ids.push(name);
            }
        }
    }
    if !found_ids.iter().any(|id| id == "00") {
        found_ids.insert(0, "00".to_string());
    }

    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for user_id in found_ids {
        let candidate = pref_path
            .join("ux0")
            .join("user")
            .join(&user_id)
            .join("savedata");
        if seen.insert(candidate.to_string_lossy().to_string()) {
            result.push(candidate);
        }
    }
    result
}

// =======================================================================
// Flycast VMU — retroarch.py:625-651
// =======================================================================

/// `^vmu([0-3]).*\.bin$`, case-insensitive — applied to a filename ALREADY
/// confirmed to end in a platform-appropriate-case `.bin` by
/// [`flycast_bin_extension_matches`] (retroarch.py:635).
static FLYCAST_VMU_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^vmu([0-3]).*\.bin$").unwrap());

/// The case-sensitivity half of Python's `directory.glob("*.bin")`
/// (case-SENSITIVE on POSIX, case-insensitive on Windows) — checked
/// SEPARATELY from [`FLYCAST_VMU_RE`]'s own case-insensitive match so a
/// `VMU0.BIN` stays invisible on Linux even though the name regex alone
/// would accept it: parity with the Python reference's two-stage filter,
/// not a bug to "fix" (doc 05's Flycast VMU rule).
fn flycast_bin_extension_matches(name: &str) -> bool {
    if cfg!(windows) {
        name.to_lowercase().ends_with(".bin")
    } else {
        name.ends_with(".bin")
    }
}

/// `flycast_vmu_file_candidates` (retroarch.py:625-651): the newest
/// `vmu[0-3]*.bin` file per slot across every directory (non-recursive),
/// newest determined by mtime with a STRICT `>` (an exact tie keeps
/// whichever file was already recorded), returned ordered slot 0->3 with
/// absent slots omitted.
pub fn flycast_vmu_file_candidates(directories: &[PathBuf]) -> Vec<PathBuf> {
    let mut latest_by_slot: HashMap<char, (std::time::SystemTime, PathBuf)> = HashMap::new();

    for directory in directories {
        if !directory.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path();
            if !candidate.is_file() {
                continue;
            }
            let Some(name) = candidate.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !flycast_bin_extension_matches(name) {
                continue;
            }
            let Some(caps) = FLYCAST_VMU_RE.captures(name) else {
                continue;
            };
            let slot = caps[1].chars().next().unwrap();
            let Ok(metadata) = candidate.metadata() else {
                continue;
            };
            let Ok(mtime) = metadata.modified() else {
                continue;
            };

            let replace = match latest_by_slot.get(&slot) {
                Some((existing_mtime, _)) => mtime > *existing_mtime,
                None => true,
            };
            if replace {
                latest_by_slot.insert(slot, (mtime, candidate));
            }
        }
    }

    ['0', '1', '2', '3']
        .iter()
        .filter_map(|slot| latest_by_slot.get(slot).map(|(_, p)| p.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env::EnvGuard;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// Every env var this module's readers consult, pointed at `dir` (or
    /// unset for the ones with no sensible test default).
    fn isolated_env(dir: &Path) -> EnvGuard {
        let dir_str = dir.to_str().unwrap();
        EnvGuard::set(&[
            ("HOME", Some(dir_str)),
            ("XDG_CONFIG_HOME", None),
            ("XDG_DATA_HOME", None),
            ("USERPROFILE", None),
            ("OneDrive", None),
            ("APPDATA", None),
            ("LOCALAPPDATA", None),
            ("RPCS3_CONFIG_DIR", None),
        ])
    }

    // ---------------------------------------------------------------
    // Shared helpers
    // ---------------------------------------------------------------

    #[test]
    fn consume_arg_value_rejoins_split_quoted_fragments() {
        let split = args(&["\"Program", "Files\"", "next"]);
        assert_eq!(
            consume_arg_value(&split, 0),
            Some("Program Files".to_string())
        );

        // A single already-whole quoted token needs no rejoining.
        let whole = args(&["\"onetoken\""]);
        assert_eq!(consume_arg_value(&whole, 0), Some("onetoken".to_string()));

        // Out of bounds and blank tokens both report no value.
        assert_eq!(consume_arg_value(&whole, 5), None);
        let blank = args(&["   "]);
        assert_eq!(consume_arg_value(&blank, 0), None);
    }

    #[test]
    fn unique_paths_dedupes_case_insensitively_keeping_the_first() {
        let deduped = unique_paths(vec![
            PathBuf::from("/Games/PCSX2"),
            PathBuf::from("/games/pcsx2"),
            PathBuf::from("/games/Dolphin"),
            PathBuf::from(""),
        ]);
        assert_eq!(
            deduped,
            vec![
                PathBuf::from("/Games/PCSX2"),
                PathBuf::from("/games/Dolphin")
            ],
            "case-insensitive dedup keeps the first spelling and drops the blank entry"
        );
    }

    // ---------------------------------------------------------------
    // PCSX2
    // ---------------------------------------------------------------

    fn pcsx2_emulator(temp: &Path) -> (String, PathBuf) {
        let dir = temp.join("PCSX2");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("pcsx2-qt.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe.to_string_lossy().to_string(), dir)
    }

    #[test]
    fn pcsx2_portable_detected_from_portable_ini() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = pcsx2_emulator(temp.path());
        std::fs::write(dir.join("portable.ini"), "").unwrap();

        let roots = pcsx2_data_root_candidates(&exe, &[]);

        assert_eq!(roots.first(), Some(&resolve_best_effort(&dir)));
    }

    #[test]
    fn pcsx2_portable_detected_from_a_portable_launch_flag() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = pcsx2_emulator(temp.path());
        let launch_args = args(&["-portable", "%rom%"]);

        let roots = pcsx2_data_root_candidates(&exe, &launch_args);

        assert_eq!(roots.first(), Some(&resolve_best_effort(&dir)));
    }

    #[test]
    fn pcsx2_portable_detected_suffix_from_portable_txt_text() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = pcsx2_emulator(temp.path());
        std::fs::write(dir.join("portable.txt"), "PortableData").unwrap();
        std::fs::create_dir_all(dir.join("PortableData")).unwrap();

        let roots = pcsx2_data_root_candidates(&exe, &[]);

        assert_eq!(
            roots.first(),
            Some(&resolve_best_effort(&dir.join("PortableData")))
        );
    }

    #[test]
    fn pcsx2_candidate_order_is_portable_then_user_then_emulator_dir() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = pcsx2_emulator(temp.path());
        std::fs::write(dir.join("portable.txt"), "Custom").unwrap();
        std::fs::create_dir_all(dir.join("Custom")).unwrap();

        let roots = pcsx2_data_root_candidates(&exe, &[]);

        let portable_root = resolve_best_effort(&dir.join("Custom"));
        let plain_dir = resolve_best_effort(&dir);
        let user_root = temp.path().join("Documents").join("PCSX2");

        assert_eq!(roots[0], portable_root, "portable root must be first");
        assert!(
            roots.iter().any(|r| r == &user_root),
            "a user root must be present: {roots:?}"
        );
        assert_eq!(
            roots.last(),
            Some(&plain_dir),
            "the plain emulator directory must be last"
        );
        let portable_index = roots.iter().position(|r| r == &portable_root).unwrap();
        let user_index = roots.iter().position(|r| r == &user_root).unwrap();
        let plain_index = roots.iter().position(|r| r == &plain_dir).unwrap();
        assert!(portable_index < user_index && user_index < plain_index);
    }

    fn pcsx2_ini_at(dir: &Path, body: &str) -> PathBuf {
        let inis = dir.join("inis");
        std::fs::create_dir_all(&inis).unwrap();
        std::fs::write(dir.join("portable.ini"), "").unwrap();
        let ini_path = inis.join("PCSX2.ini");
        std::fs::write(&ini_path, body).unwrap();
        ini_path
    }

    #[test]
    fn pcsx2_save_overrides_list_slot_files_before_the_directory() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = pcsx2_emulator(temp.path());
        pcsx2_ini_at(&dir, "[Folders]\nMemoryCards = memcards\n");

        let overrides = pcsx2_save_path_overrides(&exe, &[]);
        let memcards_dir = resolve_best_effort(&dir.join("memcards"));

        assert_eq!(
            overrides,
            vec![
                memcards_dir.join("Mcd001.ps2"),
                memcards_dir.join("Mcd002.ps2"),
                memcards_dir,
            ]
        );
    }

    #[test]
    fn pcsx2_slot_filenames_come_from_the_ini_when_set() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = pcsx2_emulator(temp.path());
        pcsx2_ini_at(
            &dir,
            "[Folders]\nMemoryCards = memcards\n[MemoryCards]\nSlot1_Filename = custom-1.ps2\nSlot2_Filename = custom-2.ps2\n",
        );

        let overrides = pcsx2_save_path_overrides(&exe, &[]);
        let memcards_dir = resolve_best_effort(&dir.join("memcards"));

        assert_eq!(
            overrides,
            vec![
                memcards_dir.join("custom-1.ps2"),
                memcards_dir.join("custom-2.ps2"),
                memcards_dir,
            ]
        );
    }

    #[test]
    fn pcsx2_relative_ini_values_resolve_against_the_data_root() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = pcsx2_emulator(temp.path());
        pcsx2_ini_at(
            &dir,
            "[Folders]\nMemoryCards = my-cards\nSavestates = my-states\n",
        );

        let settings = pcsx2_directory_settings(&exe, &[]);

        assert_eq!(
            settings.memory_cards,
            resolve_best_effort(&dir.join("my-cards")).to_string_lossy()
        );
        assert_eq!(
            settings.savestates,
            resolve_best_effort(&dir.join("my-states")).to_string_lossy()
        );
    }

    #[test]
    fn pcsx2_state_overrides_are_the_single_savestates_directory() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = pcsx2_emulator(temp.path());
        pcsx2_ini_at(&dir, "[Folders]\nSavestates = sstates\n");

        let overrides = pcsx2_state_path_overrides(&exe, &[]);

        assert_eq!(overrides, vec![resolve_best_effort(&dir.join("sstates"))]);
    }

    #[test]
    fn pcsx2_directory_settings_reads_user_documents_config() {
        // Ported oracle: tests/test_emulator_profiles.py:180-204.
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let config_dir = temp.path().join("Documents").join("PCSX2").join("inis");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("PCSX2.ini"),
            "[Folders]\nMemoryCards = custom-memcards\nSavestates = custom-sstates\n",
        )
        .unwrap();

        let settings = pcsx2_directory_settings(r"C:\Emulators\PCSX2\pcsx2-qt.exe", &[]);

        assert_eq!(
            settings.memory_cards,
            resolve_best_effort(
                &temp
                    .path()
                    .join("Documents")
                    .join("PCSX2")
                    .join("custom-memcards")
            )
            .to_string_lossy()
        );
        assert_eq!(
            settings.savestates,
            resolve_best_effort(
                &temp
                    .path()
                    .join("Documents")
                    .join("PCSX2")
                    .join("custom-sstates")
            )
            .to_string_lossy()
        );
    }

    // ---------------------------------------------------------------
    // DuckStation
    // ---------------------------------------------------------------

    #[test]
    fn duckstation_settings_stop_at_the_first_candidate_with_a_memorycards_key() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let dir = temp.path().join("DuckStation");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("duckstation.exe");
        std::fs::write(&exe, b"").unwrap();
        std::fs::write(
            dir.join("settings.ini"),
            "[MemoryCards]\nDirectory = D:/first\nCard1Type = PerGame\n",
        )
        .unwrap();

        let second = temp.path().join("Documents").join("DuckStation");
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(
            second.join("settings.ini"),
            "[MemoryCards]\nDirectory = D:/second\nCard1Type = Shared\n",
        )
        .unwrap();

        let settings = duckstation_memory_card_settings(exe.to_str().unwrap());

        assert_eq!(settings.directory, "D:/first");
        assert_eq!(settings.card1_type, "PerGame");
        assert_eq!(
            settings.config_path,
            dir.join("settings.ini").to_string_lossy()
        );
    }

    #[test]
    fn duckstation_settings_defaults_when_nothing_parses() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let dir = temp.path().join("DuckStation");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("duckstation.exe");
        std::fs::write(&exe, b"").unwrap();
        // No settings.ini anywhere in the candidate list.

        let settings = duckstation_memory_card_settings(exe.to_str().unwrap());

        assert_eq!(settings.config_path, "");
        assert_eq!(settings.directory, "memcards");
        assert_eq!(settings.card1_type, "PerGameTitle");
        assert_eq!(settings.card2_type, "None");
        assert!(settings.use_playlist_title);
    }

    // ---------------------------------------------------------------
    // Dolphin
    // ---------------------------------------------------------------

    fn dolphin_emulator(temp: &Path) -> (String, PathBuf) {
        let dir = temp.join("Dolphin");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("dolphin.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe.to_string_lossy().to_string(), dir)
    }

    #[test]
    fn dolphin_user_root_prefers_a_launch_user_flag() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = dolphin_emulator(temp.path());
        let custom_user = temp.path().join("custom-user");
        std::fs::create_dir_all(&custom_user).unwrap();
        let expected = resolve_best_effort(&custom_user);

        let short = args(&["-u", custom_user.to_str().unwrap(), "-b"]);
        assert_eq!(
            dolphin_user_root_candidates(&exe, &short).first(),
            Some(&expected)
        );

        let long = args(&["--user", custom_user.to_str().unwrap(), "-b"]);
        assert_eq!(
            dolphin_user_root_candidates(&exe, &long).first(),
            Some(&expected)
        );

        let equals = args(&[&format!("--user={}", custom_user.to_str().unwrap()), "-b"]);
        assert_eq!(
            dolphin_user_root_candidates(&exe, &equals).first(),
            Some(&expected)
        );
    }

    #[test]
    fn dolphin_user_root_uses_exe_dir_user_when_portable_txt_exists() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = dolphin_emulator(temp.path());
        std::fs::write(dir.join("portable.txt"), "").unwrap();

        let roots = dolphin_user_root_candidates(&exe, &[]);

        assert_eq!(roots.first(), Some(&resolve_best_effort(&dir.join("User"))));
    }

    fn dolphin_ini_at(dir: &Path, body: &str) -> PathBuf {
        let config_dir = dir.join("User").join("Config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(dir.join("portable.txt"), "").unwrap();
        let ini_path = config_dir.join("Dolphin.ini");
        std::fs::write(&ini_path, body).unwrap();
        ini_path
    }

    #[test]
    fn dolphin_save_overrides_emit_all_thirty_five_memcard_permutations() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = dolphin_emulator(temp.path());
        dolphin_ini_at(&dir, "[Core]\n");

        let overrides = dolphin_save_path_overrides(&exe, &[]);
        let a_permutations: Vec<&PathBuf> = overrides
            .iter()
            .filter(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().starts_with("MemoryCardA."))
                    .unwrap_or(false)
            })
            .collect();

        assert_eq!(a_permutations.len(), 35, "{overrides:?}");
        let gc_root = resolve_best_effort(&dir.join("User").join("GC"));
        assert!(overrides.contains(&gc_root.join("MemoryCardA.USA.raw")));
        assert!(overrides.contains(&gc_root.join("MemoryCardA.EUR.2043.raw")));
    }

    #[test]
    fn dolphin_gci_region_directory_uses_the_parent_when_already_a_region() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = dolphin_emulator(temp.path());
        let gc_root = dir.join("User").join("GC");
        std::fs::create_dir_all(gc_root.join("USA")).unwrap();
        dolphin_ini_at(&dir, "[Core]\nGCIFolderAPath = GC/USA\n");

        let overrides = dolphin_save_path_overrides(&exe, &[]);
        let resolved_gc_root = resolve_best_effort(&gc_root);

        assert!(
            overrides.contains(&resolved_gc_root.join("JPN")),
            "a sibling region of the parent must be present: {overrides:?}"
        );
    }

    #[test]
    fn dolphin_save_overrides_end_with_the_six_wii_title_groups() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = dolphin_emulator(temp.path());
        dolphin_ini_at(&dir, "[Core]\n");

        let overrides = dolphin_save_path_overrides(&exe, &[]);
        let wii_title = resolve_best_effort(&dir.join("User").join("Wii").join("title"));

        let mut expected_tail = vec![wii_title.clone()];
        for group in DOLPHIN_WII_TITLE_GROUPS {
            expected_tail.push(wii_title.join(group));
        }

        let tail = &overrides[overrides.len() - expected_tail.len()..];
        assert_eq!(tail, expected_tail.as_slice(), "{overrides:?}");
    }

    #[test]
    fn dolphin_wii_root_defaults_to_user_root_wii() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = dolphin_emulator(temp.path());
        dolphin_ini_at(&dir, "[Core]\n");

        let settings = dolphin_directory_settings(&exe, &[]);

        assert_eq!(
            settings.wii_root,
            resolve_best_effort(&dir.join("User").join("Wii")).to_string_lossy()
        );
    }

    #[test]
    fn dolphin_directory_settings_use_cli_user_and_configured_paths() {
        // Ported oracle: tests/test_emulator_profiles.py:303-328.
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = dolphin_emulator(temp.path());
        let user_dir = temp.path().join("custom-user");
        let config_dir = user_dir.join("Config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("Dolphin.ini"),
            "[Core]\nMemcardAPath = GC/MemoryCardA.custom.raw\nGCIFolderAPath = GCIBase\n[General]\nNANDRootPath = AltWii\n",
        )
        .unwrap();

        let launch_args = args(&["-u", user_dir.to_str().unwrap(), "-b", "-e", "%rom%"]);
        let settings = dolphin_directory_settings(&exe, &launch_args);

        assert_eq!(
            settings.user_root,
            resolve_best_effort(&user_dir).to_string_lossy()
        );
        assert_eq!(
            settings.wii_root,
            resolve_best_effort(&user_dir.join("AltWii")).to_string_lossy()
        );
        assert_eq!(
            settings.memcard_a_path,
            resolve_best_effort(&user_dir.join("GC").join("MemoryCardA.custom.raw"))
                .to_string_lossy()
        );
        assert_eq!(
            settings.gci_folder_a_path,
            resolve_best_effort(&user_dir.join("GCIBase")).to_string_lossy()
        );
    }

    // ---------------------------------------------------------------
    // RPCS3
    // ---------------------------------------------------------------

    fn rpcs3_emulator(temp: &Path) -> (String, PathBuf) {
        let dir = temp.join("rpcs3");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("rpcs3.exe");
        std::fs::write(&exe, b"").unwrap();
        (exe.to_string_lossy().to_string(), dir)
    }

    #[test]
    fn rpcs3_config_dir_env_is_inserted_at_index_one() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join("rpcs3-config-dir");
        std::fs::create_dir_all(&config_dir).unwrap();
        let _guard = EnvGuard::set(&[
            ("HOME", Some(temp.path().to_str().unwrap())),
            ("XDG_CONFIG_HOME", None),
            ("RPCS3_CONFIG_DIR", Some(config_dir.to_str().unwrap())),
        ]);
        let (exe, _dir) = rpcs3_emulator(temp.path());

        let candidates = rpcs3_data_root_candidates(&exe, &[]);

        assert_eq!(candidates.get(1), Some(&resolve_best_effort(&config_dir)));
    }

    #[test]
    fn rpcs3_candidates_start_with_exe_dir_when_portable_is_absent() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = rpcs3_emulator(temp.path());

        let candidates = rpcs3_data_root_candidates(&exe, &[]);

        assert_eq!(candidates.first(), Some(&resolve_best_effort(&dir)));
    }

    #[test]
    fn rpcs3_vfs_expands_emulator_dir_token_with_a_trailing_slash() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = rpcs3_emulator(temp.path());
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        // No "/dev_hdd0/" key at all: the default "$(EmulatorDir)dev_hdd0/"
        // must expand $(EmulatorDir) to the data root with a trailing slash.
        std::fs::write(config_dir.join("vfs.yml"), "\"$(EmulatorDir)\": \"\"\n").unwrap();

        let settings = rpcs3_directory_settings(&exe, &[]);

        assert_eq!(
            settings.dev_hdd0,
            resolve_best_effort(&dir.join("dev_hdd0")).to_string_lossy()
        );
    }

    /// A `..`-relative `/dev_hdd0/` value pointing at a directory that does
    /// NOT exist anywhere on disk (unlike the pinned oracle test above,
    /// which pre-creates it). A `..`-blind resolver would leave the literal
    /// `..` in `settings.dev_hdd0`; `resolve_against`/`resolve_rpcs3_path`
    /// must collapse it lexically before any existence check, matching
    /// Python's `Path.resolve(strict=False)`.
    #[test]
    fn rpcs3_vfs_dev_hdd0_collapses_parent_dir_through_a_nonexistent_directory() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = rpcs3_emulator(temp.path());
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("vfs.yml"),
            "\"$(EmulatorDir)\": \"\"\n\"/dev_hdd0/\": \"../elsewhere/dev_hdd0/\"\n",
        )
        .unwrap();

        let settings = rpcs3_directory_settings(&exe, &[]);

        let expected = resolve_best_effort(&temp.path().join("elsewhere").join("dev_hdd0"));
        assert_eq!(settings.dev_hdd0, expected.to_string_lossy());
        assert!(!settings.dev_hdd0.contains(".."), "{}", settings.dev_hdd0);
    }

    #[test]
    fn rpcs3_vfs_treats_empty_yaml_scalars_as_unset() {
        for raw in ["", "{}", "[]", "|", ">"] {
            let content = format!("\"key\": {raw}\n");
            assert_eq!(
                rpcs3_yaml_scalar_value(&content, "key"),
                "",
                "raw scalar {raw:?} must be treated as unset"
            );
        }
    }

    #[test]
    fn rpcs3_vfs_strips_an_unquoted_trailing_comment() {
        let content = "key: /some/path  # a comment\n";
        assert_eq!(rpcs3_yaml_scalar_value(content, "key"), "/some/path");

        // A quoted value keeps a literal "#" — only unquoted values get the
        // comment-stripping treatment (rpcs3.py:77).
        let quoted = "key: \"/some/#literal\"\n";
        assert_eq!(rpcs3_yaml_scalar_value(quoted, "key"), "/some/#literal");
    }

    #[test]
    fn rpcs3_current_user_from_launch_args_then_persistent_settings_then_default() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = rpcs3_emulator(temp.path());
        let gui_dir = dir.join("GuiConfigs");
        std::fs::create_dir_all(&gui_dir).unwrap();
        std::fs::write(
            gui_dir.join("persistent_settings.dat"),
            "[Users]\nactive_user=00000005\n",
        )
        .unwrap();

        // Launch arg wins over the persisted user.
        let with_launch_arg = args(&["--user-id", "00000009"]);
        assert_eq!(
            rpcs3_directory_settings(&exe, &with_launch_arg).current_user,
            "00000009"
        );

        // No launch arg: falls back to the persisted user.
        assert_eq!(rpcs3_directory_settings(&exe, &[]).current_user, "00000005");

        // Neither: the hardcoded default.
        let (bare_exe, _bare_dir) = rpcs3_emulator(&temp.path().join("bare"));
        assert_eq!(
            rpcs3_directory_settings(&bare_exe, &[]).current_user,
            "00000001"
        );
    }

    #[test]
    fn rpcs3_user_id_must_be_eight_digits_and_not_all_zero() {
        assert!(rpcs3_is_valid_user_id("00000001"));
        assert!(!rpcs3_is_valid_user_id("00000000"));
        assert!(!rpcs3_is_valid_user_id("1234567"));
        assert!(!rpcs3_is_valid_user_id("123456789"));
        assert!(!rpcs3_is_valid_user_id("abcdefgh"));
    }

    #[test]
    fn rpcs3_save_overrides_put_the_current_user_first_and_00000001_last() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = rpcs3_emulator(temp.path());
        let dev_hdd0 = dir.join("dev_hdd0");
        std::fs::create_dir_all(dev_hdd0.join("home").join("00000003")).unwrap();
        std::fs::create_dir_all(dev_hdd0.join("home").join("00000002")).unwrap();
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("vfs.yml"),
            "\"$(EmulatorDir)\": \"\"\n\"/dev_hdd0/\": \"dev_hdd0/\"\n",
        )
        .unwrap();

        let launch_args = args(&["--user-id", "00000003"]);
        let overrides = rpcs3_save_path_overrides(&exe, &launch_args);
        let resolved_dev_hdd0 = resolve_best_effort(&dev_hdd0);

        assert_eq!(
            overrides.first(),
            Some(
                &resolved_dev_hdd0
                    .join("home")
                    .join("00000003")
                    .join("savedata")
            )
        );
        assert_eq!(
            overrides.last(),
            Some(
                &resolved_dev_hdd0
                    .join("home")
                    .join("00000001")
                    .join("savedata")
            ),
            "00000001 must be a guaranteed tail entry even though it does not exist"
        );
        assert!(overrides.contains(
            &resolved_dev_hdd0
                .join("home")
                .join("00000002")
                .join("savedata")
        ));
    }

    #[test]
    fn rpcs3_directory_settings_reads_vfs_and_persistent_active_user() {
        // Ported oracle: tests/test_emulator_profiles.py:248-272.
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = rpcs3_emulator(temp.path());
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::create_dir_all(dir.join("GuiConfigs")).unwrap();
        let custom_hdd0 = temp.path().join("rpcs3-data").join("dev_hdd0");
        std::fs::create_dir_all(custom_hdd0.join("home").join("00000002").join("savedata"))
            .unwrap();

        std::fs::write(
            dir.join("config").join("vfs.yml"),
            "$(EmulatorDir): \"\"\n\"/dev_hdd0/\": \"../rpcs3-data/dev_hdd0/\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("GuiConfigs").join("persistent_settings.dat"),
            "[Users]\nactive_user=00000002\n",
        )
        .unwrap();

        let launch_args = args(&["--no-gui", "%RPCS3_GAMEID%:%ps3_gameid%"]);
        let settings = rpcs3_directory_settings(&exe, &launch_args);

        assert_eq!(settings.current_user, "00000002");
        assert_eq!(
            settings.dev_hdd0,
            resolve_best_effort(&custom_hdd0).to_string_lossy()
        );
    }

    #[test]
    fn rpcs3_save_path_overrides_prioritize_cli_user_and_existing_users() {
        // Ported oracle: tests/test_emulator_profiles.py:274-301.
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = rpcs3_emulator(temp.path());
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::create_dir_all(dir.join("GuiConfigs")).unwrap();
        let custom_hdd0 = temp.path().join("portable-data").join("dev_hdd0");
        let cli_user_path = custom_hdd0.join("home").join("00000003").join("savedata");
        let persistent_user_path = custom_hdd0.join("home").join("00000002").join("savedata");
        std::fs::create_dir_all(&cli_user_path).unwrap();
        std::fs::create_dir_all(&persistent_user_path).unwrap();

        std::fs::write(
            dir.join("config").join("vfs.yml"),
            "$(EmulatorDir): \"\"\n\"/dev_hdd0/\": \"../portable-data/dev_hdd0/\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("GuiConfigs").join("persistent_settings.dat"),
            "[Users]\nactive_user=00000002\n",
        )
        .unwrap();

        let launch_args = args(&[
            "--no-gui",
            "--user-id",
            "00000003",
            "%RPCS3_GAMEID%:%ps3_gameid%",
        ]);
        let overrides = rpcs3_save_path_overrides(&exe, &launch_args);

        assert_eq!(
            overrides.first(),
            Some(&resolve_best_effort(&cli_user_path))
        );
        assert!(overrides.contains(&resolve_best_effort(&persistent_user_path)));
    }

    #[test]
    fn ps3_vfs_paths_fall_back_to_the_library_dot_vfs_directories() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let library = temp.path().join("PS3 Library");
        std::fs::create_dir_all(&library).unwrap();

        let dev_hdd0 =
            ps3_vfs_dev_hdd0_path("/does/not/exist/rpcs3.exe", &[], library.to_str().unwrap());
        let games = ps3_vfs_games_path("/does/not/exist/rpcs3.exe", &[], library.to_str().unwrap());

        let resolved_library = resolve_best_effort(&library);
        assert_eq!(
            dev_hdd0,
            Some(resolved_library.join(".vfs").join("dev_hdd0"))
        );
        assert_eq!(games, Some(resolved_library.join(".vfs").join("games")));
    }

    #[test]
    fn ps3_vfs_paths_are_none_when_the_library_is_blank() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        assert_eq!(
            ps3_vfs_dev_hdd0_path("/does/not/exist/rpcs3.exe", &[], ""),
            None
        );
        assert_eq!(
            ps3_vfs_games_path("/does/not/exist/rpcs3.exe", &[], ""),
            None
        );
    }

    // ---------------------------------------------------------------
    // Part 2 shared test helper
    // ---------------------------------------------------------------

    /// Create `<temp>/<subdir>/<exe_name>` (an empty file) and return its
    /// path text plus the containing directory.
    fn make_exe(temp: &Path, subdir: &str, exe_name: &str) -> (String, PathBuf) {
        let dir = temp.join(subdir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join(exe_name);
        std::fs::write(&exe, b"").unwrap();
        (exe.to_string_lossy().to_string(), dir)
    }

    // ---------------------------------------------------------------
    // Azahar
    // ---------------------------------------------------------------

    #[test]
    fn azahar_directory_settings_defaults() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let settings = azahar_directory_settings("/nonexistent/azahar.exe", &[]);

        assert!(!settings.user_root.is_empty());
        assert!(!settings.nand_root.is_empty());
        assert!(!settings.sdmc_root.is_empty());
        assert!(!settings.states_root.is_empty());
        assert_eq!(settings.config_path, "");
        assert!(settings.use_virtual_sd);
        assert!(!settings.use_custom_storage);
    }

    #[test]
    fn azahar_use_virtual_sd_defaults_true() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "Azahar", "azahar.exe");
        let config_dir = dir.join("user").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("qt-config.ini"),
            "[Data Storage]\nuse_custom_storage = false\n",
        )
        .unwrap();

        let settings = azahar_directory_settings(&exe, &[]);

        assert!(settings.use_virtual_sd);
    }

    #[test]
    fn azahar_sdmc_skipped_when_virtual_sd_is_false() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "Azahar", "azahar.exe");
        let config_dir = dir.join("user").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("qt-config.ini"),
            "[Data Storage]\nuse_custom_storage = true\nuse_virtual_sd = false\n\
             nand_directory = CustomNand\nsdmc_directory = CustomSDMC\n",
        )
        .unwrap();
        std::fs::create_dir_all(
            dir.join("user")
                .join("CustomSDMC")
                .join("Nintendo 3DS")
                .join(ZERO_ID_32)
                .join(ZERO_ID_32)
                .join("title")
                .join("00040000"),
        )
        .unwrap();
        std::fs::create_dir_all(
            dir.join("user")
                .join("CustomNand")
                .join(ZERO_ID_32)
                .join("title")
                .join("00040010"),
        )
        .unwrap();

        let settings = azahar_directory_settings(&exe, &[]);
        assert!(!settings.use_virtual_sd);

        let overrides = azahar_save_path_overrides(&exe, &[]);
        assert!(
            !overrides
                .iter()
                .any(|p| p.to_string_lossy().contains("CustomSDMC")),
            "SDMC must be skipped entirely: {overrides:?}"
        );
        assert!(overrides
            .iter()
            .any(|p| p.to_string_lossy().contains("CustomNand")));
    }

    #[test]
    fn azahar_falls_back_to_the_all_zero_id_path_when_nothing_exists() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "Azahar", "azahar.exe");
        let config_dir = dir.join("user").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("qt-config.ini"),
            "[Data Storage]\nuse_custom_storage = true\nuse_virtual_sd = true\n\
             nand_directory = CustomNand\nsdmc_directory = CustomSDMC\n",
        )
        .unwrap();
        // No title directories exist anywhere under CustomSDMC.

        let overrides = azahar_save_path_overrides(&exe, &[]);

        let expected_fallback = resolve_best_effort(
            &dir.join("user")
                .join("CustomSDMC")
                .join("Nintendo 3DS")
                .join(ZERO_ID_32)
                .join(ZERO_ID_32)
                .join("title")
                .join("00040000"),
        );
        assert!(
            overrides.contains(&expected_fallback),
            "{overrides:?} must contain the all-zero id fallback"
        );
    }

    // ---------------------------------------------------------------
    // Eden
    // ---------------------------------------------------------------

    #[test]
    fn eden_directory_settings_defaults() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let settings = eden_directory_settings("/nonexistent/eden.exe", &[]);

        assert!(!settings.user_root.is_empty());
        assert!(!settings.nand_root.is_empty());
        assert!(!settings.sdmc_root.is_empty());
        assert!(!settings.states_root.is_empty());
        assert_eq!(settings.config_path, "");
        assert!(settings.use_virtual_sd);
        assert!(!settings.use_custom_storage);
    }

    #[test]
    fn eden_probes_alternate_app_names_including_yuzu_and_suyu() {
        let names = eden_app_name_candidates("/some/path/MyEden.AppImage");
        assert!(names.contains(&"MyEden".to_string()));
        // Case-insensitive dedup means only the first-seen spelling of each
        // name survives ("Eden" precedes "eden" in the extras list, so
        // "eden" itself is dropped) — check membership by casefold key.
        let lowered: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
        for expected in ["eden", "yuzu", "suyu"] {
            assert!(
                lowered.contains(&expected.to_string()),
                "{names:?} must contain a case-insensitive match for {expected:?}"
            );
        }
    }

    #[test]
    fn eden_save_overrides_keep_only_user_dirs_with_children() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "Eden", "eden.exe");
        let save_root = dir
            .join("user")
            .join("nand")
            .join("user")
            .join("save")
            .join("0000000000000000");
        let with_child = save_root.join("00000000000000000000000000000001");
        std::fs::create_dir_all(with_child.join("0100ABCD1234EF00")).unwrap();
        let without_child = save_root.join("00000000000000000000000000000002");
        std::fs::create_dir_all(&without_child).unwrap();

        let overrides = eden_save_path_overrides(&exe, &[]);

        assert_eq!(overrides, vec![resolve_best_effort(&with_child)]);
    }

    #[test]
    fn eden_save_overrides_fall_back_to_the_parent() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "Eden", "eden.exe");
        let save_root = dir
            .join("user")
            .join("nand")
            .join("user")
            .join("save")
            .join("0000000000000000");
        std::fs::create_dir_all(&save_root).unwrap();

        let overrides = eden_save_path_overrides(&exe, &[]);

        assert_eq!(overrides, vec![resolve_best_effort(&save_root)]);
    }

    // ---------------------------------------------------------------
    // Cemu
    // ---------------------------------------------------------------

    #[test]
    fn cemu_directory_settings_defaults() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let settings = cemu_directory_settings("/nonexistent/cemu.exe", &[]);

        assert_eq!(settings.config_path, "");
        assert_eq!(settings.mlc_path, "");
    }

    #[test]
    fn cemu_mlc_launch_flag_beats_the_settings_xml() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "Cemu", "cemu.exe");
        let portable_dir = dir.join("portable");
        std::fs::create_dir_all(&portable_dir).unwrap();
        std::fs::write(
            portable_dir.join("settings.xml"),
            "<content><mlc_path>/from/xml</mlc_path></content>",
        )
        .unwrap();

        let launch_args = args(&["-m", "/from/flag"]);
        let overrides = cemu_save_path_overrides(&exe, &launch_args);

        let expected_flag_root = PathBuf::from("/from/flag").join("usr").join("save");
        assert_eq!(overrides.first(), Some(&expected_flag_root));
        let expected_xml_root = PathBuf::from("/from/xml").join("usr").join("save");
        assert!(overrides.contains(&expected_xml_root));
    }

    #[test]
    fn cemu_save_root_appends_usr_save_only_when_absent() {
        let plain = PathBuf::from("/data/mlc")
            .join("usr")
            .join("save")
            .to_string_lossy()
            .to_string();
        assert_eq!(cemu_save_root_from_mlc_path("/data/mlc"), plain);
        assert_eq!(
            cemu_save_root_from_mlc_path("/data/mlc/usr/save"),
            "/data/mlc/usr/save"
        );
        assert_eq!(
            cemu_save_root_from_mlc_path("/data/mlc/usr/save/"),
            "/data/mlc/usr/save"
        );
        // Case-insensitive slash-normalized detection, original text kept.
        assert_eq!(
            cemu_save_root_from_mlc_path(r"C:\data\mlc\USR\SAVE"),
            r"C:\data\mlc\USR\SAVE"
        );
        assert_eq!(cemu_save_root_from_mlc_path("   "), "");
    }

    // ---------------------------------------------------------------
    // Xemu
    // ---------------------------------------------------------------

    #[test]
    fn xemu_directory_settings_defaults() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let settings = xemu_directory_settings("/nonexistent/xemu.exe", &[]);

        assert!(!settings.base_path.is_empty());
        assert!(!settings.config_path.is_empty());
        assert!(!settings.hdd_path.is_empty());
        assert!(!settings.eeprom_path.is_empty());
    }

    #[test]
    fn xemu_config_path_launch_flag_appends_xemu_toml_to_a_directory() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = make_exe(temp.path(), "xemu", "xemu.exe");
        let config_dir = temp.path().join("xemu-config");
        std::fs::create_dir_all(&config_dir).unwrap();

        let launch_args = args(&["-config_path", config_dir.to_str().unwrap()]);
        let candidates = xemu_config_path_candidates(&exe, &launch_args);

        assert_eq!(
            candidates.first(),
            Some(&resolve_best_effort(&config_dir.join("xemu.toml")))
        );
    }

    #[test]
    fn xemu_toml_reader_flattens_dotted_keys_and_inline_files_table() {
        let content =
            "[sys]\nfiles = { hdd_path = \"/abs/hdd.qcow2\", eeprom_path = \"/abs/eeprom.bin\" }\n\
                        [display]\nwindow.fullscreen_on_startup = true\n";

        let sections = xemu_parse_toml_sections(content);

        let files_section = sections
            .get("sys.files")
            .expect("an inline `files` table must expand into a <section>.files pseudo-section");
        assert_eq!(
            files_section.get("hdd_path").map(String::as_str),
            Some("\"/abs/hdd.qcow2\"")
        );
        assert_eq!(
            files_section.get("eeprom_path").map(String::as_str),
            Some("\"/abs/eeprom.bin\"")
        );

        let window_section = sections
            .get("display.window")
            .expect("a dotted key must flatten into a synthetic <section>.<prefix> section");
        assert_eq!(
            window_section
                .get("fullscreen_on_startup")
                .map(String::as_str),
            Some("true")
        );
    }

    // ---------------------------------------------------------------
    // Xenia
    // ---------------------------------------------------------------

    #[test]
    fn xenia_directory_settings_defaults() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let settings = xenia_directory_settings("/nonexistent/xenia.exe", &[]);

        assert_eq!(settings.variant, "master");
        assert!(!settings.storage_root.is_empty());
        assert!(!settings.content_root.is_empty());
        assert!(!settings.cache_root.is_empty());
        assert_eq!(settings.config_path, "");
        assert!(!settings.portable);
    }

    #[test]
    fn xenia_variant_detection_table() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let cases: &[(&str, &str)] = &[
            ("/opt/xenia_canary/xenia_canary.exe", "canary"),
            ("/opt/xenia-canary/xenia.exe", "canary"),
            ("/opt/canary/xenia.exe", "canary"),
            ("/opt/xenia_edge/xenia_edge.exe", "edge"),
            ("/opt/xenia-edge/xenia.exe", "edge"),
            ("/opt/xenia/xenia.exe", "master"),
        ];
        for (path, expected_variant) in cases {
            let settings = xenia_directory_settings(path, &[]);
            assert_eq!(settings.variant, *expected_variant, "path={path}");
        }
    }

    #[test]
    fn xenia_save_overrides_walk_xuid_and_bare_title_directories() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "xenia-canary", "xenia_canary.exe");
        std::fs::write(dir.join("portable.txt"), "").unwrap();
        let content_root = dir.join("content");
        let xuid_title = content_root.join("0000000000000001").join("4D5307E6");
        std::fs::create_dir_all(xuid_title.join("00000001")).unwrap();
        let bare_title = content_root.join("4D5307E7");
        std::fs::create_dir_all(bare_title.join("profile")).unwrap();
        std::fs::create_dir_all(content_root.join("not-hex")).unwrap();

        let overrides = xenia_save_path_overrides(&exe, &[]);

        assert!(overrides.contains(&resolve_best_effort(&xuid_title.join("00000001"))));
        assert!(overrides.contains(&resolve_best_effort(&bare_title.join("profile"))));
        assert!(overrides
            .iter()
            .all(|p| !p.to_string_lossy().contains("not-hex")));
    }

    #[test]
    fn xenia_state_overrides_are_always_empty() {
        assert_eq!(
            xenia_state_path_overrides(
                "/any/xenia.exe",
                &["--config".to_string(), "x".to_string()]
            ),
            Vec::<PathBuf>::new()
        );
    }

    // ---------------------------------------------------------------
    // Redream
    // ---------------------------------------------------------------

    #[test]
    fn redream_directory_settings_defaults() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, _dir) = make_exe(temp.path(), "redream", "redream.exe");

        let settings = redream_directory_settings(&exe, &[]);

        assert!(!settings.data_root.is_empty());
        assert!(!settings.config_path.is_empty());
    }

    #[test]
    fn redream_portable_detected_from_each_marker() {
        let _lock = crate::test_env::lock();
        for marker in [
            "redream.cfg",
            "flash.bin",
            "vmu0.bin",
            "save.sav",
            "cover.png",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let (exe, dir) = make_exe(temp.path(), "redream", "redream.exe");
            std::fs::write(dir.join(marker), "").unwrap();

            let settings = redream_directory_settings(&exe, &[]);

            assert_eq!(
                settings.data_root,
                resolve_best_effort(&dir).to_string_lossy(),
                "marker={marker}"
            );
            assert!(settings.portable, "marker={marker}");
        }
    }

    #[test]
    fn redream_save_overrides_are_existing_vmu_files() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "redream", "redream.exe");
        for name in ["vmu0.bin", "vmu1.bin", "vmu2.bin", "vmu3.bin"] {
            std::fs::write(dir.join(name), "").unwrap();
        }

        let overrides = redream_save_path_overrides(&exe, &[]);

        assert_eq!(
            overrides,
            vec![
                resolve_best_effort(&dir.join("vmu0.bin")),
                resolve_best_effort(&dir.join("vmu1.bin")),
                resolve_best_effort(&dir.join("vmu2.bin")),
                resolve_best_effort(&dir.join("vmu3.bin")),
            ]
        );
    }

    // ---------------------------------------------------------------
    // FBNeo
    // ---------------------------------------------------------------

    #[test]
    fn fbneo_directory_settings_defaults() {
        let settings = fbneo_directory_settings("/nonexistent/fbneo.exe", &[]);

        assert!(!settings.base_path.is_empty());
        assert!(!settings.config_path.is_empty());
        assert!(!settings.eeprom_path.is_empty());
        assert!(!settings.memcard_path.is_empty());
        assert!(!settings.hiscore_path.is_empty());
        assert!(!settings.hdd_path.is_empty());
        assert!(!settings.state_path.is_empty());
    }

    #[test]
    fn fbneo_memcard_and_state_paths_are_not_configurable() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path(), "fbneo", "fbneo.exe");
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("fbneo.ini"),
            "szAppEEPROMPath custom/eeprom\nszAppHiscorePath custom/hiscore\n",
        )
        .unwrap();

        let settings = fbneo_directory_settings(&exe, &[]);

        assert_eq!(
            settings.memcard_path,
            resolve_best_effort(&dir.join("config").join("memcards")).to_string_lossy()
        );
        assert_eq!(
            settings.state_path,
            resolve_best_effort(&dir.join("savestates")).to_string_lossy()
        );
    }

    #[test]
    fn fbneo_probes_the_three_config_names_in_order() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path(), "fbneo", "MyBuild.exe");
        let config_dir = dir.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();

        let candidates = fbneo_config_path_candidates(&exe);

        assert_eq!(
            candidates,
            vec![
                resolve_best_effort(&config_dir.join("MyBuild.ini")),
                resolve_best_effort(&config_dir.join("fbneo.ini")),
                resolve_best_effort(&config_dir.join("FinalBurn Neo.ini")),
            ]
        );
    }

    // ---------------------------------------------------------------
    // MAME
    // ---------------------------------------------------------------

    #[test]
    fn mame_directory_settings_defaults() {
        let settings = mame_directory_settings("/nonexistent/mame.exe", &[]);

        assert!(!settings.base_path.is_empty());
        assert!(!settings.ini_path.is_empty());
        assert!(!settings.cfg_directory.is_empty());
        assert!(!settings.nvram_directory.is_empty());
        assert!(!settings.memcard_directory.is_empty());
        assert!(!settings.diff_directory.is_empty());
        assert!(!settings.state_directory.is_empty());
    }

    #[test]
    fn mame_launch_args_beat_the_ini() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path(), "mame", "mame.exe");
        std::fs::write(dir.join("mame.ini"), "nvram_directory ini-nvram\n").unwrap();

        let launch_args = args(&["-nvram_directory", "flag-nvram"]);
        let settings = mame_directory_settings(&exe, &launch_args);

        assert_eq!(
            settings.nvram_directory,
            resolve_best_effort(&dir.join("flag-nvram")).to_string_lossy()
        );
    }

    #[test]
    fn mame_arg_parser_refuses_a_dash_prefixed_value() {
        let launch_args = args(&["-nvram_directory", "-diff_directory", "value"]);

        let overrides = mame_launch_overrides(&launch_args);

        assert!(
            !overrides.contains_key("nvram_directory"),
            "a following dash-prefixed token must not be consumed as this option's value: {overrides:?}"
        );
    }

    #[test]
    fn mame_inipath_is_semicolon_separated() {
        let temp = tempfile::tempdir().unwrap();
        let (exe, dir) = make_exe(temp.path(), "mame", "mame.exe");
        let dir_a = dir.join("a");
        let dir_b = dir.join("b");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();
        std::fs::write(dir_b.join("mame.ini"), "state_directory custom-sta\n").unwrap();

        let launch_args = args(&["-inipath", "a;b"]);
        let settings = mame_directory_settings(&exe, &launch_args);

        assert_eq!(
            settings.ini_path,
            resolve_best_effort(&dir_b.join("mame.ini")).to_string_lossy()
        );
        assert_eq!(
            settings.state_directory,
            resolve_best_effort(&dir.join("custom-sta")).to_string_lossy()
        );
    }

    // ---------------------------------------------------------------
    // Pico-8
    // ---------------------------------------------------------------

    #[test]
    fn pico8_directory_settings_defaults() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());

        let settings = pico8_directory_settings("/nonexistent/pico8.exe", &[]);

        assert!(!settings.user_root.is_empty());
        assert!(!settings.carts_root.is_empty());
        assert!(!settings.cdata_root.is_empty());
        assert!(!settings.cstore_root.is_empty());
        assert!(!settings.backup_root.is_empty());
        assert!(!settings.desktop_path.is_empty());
    }

    #[test]
    fn pico8_user_root_requires_a_marker_file() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (_exe, dir) = make_exe(temp.path(), "pico8", "pico8.exe");
        // No config.txt/cdata/cstore anywhere under the emulator dir.

        let candidates = pico8_user_root_candidates(&dir.join("pico8.exe").to_string_lossy(), &[]);

        assert!(
            !candidates.iter().any(|c| c == &resolve_best_effort(&dir)),
            "the bare emulator dir must be excluded without a marker: {candidates:?}"
        );
    }

    #[test]
    fn pico8_root_path_and_desktop_overrides() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let (exe, dir) = make_exe(temp.path(), "pico8", "pico8.exe");
        std::fs::write(
            dir.join("config.txt"),
            "root_path my-carts\ndesktop my-desktop\n",
        )
        .unwrap();

        let settings = pico8_directory_settings(&exe, &[]);

        assert_eq!(
            settings.carts_root,
            resolve_best_effort(&dir.join("my-carts")).to_string_lossy()
        );
        assert_eq!(
            settings.desktop_path,
            resolve_best_effort(&dir.join("my-desktop")).to_string_lossy()
        );
    }

    // ---------------------------------------------------------------
    // Vita3K
    // ---------------------------------------------------------------

    #[test]
    fn vita3k_pref_path_priority() {
        let _lock = crate::test_env::lock();

        // Portable directory wins over config.yml.
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let emu_dir = temp.path().join("emu");
            std::fs::create_dir_all(&emu_dir).unwrap();
            let portable_dir = emu_dir.join("portable");
            std::fs::create_dir_all(&portable_dir).unwrap();
            std::fs::write(emu_dir.join("config.yml"), "pref-path: /other/path\n").unwrap();
            let exe = emu_dir.join("Vita3K").to_string_lossy().to_string();

            assert_eq!(vita3k_pref_path(&exe), Some(portable_dir));
        }

        // config.yml double-quoted value.
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let emu_dir = temp.path().join("emu");
            std::fs::create_dir_all(&emu_dir).unwrap();
            std::fs::write(
                emu_dir.join("config.yml"),
                "pref-path: \"/path/with spaces\"\n",
            )
            .unwrap();
            let exe = emu_dir.join("Vita3K").to_string_lossy().to_string();

            assert_eq!(
                vita3k_pref_path(&exe),
                Some(PathBuf::from("/path/with spaces"))
            );
        }

        // config.yml single-quoted value.
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let emu_dir = temp.path().join("emu");
            std::fs::create_dir_all(&emu_dir).unwrap();
            std::fs::write(emu_dir.join("config.yml"), "pref-path: '/single/quoted'\n").unwrap();
            let exe = emu_dir.join("Vita3K").to_string_lossy().to_string();

            assert_eq!(
                vita3k_pref_path(&exe),
                Some(PathBuf::from("/single/quoted"))
            );
        }

        // config.yml `~` expansion.
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let emu_dir = temp.path().join("emu");
            std::fs::create_dir_all(&emu_dir).unwrap();
            std::fs::write(emu_dir.join("config.yml"), "pref-path: ~/Vita3K\n").unwrap();
            let exe = emu_dir.join("Vita3K").to_string_lossy().to_string();

            assert_eq!(vita3k_pref_path(&exe), Some(temp.path().join("Vita3K")));
        }

        // config.yml missing the key falls through to the platform default.
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let emu_dir = temp.path().join("emu");
            std::fs::create_dir_all(&emu_dir).unwrap();
            std::fs::write(emu_dir.join("config.yml"), "some-other-key: value\n").unwrap();
            let exe = emu_dir.join("Vita3K").to_string_lossy().to_string();

            assert_eq!(
                vita3k_pref_path_for_host(&exe, Vita3kHost::Linux),
                Some(
                    temp.path()
                        .join(".local")
                        .join("share")
                        .join("Vita3K")
                        .join("Vita3K")
                )
            );
        }

        // The three platform defaults — driven explicitly via the `_for_host`
        // seam, since this binary is compiled for exactly one host OS.
        {
            let temp = tempfile::tempdir().unwrap();
            let _guard = isolated_env(temp.path());
            let exe = temp
                .path()
                .join("emu")
                .join("Vita3K")
                .to_string_lossy()
                .to_string();

            assert_eq!(
                vita3k_pref_path_for_host(&exe, Vita3kHost::Linux),
                Some(
                    temp.path()
                        .join(".local")
                        .join("share")
                        .join("Vita3K")
                        .join("Vita3K")
                )
            );
            assert_eq!(
                vita3k_pref_path_for_host(&exe, Vita3kHost::Windows),
                Some(
                    temp.path()
                        .join("AppData")
                        .join("Roaming")
                        .join("Vita3K")
                        .join("Vita3K")
                )
            );
            assert_eq!(
                vita3k_pref_path_for_host(&exe, Vita3kHost::Macos),
                Some(
                    temp.path()
                        .join("Library")
                        .join("Application Support")
                        .join("Vita3K")
                        .join("Vita3K")
                )
            );
        }
    }

    #[test]
    fn vita3k_save_overrides_always_prepend_user_00() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let emu_dir = temp.path().join("emu");
        let portable_dir = emu_dir.join("portable");
        std::fs::create_dir_all(portable_dir.join("ux0").join("user").join("01")).unwrap();
        let exe = emu_dir.join("Vita3K").to_string_lossy().to_string();

        let overrides = vita3k_save_path_overrides(&exe, &[]);

        assert_eq!(
            overrides.first(),
            Some(
                &portable_dir
                    .join("ux0")
                    .join("user")
                    .join("00")
                    .join("savedata")
            ),
            "user 00 must always be first even though it does not exist: {overrides:?}"
        );
        assert!(overrides.contains(
            &portable_dir
                .join("ux0")
                .join("user")
                .join("01")
                .join("savedata")
        ));
    }

    #[test]
    fn vita3k_excludes_non_two_digit_directories() {
        let _lock = crate::test_env::lock();
        let temp = tempfile::tempdir().unwrap();
        let _guard = isolated_env(temp.path());
        let emu_dir = temp.path().join("emu");
        let portable_dir = emu_dir.join("portable");
        let user_root = portable_dir.join("ux0").join("user");
        std::fs::create_dir_all(user_root.join("00")).unwrap();
        std::fs::create_dir_all(user_root.join("temp")).unwrap();
        std::fs::create_dir_all(user_root.join("abc")).unwrap();
        std::fs::create_dir_all(user_root.join("001")).unwrap();
        let exe = emu_dir.join("Vita3K").to_string_lossy().to_string();

        let overrides = vita3k_save_path_overrides(&exe, &[]);

        assert_eq!(overrides.len(), 1, "{overrides:?}");
        assert_eq!(overrides[0], user_root.join("00").join("savedata"));
    }

    // ---------------------------------------------------------------
    // Flycast VMU
    // ---------------------------------------------------------------

    #[test]
    fn flycast_vmu_keeps_the_newest_per_slot_and_orders_zero_to_three() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let vmu0_old = root.join("vmu0 [old].bin");
        let vmu0_new = root.join("vmu0 [new].bin");
        let vmu2 = root.join("vmu2.bin");
        std::fs::write(&vmu0_old, "old").unwrap();
        std::fs::write(&vmu0_new, "new").unwrap();
        std::fs::write(&vmu2, "x").unwrap();

        let old_time =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        let new_time = old_time + std::time::Duration::from_secs(60);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&vmu0_old)
            .unwrap()
            .set_modified(old_time)
            .unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&vmu0_new)
            .unwrap()
            .set_modified(new_time)
            .unwrap();

        let result = flycast_vmu_file_candidates(&[root.to_path_buf()]);

        assert_eq!(result, vec![vmu0_new, vmu2]);
    }

    #[test]
    fn flycast_vmu_rejects_non_vmu_names_and_a_missing_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(root.join("game.srm"), "x").unwrap();
        // Wrong-case extension: invisible on a case-sensitive (Linux)
        // filesystem, even though the name regex itself is case-insensitive
        // — parity with the Python reference's `Path.glob("*.bin")`.
        std::fs::write(root.join("VMU0.BIN"), "x").unwrap();

        let result = flycast_vmu_file_candidates(&[root.to_path_buf()]);
        assert!(result.is_empty(), "{result:?}");

        let missing = root.join("does-not-exist");
        assert!(flycast_vmu_file_candidates(&[missing]).is_empty());
    }
}
