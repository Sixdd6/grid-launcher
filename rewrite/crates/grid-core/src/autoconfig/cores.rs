//! RetroArch core metadata: the embedded `retroarch-core-list.json` catalog
//! and `romm-platform-cores.json` slug map, core-id derivation, platform
//! fuzzy matching, and installed-core discovery.
//!
//! Ports `grid_launcher/emulator/retroarch.py`'s core-list machinery (module
//! docstring at the top of that file; function-level citations below). See
//! `docs/porting/05-emulator-autoconfig.md` ("RetroArch core-list entry",
//! "RomM slug → core map", "Core list handling").

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};

use regex::Regex;

/// One `retroarch-core-list.json` element, as parsed.
///
/// Every field is defaulted rather than required: an element that is a JSON
/// object but is missing a key (or has the "wrong" JSON type for a
/// capability/metadata field) still becomes a `CoreEntry` — the field simply
/// reads as absent, matching Python's `entry.get(key, default)` idiom
/// (retroarch.py:14-116, retroarch.py:529-622).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CoreEntry {
    #[serde(default)]
    pub core_file: String,
    #[serde(default)]
    pub platforms: Vec<serde_json::Value>,
    #[serde(default)]
    pub supports_save_states: Option<serde_json::Value>,
    #[serde(default)]
    pub supports_saves: Option<serde_json::Value>,
    #[serde(default)]
    pub cloud_sync_safe: Option<serde_json::Value>,
    #[serde(default)]
    pub vmu_shared_saves: Option<serde_json::Value>,
    #[serde(default)]
    pub firmware: Option<serde_json::Value>,
    #[serde(default)]
    pub config_files: Option<serde_json::Value>,
    #[serde(default)]
    pub saves_files: Option<serde_json::Value>,
}

/// A core's capability flags (`retroarch_core_flags`, retroarch.py:581-604).
///
/// Spec note: three flags default true and `vmu_shared_saves` defaults
/// false — the Python docstring claims all four default true, but the code
/// does not; this struct follows the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreFlags {
    pub supports_save_states: bool,
    pub supports_saves: bool,
    pub cloud_sync_safe: bool,
    pub vmu_shared_saves: bool,
}

impl Default for CoreFlags {
    fn default() -> Self {
        Self {
            supports_save_states: true,
            supports_saves: true,
            cloud_sync_safe: true,
            vmu_shared_saves: false,
        }
    }
}

/// Ordering-sensitive platform-key -> core-id-list map.
///
/// A `BTreeMap` cannot represent `compatibility_map()`: the fuzzy tie-break
/// in [`system_keys_for_platform`] is a STRICT `>`, so the first key at a
/// tied score wins, which is only well-defined when key order is the
/// original JSON/Markdown insertion order (retroarch.py:367-463). Tests
/// build one directly with [`CompatMap::from_pairs`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompatMap(Vec<(String, Vec<String>)>);

impl CompatMap {
    /// Build a map from explicit, already-ordered pairs.
    pub fn from_pairs(pairs: Vec<(String, Vec<String>)>) -> Self {
        Self(pairs)
    }

    /// The core-id list for an exact key, if present.
    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &(String, Vec<String>)> {
        self.0.iter()
    }
}

const CORE_LIST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../retroarch-core-list.json"
));
const SLUG_CORE_MAP_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../romm-platform-cores.json"
));

static CORE_ENTRIES: OnceLock<Vec<CoreEntry>> = OnceLock::new();
static COMPATIBILITY_MAP: OnceLock<CompatMap> = OnceLock::new();
static SLUG_CORE_MAP: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();

/// The embedded core-list catalog, parsed once (retroarch.py:14, 233
/// entries as of this port). Panics on malformed JSON — the file is a
/// build-time asset covered by
/// [`embedded_core_list_parses_and_has_233_entries`](tests), so this path
/// is unreachable in a passing CI run.
pub fn core_entries() -> &'static [CoreEntry] {
    CORE_ENTRIES.get_or_init(|| parse_core_entries(CORE_LIST_JSON))
}

/// The embedded core-list catalog's platform-compatibility map, parsed once
/// (`load_retroarch_compatibility_map`, retroarch.py:367-432).
pub fn compatibility_map() -> &'static CompatMap {
    COMPATIBILITY_MAP.get_or_init(|| parse_compatibility_map(CORE_LIST_JSON))
}

/// The embedded RomM slug -> core-id-list map, parsed once
/// (`load_retroarch_slug_core_map`, retroarch.py:18, retroarch.py:22-40).
pub fn slug_core_map() -> &'static BTreeMap<String, Vec<String>> {
    SLUG_CORE_MAP.get_or_init(|| parse_slug_map(SLUG_CORE_MAP_JSON))
}

/// A raw JSON array's elements, filtered to objects and parsed as
/// [`CoreEntry`]. An element that is not a JSON object, or that fails to
/// deserialize (a field present with an incompatible JSON type), is
/// dropped — the closest safe Rust analogue of Python's per-field
/// `isinstance` guards, since a hard type mismatch would raise in Python
/// too rather than degrade gracefully.
fn entries_from_json_array(items: Vec<serde_json::Value>) -> Vec<CoreEntry> {
    items
        .into_iter()
        .filter(serde_json::Value::is_object)
        .filter_map(|v| serde_json::from_value::<CoreEntry>(v).ok())
        .collect()
}

