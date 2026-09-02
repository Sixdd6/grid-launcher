//! Cloud-save match tokens, id extraction, and state-file naming rules.
//!
//! Ported from `grid_launcher/ui/mixins/cloud_mixin.py`
//! (`_game_save_match_tokens` :1204-1263, `_is_state_file_candidate` :1334,
//! `_ps2_game_id_tokens` :1401-1411, `_psp_game_id_tokens` :1414-1424, and
//! `_rpcs3_save_directories_for_game` :1177-1200 — that method calls
//! `self._ps3_game_ids_for_game(game)`, which is never defined anywhere in
//! the Python source tree; see [`ps3_id_tokens`]'s doc comment for how this
//! port reconstructs it) and `grid_launcher/library/cloud_sync.py`
//! (`_compact_match_text` :358-359, `_state_candidate_base_variants` :360-378,
//! `_state_candidate_matches_game_tokens` :381-399,
//! `_state_candidate_hash_group_key` :401-409, and
//! `cemu_save_directories_for_game`'s title-id preference ladder :502-513).

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::CloudGame;

/// Image extensions state-file detection rejects before any other rule.
/// `cloud_transfer.py:25-31`'s `SUPPORTED_IMAGE_EXTENSIONS`, duplicated here
/// per this task's brief: that constant is not yet ported to `grid-core`
/// (it lands in a later task), so this module carries its own private copy
/// rather than depend on it or invent a new shared name early.
const REJECTED_IMAGE_EXTENSIONS: &[&str] = &[".jpg", ".jpeg", ".png", ".webp", ".gif", ".bmp"];

static NON_ALNUM_LOWER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
static NON_ALNUM_UPPER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^A-Z0-9]+").unwrap());

/// `cloud_mixin.py:1214`'s possessive-stripping pattern: a straight or
/// curly apostrophe followed by `s` at a word boundary.
static POSSESSIVE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\u{2019}']s\b").unwrap());

/// `cloud_mixin.py:1221`'s Nintendo short-code pattern.
static NINTENDO_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z][A-Z0-9]{3,5}\b").unwrap());
/// `cloud_mixin.py:1228`'s 16-hex-digit run pattern.
static HEX16_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9A-F]{16}").unwrap());
/// `cloud_mixin.py:1232`'s `<8hex><separator><8hex>` pair pattern.
static HEX_PAIR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([0-9A-F]{8})[^0-9A-F]+([0-9A-F]{8})").unwrap());

/// `cloud_mixin.py:1408`'s PS2 serial pattern (dotted or plain form).
static PS2_SERIAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z]{4}[-_ ]?\d{3}\.\d{2}|[A-Z]{4}[-_ ]?\d{5}").unwrap());
/// `cloud_mixin.py:1421`'s PSP id pattern.
static PSP_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Z]{4}[-_ ]?\d{5}").unwrap());

/// `cloud_mixin.py:1341`'s numbered-slot `.sav` acceptance pattern.
static STATE_DIGIT_SAV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[._]\d+\.sav$").unwrap());
/// `cloud_mixin.py:1343`'s `_resume.sav` acceptance pattern.
static STATE_RESUME_SAV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"_resume\.sav$").unwrap());

/// `cloud_sync.py:370-374`'s five variant-stripping patterns, in order.
static P2S_STRIP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\s*\([0-9a-f]+\))?(?:\.\d+)?\.p2s$").unwrap());
static STATE_EXT_STRIP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.(?:savestate|state|st|ss|ppst)(?:\.auto|auto|[0-9]+)?$").unwrap()
});
static SAV_DOTNUM_STRIP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\.\d+)?\.sav$").unwrap());
static SAV_UNDERSCORE_STRIP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[_](?:\d+|resume)\.sav$").unwrap());
static TRAILING_NUM_STRIP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.\d+$").unwrap());

