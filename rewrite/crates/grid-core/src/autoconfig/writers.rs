//! The section-writer families every emulator module builds on.
//!
//! Each Python module carries its own near-duplicate
//! `_ensure_*_section_values(raw, section, desired)` returning
//! `(new_text, changed)`. There are three write policies — overwrite,
//! add-only, and append-if-absent — described in
//! `docs/porting/05-emulator-autoconfig.md` ("Section-writer helpers and the
//! overwrite question"). The functions here are the exact ports:
//!
//! | Function | Policy | Python source |
//! |---|---|---|
//! | [`ini_overwrite_section`] | overwrite | pcsx2.py:56-122, duckstation.py:54-120, dolphin.py:159-225, ppsspp.py:6-72 |
//! | [`azahar_section`] | overwrite, widened key charset | azahar.py:55-121 |
//! | [`eden_annotated_section`] | overwrite + `key\default=false` | eden.py:111-203 |
//! | [`rpcs3_gui_section`] | overwrite, annotations optional | rpcs3.py:172-271 |
//! | [`yaml_add_only_section`] | add-only | rpcs3.py:113-169 |
//! | [`toml_add_only_section`] | add-only | xemu.py:184-240 |
//! | [`flat_cfg`] | overwrite, no sections | retroarch.py:301-350 |
//! | [`append_block_if_absent`] | append-if-absent | dolphin.py:390, cemu.py:343 |
//!
//! Two normalizations are shared by every section family and are load-bearing
//! for callers that compare before/after text: the input is split with
//! `str::lines()` (Python's `splitlines()`, so a `\r\n` file comes back `\n`)
//! and the output is always `lines.join("\n").trim_end() + "\n"`
//! (pcsx2.py:122), which drops every trailing blank line and trailing space.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

/// Desired section values in Python dict insertion order. Never a hash map:
/// flush order and append order are observable in the written file.
///
/// A key repeated in one `Desired` is a caller bug; the first pair wins for
/// lookups and every pair is visited when a whole section is appended.
pub type Desired = Vec<(String, String)>;

/// `desired![("Key", "value"), ...]` — builds a [`Desired`] from `&str` pairs.
#[macro_export]
macro_rules! desired {
    ($(($key:expr, $value:expr)),* $(,)?) => {{
        let __desired: $crate::autoconfig::writers::Desired = ::std::vec![$(
            (::std::string::String::from($key), ::std::string::String::from($value))
        ),*];
        __desired
    }};
}

/// `^\[(.+?)\]\s*$` applied to the TRIMMED line (pcsx2.py:83).
static SECTION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[(.+?)\]\s*$").unwrap());
/// The key regex used by every INI family except Azahar (pcsx2.py:95).
static NARROW_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z0-9_]+)\s*=").unwrap());
/// Azahar widens the charset with `%` and `\` so it can manage keys like
/// `Shortcuts\Main%20Window\Fullscreen\KeySeq` (azahar.py:94).
static AZAHAR_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z0-9_%\\]+)\s*=").unwrap());
/// The `key\default=` annotation line Qt's QSettings writes (eden.py:155).
static ANNOTATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z0-9_]+)\\default\s*=").unwrap());
/// TOML allows `-` in a bare key (xemu.py:223).
static TOML_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z0-9_\-]+)\s*=").unwrap());
/// A top-level YAML mapping key, applied to the RAW line (rpcs3.py:140).
static YAML_SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Za-z][^:\n]*):[ \t]*$").unwrap());
/// A YAML key nested exactly one level (rpcs3.py:152).
static YAML_KEY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^  ([^:]+):").unwrap());

/// How an INI-family writer treats `key\default=` annotation lines.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Annotations {
    /// Annotations are not a concept: the annotation regex is never applied
    /// (pcsx2/duckstation/dolphin/ppsspp/azahar).
    Off,
    /// Every managed key gets a canonical `key\default=false` line before it
    /// (eden.py:179, rpcs3.py:246 with `annotate=True`).
    Emit,
    /// Every managed key's annotation line is deleted (rpcs3.py:223 with
    /// `annotate=False`).
    Strip,
}