fn parse_core_entries(raw: &str) -> Vec<CoreEntry> {
    let items: Vec<serde_json::Value> = serde_json::from_str(raw)
        .expect("retroarch-core-list.json is embedded at build time and must be a JSON array");
    entries_from_json_array(items)
}

/// `load_retroarch_compatibility_map` (retroarch.py:367-432): a JSON array
/// top level builds the map from entries; anything else (parse failure or a
/// JSON value that parses but is not an array, e.g. an object) falls
/// through to the Markdown-table parser applied to the same raw text.
fn parse_compatibility_map(raw: &str) -> CompatMap {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Array(items)) => {
            build_compat_map_from_entries(&entries_from_json_array(items))
        }
        _ => build_compat_map_from_markdown(raw),
    }
}

/// Append `core_id` to `key`'s list, creating the key at the end of `map`
/// (first-encountered order) when absent, and skipping a core id already
/// recorded under that key.
fn compat_insert(map: &mut Vec<(String, Vec<String>)>, key: &str, core_id: &str) {
    if let Some((_, cores)) = map.iter_mut().find(|(k, _)| k == key) {
        if !cores.iter().any(|c| c == core_id) {
            cores.push(core_id.to_string());
        }
    } else {
        map.push((key.to_string(), vec![core_id.to_string()]));
    }
}

/// The JSON branch of `load_retroarch_compatibility_map` (retroarch.py:382-407).
fn build_compat_map_from_entries(entries: &[CoreEntry]) -> CompatMap {
    let mut map: Vec<(String, Vec<String>)> = Vec::new();
    for entry in entries {
        if entry.core_file.trim().is_empty() {
            continue;
        }
        let core_id = core_id_from_file_name(&entry.core_file);
        if core_id.is_empty() {
            continue;
        }
        for platform in &entry.platforms {
            let Some(platform_str) = platform.as_str() else {
                continue;
            };
            let key = normalize_platform_key(platform_str);
            if key.is_empty() {
                continue;
            }
            compat_insert(&mut map, &key, &core_id);
        }
    }
    CompatMap(map)
}

/// The Markdown-table fallback of `load_retroarch_compatibility_map`
/// (retroarch.py:409-432): keep lines whose trimmed form starts with `|`,
/// split on `|` (trimming each column), require at least 4 columns, take
/// column 1 as the core cell and column 2 as the system cell, and skip
/// blank cells, the header row (`core_cell.to_lowercase() == "core"`),
/// separator rows (`system_cell` starting with `:`) and `-` cells.
fn build_compat_map_from_markdown(raw: &str) -> CompatMap {
    let mut map: Vec<(String, Vec<String>)> = Vec::new();
    for line in raw.lines() {
        if !line.trim().starts_with('|') {
            continue;
        }
        let columns: Vec<&str> = line.split('|').map(str::trim).collect();
        if columns.len() < 4 {
            continue;
        }
        let core_cell = columns[1];
        let system_cell = columns[2];
        if core_cell.is_empty() || system_cell.is_empty() {
            continue;
        }
        if core_cell.to_lowercase() == "core" || system_cell.starts_with(':') || system_cell == "-"
        {
            continue;
        }

        let core_id = core_id_from_display_name(core_cell);
        let system_key = normalize_platform_key(system_cell);
        if core_id.is_empty() || system_key.is_empty() {
            continue;
        }
        compat_insert(&mut map, &system_key, &core_id);
    }
    CompatMap(map)
}

/// `load_retroarch_slug_core_map` (retroarch.py:22-40), applied to already
/// -embedded text: a non-object top level yields an empty map; a blank
/// (trimmed) slug or a non-array value is dropped; kept elements are the
/// non-blank string entries of the array, stored UNTRIMMED and undeduped.
fn parse_slug_map(raw: &str) -> BTreeMap<String, Vec<String>> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .expect("romm-platform-cores.json is embedded at build time and must be valid JSON");
    let serde_json::Value::Object(map) = value else {
        return BTreeMap::new();
    };

    let mut result = BTreeMap::new();
    for (slug, cores) in map {
        let trimmed_slug = slug.trim();
        if trimmed_slug.is_empty() {
            continue;
        }
        let Some(cores_arr) = cores.as_array() else {
            continue;
        };
        let valid: Vec<String> = cores_arr
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .collect();
        result.insert(trimmed_slug.to_string(), valid);
    }
    result
}

/// `retroarch_core_id_from_file_name` (retroarch.py:104-116): trim, `\` ->
/// `/`, take the last `/` segment, lowercase, strip one of `.dll`/`.so`/
/// `.dylib` (first match wins), then strip a trailing `_libretro`, then
/// trim again.
pub fn core_id_from_file_name(name: &str) -> String {
    let normalized = name.trim().replace('\\', "/");
    if normalized.is_empty() {
        return String::new();
    }

    let mut file_name = normalized.rsplit('/').next().unwrap_or("").to_lowercase();
    for suffix in [".dll", ".so", ".dylib"] {
        if let Some(stripped) = file_name.strip_suffix(suffix) {
            file_name = stripped.to_string();
            break;
        }
    }
    if let Some(stripped) = file_name.strip_suffix("_libretro") {
        file_name = stripped.to_string();
    }
    file_name.trim().to_string()
}