/// `cloud_sync.py:405-406`'s hash-key pattern (8-hex prefix of a `.sav`).
static HASH_GROUP_HEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9a-f]{8})(?:\.\d+)?\.sav$").unwrap());
/// `cloud_sync.py:407-408`'s hash-key pattern (stem before `_<n>`/`_resume`).
/// Anchored with `^...$` to stand in for Python's `re.fullmatch`, which the
/// `regex` crate has no direct equivalent for.
static HASH_GROUP_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([a-z0-9][\w-]+?)(?:_(?:\d+|resume))\.sav$").unwrap());

/// Lowercased, `[a-z0-9]`-only form of `s`. `cloud_sync.py:358-359`'s
/// `_compact_match_text`.
pub fn compact_alnum(s: &str) -> String {
    NON_ALNUM_LOWER_RE
        .replace_all(&s.trim().to_lowercase(), "")
        .into_owned()
}

/// `pathlib.Path(name).stem` / `.suffix` for a bare, directory-free file
/// name: the suffix is the run of characters from the last `.` onward, but
/// only when that `.` is neither the first nor the last character — so
/// `.hidden`, `noext`, and `trailing.` all have an empty suffix and the
/// whole name as their stem. Mirrors `launch/emu_install.rs`'s private
/// `split_suffix` (not exported from there, hence duplicated rather than
/// plumbed through crate-internal visibility).
fn split_suffix(name: &str) -> (String, String) {
    let chars: Vec<char> = name.chars().collect();
    let n = chars.len();
    let split_at = match chars.iter().rposition(|&c| c == '.') {
        Some(i) if i > 0 && i < n - 1 => i,
        _ => n,
    };
    (
        chars[..split_at].iter().collect(),
        chars[split_at..].iter().collect(),
    )
}

/// The final path segment of `value`, treating both `/` and `\` as
/// separators — the same normalize-then-`rsplit('/')` convention
/// `library/paths.rs::archive_name` already uses for the server-reported
/// string path fields on [`CloudGame`]. Python's `Path(value).name` only
/// ever splits on the host OS's native separator; this deviates from a
/// literal single-OS replication in favor of the project's established
/// cross-platform convention for these string fields.
fn final_path_component(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    normalized.rsplit('/').next().unwrap_or("").to_string()
}

/// `Path(value).stem` for a possibly-directoried string path field: the
/// final path segment (see [`final_path_component`]), minus its last
/// extension (see [`split_suffix`]).
fn path_stem(value: &str) -> String {
    split_suffix(&final_path_component(value)).0
}

/// `cloud_mixin.py:1206-1214`'s `add_token_variants` closure: the
/// casefolded value, its possessive-stripped form (both apostrophe
/// spellings), and the `[a-z0-9]`-only compaction of each — all inserted
/// into `tokens`, blanks dropped.
fn add_token_variants(value: &str, tokens: &mut BTreeSet<String>) {
    let text = value.trim().to_lowercase();
    if text.is_empty() {
        return;
    }
    let stripped = POSSESSIVE_RE.replace_all(&text, "").trim().to_string();

    let mut variants: BTreeSet<String> = BTreeSet::new();
    variants.insert(text);
    variants.insert(stripped);

    for variant in variants {
        if variant.is_empty() {
            continue;
        }
        let compact = compact_alnum(&variant);
        tokens.insert(variant);
        if !compact.is_empty() {
            tokens.insert(compact);
        }
    }
}