/// The first value in `desired` recorded under `key`, compared
/// case-sensitively (Python dict lookup).
fn desired_get<'a>(desired: &'a Desired, key: &str) -> Option<&'a str> {
    desired
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// `"\n".join(lines).rstrip() + "\n"` (pcsx2.py:122).
fn normalize(lines: Vec<String>) -> String {
    format!("{}\n", lines.join("\n").trim_end())
}

/// Append a blank separator line before a section that is about to be
/// appended, unless the file already ends in a blank line (pcsx2.py:114).
fn push_separator(out: &mut Vec<String>) {
    if out.last().is_some_and(|line| !line.trim().is_empty()) {
        out.push(String::new());
    }
}

/// Emit every desired key not yet written in the target section, in order
/// (pcsx2.py:72, eden.py:129, rpcs3.py:193).
fn flush_missing_keys(
    desired: &Desired,
    annotations: Annotations,
    out: &mut Vec<String>,
    seen_keys: &mut HashSet<String>,
    seen_annotations: &mut HashSet<String>,
    changed: &mut bool,
) {
    for (key, value) in desired {
        if seen_keys.contains(key) {
            continue;
        }
        if annotations == Annotations::Emit && !seen_annotations.contains(key) {
            out.push(format!("{key}\\default=false"));
            seen_annotations.insert(key.clone());
        }
        out.push(format_value(annotations, key, value));
        seen_keys.insert(key.clone());
        *changed = true;
    }
}

/// `key = value`, or `key=value` when annotations are being stripped —
/// RPCS3's `_fmt` writes the unspaced form whenever `annotate` is false
/// (rpcs3.py:190).
fn format_value(annotations: Annotations, key: &str, value: &str) -> String {
    if annotations == Annotations::Strip {
        format!("{key}={value}")
    } else {
        format!("{key} = {value}")
    }
}

/// The overwrite-policy core shared by the four INI families. `key_re` picks
/// the key charset and `annotations` picks the `key\default=` behavior;
/// everything else is identical across pcsx2.py, azahar.py, eden.py and
/// rpcs3.py, which are line-for-line copies of each other.
fn ini_core(
    raw: &str,
    section: &str,
    desired: &Desired,
    key_re: &Regex,
    annotations: Annotations,
) -> (String, bool) {
    if desired.is_empty() {
        return (raw.to_string(), false);
    }

    let target = section.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    let mut changed = false;
    let mut in_target = false;
    let mut section_found = false;
    let mut seen_keys: HashSet<String> = HashSet::new();
    let mut seen_annotations: HashSet<String> = HashSet::new();

    for raw_line in raw.lines() {
        let stripped = raw_line.trim();

        if let Some(caps) = SECTION_RE.captures(stripped) {
            if in_target {
                flush_missing_keys(
                    desired,
                    annotations,
                    &mut out,
                    &mut seen_keys,
                    &mut seen_annotations,
                    &mut changed,
                );
            }
            in_target = caps[1].trim().to_lowercase() == target;
            if in_target {
                section_found = true;
            }
            out.push(raw_line.to_string());
            continue;
        }

        if in_target {
            if annotations != Annotations::Off {
                if let Some(caps) = ANNOTATION_RE.captures(raw_line) {
                    let key = caps[1].to_string();
                    if desired_get(desired, &key).is_some() {
                        // An annotation for a managed key: rewritten,
                        // deduplicated, or deleted outright.
                        if annotations == Annotations::Strip || seen_annotations.contains(&key) {
                            changed = true;
                            continue;
                        }
                        let replacement = format!("{key}\\default=false");
                        if stripped != replacement {
                            changed = true;
                        }
                        out.push(replacement);
                        seen_annotations.insert(key);
                        continue;
                    }
                    // An annotation for an unmanaged key passes through, and
                    // the line is consumed so it cannot reach the key branch.
                    out.push(raw_line.to_string());
                    continue;
                }
            }

            if let Some(caps) = key_re.captures(raw_line) {
                let key = caps[1].to_string();
                if let Some(value) = desired_get(desired, &key) {
                    if seen_keys.contains(&key) {
                        // A second occurrence of a managed key is deleted.
                        changed = true;
                        continue;
                    }
                    if annotations == Annotations::Emit && !seen_annotations.contains(&key) {
                        out.push(format!("{key}\\default=false"));
                        seen_annotations.insert(key.clone());
                        changed = true;
                    }
                    let replacement = format_value(annotations, &key, value);
                    if stripped != replacement {
                        changed = true;
                    }
                    out.push(replacement);
                    seen_keys.insert(key);
                    continue;
                }
            }
        }

        out.push(raw_line.to_string());
    }

    if in_target {
        flush_missing_keys(
            desired,
            annotations,
            &mut out,
            &mut seen_keys,
            &mut seen_annotations,
            &mut changed,
        );
    }

    if !section_found {
        push_separator(&mut out);
        out.push(format!("[{section}]"));
        for (key, value) in desired {
            if annotations == Annotations::Emit {
                out.push(format!("{key}\\default=false"));
            }
            out.push(format_value(annotations, key, value));
        }
        changed = true;
    }

    (normalize(out), changed)
}