/// `retroarch_markdown_label` (retroarch.py:49-56): not a regex. Trim;
/// return as-is unless it starts with `[`; find `"]("`; return as-is when
/// that index is `<= 1` or the string does not end with `)`; else the
/// trimmed slice between index 1 and the marker.
pub fn markdown_label(value: &str) -> String {
    let text = value.trim();
    if !text.starts_with('[') {
        return text.to_string();
    }
    match text.find("](") {
        Some(marker) if marker > 1 && text.ends_with(')') => text[1..marker].trim().to_string(),
        _ => text.to_string(),
    }
}

/// `retroarch_core_id_from_name` (retroarch.py:59-101): [`markdown_label`]
/// -> trim -> lowercase, then a 22-entry override table; on a miss,
/// collapse every run of non-alphanumeric characters to a single `_` and
/// trim leading/trailing `_`.
pub fn core_id_from_display_name(name: &str) -> String {
    let label = markdown_label(name);
    let normalized = label.trim().to_lowercase();

    if let Some(mapped) = override_core_id(&normalized) {
        return mapped.to_string();
    }

    let mut result = String::new();
    let mut previous_underscore = false;
    for ch in normalized.chars() {
        if ch.is_alphanumeric() {
            result.push(ch);
            previous_underscore = false;
        } else if !previous_underscore {
            result.push('_');
            previous_underscore = true;
        }
    }
    result.trim_matches('_').to_string()
}

/// The 22-entry override table, quoted verbatim from retroarch.py:61-84.
fn override_core_id(normalized: &str) -> Option<&'static str> {
    Some(match normalized {
        "beetle psx" => "mednafen_psx",
        "beetle psx hw" => "mednafen_psx_hw",
        "beetle saturn" => "mednafen_saturn",
        "beetle vb" => "mednafen_vb",
        "fb neo" => "fbneo",
        "fceumm" => "fceumm",
        "flycast gles2" => "flycast",
        "lrps2" => "lrps2",
        "mame 2003-plus" => "mame2003_plus",
        "mesen-s" => "mesen_s",
        "mupen64plus-next" => "mupen64plus_next",
        "mupen64plus-next gles2" => "mupen64plus_next",
        "mupen64plus-next gles3" => "mupen64plus_next",
        "parallel n64" => "parallel_n64",
        "pcsx rearmed" => "pcsx_rearmed",
        "snes9x 2002" => "snes9x2002",
        "snes9x 2005" => "snes9x2005",
        "snes9x 2005 plus" => "snes9x2005_plus",
        "snes9x 2010" => "snes9x2010",
        "same cdi" => "same_cdi",
        "vba-m" => "vbam",
        "vba next" => "vba_next",
        _ => return None,
    })
}

/// The non-alphanumeric-run regex shared by [`normalize_platform_key`] and
/// [`platform_tokens`] (`[^a-z0-9]+` applied to a lowercased string,
/// retroarch.py:125, retroarch.py:131).
static NON_ALNUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
/// The whitespace-run collapse [`normalize_platform_key`] applies after
/// punctuation replacement (retroarch.py:126).
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// `normalize_retroarch_platform_key` (retroarch.py:119-127): trim,
/// lowercase, return `""` if empty, `\` -> `/`, replace every run of
/// `[^a-z0-9]` with one space, collapse whitespace runs, trim.
pub fn normalize_platform_key(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return String::new();
    }
    let normalized = normalized.replace('\\', "/");
    let normalized = NON_ALNUM_RE.replace_all(&normalized, " ");
    let normalized = WHITESPACE_RE.replace_all(&normalized, " ");
    normalized.trim().to_string()
}

/// `retroarch_platform_tokens` (retroarch.py:130-133): replace
/// `[^a-z0-9]+` with a space on the trimmed lowercased input, split on
/// whitespace, drop the five stopwords `the`, `and`, `of`, `for`, `system`.
pub fn platform_tokens(value: &str) -> BTreeSet<String> {
    const STOPWORDS: [&str; 5] = ["the", "and", "of", "for", "system"];
    let lowered = value.trim().to_lowercase();
    let normalized = NON_ALNUM_RE.replace_all(&lowered, " ");
    normalized
        .split_whitespace()
        .filter(|token| !STOPWORDS.contains(token))
        .map(str::to_string)
        .collect()
}

/// `retroarch_system_keys_for_platform` (retroarch.py:435-463): normalize
/// `platform`; `[]` on an empty key or an empty map; an exact key hit
/// returns `vec![key]`; otherwise fuzzy-match `platform_tokens` of the RAW
/// `platform` string against every map key's tokens (Jaccard similarity),
/// keeping the best score with a STRICT `>` (so the first key wins a tie),
/// and returning `vec![best]` only when `best_score >= 0.7`.
pub fn system_keys_for_platform(platform: &str, compat: &CompatMap) -> Vec<String> {
    let normalized = normalize_platform_key(platform);
    if normalized.is_empty() || compat.is_empty() {
        return Vec::new();
    }
    if compat.get(&normalized).is_some() {
        return vec![normalized];
    }

    let input_tokens = platform_tokens(platform);
    if input_tokens.is_empty() {
        return Vec::new();
    }

    let mut best_key = String::new();
    let mut best_score = 0.0f64;
    for (key, _) in compat.iter() {
        let key_tokens = platform_tokens(key);
        if key_tokens.is_empty() {
            continue;
        }
        let union_len = input_tokens.union(&key_tokens).count();
        if union_len == 0 {
            continue;
        }
        let intersection_len = input_tokens.intersection(&key_tokens).count();
        let score = intersection_len as f64 / union_len as f64;
        if score > best_score {
            best_score = score;
            best_key = key.clone();
        }
    }

    if !best_key.is_empty() && best_score >= 0.7 {
        vec![best_key]
    } else {
        Vec::new()
    }
}