/// `cloud_mixin.py:1216-1235`'s `add_nintendo_id_variants` closure: for
/// each `\b[A-Z][A-Z0-9]{3,5}\b` match in `value.upper()`, the lowercased
/// first four characters AND their ASCII-hex encoding; for each 16-hex-digit
/// run, the whole run plus its high and low 8-hex halves (all lowercased);
/// for each `<8hex><non-hex-separator><8hex>` pair, the lowercased high,
/// low, and their concatenation.
fn add_nintendo_id_variants(value: &str, tokens: &mut BTreeSet<String>) {
    let raw_text = value.trim().to_uppercase();
    if raw_text.is_empty() {
        return;
    }

    for m in NINTENDO_CODE_RE.find_iter(&raw_text) {
        let matched = m.as_str();
        // The pattern requires at least 4 chars (`[A-Z]` + `{3,5}` more),
        // and every char is ASCII, so byte-slicing the first 4 is safe.
        let first4 = &matched[..4];
        let short_code = first4.to_lowercase();
        if !short_code.is_empty() {
            tokens.insert(short_code);
            let ascii_hex: String = first4.bytes().map(|b| format!("{b:02x}")).collect();
            if !ascii_hex.is_empty() {
                tokens.insert(ascii_hex);
            }
        }
    }

    for m in HEX16_RE.find_iter(&raw_text) {
        let normalized = m.as_str().to_lowercase();
        tokens.insert(normalized.clone());
        tokens.insert(normalized[..8].to_string());
        tokens.insert(normalized[8..].to_string());
    }

    for caps in HEX_PAIR_RE.captures_iter(&raw_text) {
        let high = caps[1].to_lowercase();
        let low = caps[2].to_lowercase();
        tokens.insert(high.clone());
        tokens.insert(low.clone());
        tokens.insert(format!("{high}{low}"));
    }
}

/// The full match-token set for `game`: title (casefolded, possessive
/// stripped, compacted) plus `title_id`/`base_title_id` (plain and
/// Nintendo-id variants) plus stems of `rom_file_name`/`extracted_path`/
/// `archive_path` (plain and Nintendo-id variants) plus `ps3_game_id`
/// lowercased verbatim. `cloud_mixin.py:1204-1263`'s
/// `_game_save_match_tokens`.
pub fn game_save_match_tokens(game: &CloudGame) -> BTreeSet<String> {
    let mut tokens: BTreeSet<String> = BTreeSet::new();

    add_token_variants(&game.title, &mut tokens);

    for value in [&game.title_id, &game.base_title_id] {
        if !value.trim().is_empty() {
            add_token_variants(value, &mut tokens);
            add_nintendo_id_variants(value, &mut tokens);
        }
    }

    for value in [
        &game.rom_file_name,
        &game.extracted_path,
        &game.archive_path,
    ] {
        if value.trim().is_empty() {
            continue;
        }
        let stem = path_stem(value);
        add_token_variants(&stem, &mut tokens);
        add_nintendo_id_variants(&stem, &mut tokens);
    }

    let ps3_game_id = game.ps3_game_id.trim().to_lowercase();
    if !ps3_game_id.is_empty() {
        tokens.insert(ps3_game_id);
    }

    tokens.retain(|t| !t.is_empty());
    tokens
}

/// PS2 serial tokens (normalized, no separators) scraped from `title`,
/// `rom_file_name`, `extracted_path`, and `archive_path`.
/// `cloud_mixin.py:1401-1411`'s `_ps2_game_id_tokens`.
pub fn ps2_serial_tokens(game: &CloudGame) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    for value in [
        &game.title,
        &game.rom_file_name,
        &game.extracted_path,
        &game.archive_path,
    ] {
        if value.trim().is_empty() {
            continue;
        }
        let upper = value.trim().to_uppercase();
        for m in PS2_SERIAL_RE.find_iter(&upper) {
            let normalized = NON_ALNUM_UPPER_RE.replace_all(m.as_str(), "").into_owned();
            if !normalized.is_empty() {
                tokens.insert(normalized);
            }
        }
    }
    tokens
}

/// PSP id tokens (normalized, no separators) scraped from `title`,
/// `rom_file_name`, `extracted_path`, and `archive_path`.
/// `cloud_mixin.py:1414-1424`'s `_psp_game_id_tokens`.
pub fn psp_id_tokens(game: &CloudGame) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    for value in [
        &game.title,
        &game.rom_file_name,
        &game.extracted_path,
        &game.archive_path,
    ] {
        if value.trim().is_empty() {
            continue;
        }
        let upper = value.trim().to_uppercase();
        for m in PSP_ID_RE.find_iter(&upper) {
            let normalized = NON_ALNUM_UPPER_RE.replace_all(m.as_str(), "").into_owned();
            if !normalized.is_empty() {
                tokens.insert(normalized);
            }
        }
    }
    tokens
}