/// Overwrite policy, narrow key charset `[A-Za-z0-9_]`.
/// pcsx2.py:56-122, duckstation.py:54-120, dolphin.py:159-225, ppsspp.py:6-72.
///
/// The section name is compared case-insensitively, keys case-sensitively.
/// An existing managed key's line becomes `key = value`, a second occurrence
/// of it is deleted, unmanaged keys and comments pass through verbatim, and
/// missing keys are flushed at the end of the section (or at the next
/// `[Header]`). "Do not clobber the user" is implemented one level up, by
/// leaving a key out of `desired` — see [`section_has_key`].
pub fn ini_overwrite_section(raw: &str, section: &str, desired: &Desired) -> (String, bool) {
    ini_core(raw, section, desired, &NARROW_KEY_RE, Annotations::Off)
}

/// Overwrite policy with the WIDENED key charset `[A-Za-z0-9_%\]`
/// (azahar.py:94) — `%` and `\` are what let it manage
/// `Shortcuts\Main%20Window\Fullscreen\KeySeq`. Identical to
/// [`ini_overwrite_section`] otherwise.
pub fn azahar_section(raw: &str, section: &str, desired: &Desired) -> (String, bool) {
    ini_core(raw, section, desired, &AZAHAR_KEY_RE, Annotations::Off)
}

/// Overwrite policy plus the generated `key\default=false` annotation lines
/// Eden's QSettings format expects (eden.py:111-203).
///
/// A managed key with no annotation yet gets one emitted immediately before
/// it; an existing annotation is rewritten to the canonical no-space form; a
/// duplicate annotation is dropped; an annotation for an unmanaged key
/// passes through untouched.
pub fn eden_annotated_section(raw: &str, section: &str, desired: &Desired) -> (String, bool) {
    ini_core(raw, section, desired, &NARROW_KEY_RE, Annotations::Emit)
}

/// Overwrite policy with annotation handling driven by `annotate`
/// (rpcs3.py:172-271).
///
/// `annotate = true` behaves exactly like [`eden_annotated_section`].
/// `annotate = false` DELETES every managed `key\default=` line and writes
/// `key=value` with no spaces — the form RPCS3 uses for its non-GUI
/// settings blocks.
pub fn rpcs3_gui_section(
    raw: &str,
    section: &str,
    desired: &Desired,
    annotate: bool,
) -> (String, bool) {
    let annotations = if annotate {
        Annotations::Emit
    } else {
        Annotations::Strip
    };
    ini_core(raw, section, desired, &NARROW_KEY_RE, annotations)
}