/// `retroarch_cores_for_platform` (retroarch.py:466-478): an EMPTY
/// compatibility map returns the hardcoded arcade fallback
/// `["fbneo", "mame2003_plus"]`; otherwise the order-preserving-deduped
/// union of cores for every [`system_keys_for_platform`] match, or `[]`
/// when there is no match.
pub fn cores_for_platform(platform: &str, compat: &CompatMap) -> Vec<String> {
    if compat.is_empty() {
        return vec!["fbneo".to_string(), "mame2003_plus".to_string()];
    }

    let mut resolved: Vec<String> = Vec::new();
    for key in system_keys_for_platform(platform, compat) {
        if let Some(cores) = compat.get(&key) {
            for core in cores {
                if !resolved.contains(core) {
                    resolved.push(core.clone());
                }
            }
        }
    }
    resolved
}

/// `all_retroarch_cores` (retroarch.py:358-364): every core id across every
/// map value, order-preserving-deduped.
pub fn all_cores(compat: &CompatMap) -> Vec<String> {
    let mut cores: Vec<String> = Vec::new();
    for (_, list) in compat.iter() {
        for core in list {
            if !cores.contains(core) {
                cores.push(core.clone());
            }
        }
    }
    cores
}

/// `retroarch_cores_for_slug` (retroarch.py:43-46): `[]` for a blank slug
/// (checked untrimmed) or an empty map; else an exact, case-sensitive
/// lookup on the TRIMMED slug, cloned.
pub fn cores_for_slug(slug: &str, map: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    if slug.is_empty() || map.is_empty() {
        return Vec::new();
    }
    map.get(slug.trim()).cloned().unwrap_or_default()
}

/// The extension [`installed_core_ids`] looks for, by host OS:
/// `dll` on Windows, `dylib` on macOS, `so` elsewhere
/// (`installed_retroarch_core_ids`, retroarch.py:511-517).
fn host_core_extension() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

/// Installed core ids (not paths) — `installed_retroarch_core_ids`
/// (retroarch.py:481-526). With an explicit `cores_dir`, every
/// emulator-path check is skipped. Otherwise: a blank `emulator_path` or
/// one that does not name an existing file yields an empty set; the
/// AppImage-portable layout `<parent>/<file name>.home/.config/retroarch/
/// cores` is preferred when it exists and is a directory, else
/// `<parent>/cores`. The chosen directory is read non-recursively, files
/// only, matching the host extension; each match's derived core id is
/// collected when non-empty.
pub fn installed_core_ids(emulator_path: &str, cores_dir: Option<&Path>) -> BTreeSet<String> {
    installed_core_ids_with_extension(emulator_path, cores_dir, host_core_extension())
}

/// [`installed_core_ids`] with an explicit extension, so a test can drive
/// all three host branches without changing `cfg!(target_os = ...)`.
fn installed_core_ids_with_extension(
    emulator_path: &str,
    cores_dir: Option<&Path>,
    extension: &str,
) -> BTreeSet<String> {
    let resolved_cores_dir: PathBuf = if let Some(dir) = cores_dir {
        crate::autoconfig::paths::expand_user(&dir.to_string_lossy())
    } else {
        if emulator_path.is_empty() {
            return BTreeSet::new();
        }
        let expanded = crate::autoconfig::paths::expand_user(emulator_path);
        if !expanded.exists() || !expanded.is_file() {
            return BTreeSet::new();
        }

        let parent = expanded.parent().unwrap_or_else(|| Path::new(""));
        let file_name = expanded
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let appimage_home_cores = parent
            .join(format!("{file_name}.home"))
            .join(".config")
            .join("retroarch")
            .join("cores");

        if appimage_home_cores.exists() && appimage_home_cores.is_dir() {
            appimage_home_cores
        } else {
            parent.join("cores")
        }
    };

    if !resolved_cores_dir.exists() || !resolved_cores_dir.is_dir() {
        return BTreeSet::new();
    }

    let mut ids = BTreeSet::new();
    let Ok(read_dir) = std::fs::read_dir(&resolved_cores_dir) else {
        return ids;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some(extension) {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let core_id = core_id_from_file_name(file_name);
        if !core_id.is_empty() {
            ids.insert(core_id);
        }
    }
    ids
}

/// Python's `bool()` coercion of a JSON-decoded value: `null`, `0`/`0.0`,
/// `""`, `[]` and `{}` are falsy; everything else (including a non-empty
/// string, a non-zero number, and a non-empty array/object) is truthy.
fn json_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(o) => !o.is_empty(),
    }
}