/// RPCS3 game-id tokens for `game`, normalized uppercase with no
/// separators, for substring matching against `re.sub(r"[^A-Z0-9]+", "",
/// child.name.upper())`-normalized save directory names.
///
/// RECONSTRUCTED, NOT A DIRECT PORT: `cloud_mixin.py:1178`'s
/// `_rpcs3_save_directories_for_game` calls
/// `self._ps3_game_ids_for_game(game)`, but no method of that name is
/// defined anywhere in `grid_launcher/` — grepping the whole tree turns up
/// only the one call site. It is dead code that would raise
/// `AttributeError` the moment RPCS3 save-directory scanning actually ran.
/// No test exercises it either. The only PS3-id data [`CloudGame`] carries
/// is `ps3_game_id`, which `ps3_install.py::ps3_game_id_from_text` already
/// produces in the exact normalized form (`^[A-Z]{4}\d{5}$`, no separators)
/// that the call site's substring match needs — so this reconstructs the
/// missing method as "that one field, normalized defensively the same way,
/// as a single-element list, or empty when blank." See the task report for
/// this call flagged as a discrepancy to confirm with a human.
pub fn ps3_id_tokens(game: &CloudGame) -> Vec<String> {
    let normalized = NON_ALNUM_UPPER_RE
        .replace_all(&game.ps3_game_id.trim().to_uppercase(), "")
        .into_owned();
    if normalized.is_empty() {
        Vec::new()
    } else {
        vec![normalized]
    }
}

/// Applies `cloud_sync.py:502-513`'s Cemu title-id preference ladder to an
/// already-collected raw token set: normalize each (trim, uppercase, strip
/// non-`[A-Z0-9]` runs) keeping any token whose RAW form was non-blank
/// (even if normalization collapses it to `""` — Python's filter checks
/// `token.strip()` on the raw value, before the substitution, so an
/// all-punctuation raw token does contribute an empty string to the
/// normalized set); then prefer tokens of length >= 16, else tokens of
/// length exactly 8 not starting with `"0005"`, else the whole normalized
/// set. Returned sorted (Python's ladder result is an unordered set; a
/// `Vec` return needs a deterministic order).
pub fn cemu_title_id_tokens(tokens: &BTreeSet<String>) -> Vec<String> {
    let normalized: BTreeSet<String> = tokens
        .iter()
        .filter(|t| !t.trim().is_empty())
        .map(|t| {
            let upper = t.trim().to_uppercase();
            NON_ALNUM_UPPER_RE.replace_all(&upper, "").into_owned()
        })
        .collect();

    let full_tokens: BTreeSet<String> = normalized
        .iter()
        .filter(|t| t.len() >= 16)
        .cloned()
        .collect();
    if !full_tokens.is_empty() {
        return full_tokens.into_iter().collect();
    }

    let low_tokens: BTreeSet<String> = normalized
        .iter()
        .filter(|t| t.len() == 8 && !t.starts_with("0005"))
        .cloned()
        .collect();
    if !low_tokens.is_empty() {
        return low_tokens.into_iter().collect();
    }

    normalized.into_iter().collect()
}