/// Add-only, 2-space-indented YAML sections (rpcs3.py:113-169). An existing
/// key is recorded as seen and its line is emitted verbatim; only missing
/// keys are appended, unquoted, as `  key: value`.
///
/// Section names are compared with `trim()` and CASE-SENSITIVELY — the only
/// case-sensitive section compare in this module. An absent section is
/// appended using the UNTRIMMED `section` argument.
///
/// Deviation from Python: rpcs3.py:154 records `group(1).strip()`, so a
/// 4-space (nested) key whose trimmed name matches a desired key suppresses
/// that key. Here the captured group is compared untrimmed, so only a key at
/// exactly two spaces of indent counts as seen. No current call site
/// ("Start games in fullscreen mode", "Master Volume") writes either form
/// with stray spaces, so the two agree on every real config.yml.
pub fn yaml_add_only_section(raw: &str, section: &str, desired: &Desired) -> (String, bool) {
    if desired.is_empty() {
        return (raw.to_string(), false);
    }

    let target = section.trim();
    let mut out: Vec<String> = Vec::new();
    let mut changed = false;
    let mut in_target = false;
    let mut section_found = false;
    let mut seen_keys: HashSet<String> = HashSet::new();

    let flush = |out: &mut Vec<String>, seen_keys: &mut HashSet<String>, changed: &mut bool| {
        for (key, value) in desired {
            if seen_keys.contains(key) {
                continue;
            }
            out.push(format!("  {key}: {value}"));
            seen_keys.insert(key.clone());
            *changed = true;
        }
    };

    for raw_line in raw.lines() {
        if let Some(caps) = YAML_SECTION_RE.captures(raw_line) {
            if in_target {
                flush(&mut out, &mut seen_keys, &mut changed);
            }
            in_target = caps[1].trim() == target;
            if in_target {
                section_found = true;
            }
            out.push(raw_line.to_string());
            continue;
        }

        if in_target {
            if let Some(caps) = YAML_KEY_RE.captures(raw_line) {
                seen_keys.insert(caps[1].to_string());
            }
        }

        out.push(raw_line.to_string());
    }

    if in_target {
        flush(&mut out, &mut seen_keys, &mut changed);
    }

    if !section_found {
        push_separator(&mut out);
        out.push(format!("{section}:"));
        for (key, value) in desired {
            out.push(format!("  {key}: {value}"));
        }
        changed = true;
    }

    (normalize(out), changed)
}

/// Add-only TOML sections (xemu.py:184-240). The key charset allows `-`, and
/// EVERY matched key is recorded as seen — managed or not, which differs
/// from every other family here but is harmless under an add-only policy.
///
/// Dotted section names like `display.window` are matched as literal whole
/// strings and are never resolved as a path into a `[display]` table.
pub fn toml_add_only_section(raw: &str, section: &str, desired: &Desired) -> (String, bool) {
    if desired.is_empty() {
        return (raw.to_string(), false);
    }

    let target = section.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    let mut changed = false;
    let mut in_target = false;
    let mut section_found = false;
    let mut seen_keys: HashSet<String> = HashSet::new();

    let flush = |out: &mut Vec<String>, seen_keys: &mut HashSet<String>, changed: &mut bool| {
        for (key, value) in desired {
            if seen_keys.contains(key) {
                continue;
            }
            out.push(format!("{key} = {value}"));
            seen_keys.insert(key.clone());
            *changed = true;
        }
    };

    for raw_line in raw.lines() {
        let stripped = raw_line.trim();

        if let Some(caps) = SECTION_RE.captures(stripped) {
            if in_target {
                flush(&mut out, &mut seen_keys, &mut changed);
            }
            in_target = caps[1].trim().to_lowercase() == target;
            if in_target {
                section_found = true;
            }
            out.push(raw_line.to_string());
            continue;
        }

        if in_target {
            if let Some(caps) = TOML_KEY_RE.captures(raw_line) {
                seen_keys.insert(caps[1].to_string());
            }
        }

        out.push(raw_line.to_string());
    }

    if in_target {
        flush(&mut out, &mut seen_keys, &mut changed);
    }

    if !section_found {
        push_separator(&mut out);
        out.push(format!("[{section}]"));
        for (key, value) in desired {
            out.push(format!("{key} = {value}"));
        }
        changed = true;
    }

    (normalize(out), changed)
}