/// `entry.get(key, default)` then `bool(...)` (retroarch.py:600-603).
///
/// Spec deviation: `CoreEntry`'s flag fields are `Option<serde_json::Value>`
/// with `#[serde(default)]`, so a JSON key that is entirely ABSENT and one
/// explicitly set to `null` both decode to Rust `None` (serde's blanket
/// `Option<T>` impl treats `null` as absent) — the two cases are not
/// distinguishable here. Python tells them apart: an absent key falls back
/// to `default`, while an explicit `null` is `bool(None)` = `false`
/// regardless of `default`. This only differs from Python when a field
/// whose default is `true` is explicitly nulled; every other combination
/// (present-falsy, present-truthy, or genuinely absent) matches exactly.
fn flag_value(value: &Option<serde_json::Value>, default: bool) -> bool {
    match value {
        Some(v) => json_truthy(v),
        None => default,
    }
}

/// `retroarch_core_flags` (retroarch.py:581-604): the first entry whose
/// `core_file` is non-blank and whose derived core id equals `core_id`
/// wins; each flag is `bool()`-coerced from the entry's value, falling back
/// to its default when absent. No match -> [`CoreFlags::default`].
pub fn core_flags(core_id: &str, entries: &[CoreEntry]) -> CoreFlags {
    let defaults = CoreFlags::default();
    for entry in entries {
        if entry.core_file.trim().is_empty() {
            continue;
        }
        if core_id_from_file_name(&entry.core_file) == core_id {
            return CoreFlags {
                supports_save_states: flag_value(
                    &entry.supports_save_states,
                    defaults.supports_save_states,
                ),
                supports_saves: flag_value(&entry.supports_saves, defaults.supports_saves),
                cloud_sync_safe: flag_value(&entry.cloud_sync_safe, defaults.cloud_sync_safe),
                vmu_shared_saves: flag_value(&entry.vmu_shared_saves, defaults.vmu_shared_saves),
            };
        }
    }
    defaults
}

/// `retroarch_core_flags_for_platform` (retroarch.py:607-622): `target =
/// platform.trim().to_lowercase()`, `None` for a blank target. Each
/// entry's `platforms` strings are compared trimmed+lowercased, EXACTLY
/// (not [`normalize_platform_key`]). On a match, the entry's core id is
/// derived; when non-empty, [`core_flags`] is re-run over the WHOLE entry
/// list (so a different entry sharing the id may answer); an empty core id
/// continues the scan. No match anywhere -> `None`, distinct from the
/// all-defaults [`CoreFlags`].
pub fn core_flags_for_platform(platform: &str, entries: &[CoreEntry]) -> Option<CoreFlags> {
    let target = platform.trim().to_lowercase();
    if target.is_empty() {
        return None;
    }

    for entry in entries {
        for candidate in &entry.platforms {
            let Some(candidate_str) = candidate.as_str() else {
                continue;
            };
            if candidate_str.trim().to_lowercase() == target {
                let core_id = core_id_from_file_name(&entry.core_file);
                if !core_id.is_empty() {
                    return Some(core_flags(&core_id, entries));
                }
            }
        }
    }
    None
}

/// Scan `entries` for the first `core_file`-matching-`core_id` entry and
/// return `field(entry)` only when it is a JSON object — shared by the
/// three metadata accessors below (retroarch.py:529-578).
fn metadata_field<'a>(
    core_id: &str,
    entries: &'a [CoreEntry],
    field: fn(&'a CoreEntry) -> Option<&'a serde_json::Value>,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    if core_id.is_empty() || entries.is_empty() {
        return None;
    }
    for entry in entries {
        if entry.core_file.trim().is_empty() {
            continue;
        }
        if core_id_from_file_name(&entry.core_file) == core_id {
            return field(entry).and_then(serde_json::Value::as_object);
        }
    }
    None
}

/// `retroarch_core_firmware_metadata` (retroarch.py:529-544).
pub fn core_firmware_metadata<'a>(
    core_id: &str,
    entries: &'a [CoreEntry],
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    metadata_field(core_id, entries, |e| e.firmware.as_ref())
}

/// `retroarch_core_config_files_metadata` (retroarch.py:547-561).
pub fn core_config_files_metadata<'a>(
    core_id: &str,
    entries: &'a [CoreEntry],
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    metadata_field(core_id, entries, |e| e.config_files.as_ref())
}