/// True when `path`'s file name looks like a save-state (or state-adjacent
/// slot save) file. Rejects the shared image-sidecar extensions FIRST,
/// then accepts a known state extension, any name containing `.state`, a
/// `[._]<digits>.sav` numbered slot, or a `_resume.sav` file.
/// `cloud_mixin.py:1334-1345`'s `_is_state_file_candidate`.
pub fn is_state_file_candidate(path: &Path) -> bool {
    let raw_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let (_, suffix) = split_suffix(&raw_name);

    if REJECTED_IMAGE_EXTENSIONS.contains(&suffix.as_str()) {
        return false;
    }
    if matches!(
        suffix.as_str(),
        ".state" | ".savestate" | ".st" | ".ss" | ".ppst" | ".p2s"
    ) {
        return true;
    }
    if raw_name.contains(".state") {
        return true;
    }
    if STATE_DIGIT_SAV_RE.is_match(&raw_name) {
        return true;
    }
    if STATE_RESUME_SAV_RE.is_match(&raw_name) {
        return true;
    }
    false
}

/// The set of lowercased "base" strings `name` could plausibly be matched
/// against: `name` itself and its stem, each optionally stripped of a
/// PCSX2 `p2s` hash/slot suffix, a state-emulator extension (with `.auto`/
/// digit slot), a `.sav` (with optional `.<n>`), a DuckStation
/// `_<n>`/`_resume.sav` suffix, or a bare trailing `.<n>`.
/// `cloud_sync.py:370-378`'s `_state_candidate_base_variants`.
pub fn state_candidate_base_variants(name: &str) -> Vec<String> {
    let mut variants: BTreeSet<String> = BTreeSet::new();
    let stem = split_suffix(name).0;

    for value in [name, stem.as_str()] {
        let normalized = value.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        variants.insert(normalized.clone());

        for stripped in [
            P2S_STRIP_RE.replace_all(&normalized, "").into_owned(),
            STATE_EXT_STRIP_RE.replace_all(&normalized, "").into_owned(),
            SAV_DOTNUM_STRIP_RE
                .replace_all(&normalized, "")
                .into_owned(),
            SAV_UNDERSCORE_STRIP_RE
                .replace_all(&normalized, "")
                .into_owned(),
            TRAILING_NUM_STRIP_RE
                .replace_all(&normalized, "")
                .into_owned(),
        ] {
            if !stripped.is_empty() {
                variants.insert(stripped);
            }
        }
    }

    variants.into_iter().filter(|v| !v.is_empty()).collect()
}

/// True when `name` matches any of `tokens`, by exact base-variant or by
/// `[a-z0-9]`-compacted base-variant. An empty `tokens` matches everything.
/// `cloud_sync.py:381-399`'s `_state_candidate_matches_game_tokens`.
pub fn state_candidate_matches_tokens(name: &str, tokens: &BTreeSet<String>) -> bool {
    if tokens.is_empty() {
        return true;
    }

    let variants = state_candidate_base_variants(name);
    let variant_set: BTreeSet<&str> = variants.iter().map(String::as_str).collect();
    let compact_variants: BTreeSet<String> = variants.iter().map(|v| compact_alnum(v)).collect();

    for token in tokens {
        let normalized_token = token.trim().to_lowercase();
        if normalized_token.is_empty() {
            continue;
        }
        if variant_set.contains(normalized_token.as_str()) {
            return true;
        }
        let compact_token = compact_alnum(&normalized_token);
        if !compact_token.is_empty() && compact_variants.contains(&compact_token) {
            return true;
        }
    }
    false
}

