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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::{duckstation, paths};

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
/// the Python reference, this reports only the value: callers in this
/// module never need the "how many tokens did that consume" index the
/// Python tuple carries, since each caller advances by a fixed one or two
/// positions of its own.
///
/// No caller in this file needs it yet — every `-u`/`--user`/`--user-id`
/// probe in this module reads its value with a plain next-token check,
/// matching the Python reference precisely (`dolphin.py:53-67`,
/// `rpcs3.py:46-72`, neither of which calls `_consume_arg_value`). Task 9's
/// Xemu/Cemu readers are the first real callers (`xemu.py:36` IS the
/// definition site), so this is allowed to sit unused until then rather than
/// changing this task's readers' behavior to manufacture a call site.
#[allow(dead_code)]
pub(crate) fn consume_arg_value(args: Args, index: usize) -> Option<String> {
    let raw_token = args.get(index)?;
    let token = raw_token.trim();
    if token.is_empty() {
        return None;
    }

    let first = token.chars().next().unwrap();
    let quote = (first == '"' || first == '\'').then_some(first);

    if let Some(quote) = quote {
        if token.chars().count() == 1 || !token.ends_with(quote) {
            let mut parts = vec![token.to_string()];
            let mut i = index + 1;
            while i < args.len() {
                parts.push(args[i].clone());
                if args[i].trim().ends_with(quote) {
                    break;
                }
                i += 1;
            }
            return Some(clean_ini_value(&parts.join(" ")));
        }
    }

    Some(clean_ini_value(token))
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
}