/// The flat `key = "value"` writer RetroArch's sectionless config needs
/// (retroarch.py:301-350).
///
/// Unmatched lines and unmanaged keys pass through verbatim; a duplicate
/// managed key is dropped and marks `changed`; remaining desired keys are
/// appended in order. Values are double-quoted here, unlike every INI
/// family.
///
/// `preserve_if_present` names keys whose existing line is kept byte-for-byte
/// and NOT counted as a change — only `audio_volume` today, so a user's own
/// volume survives (retroarch.py:326). The key still counts as seen, so the
/// append phase skips it.
///
/// `changed` starts false: retroarch.py seeds it with "the file did not
/// exist", which is the caller's knowledge, not this function's.
pub fn flat_cfg(raw: &str, desired: &Desired, preserve_if_present: &[&str]) -> (String, bool) {
    let mut out: Vec<String> = Vec::new();
    let mut changed = false;
    let mut seen_keys: HashSet<String> = HashSet::new();

    for raw_line in raw.lines() {
        let Some(caps) = NARROW_KEY_RE.captures(raw_line) else {
            out.push(raw_line.to_string());
            continue;
        };

        let key = caps[1].to_string();
        let Some(value) = desired_get(desired, &key) else {
            out.push(raw_line.to_string());
            continue;
        };

        if seen_keys.contains(&key) {
            changed = true;
            continue;
        }

        if preserve_if_present.contains(&key.as_str()) {
            out.push(raw_line.to_string());
            seen_keys.insert(key);
            continue;
        }

        let replacement = format!("{key} = \"{value}\"");
        if raw_line.trim() != replacement {
            changed = true;
        }
        out.push(replacement);
        seen_keys.insert(key);
    }

    for (key, value) in desired {
        if seen_keys.contains(key) {
            continue;
        }
        out.push(format!("{key} = \"{value}\""));
        seen_keys.insert(key.clone());
        changed = true;
    }

    (normalize(out), changed)
}

/// Append a whole block only when `marker` does not already match
/// (dolphin.py:390's `[GCPad1]` probe, cemu.py:343's controller profile).
///
/// `marker` is a pre-built regex — the Python probes are case-insensitive and
/// multiline. A missing trailing newline on `raw` is supplied before the
/// block; there is no blank separator line and no trailing-whitespace
/// normalization, so `block` lands byte-for-byte.
pub fn append_block_if_absent(raw: &str, marker: &Regex, block: &str) -> (String, bool) {
    if marker.is_match(raw) {
        return (raw.to_string(), false);
    }
    let mut out = raw.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(block);
    (out, true)
}