/// The grouping key for `name`: the 8-hex prefix of `<hash>[.<n>].sav`, else
/// the stem before a trailing `_<digits>`/`_resume` in `<name>_<n>.sav`,
/// else `""` when neither shape matches. `cloud_sync.py:401-409`'s
/// `_state_candidate_hash_group_key`.
pub fn state_candidate_hash_group_key(name: &str) -> String {
    let normalized = name.trim().to_lowercase();
    if let Some(caps) = HASH_GROUP_HEX_RE.captures(&normalized) {
        return caps[1].to_string();
    }
    if let Some(caps) = HASH_GROUP_NAME_RE.captures(&normalized) {
        return caps[1].to_string();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(overrides: impl FnOnce(&mut CloudGame)) -> CloudGame {
        let mut g = CloudGame::default();
        overrides(&mut g);
        g
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // -- state_candidate_* / is_state_file_candidate ------------------

    #[test]
    fn state_candidate_rejects_image_sidecars() {
        assert!(!is_state_file_candidate(Path::new("game.state.png")));
        assert!(!is_state_file_candidate(Path::new("game.state1.png")));
        assert!(!is_state_file_candidate(Path::new("game.state.jpg")));
        assert!(!is_state_file_candidate(Path::new("slot4.state.webp")));
    }

    #[test]
    fn state_candidate_accepts_state_and_slot_files() {
        assert!(is_state_file_candidate(Path::new("game.state")));
        assert!(is_state_file_candidate(Path::new("game.state1")));
        // Not one of the exact-suffix set, but still contains ".state".
        assert!(is_state_file_candidate(Path::new("slot3.state.bin")));
    }

    #[test]
    fn state_candidate_accepts_duckstation_slots() {
        assert!(is_state_file_candidate(Path::new("SCUS-94900_1.sav")));
        assert!(is_state_file_candidate(Path::new("SCUS-94900_2.sav")));
        assert!(is_state_file_candidate(Path::new("SCUS-94900_resume.sav")));
        assert!(is_state_file_candidate(Path::new("SLUS-01234_0.sav")));
    }

    #[test]
    fn state_candidate_accepts_numbered_sav() {
        assert!(is_state_file_candidate(Path::new("GameName.0.sav")));
        assert!(is_state_file_candidate(Path::new("GameName.1.sav")));
        assert!(is_state_file_candidate(Path::new("D4A53E48.0.sav")));
    }

    #[test]
    fn state_candidate_accepts_pcsx2_p2s_files() {
        assert!(is_state_file_candidate(Path::new(
            "SLUS-12345 (00000000).00.p2s"
        )));
        assert!(is_state_file_candidate(Path::new(
            "SCPS-12345 (ABCDEF12).01.p2s"
        )));
        assert!(is_state_file_candidate(Path::new("game.p2s")));
    }

    #[test]
    fn base_variants_strip_duckstation_naming() {
        // Exact sets, hand-verified against the real Python
        // `_state_candidate_base_variants` (run standalone against these
        // same names) rather than membership checks, so a spurious extra
        // variant would fail the test.
        assert_eq!(
            state_candidate_base_variants("SCUS-94900_1.sav"),
            vec![
                "scus-94900".to_string(),
                "scus-94900_1".to_string(),
                "scus-94900_1.sav".to_string(),
            ]
        );
        assert_eq!(
            state_candidate_base_variants("SCUS-94900_resume.sav"),
            vec![
                "scus-94900".to_string(),
                "scus-94900_resume".to_string(),
                "scus-94900_resume.sav".to_string(),
            ]
        );
        assert_eq!(
            state_candidate_base_variants("GameName.0.sav"),
            vec![
                "gamename".to_string(),
                "gamename.0".to_string(),
                "gamename.0.sav".to_string(),
            ]
        );
    }

    #[test]
    fn base_variants_strip_pcsx2_p2s_naming() {
        assert_eq!(
            state_candidate_base_variants("SLUS-12345 (00000000).00.p2s"),
            vec![
                "slus-12345".to_string(),
                "slus-12345 (00000000)".to_string(),
                "slus-12345 (00000000).00".to_string(),
                "slus-12345 (00000000).00.p2s".to_string(),
            ]
        );
        assert_eq!(
            state_candidate_base_variants("SLUS-12345.01.p2s"),
            vec![
                "slus-12345".to_string(),
                "slus-12345.01".to_string(),
                "slus-12345.01.p2s".to_string(),
            ]
        );
        assert_eq!(
            state_candidate_base_variants("game.p2s"),
            vec!["game".to_string(), "game.p2s".to_string()]
        );
    }

    #[test]
    fn hash_group_key_handles_duckstation_names() {
        assert_eq!(state_candidate_hash_group_key("D4A53E48.0.sav"), "d4a53e48");
        assert_eq!(state_candidate_hash_group_key("D4A53E48.1.sav"), "d4a53e48");
        assert_eq!(
            state_candidate_hash_group_key("SCUS-94900_1.sav"),
            "scus-94900"
        );
        assert_eq!(
            state_candidate_hash_group_key("SCUS-94900_resume.sav"),
            "scus-94900"
        );
        assert_eq!(
            state_candidate_hash_group_key("SLUS-01234_0.sav"),
            "slus-01234"
        );
        assert_eq!(
            state_candidate_hash_group_key("SCUS-94900_1.sav"),
            state_candidate_hash_group_key("SCUS-94900_resume.sav")
        );
        assert_eq!(state_candidate_hash_group_key("random.txt"), "");
    }

    #[test]
    fn p2s_candidate_matches_serial_tokens() {
        let tokens = set(&["slus12345"]);
        assert!(state_candidate_matches_tokens(
            "SLUS-12345 (00000000).00.p2s",
            &tokens
        ));
        assert!(state_candidate_matches_tokens("SLUS-12345.01.p2s", &tokens));
        assert!(!state_candidate_matches_tokens(
            "SLUS-99999 (00000000).00.p2s",
            &tokens
        ));
    }

    #[test]
    fn empty_token_set_matches_everything() {
        let empty: BTreeSet<String> = BTreeSet::new();
        assert!(state_candidate_matches_tokens(
            "anything_at_all.sav",
            &empty
        ));
        assert!(state_candidate_matches_tokens("", &empty));
    }

    // -- game_save_match_tokens ----------------------------------------

    #[test]
    fn tokens_include_possessive_stripped_and_compacted_title() {
        let g = game(|g| g.title = "Luigi's Mansion".to_string());
        let tokens = game_save_match_tokens(&g);

        // Direct port of cloud_mixin.py's add_token_variants: the
        // possessive-stripped variant drops BOTH the apostrophe and the
        // "s" (the pattern is `[’']s\b`, matched and removed whole), so
        // the stripped variant is "luigi mansion", not "luigis mansion".
        // Exact set (not membership) — hand-verified by running the real
        // Python `_game_save_match_tokens` standalone against a game dict
        // with only `title` set (every other field blank, matching
        // `CloudGame::default()`), so a spurious extra token would fail.
        assert_eq!(
            tokens,
            [
                "luigi's mansion",
                "luigi mansion",
                "luigismansion",
                "luigimansion",
            ]
            .into_iter()
            .map(String::from)
            .collect::<BTreeSet<String>>()
        );
    }

    #[test]
    fn tokens_strip_curly_apostrophe_possessive_too() {
        let g = game(|g| g.title = "Luigi\u{2019}s Mansion".to_string());
        let tokens = game_save_match_tokens(&g);
        assert!(tokens.contains("luigi mansion"));
        assert!(tokens.contains("luigimansion"));
    }

    #[test]
    fn nintendo_variants_add_hex_forms() {
        // Exact sets, hand-verified by running the real Python
        // `add_nintendo_id_variants` closure standalone against each input.
        let mut tokens = BTreeSet::new();
        add_nintendo_id_variants("GALE01", &mut tokens);
        assert_eq!(tokens, set(&["gale", "47414c45"]));

        let mut hex16_tokens = BTreeSet::new();
        add_nintendo_id_variants("0004000000123456", &mut hex16_tokens);
        assert_eq!(
            hex16_tokens,
            set(&["0004000000123456", "00040000", "00123456"])
        );

        let mut pair_tokens = BTreeSet::new();
        add_nintendo_id_variants("00050000-12345678", &mut pair_tokens);
        assert_eq!(
            pair_tokens,
            set(&["00050000", "12345678", "0005000012345678"])
        );
    }

    #[test]
    fn title_id_field_contributes_nintendo_variants_but_title_field_does_not() {
        let g = game(|g| {
            // A different Nintendo-shaped code lives ONLY in `title`; if
            // the Nintendo scan wrongly ran over `title` too, its "zqxw"
            // tokens would leak in alongside "gale"'s.
            g.title = "ZQXW99 Edition".to_string();
            g.title_id = "GALE01".to_string();
        });
        let tokens = game_save_match_tokens(&g);
        // The Nintendo-id scan never runs over `title` in Python — only
        // over title_id/base_title_id and the three path stems.
        assert!(tokens.contains("gale"));
        assert!(tokens.contains("47414c45"));
        assert!(!tokens.contains("zqxw"));
        assert!(!tokens.contains("7a717877"));
    }

    #[test]
    fn ps3_game_id_token_is_lowercased_verbatim() {
        let g = game(|g| g.ps3_game_id = "BLUS30443".to_string());
        let tokens = game_save_match_tokens(&g);
        assert!(tokens.contains("blus30443"));
    }

    // -- ps2_serial_tokens / psp_id_tokens ------------------------------

    #[test]
    fn ps2_serials_extracted_from_all_four_fields() {
        let g = game(|g| {
            g.title = "Some Game (SLUS-203.12)".to_string();
            g.rom_file_name = "SCES12345.iso".to_string();
            g.extracted_path = "/roms/ps2/SLUS_203.12/disc".to_string();
            g.archive_path = "/roms/ps2/SCES-12345.7z".to_string();
        });
        let tokens = ps2_serial_tokens(&g);
        assert!(tokens.contains("SLUS20312"));
        assert!(tokens.contains("SCES12345"));
    }

    #[test]
    fn psp_id_tokens_match_five_digit_ids_only() {
        let g = game(|g| g.title = "UMD Game (ULUS-12345)".to_string());
        let tokens = psp_id_tokens(&g);
        assert!(tokens.contains("ULUS12345"));
    }

    // -- ps3_id_tokens ---------------------------------------------------

    #[test]
    fn ps3_id_tokens_single_normalized_id_or_empty() {
        let g = game(|g| g.ps3_game_id = "BLUS30443".to_string());
        assert_eq!(ps3_id_tokens(&g), vec!["BLUS30443".to_string()]);

        let blank = CloudGame::default();
        assert!(ps3_id_tokens(&blank).is_empty());
    }

    // -- cemu_title_id_tokens ladder -------------------------------------

    #[test]
    fn cemu_ladder_prefers_16_then_8_not_0005() {
        // Full (>=16) tokens win over everything else when present.
        let with_full = set(&["0004000000123456", "00123456", "0005ABCD"]);
        let ladder = cemu_title_id_tokens(&with_full);
        assert_eq!(ladder, vec!["0004000000123456".to_string()]);

        // No full token: prefer exactly-8-length tokens not starting with
        // "0005" over one that does.
        let with_low = set(&["00123456", "0005ABCD"]);
        let ladder = cemu_title_id_tokens(&with_low);
        assert_eq!(ladder, vec!["00123456".to_string()]);

        // Neither ladder rung has anything: fall back to the whole
        // normalized set.
        let fallback_only = set(&["0005ABCD", "AB"]);
        let ladder = cemu_title_id_tokens(&fallback_only);
        assert_eq!(ladder, vec!["0005ABCD".to_string(), "AB".to_string()]);
    }

    #[test]
    fn cemu_ladder_normalizes_case_and_strips_separators() {
        let raw = set(&["abcd-1234-5678", "  "]);
        let ladder = cemu_title_id_tokens(&raw);
        assert_eq!(ladder, vec!["ABCD12345678".to_string()]);
    }

    // -- compact_alnum -----------------------------------------------------

    #[test]
    fn compact_alnum_lowercases_and_strips_non_alnum() {
        assert_eq!(compact_alnum("SLUS-12345"), "slus12345");
        assert_eq!(compact_alnum("  Luigi's Mansion  "), "luigismansion");
    }
}