/// `retroarch_core_saves_files_metadata` (retroarch.py:564-578).
pub fn core_saves_files_metadata<'a>(
    core_id: &str,
    entries: &'a [CoreEntry],
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    metadata_field(core_id, entries, |e| e.saves_files.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(core_file: &str, platforms: &[&str]) -> CoreEntry {
        CoreEntry {
            core_file: core_file.to_string(),
            platforms: platforms
                .iter()
                .map(|p| serde_json::Value::String(p.to_string()))
                .collect(),
            supports_save_states: None,
            supports_saves: None,
            cloud_sync_safe: None,
            vmu_shared_saves: None,
            firmware: None,
            config_files: None,
            saves_files: None,
        }
    }

    // --- embedded catalogs ---------------------------------------------

    #[test]
    fn embedded_core_list_parses_and_has_233_entries() {
        assert_eq!(core_entries().len(), 233);
    }

    #[test]
    fn embedded_slug_map_parses_and_has_75_slugs() {
        assert_eq!(slug_core_map().len(), 75);
    }

    // --- core_id_from_file_name -----------------------------------------

    #[test]
    fn core_id_from_file_name_strips_extension_then_libretro() {
        let cases: &[(&str, &str)] = &[
            ("flycast_libretro.dll", "flycast"),
            ("MGBA_LIBRETRO.SO", "mgba"),
            ("a/b/snes9x_libretro.dylib", "snes9x"),
            ("a\\b\\x.dll", "x"),
            ("no_extension", "no_extension"),
            ("", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(core_id_from_file_name(input), *expected, "input={input:?}");
        }
    }

    // --- core_id_from_display_name / markdown_label ----------------------

    #[test]
    fn core_id_from_display_name_applies_all_22_overrides() {
        let cases: &[(&str, &str)] = &[
            ("beetle psx", "mednafen_psx"),
            ("beetle psx hw", "mednafen_psx_hw"),
            ("beetle saturn", "mednafen_saturn"),
            ("beetle vb", "mednafen_vb"),
            ("fb neo", "fbneo"),
            ("fceumm", "fceumm"),
            ("flycast gles2", "flycast"),
            ("lrps2", "lrps2"),
            ("mame 2003-plus", "mame2003_plus"),
            ("mesen-s", "mesen_s"),
            ("mupen64plus-next", "mupen64plus_next"),
            ("mupen64plus-next gles2", "mupen64plus_next"),
            ("mupen64plus-next gles3", "mupen64plus_next"),
            ("parallel n64", "parallel_n64"),
            ("pcsx rearmed", "pcsx_rearmed"),
            ("snes9x 2002", "snes9x2002"),
            ("snes9x 2005", "snes9x2005"),
            ("snes9x 2005 plus", "snes9x2005_plus"),
            ("snes9x 2010", "snes9x2010"),
            ("same cdi", "same_cdi"),
            ("vba-m", "vbam"),
            ("vba next", "vba_next"),
        ];
        assert_eq!(cases.len(), 22);
        for (input, expected) in cases {
            // Mixed case exercises the trim->lowercase step too.
            let mixed_case = input.to_uppercase();
            assert_eq!(
                core_id_from_display_name(&mixed_case),
                *expected,
                "input={input:?}"
            );
        }
    }

    #[test]
    fn core_id_from_display_name_slugifies_unknown_names() {
        assert_eq!(core_id_from_display_name("FB Neo (2019)"), "fb_neo_2019");
        assert_eq!(core_id_from_display_name("  --x--  "), "x");
    }

    #[test]
    fn markdown_label_extracts_link_text_and_passes_plain_text_through() {
        assert_eq!(markdown_label("[FB Neo](u)"), "FB Neo");
        assert_eq!(markdown_label("[](u)"), "[](u)");
        assert_eq!(markdown_label("[x](u"), "[x](u");
    }

    // --- normalize_platform_key / platform_tokens ------------------------

    #[test]
    fn normalize_platform_key_collapses_punctuation() {
        assert_eq!(
            normalize_platform_key("  Sony\u{ae} PlayStation--2!! "),
            "sony playstation 2"
        );
        assert_eq!(normalize_platform_key(""), "");
        assert_eq!(normalize_platform_key("   "), "");
    }

    #[test]
    fn platform_tokens_drops_the_five_stopwords() {
        assert_eq!(
            platform_tokens("The Game of the West for System"),
            BTreeSet::from(["game".to_string(), "west".to_string()])
        );
    }

    // --- system_keys_for_platform -----------------------------------------

    #[test]
    fn system_keys_exact_match_wins_before_fuzzy() {
        let compat = CompatMap::from_pairs(vec![
            ("playstation 2".to_string(), vec!["pcsx2".to_string()]),
            ("playstation".to_string(), vec!["duckstation".to_string()]),
        ]);
        assert_eq!(
            system_keys_for_platform("PlayStation 2", &compat),
            vec!["playstation 2".to_string()]
        );
    }

    #[test]
    fn system_keys_fuzzy_accepts_at_exactly_070() {
        // key tokens {a..g} (7), input tokens {a..j} (10):
        // intersection 7 / union 10 = 0.7 exactly.
        let compat = CompatMap::from_pairs(vec![(
            "a b c d e f g".to_string(),
            vec!["core".to_string()],
        )]);
        assert_eq!(
            system_keys_for_platform("a b c d e f g h i j", &compat),
            vec!["a b c d e f g".to_string()]
        );
    }

    #[test]
    fn system_keys_fuzzy_rejects_just_below_070() {
        // key tokens {a,b,c} (3), input tokens {a,b,c,d,e} (5):
        // intersection 3 / union 5 = 0.6.
        let compat = CompatMap::from_pairs(vec![("a b c".to_string(), vec!["core".to_string()])]);
        assert_eq!(
            system_keys_for_platform("a b c d e", &compat),
            Vec::<String>::new()
        );
    }

    #[test]
    fn system_keys_fuzzy_tie_break_keeps_first_map_key() {
        // Both keys score 3/4 = 0.75 against "a b c d"; first-in-order wins.
        let compat = CompatMap::from_pairs(vec![
            ("a b c".to_string(), vec!["first".to_string()]),
            ("a b d".to_string(), vec!["second".to_string()]),
        ]);
        assert_eq!(
            system_keys_for_platform("a b c d", &compat),
            vec!["a b c".to_string()]
        );

        let reordered = CompatMap::from_pairs(vec![
            ("a b d".to_string(), vec!["second".to_string()]),
            ("a b c".to_string(), vec!["first".to_string()]),
        ]);
        assert_eq!(
            system_keys_for_platform("a b c d", &reordered),
            vec!["a b d".to_string()]
        );
    }

    // --- cores_for_platform / all_cores -----------------------------------

    #[test]
    fn cores_for_platform_returns_arcade_fallback_only_for_an_empty_map() {
        assert_eq!(
            cores_for_platform("Arcade", &CompatMap::default()),
            vec!["fbneo".to_string(), "mame2003_plus".to_string()]
        );
    }

    #[test]
    fn cores_for_platform_returns_empty_for_a_populated_map_with_no_match() {
        let compat = CompatMap::from_pairs(vec![(
            "playstation 2".to_string(),
            vec!["pcsx2".to_string()],
        )]);
        assert_eq!(
            cores_for_platform("Completely Unrelated Platform", &compat),
            Vec::<String>::new()
        );
    }

    #[test]
    fn all_cores_dedupes_across_platforms_preserving_first_seen_order() {
        let compat = CompatMap::from_pairs(vec![
            ("a".to_string(), vec!["x".to_string(), "y".to_string()]),
            ("b".to_string(), vec!["y".to_string(), "z".to_string()]),
        ]);
        assert_eq!(
            all_cores(&compat),
            vec!["x".to_string(), "y".to_string(), "z".to_string()]
        );
    }

    // --- cores_for_slug / slug map loading ---------------------------------

    #[test]
    fn cores_for_slug_is_exact_on_the_trimmed_slug() {
        let mut map = BTreeMap::new();
        map.insert("psx".to_string(), vec!["mednafen_psx".to_string()]);

        assert_eq!(
            cores_for_slug(" psx ", &map),
            vec!["mednafen_psx".to_string()]
        );
        assert_eq!(cores_for_slug("PSX", &map), Vec::<String>::new());
        assert_eq!(cores_for_slug("", &map), Vec::<String>::new());
        assert_eq!(
            cores_for_slug("psx", &BTreeMap::new()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn slug_map_drops_non_string_slugs_blank_slugs_and_non_list_values() {
        let raw = r#"{"": ["a"], "bad": "not-a-list", "good": ["b", "", "  c  ", 5]}"#;
        let map = parse_slug_map(raw);
        let mut expected = BTreeMap::new();
        expected.insert(
            "good".to_string(),
            vec!["b".to_string(), "  c  ".to_string()],
        );
        assert_eq!(map, expected);
    }

    // --- compatibility_map JSON / Markdown loading -------------------------

    #[test]
    fn compatibility_map_json_array_branch_builds_ordered_map() {
        let raw = r#"[
            {"core_file": "b_libretro.so", "platforms": ["Sys One"]},
            {"core_file": "a_libretro.so", "platforms": ["Sys One", "Sys Two"]}
        ]"#;
        let map = parse_compatibility_map(raw);
        assert_eq!(
            map.get("sys one"),
            Some(&vec!["b".to_string(), "a".to_string()])
        );
        assert_eq!(map.get("sys two"), Some(&vec!["a".to_string()]));
    }

    #[test]
    fn compatibility_map_falls_back_to_markdown_when_top_level_is_not_an_array() {
        let raw = "{\"not\": \"an array\"}\n| Core | System | Notes |\n|---|---|---|\n| FB Neo | Arcade | x |\n";
        let map = parse_compatibility_map(raw);
        assert_eq!(map.get("arcade"), Some(&vec!["fbneo".to_string()]));
    }

    #[test]
    fn compatibility_map_markdown_skips_header_separator_and_dash_rows() {
        let raw =
            "| Core | System |\n|:---|:---|\n| Core | System |\n| x | - |\n| FB Neo | Arcade |\n";
        let map = parse_compatibility_map(raw);
        assert_eq!(map.get("arcade"), Some(&vec!["fbneo".to_string()]));
        assert_eq!(map.iter().count(), 1, "only the FB Neo row should count");
    }

    // --- installed_core_ids -------------------------------------------------

    #[test]
    fn installed_cores_prefers_appimage_home_layout() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("RetroArch.AppImage");
        std::fs::write(&exe, b"binary").unwrap();

        let home_cores = temp
            .path()
            .join("RetroArch.AppImage.home")
            .join(".config")
            .join("retroarch")
            .join("cores");
        std::fs::create_dir_all(&home_cores).unwrap();
        std::fs::write(home_cores.join("a_libretro.so"), b"").unwrap();

        let sibling_cores = temp.path().join("cores");
        std::fs::create_dir_all(&sibling_cores).unwrap();
        std::fs::write(sibling_cores.join("b_libretro.so"), b"").unwrap();

        let ids = installed_core_ids_with_extension(exe.to_str().unwrap(), None, "so");
        assert_eq!(ids, BTreeSet::from(["a".to_string()]));
    }

    #[test]
    fn installed_cores_falls_back_to_sibling_cores_dir() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("retroarch");
        std::fs::write(&exe, b"binary").unwrap();

        let sibling_cores = temp.path().join("cores");
        std::fs::create_dir_all(&sibling_cores).unwrap();
        std::fs::write(sibling_cores.join("b_libretro.so"), b"").unwrap();

        let ids = installed_core_ids_with_extension(exe.to_str().unwrap(), None, "so");
        assert_eq!(ids, BTreeSet::from(["b".to_string()]));
    }

    #[test]
    fn installed_cores_requires_an_existing_file_without_an_override() {
        assert_eq!(
            installed_core_ids_with_extension("", None, "so"),
            BTreeSet::new()
        );

        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("does-not-exist");
        assert_eq!(
            installed_core_ids_with_extension(missing.to_str().unwrap(), None, "so"),
            BTreeSet::new()
        );

        // A directory (not a file) also fails the check.
        assert_eq!(
            installed_core_ids_with_extension(temp.path().to_str().unwrap(), None, "so"),
            BTreeSet::new()
        );
    }

    #[test]
    fn installed_cores_explicit_dir_skips_the_executable_checks() {
        let temp = tempfile::tempdir().unwrap();
        let cores_dir = temp.path().join("cores");
        std::fs::create_dir_all(&cores_dir).unwrap();
        std::fs::write(cores_dir.join("c_libretro.so"), b"").unwrap();

        // Blank/nonexistent emulator_path would normally short-circuit, but
        // an explicit cores_dir bypasses those checks entirely.
        let ids = installed_core_ids_with_extension("", Some(&cores_dir), "so");
        assert_eq!(ids, BTreeSet::from(["c".to_string()]));
    }

    #[test]
    fn installed_cores_extension_per_platform() {
        let temp = tempfile::tempdir().unwrap();
        let cores_dir = temp.path().join("cores");
        std::fs::create_dir_all(&cores_dir).unwrap();
        std::fs::write(cores_dir.join("a_libretro.dll"), b"").unwrap();
        std::fs::write(cores_dir.join("a_libretro.dylib"), b"").unwrap();
        std::fs::write(cores_dir.join("a_libretro.so"), b"").unwrap();

        for extension in ["dll", "dylib", "so"] {
            let ids = installed_core_ids_with_extension("", Some(&cores_dir), extension);
            assert_eq!(ids, BTreeSet::from(["a".to_string()]), "ext={extension}");
        }
    }

    // --- core_flags -----------------------------------------------------

    #[test]
    fn core_flags_defaults_vmu_shared_saves_to_false() {
        let flags = core_flags("missing", &[]);
        assert!(flags.supports_save_states);
        assert!(flags.supports_saves);
        assert!(flags.cloud_sync_safe);
        assert!(!flags.vmu_shared_saves);
        assert_eq!(flags, CoreFlags::default());
    }

    #[test]
    fn core_flags_coerces_json_falsy_values() {
        let mut e = entry("x_libretro.so", &[]);
        e.supports_save_states = Some(serde_json::json!(0));
        e.supports_saves = Some(serde_json::json!(""));
        e.cloud_sync_safe = Some(serde_json::json!([]));
        e.vmu_shared_saves = Some(serde_json::json!(true));

        let flags = core_flags("x", std::slice::from_ref(&e));
        assert!(!flags.supports_save_states, "0 is falsy");
        assert!(!flags.supports_saves, "\"\" is falsy");
        assert!(
            !flags.cloud_sync_safe,
            "[] is falsy, overriding a true default"
        );
        assert!(flags.vmu_shared_saves, "true overrides the false default");
    }

    // --- core_flags_for_platform --------------------------------------------

    #[test]
    fn core_flags_for_platform_matches_case_insensitively_and_exactly() {
        let entries = vec![entry("x_libretro.so", &["PlayStation 2"])];
        let flags = core_flags_for_platform("playstation 2", &entries);
        assert_eq!(flags, Some(CoreFlags::default()));
        let flags_upper = core_flags_for_platform("PLAYSTATION 2", &entries);
        assert_eq!(flags_upper, Some(CoreFlags::default()));

        // Exact compare only, not normalize_platform_key: extra internal
        // whitespace must NOT match.
        assert_eq!(core_flags_for_platform("PlayStation  2", &entries), None);
    }

    #[test]
    fn core_flags_for_platform_returns_none_when_no_platform_matches() {
        let entries = vec![entry("x_libretro.so", &["PlayStation 2"])];
        assert_eq!(core_flags_for_platform("Nintendo 64", &entries), None);
        assert_eq!(core_flags_for_platform("   ", &entries), None);
    }

    // --- metadata accessors --------------------------------------------

    #[test]
    fn metadata_accessors_return_none_for_a_matching_entry_without_the_field() {
        let no_field = entry("x_libretro.so", &[]);
        let mut has_field = entry("x_libretro.so", &[]);
        has_field.firmware = Some(serde_json::json!({"needs_bios": true}));

        // The FIRST matching entry (no_field) wins, so the field-bearing
        // second entry is never reached.
        let entries = vec![no_field, has_field];
        assert_eq!(core_firmware_metadata("x", &entries), None);
        assert_eq!(core_config_files_metadata("x", &entries), None);
        assert_eq!(core_saves_files_metadata("x", &entries), None);

        assert_eq!(core_firmware_metadata("x", &[]), None);
        assert_eq!(core_firmware_metadata("", &entries), None);
    }

    #[test]
    fn real_catalog_flycast_has_vmu_shared_saves() {
        let flags = core_flags("flycast", core_entries());
        assert!(flags.vmu_shared_saves);

        let other = core_flags("snes9x", core_entries());
        assert!(!other.vmu_shared_saves);
    }
}