/// Case-INSENSITIVE probe for a key inside a section, narrow key charset
/// (pcsx2.py:125-143, duckstation.py:123-141).
///
/// This is how the modules avoid clobbering a user's own value: a key the
/// probe reports is left out of the `desired` map entirely. `in_target` is
/// reassigned at every header, so the probe stops at the next section.
pub fn section_has_key(raw: &str, section: &str, key: &str) -> bool {
    let target_section = section.to_lowercase();
    let target_key = key.to_lowercase();
    let mut in_target = false;

    for raw_line in raw.lines() {
        let stripped = raw_line.trim();
        if let Some(caps) = SECTION_RE.captures(stripped) {
            in_target = caps[1].trim().to_lowercase() == target_section;
            continue;
        }
        if !in_target {
            continue;
        }
        if let Some(caps) = NARROW_KEY_RE.captures(raw_line) {
            if caps[1].to_lowercase() == target_key {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ini_overwrite_section (pcsx2 / duckstation / dolphin / ppsspp) ------

    #[test]
    fn ini_overwrite_replaces_existing_key_and_reports_changed() {
        let (out, changed) =
            ini_overwrite_section("[Sec]\nKey = old\n", "Sec", &desired![("Key", "new")]);
        assert_eq!(out, "[Sec]\nKey = new\n");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_reports_unchanged_when_text_already_matches() {
        let want = desired![("Key", "new"), ("Extra", "2")];
        let (first, first_changed) = ini_overwrite_section("[Sec]\nKey = old\n", "Sec", &want);
        assert!(first_changed);
        let (second, second_changed) = ini_overwrite_section(&first, "Sec", &want);
        assert_eq!(second, first);
        assert!(!second_changed, "a second pass must be a no-op");
    }

    #[test]
    fn ini_overwrite_deletes_duplicate_managed_key_in_section() {
        let (out, changed) = ini_overwrite_section(
            "[Sec]\nKey = old\nKey = other\n",
            "Sec",
            &desired![("Key", "new")],
        );
        assert_eq!(out, "[Sec]\nKey = new\n");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_leaves_unmanaged_keys_and_comments_verbatim() {
        let (out, changed) = ini_overwrite_section(
            "[Sec]\n; a comment\nOther=1\nKey = old\n",
            "Sec",
            &desired![("Key", "new")],
        );
        assert_eq!(out, "[Sec]\n; a comment\nOther=1\nKey = new\n");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_flushes_missing_keys_before_next_section_header() {
        let (out, changed) = ini_overwrite_section(
            "[Sec]\nKey = old\n[Next]\nZ = 1\n",
            "Sec",
            &desired![("Key", "new"), ("Extra", "2")],
        );
        assert_eq!(out, "[Sec]\nKey = new\nExtra = 2\n[Next]\nZ = 1\n");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_flushes_missing_keys_at_eof() {
        let (out, changed) = ini_overwrite_section(
            "[Sec]\nKey = old\n",
            "Sec",
            &desired![("Key", "new"), ("Extra", "2")],
        );
        assert_eq!(out, "[Sec]\nKey = new\nExtra = 2\n");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_appends_absent_section_with_one_blank_separator() {
        let (out, changed) =
            ini_overwrite_section("[Other]\nX = 1\n", "Sec", &desired![("Key", "new")]);
        assert_eq!(out, "[Other]\nX = 1\n\n[Sec]\nKey = new\n");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_appends_absent_section_without_separator_after_blank_line() {
        let (out, changed) =
            ini_overwrite_section("[Other]\nX = 1\n\n", "Sec", &desired![("Key", "new")]);
        assert_eq!(out, "[Other]\nX = 1\n\n[Sec]\nKey = new\n");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_matches_section_case_insensitively() {
        let (out, changed) =
            ini_overwrite_section("[ui]\nKey = old\n", "UI", &desired![("Key", "new")]);
        assert_eq!(out, "[ui]\nKey = new\n", "the header keeps its own case");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_matches_keys_case_sensitively() {
        let (out, changed) = ini_overwrite_section(
            "[Sec]\nstartfullscreen = x\n",
            "Sec",
            &desired![("StartFullscreen", "true")],
        );
        assert_eq!(out, "[Sec]\nstartfullscreen = x\nStartFullscreen = true\n");
        assert!(changed);
    }

    #[test]
    fn ini_overwrite_empty_desired_returns_input_verbatim() {
        let raw = "[Sec]\nKey = old\n\n\n";
        let (out, changed) = ini_overwrite_section(raw, "Sec", &desired![]);
        assert_eq!(out, raw, "no desired keys means no normalization either");
        assert!(!changed);
    }

    #[test]
    fn ini_overwrite_normalizes_trailing_whitespace_and_crlf() {
        let (out, changed) = ini_overwrite_section(
            "[Sec]\r\nKey = old\r\n\r\n   \n",
            "Sec",
            &desired![("Key", "new")],
        );
        assert_eq!(out, "[Sec]\nKey = new\n");
        assert!(changed);
    }

    // --- azahar_section -----------------------------------------------------

    #[test]
    fn azahar_key_regex_manages_backslash_and_percent_keys() {
        let key = "Shortcuts\\Main%20Window\\Fullscreen\\KeySeq";
        let raw = format!("[UI]\n{key} = F11\n");
        let (out, changed) = azahar_section(&raw, "UI", &desired![(key, "F1")]);
        assert_eq!(out, format!("[UI]\n{key} = F1\n"));
        assert!(changed);

        let (narrow, _) = ini_overwrite_section(&raw, "UI", &desired![(key, "F1")]);
        assert_ne!(
            narrow, out,
            "the narrow key charset cannot see this key and appends a duplicate"
        );
    }

    // --- eden_annotated_section ---------------------------------------------

    #[test]
    fn eden_generates_annotation_line_before_managed_key() {
        let (out, changed) =
            eden_annotated_section("[UI]\nKey = old\n", "UI", &desired![("Key", "new")]);
        assert_eq!(out, "[UI]\nKey\\default=false\nKey = new\n");
        assert!(changed);
    }

    #[test]
    fn eden_rewrites_existing_annotation_to_canonical_no_space_form() {
        let (out, changed) = eden_annotated_section(
            "[UI]\nKey\\default = true\nKey = old\n",
            "UI",
            &desired![("Key", "new")],
        );
        assert_eq!(out, "[UI]\nKey\\default=false\nKey = new\n");
        assert!(changed);
    }

    #[test]
    fn eden_drops_duplicate_annotation_and_passes_unmanaged_annotation_through() {
        let (out, changed) = eden_annotated_section(
            "[UI]\nKey\\default=false\nKey\\default=true\nOther\\default=true\nKey = new\n",
            "UI",
            &desired![("Key", "new")],
        );
        assert_eq!(
            out,
            "[UI]\nKey\\default=false\nOther\\default=true\nKey = new\n"
        );
        assert!(changed, "dropping the duplicate annotation is a change");
    }

    // --- rpcs3_gui_section --------------------------------------------------

    #[test]
    fn rpcs3_gui_annotate_true_emits_annotation_pairs() {
        let (out, changed) = rpcs3_gui_section(
            "[main_window]\n",
            "main_window",
            &desired![("confirmationBoxExitGame", "false")],
            true,
        );
        assert_eq!(
            out,
            "[main_window]\nconfirmationBoxExitGame\\default=false\nconfirmationBoxExitGame = false\n"
        );
        assert!(changed);
    }

    #[test]
    fn rpcs3_gui_annotate_false_deletes_managed_annotation_lines() {
        let (out, changed) = rpcs3_gui_section(
            "[Meta]\nkey\\default=false\nkey=old\n",
            "Meta",
            &desired![("key", "new")],
            false,
        );
        assert_eq!(out, "[Meta]\nkey=new\n");
        assert!(changed);
    }

    #[test]
    fn rpcs3_gui_annotate_false_writes_key_equals_value_without_spaces() {
        let (out, changed) =
            rpcs3_gui_section("[Meta]\n", "Meta", &desired![("key", "new")], false);
        assert_eq!(out, "[Meta]\nkey=new\n");
        assert!(changed);
    }

    // --- yaml_add_only_section ----------------------------------------------

    #[test]
    fn yaml_add_only_keeps_existing_value() {
        let (out, changed) =
            yaml_add_only_section("Sec:\n  Key: old\n", "Sec", &desired![("Key", "new")]);
        assert_eq!(out, "Sec:\n  Key: old\n");
        assert!(!changed);
    }

    #[test]
    fn yaml_add_only_appends_missing_key_with_two_space_indent() {
        let (out, changed) = yaml_add_only_section(
            "Sec:\n  Key: old\n",
            "Sec",
            &desired![("Master Volume", "40")],
        );
        assert_eq!(out, "Sec:\n  Key: old\n  Master Volume: 40\n");
        assert!(changed);
    }

    #[test]
    fn yaml_section_compare_is_case_sensitive() {
        let (out, changed) =
            yaml_add_only_section("audio:\n  Key: old\n", "Audio", &desired![("Key", "new")]);
        assert_eq!(out, "audio:\n  Key: old\n\nAudio:\n  Key: new\n");
        assert!(changed);
    }

    #[test]
    fn yaml_key_requires_exactly_two_space_indent() {
        let (out, changed) =
            yaml_add_only_section("Sec:\n    Key: old\n", "Sec", &desired![("Key", "new")]);
        assert_eq!(out, "Sec:\n    Key: old\n  Key: new\n");
        assert!(changed);
    }

    // --- toml_add_only_section ----------------------------------------------

    #[test]
    fn toml_add_only_keeps_existing_value_and_allows_dashed_keys() {
        let (out, changed) = toml_add_only_section(
            "[sec]\nkey = old\nsome-key = 1\n",
            "sec",
            &desired![("key", "new"), ("some-key", "2")],
        );
        assert_eq!(out, "[sec]\nkey = old\nsome-key = 1\n");
        assert!(!changed);
    }

    #[test]
    fn toml_dotted_section_is_literal() {
        let (out, changed) = toml_add_only_section(
            "[display]\nx = 1\n",
            "display.window",
            &desired![("y", "2")],
        );
        assert_eq!(out, "[display]\nx = 1\n\n[display.window]\ny = 2\n");
        assert!(changed);
    }

    // --- flat_cfg (RetroArch) -----------------------------------------------

    #[test]
    fn flat_cfg_quotes_values_and_appends_missing_keys() {
        let (out, changed) = flat_cfg(
            "video_fullscreen = \"false\"\n# a comment\n",
            &desired![
                ("video_fullscreen", "true"),
                ("savefile_directory", "saves")
            ],
            &["audio_volume"],
        );
        assert_eq!(
            out,
            "video_fullscreen = \"true\"\n# a comment\nsavefile_directory = \"saves\"\n"
        );
        assert!(changed);
    }

    #[test]
    fn flat_cfg_preserves_audio_volume_line_verbatim_without_marking_changed() {
        let (out, changed) = flat_cfg(
            "audio_volume = \"3.500000\"\n",
            &desired![("audio_volume", "0.000000")],
            &["audio_volume"],
        );
        assert_eq!(out, "audio_volume = \"3.500000\"\n");
        assert!(!changed, "the user's own volume is never a change");
    }

    #[test]
    fn flat_cfg_drops_duplicate_managed_keys() {
        let (out, changed) = flat_cfg(
            "k = \"1\"\nk = \"2\"\n",
            &desired![("k", "1")],
            &["audio_volume"],
        );
        assert_eq!(out, "k = \"1\"\n");
        assert!(changed);
    }

    // --- append_block_if_absent ---------------------------------------------

    fn gcpad_marker() -> Regex {
        regex::RegexBuilder::new(r"^\[GCPad1\]")
            .case_insensitive(true)
            .multi_line(true)
            .build()
            .unwrap()
    }

    #[test]
    fn append_block_if_absent_skips_when_marker_matches_case_insensitively() {
        let raw = "[gcpad1]\nDevice = x\n";
        let (out, changed) = append_block_if_absent(raw, &gcpad_marker(), "[GCPad1]\nDevice = y\n");
        assert_eq!(out, raw);
        assert!(!changed);
    }

    #[test]
    fn append_block_if_absent_adds_newline_only_when_missing() {
        let block = "[GCPad1]\nDevice = y\n";

        let (out, changed) = append_block_if_absent("[Other]\nX = 1", &gcpad_marker(), block);
        assert_eq!(out, "[Other]\nX = 1\n[GCPad1]\nDevice = y\n");
        assert!(changed);

        let (out, changed) = append_block_if_absent("[Other]\nX = 1\n", &gcpad_marker(), block);
        assert_eq!(out, "[Other]\nX = 1\n[GCPad1]\nDevice = y\n");
        assert!(changed);

        let (out, changed) = append_block_if_absent("", &gcpad_marker(), block);
        assert_eq!(out, block, "an empty file gets no leading newline");
        assert!(changed);
    }

    // --- section_has_key ----------------------------------------------------

    #[test]
    fn section_has_key_is_case_insensitive_on_key_and_section() {
        let raw = "[UI]\nStartFullscreen = true\n[Other]\nFoo = 1\n";
        assert!(section_has_key(raw, "ui", "startfullscreen"));
        assert!(section_has_key(raw, "UI", "STARTFULLSCREEN"));
        assert!(
            !section_has_key(raw, "ui", "Foo"),
            "the target section must turn off at the next header"
        );
        assert!(!section_has_key(raw, "ui", "Missing"));
    }

    // --- the doc 05 policy table --------------------------------------------

    /// doc 05's three write policies, pinned as a table. `raw` already
    /// contains the managed key with a DIFFERENT value; only the overwrite
    /// family may change it.
    #[test]
    fn write_policy_table_matches_doc_05() {
        let ini_raw = "[Sec]\nKey = old\n";
        let (out, changed) = ini_overwrite_section(ini_raw, "Sec", &desired![("Key", "new")]);
        assert_eq!(out, "[Sec]\nKey = new\n");
        assert!(changed, "overwrite policy must rewrite an existing key");

        let yaml_raw = "Sec:\n  Key: old\n";
        let (out, changed) = yaml_add_only_section(yaml_raw, "Sec", &desired![("Key", "new")]);
        assert_eq!(out, "Sec:\n  Key: old\n");
        assert!(!changed, "add-only policy must never touch an existing key");

        let toml_raw = "[sec]\nkey = old\n";
        let (out, changed) = toml_add_only_section(toml_raw, "sec", &desired![("key", "new")]);
        assert_eq!(out, "[sec]\nkey = old\n");
        assert!(!changed, "add-only policy must never touch an existing key");

        let block_raw = "[GCPad1]\nDevice = x\n";
        let marker = regex::RegexBuilder::new(r"^\[GCPad1\]")
            .case_insensitive(true)
            .multi_line(true)
            .build()
            .unwrap();
        let (out, changed) = append_block_if_absent(block_raw, &marker, "[GCPad1]\nDevice = y\n");
        assert_eq!(out, block_raw);
        assert!(
            !changed,
            "append-if-absent must not append when the marker exists"
        );
    }
}
