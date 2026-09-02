//! Emulator source metadata: normalization and pure release/asset selection.
//! Ports `grid_launcher/emulator/source.py`
//! (`normalize_emulator_source_metadata`, `_extract_releases`,
//! `_select_github_release`, `_select_github_asset`) and the
//! `platform_overrides` merge step in
//! `grid_launcher/background/workers.py:165-175`. See
//! `docs/porting/04-emulator-launch.md` §12. No network I/O lives here —
//! that is `launch/forge.rs`'s job (a later task).

use serde_json::{Map, Value};

/// This build's platform string, matching the values Python's
/// `sys.platform` takes on the platforms GRID targets — the string
/// `merge_platform_override` compares `platform_overrides` keys against as
/// a prefix relation (workers.py:172).
#[cfg(target_os = "linux")]
pub const HOST_PLATFORM: &str = "linux";
#[cfg(target_os = "windows")]
pub const HOST_PLATFORM: &str = "win32";
#[cfg(target_os = "macos")]
pub const HOST_PLATFORM: &str = "darwin";

/// An error raised while normalizing source metadata or selecting a release
/// or asset (`EmulatorSourceResolutionError`, source.py:7). Every message is
/// ported byte-for-byte from the Python reference.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SourceError(pub String);

/// Normalized source metadata: a JSON object map, exactly like the Python
/// dict `normalize_emulator_source_metadata` returns, so
/// [`merge_platform_override`] can shallow-merge an override object over it
/// without a typed re-parse. Release and asset objects use the same alias.
pub type SourceMap = Map<String, Value>;

/// `map.get(key)`, trimmed — `""` when the key is missing or its value is
/// not a JSON string. The accessor every call site here uses to read a
/// string field, matching the many `str(x.get(k, "")).strip()` call sites
/// in source.py (a real GitHub/Gitea API response never puts a non-string
/// in these fields, so this is behaviorally equivalent there).
pub fn str_field(map: &SourceMap, key: &str) -> String {
    match map.get(key) {
        Some(Value::String(s)) => s.trim().to_string(),
        _ => String::new(),
    }
}

/// `value`, trimmed, when it is a non-blank JSON string — `None` otherwise
/// (including when `value` is `None`). The shared core of
/// `_normalized_optional_string` and `_normalized_required_string`
/// (source.py:171-205): both are "is this a usable string", just with
/// different fallback and error handling wrapped around it.
fn trimmed_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        _ => None,
    }
}

/// `metadata.get(key)`, falling back to `metadata.get(fallback_key)` only
/// when `key` itself is absent — matching `dict.get(key, dict.get(...))`,
/// where an explicit `null` at `key` is NOT "absent" and short-circuits the
/// fallback.
fn get_with_fallback<'a>(
    map: &'a Map<String, Value>,
    key: &str,
    fallback_key: &str,
) -> Option<&'a Value> {
    map.get(key).or_else(|| map.get(fallback_key))
}

/// `_normalized_optional_string` (source.py:171-186): the first non-blank
/// string among `key` and `fallback_keys`, in order, else `""`.
fn optional_string(map: &Map<String, Value>, key: &str, fallback_keys: &[&str]) -> String {
    if let Some(v) = trimmed_string(map.get(key)) {
        return v;
    }
    for fallback_key in fallback_keys {
        if let Some(v) = trimmed_string(map.get(*fallback_key)) {
            return v;
        }
    }
    String::new()
}

/// `_normalized_required_string` (source.py:189-205): `key`'s trimmed
/// string value, or `fallback_key`'s when `key` is missing, blank, or not a
/// string. Errors, quoting `fallback_key` in the message when given, when
/// neither yields a usable string.
fn required_string(
    map: &Map<String, Value>,
    key: &str,
    fallback_key: Option<&str>,
) -> Result<String, SourceError> {
    let mut value = trimmed_string(map.get(key));
    if value.is_none() {
        if let Some(fallback_key) = fallback_key {
            value = trimmed_string(map.get(fallback_key));
        }
    }
    value.ok_or_else(|| {
        let fallback_note = fallback_key
            .map(|k| format!(" (or '{k}')"))
            .unwrap_or_default();
        SourceError(format!(
            "Source metadata is missing required field '{key}'{fallback_note}."
        ))
    })
}

/// `_normalized_patterns` (source.py:208-220): `None`/JSON `null` become
/// `default`; a non-blank string becomes a one-element list; an array keeps
/// its trimmed non-blank string elements, or `default` when that leaves
/// none; anything else becomes `default`.
fn normalize_patterns(value: Option<&Value>, default: &[&str]) -> Vec<String> {
    let default_vec = || default.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match value {
        None | Some(Value::Null) => default_vec(),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                default_vec()
            } else {
                vec![trimmed.to_string()]
            }
        }
        Some(Value::Array(items)) => {
            let patterns: Vec<String> = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(s) => {
                        let trimmed = s.trim();
                        (!trimmed.is_empty()).then(|| trimmed.to_string())
                    }
                    _ => None,
                })
                .collect();
            if patterns.is_empty() {
                default_vec()
            } else {
                patterns
            }
        }
        Some(_) => default_vec(),
    }
}

/// Python truthiness of an arbitrary JSON value: `null`/`false` are falsy,
/// a number is falsy only when zero, a string/array/object is falsy only
/// when empty.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_none_or(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn string_array(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::String).collect())
}

/// Normalizes raw emulator source metadata
/// (`normalize_emulator_source_metadata`, source.py:59-168).
pub fn normalize_source(raw: &Value) -> Result<SourceMap, SourceError> {
    let obj = raw
        .as_object()
        .ok_or_else(|| SourceError("Source metadata must be a dictionary.".to_string()))?;

    let provider_value = obj
        .get("provider")
        .cloned()
        .or_else(|| obj.get("type").cloned())
        .unwrap_or_else(|| Value::String("github".to_string()));
    let provider = match &provider_value {
        Value::String(s) => s.trim().to_lowercase(),
        _ => String::new(),
    };
    let normalized_provider = match provider.as_str() {
        "github" | "github-release" | "github_release" | "githubrelease" => "github".to_string(),
        "gitea" | "gitea-release" | "gitea_release" => "gitea".to_string(),
        "direct" | "direct-download" | "direct_download" | "download" | "url" => {
            "direct".to_string()
        }
        other => other.to_string(),
    };
    if normalized_provider.is_empty() {
        return Err(SourceError(
            "Source metadata is missing provider.".to_string(),
        ));
    }

    let owner = required_string(obj, "owner", None)?;
    let repo = required_string(obj, "repo", Some("repository"))?;

    let include_patterns = normalize_patterns(
        get_with_fallback(obj, "asset_patterns", "asset_globs"),
        &["*"],
    );
    let exclude_patterns = normalize_patterns(
        get_with_fallback(obj, "asset_exclude_patterns", "exclude_asset_patterns"),
        &[],
    );
    let preferred_patterns = normalize_patterns(
        get_with_fallback(obj, "asset_preferred_patterns", "preferred_asset_patterns"),
        &[],
    );

    let mut release_tag = String::new();
    for key in ["tag", "release_tag", "version"] {
        if let Some(Value::String(s)) = obj.get(key) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                release_tag = trimmed.to_string();
                break;
            }
        }
    }

    let allow_prerelease = truthy(obj.get("allow_prerelease"));

    let mut normalized: SourceMap = Map::new();
    normalized.insert(
        "provider".to_string(),
        Value::String(normalized_provider.clone()),
    );
    normalized.insert("owner".to_string(), Value::String(owner));
    normalized.insert("repo".to_string(), Value::String(repo));
    normalized.insert("release_tag".to_string(), Value::String(release_tag));
    normalized.insert(
        "allow_prerelease".to_string(),
        Value::Bool(allow_prerelease),
    );
    normalized.insert(
        "asset_patterns".to_string(),
        string_array(&include_patterns),
    );
    normalized.insert(
        "asset_exclude_patterns".to_string(),
        string_array(&exclude_patterns),
    );
    normalized.insert(
        "asset_preferred_patterns".to_string(),
        string_array(&preferred_patterns),
    );

    if normalized_provider == "github" || normalized_provider == "gitea" {
        if let Some(Value::Object(overrides)) = obj.get("platform_overrides") {
            if !overrides.is_empty() {
                normalized.insert(
                    "platform_overrides".to_string(),
                    Value::Object(overrides.clone()),
                );
            }
        }
    }

    if normalized_provider == "gitea" {
        let base_url = required_string(obj, "base_url", None)?;
        normalized.insert(
            "base_url".to_string(),
            Value::String(base_url.trim_end_matches('/').to_string()),
        );
    }

    if normalized_provider == "direct" {
        let download_url = optional_string(obj, "download_url", &["url", "browser_download_url"]);
        let page_url = optional_string(obj, "page_url", &["index_url", "listing_url"]);
        let download_url_regex =
            optional_string(obj, "download_url_regex", &["url_regex", "asset_url_regex"]);
        let asset_name = optional_string(obj, "asset_name", &[]);
        if download_url.is_empty() && page_url.is_empty() {
            return Err(SourceError(
                "Direct source metadata must include either 'download_url' or 'page_url'."
                    .to_string(),
            ));
        }
        normalized.insert("download_url".to_string(), Value::String(download_url));
        normalized.insert("page_url".to_string(), Value::String(page_url));
        normalized.insert(
            "download_url_regex".to_string(),
            Value::String(download_url_regex),
        );
        normalized.insert("asset_name".to_string(), Value::String(asset_name));

        let supplemental_value = obj
            .get("supplemental_downloads")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        if let Value::Array(items) = supplemental_value {
            let filtered: Vec<Value> = items.into_iter().filter(Value::is_object).collect();
            normalized.insert("supplemental_downloads".to_string(), Value::Array(filtered));
        }

        if let Some(Value::Object(overrides)) = obj.get("platform_overrides") {
            if !overrides.is_empty() {
                normalized.insert(
                    "platform_overrides".to_string(),
                    Value::Object(overrides.clone()),
                );
            }
        }
    }

    Ok(normalized)
}

/// Applies the first `platform_overrides` entry (in JSON object order)
/// whose key is a prefix of [`HOST_PLATFORM`] and whose value is a JSON
/// object, shallow-merging it over `source` in place
/// (`_resolve_source_download`, workers.py:167-174). A no-op when
/// `platform_overrides` is absent, not an object, or has no matching entry.
pub fn merge_platform_override(source: &mut SourceMap) {
    let matched = match source.get("platform_overrides") {
        Some(Value::Object(overrides)) => overrides.iter().find_map(|(key, value)| match value {
            Value::Object(override_map) if HOST_PLATFORM.starts_with(key.as_str()) => {
                Some(override_map.clone())
            }
            _ => None,
        }),
        _ => None,
    };
    if let Some(override_map) = matched {
        for (key, value) in override_map {
            source.insert(key, value);
        }
    }
}

/// `_extract_releases` (source.py:223-240): an array's object elements; a
/// single release object recognized by an `assets` array; or an object with
/// a `releases` array, unwrapped the same way. Anything else — including an
/// object with `releases` present but not an array — errors.
fn extract_releases(releases: &Value) -> Result<Vec<&Map<String, Value>>, SourceError> {
    let shape_error = || {
        SourceError(
            "GitHub release metadata must be a release object, a list of release objects, or a dictionary with 'releases'."
                .to_string(),
        )
    };
    match releases {
        Value::Array(items) => Ok(items.iter().filter_map(Value::as_object).collect()),
        Value::Object(obj) => {
            if matches!(obj.get("assets"), Some(Value::Array(_))) {
                return Ok(vec![obj]);
            }
            match obj.get("releases") {
                Some(Value::Array(items)) => {
                    Ok(items.iter().filter_map(Value::as_object).collect())
                }
                _ => Err(shape_error()),
            }
        }
        _ => Err(shape_error()),
    }
}

/// Selects the GitHub/Gitea release `source` resolves to out of `releases`
/// (`_extract_releases` + `_select_github_release`, source.py:223-292).
pub fn select_release<'a>(
    source: &SourceMap,
    releases: &'a Value,
) -> Result<&'a Map<String, Value>, SourceError> {
    let owner = str_field(source, "owner");
    let repo = str_field(source, "repo");
    let extracted = extract_releases(releases)?;
    if extracted.is_empty() {
        return Err(SourceError(format!(
            "No GitHub releases were provided for '{owner}/{repo}'."
        )));
    }

    let mut release_tag = str_field(source, "release_tag");
    if release_tag.to_lowercase() == "latest" {
        release_tag = String::new();
    }
    let allow_prerelease = truthy(source.get("allow_prerelease"));

    let mut selected: Option<&Map<String, Value>> = None;
    for release in &extracted {
        if truthy(release.get("draft")) {
            continue;
        }
        if truthy(release.get("prerelease")) && !allow_prerelease {
            continue;
        }
        let candidate_tag = str_field(release, "tag_name");
        if !release_tag.is_empty() && candidate_tag.to_lowercase() != release_tag.to_lowercase() {
            continue;
        }
        selected = Some(release);
        break;
    }

    if let Some(selected) = selected {
        return Ok(selected);
    }

    let filtered_tags: Vec<String> = extracted
        .iter()
        .map(|release| str_field(release, "tag_name"))
        .filter(|tag| !tag.is_empty())
        .collect();

    if !release_tag.is_empty() {
        let available = if filtered_tags.is_empty() {
            "none".to_string()
        } else {
            filtered_tags.join(", ")
        };
        return Err(SourceError(format!(
            "No matching GitHub release was found for tag '{release_tag}' in '{owner}/{repo}'. Available tags: {available}."
        )));
    }

    Err(SourceError(format!(
        "No usable GitHub release was found for '{owner}/{repo}'. All releases were drafts or prereleases."
    )))
}

/// The index of the first pattern in `patterns` that `fnmatch_case`-matches
/// `name`, both casefolded (`_asset_pattern_index`, source.py:295-300).
fn asset_pattern_index(name: &str, patterns: &[String]) -> Option<usize> {
    let normalized_name = name.to_lowercase();
    patterns
        .iter()
        .position(|pattern| fnmatch_case(&pattern.to_lowercase(), &normalized_name))
}

fn pattern_list_field(source: &SourceMap, key: &str, default: &[&str]) -> Vec<String> {
    match source.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(|s| s.to_string()))
            .collect(),
        _ => default.iter().map(|s| s.to_string()).collect(),
    }
}

/// Python's `repr()` of a list of strings — `['a', 'b']` or `[]` — used
/// verbatim inside the "no release asset matched" error message
/// (source.py:358-362).
fn py_list(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let rendered: Vec<String> = items.iter().map(|s| py_repr_str(s)).collect();
    format!("[{}]", rendered.join(", "))
}

/// Python `repr()` of a single string: single-quoted unless the string
/// contains a `'` and no `"`, in which case double-quoted; `\` and the
/// chosen quote character are backslash-escaped.
fn py_repr_str(s: &str) -> String {
    let has_single = s.contains('\'');
    let has_double = s.contains('"');
    let quote = if has_single && !has_double { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Selects the release asset `source` resolves to out of `release`'s
/// `assets` array (`_select_github_asset`, source.py:303-365).
/// Sort key for asset ranking: (include-pattern index, preferred-pattern
/// index, state penalty, casefolded name) — lower wins on each field in turn.
type AssetRank = (usize, usize, u8, String);

pub fn select_asset<'a>(
    source: &SourceMap,
    release: &'a Map<String, Value>,
) -> Result<&'a Map<String, Value>, SourceError> {
    let no_assets_error = || {
        SourceError(format!(
            "Selected GitHub release has no assets. release_tag='{}'",
            str_field(release, "tag_name")
        ))
    };
    let assets = match release.get("assets") {
        Some(Value::Array(items)) if !items.is_empty() => items,
        _ => return Err(no_assets_error()),
    };

    let include_patterns = pattern_list_field(source, "asset_patterns", &["*"]);
    let exclude_patterns = pattern_list_field(source, "asset_exclude_patterns", &[]);
    let preferred_patterns = pattern_list_field(source, "asset_preferred_patterns", &[]);

    let mut candidates: Vec<(AssetRank, &Map<String, Value>)> = Vec::new();
    let mut available_names: Vec<String> = Vec::new();

    for asset_value in assets {
        let Some(asset) = asset_value.as_object() else {
            continue;
        };
        let name = str_field(asset, "name");
        if !name.is_empty() {
            available_names.push(name.clone());
        }
        let url = str_field(asset, "browser_download_url");
        if name.is_empty() || url.is_empty() {
            continue;
        }

        let Some(include_index) = asset_pattern_index(&name, &include_patterns) else {
            continue;
        };
        if asset_pattern_index(&name, &exclude_patterns).is_some() {
            continue;
        }
        let preferred_index =
            asset_pattern_index(&name, &preferred_patterns).unwrap_or(preferred_patterns.len());

        let state = str_field(asset, "state").to_lowercase();
        let state_penalty: u8 = if state.is_empty() || state == "uploaded" {
            0
        } else {
            1
        };

        candidates.push((
            (
                include_index,
                preferred_index,
                state_penalty,
                name.to_lowercase(),
            ),
            asset,
        ));
    }

    if candidates.is_empty() {
        return Err(SourceError(format!(
            "No release asset matched configured patterns. include={}, exclude={}, available_assets={}",
            py_list(&include_patterns),
            py_list(&exclude_patterns),
            py_list(&available_names)
        )));
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(candidates[0].1)
}

// --- fnmatch_case -------------------------------------------------------------

enum Token {
    Lit(char),
    Any,
    Star,
    Class {
        negate: bool,
        singles: Vec<char>,
        ranges: Vec<(char, char)>,
    },
}

/// Parses a `[`-starting run at the front of `chars` into a character-class
/// token, fnmatch-style: an optional leading `!` negates the class; a `]`
/// immediately after that (or after `[`) is a literal member rather than
/// the closing bracket; `x-y` forms a range. Returns `None` (no closing
/// `]` found) when the whole thing should be treated as a literal `[`
/// instead.
fn try_parse_class(chars: &[char]) -> Option<(Token, usize)> {
    let n = chars.len();
    let mut i = 1usize; // chars[0] == '['
    let mut negate = false;
    if i < n && chars[i] == '!' {
        negate = true;
        i += 1;
    }
    let content_start = i;
    if i < n && chars[i] == ']' {
        i += 1;
    }
    while i < n && chars[i] != ']' {
        i += 1;
    }
    if i >= n {
        return None;
    }
    let content = &chars[content_start..i];
    let consumed = i + 1;

    let mut singles = Vec::new();
    let mut ranges = Vec::new();
    let mut k = 0;
    while k < content.len() {
        if k + 2 < content.len() && content[k + 1] == '-' {
            ranges.push((content[k], content[k + 2]));
            k += 3;
        } else {
            singles.push(content[k]);
            k += 1;
        }
    }
    Some((
        Token::Class {
            negate,
            singles,
            ranges,
        },
        consumed,
    ))
}

fn parse_tokens(pattern: &str) -> Vec<Token> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '?' => {
                tokens.push(Token::Any);
                i += 1;
            }
            '[' => {
                if let Some((token, consumed)) = try_parse_class(&chars[i..]) {
                    tokens.push(token);
                    i += consumed;
                } else {
                    tokens.push(Token::Lit('['));
                    i += 1;
                }
            }
            c => {
                tokens.push(Token::Lit(c));
                i += 1;
            }
        }
    }
    tokens
}

fn token_matches_char(token: &Token, c: char) -> bool {
    match token {
        Token::Lit(l) => *l == c,
        Token::Any => true,
        Token::Star => false,
        Token::Class {
            negate,
            singles,
            ranges,
        } => {
            let hit = singles.contains(&c) || ranges.iter().any(|(a, b)| *a <= c && c <= *b);
            if *negate {
                !hit
            } else {
                hit
            }
        }
    }
}

/// The classic greedy/backtracking glob matcher, generalized from two
/// wildcards (`*`, `?`) to arbitrary single-character-matching tokens so it
/// also covers `[seq]`/`[!seq]` classes.
fn tokens_match(tokens: &[Token], text: &[char]) -> bool {
    let (mut ti, mut xi) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut star_xi = 0usize;

    while xi < text.len() {
        if ti < tokens.len() && matches!(tokens[ti], Token::Star) {
            star = Some(ti);
            star_xi = xi;
            ti += 1;
        } else if ti < tokens.len() && token_matches_char(&tokens[ti], text[xi]) {
            ti += 1;
            xi += 1;
        } else if let Some(star_ti) = star {
            ti = star_ti + 1;
            star_xi += 1;
            xi = star_xi;
        } else {
            return false;
        }
    }
    while ti < tokens.len() && matches!(tokens[ti], Token::Star) {
        ti += 1;
    }
    ti == tokens.len()
}

/// `fnmatch.fnmatchcase(text, pattern)` semantics: `*` matches any run of
/// characters (including none), `?` matches exactly one, `[seq]`/`[!seq]`
/// are character classes (`-` forms a range; a class left unclosed by a
/// matching `]` is not a class at all — its `[` matches a literal `[`).
/// Neither side is casefolded here; every caller casefolds both first.
pub fn fnmatch_case(pattern: &str, text: &str) -> bool {
    let tokens = parse_tokens(pattern);
    let text_chars: Vec<char> = text.chars().collect();
    tokens_match(&tokens, &text_chars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const AUTOPROFILES_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../emulator-autoprofiles.json"
    ));

    fn map_of(value: Value) -> SourceMap {
        value.as_object().unwrap().clone()
    }

    // --- normalize_source: provider alias table -----------------------------

    #[test]
    fn provider_alias_table() {
        let cases: &[(&str, &str)] = &[
            ("github", "github"),
            ("github-release", "github"),
            ("github_release", "github"),
            ("githubrelease", "github"),
            ("gitea", "gitea"),
            ("gitea-release", "gitea"),
            ("gitea_release", "gitea"),
            ("direct", "direct"),
            ("direct-download", "direct"),
            ("direct_download", "direct"),
            ("download", "direct"),
            ("url", "direct"),
            ("weird", "weird"),
        ];
        for (input, expected) in cases {
            let raw = json!({
                "provider": input, "owner": "o", "repo": "r",
                "base_url": "https://x", "page_url": "https://x/"
            });
            let normalized = normalize_source(&raw).unwrap();
            assert_eq!(
                normalized.get("provider").and_then(Value::as_str),
                Some(*expected),
                "provider={input:?}"
            );
        }
    }

    #[test]
    fn provider_defaults_to_github_when_key_absent() {
        let raw = json!({"owner": "o", "repo": "r"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("provider").and_then(Value::as_str),
            Some("github")
        );
    }

    #[test]
    fn provider_falls_back_to_type_key() {
        let raw =
            json!({"type": "gitea-release", "owner": "o", "repo": "r", "base_url": "https://x"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("provider").and_then(Value::as_str),
            Some("gitea")
        );
    }

    #[test]
    fn non_string_provider_errors_missing_provider() {
        let raw = json!({"provider": 5, "owner": "o", "repo": "r"});
        let err = normalize_source(&raw).unwrap_err();
        assert_eq!(err.0, "Source metadata is missing provider.");
    }

    #[test]
    fn non_object_source_errors() {
        let raw = json!(["not", "a", "dict"]);
        let err = normalize_source(&raw).unwrap_err();
        assert_eq!(err.0, "Source metadata must be a dictionary.");
    }

    // --- normalize_source: required fields -----------------------------------

    #[test]
    fn missing_owner_errors_verbatim() {
        let raw = json!({"provider": "github", "repo": "r"});
        let err = normalize_source(&raw).unwrap_err();
        assert_eq!(err.0, "Source metadata is missing required field 'owner'.");
    }

    #[test]
    fn missing_repo_errors_verbatim_with_repository_fallback_note() {
        let raw = json!({"provider": "github", "owner": "o"});
        let err = normalize_source(&raw).unwrap_err();
        assert_eq!(
            err.0,
            "Source metadata is missing required field 'repo' (or 'repository')."
        );
    }

    #[test]
    fn repository_key_satisfies_repo_fallback() {
        let raw = json!({"provider": "github", "owner": "o", "repository": "r"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(normalized.get("repo").and_then(Value::as_str), Some("r"));
    }

    #[test]
    fn missing_base_url_errors_verbatim_for_gitea() {
        let raw = json!({"provider": "gitea", "owner": "o", "repo": "r"});
        let err = normalize_source(&raw).unwrap_err();
        assert_eq!(
            err.0,
            "Source metadata is missing required field 'base_url'."
        );
    }

    #[test]
    fn base_url_is_right_trimmed_of_slashes() {
        let raw =
            json!({"provider": "gitea", "owner": "o", "repo": "r", "base_url": "https://x///"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("base_url").and_then(Value::as_str),
            Some("https://x")
        );
    }

    // --- normalize_source: release_tag chain ---------------------------------

    #[test]
    fn release_tag_prefers_tag_over_release_tag() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r", "tag": "a", "release_tag": "b"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("release_tag").and_then(Value::as_str),
            Some("a")
        );
    }

    #[test]
    fn release_tag_falls_back_to_release_tag_then_version() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r", "release_tag": "b", "version": "c"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("release_tag").and_then(Value::as_str),
            Some("b")
        );
    }

    #[test]
    fn release_tag_falls_back_to_version_last() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r", "version": "c"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("release_tag").and_then(Value::as_str),
            Some("c")
        );
    }

    #[test]
    fn release_tag_blank_when_no_key_matches() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("release_tag").and_then(Value::as_str),
            Some("")
        );
    }

    // --- normalize_source: patterns ------------------------------------------

    #[test]
    fn include_patterns_default_to_star() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(normalized["asset_patterns"], json!(["*"]));
    }

    #[test]
    fn include_patterns_fall_back_to_asset_globs() {
        let raw =
            json!({"provider": "github", "owner": "o", "repo": "r", "asset_globs": ["a.zip"]});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(normalized["asset_patterns"], json!(["a.zip"]));
    }

    #[test]
    fn include_patterns_string_form_becomes_single_element_list() {
        let raw =
            json!({"provider": "github", "owner": "o", "repo": "r", "asset_patterns": "a.zip"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(normalized["asset_patterns"], json!(["a.zip"]));
    }

    #[test]
    fn exclude_and_preferred_patterns_default_to_empty() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(normalized["asset_exclude_patterns"], json!([]));
        assert_eq!(normalized["asset_preferred_patterns"], json!([]));
    }

    #[test]
    fn exclude_patterns_fall_back_to_exclude_asset_patterns_key() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r", "exclude_asset_patterns": ["*.sha256"]});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(normalized["asset_exclude_patterns"], json!(["*.sha256"]));
    }

    #[test]
    fn preferred_patterns_fall_back_to_preferred_asset_patterns_key() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r", "preferred_asset_patterns": ["*.AppImage"]});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized["asset_preferred_patterns"],
            json!(["*.AppImage"])
        );
    }

    #[test]
    fn blank_pattern_entries_are_dropped_and_empty_result_falls_back_to_default() {
        let raw =
            json!({"provider": "github", "owner": "o", "repo": "r", "asset_patterns": ["  ", ""]});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(normalized["asset_patterns"], json!(["*"]));
    }

    // --- normalize_source: direct provider -----------------------------------

    #[test]
    fn direct_download_url_falls_back_through_url_then_browser_download_url() {
        let raw =
            json!({"provider": "direct", "owner": "o", "repo": "r", "url": "https://x/a.zip"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("download_url").and_then(Value::as_str),
            Some("https://x/a.zip")
        );
    }

    #[test]
    fn direct_page_url_falls_back_through_index_url_then_listing_url() {
        let raw =
            json!({"provider": "direct", "owner": "o", "repo": "r", "listing_url": "https://x/"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("page_url").and_then(Value::as_str),
            Some("https://x/")
        );
    }

    #[test]
    fn direct_download_url_regex_falls_back_through_url_regex_then_asset_url_regex() {
        let raw = json!({"provider": "direct", "owner": "o", "repo": "r", "page_url": "https://x/", "asset_url_regex": "a.*\\.zip"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("download_url_regex").and_then(Value::as_str),
            Some("a.*\\.zip")
        );
    }

    #[test]
    fn direct_asset_name_has_no_fallback() {
        let raw = json!({"provider": "direct", "owner": "o", "repo": "r", "page_url": "https://x/", "asset_name": "a.zip"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized.get("asset_name").and_then(Value::as_str),
            Some("a.zip")
        );
    }

    #[test]
    fn direct_both_download_url_and_page_url_empty_errors_verbatim() {
        let raw = json!({"provider": "direct", "owner": "o", "repo": "r"});
        let err = normalize_source(&raw).unwrap_err();
        assert_eq!(
            err.0,
            "Direct source metadata must include either 'download_url' or 'page_url'."
        );
    }

    #[test]
    fn direct_supplemental_downloads_keeps_only_object_elements() {
        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r", "page_url": "https://x/",
            "supplemental_downloads": [{"a": 1}, "not a dict", 5, {"b": 2}]
        });
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(
            normalized["supplemental_downloads"],
            json!([{"a": 1}, {"b": 2}])
        );
    }

    #[test]
    fn direct_supplemental_downloads_absent_still_yields_empty_list() {
        let raw =
            json!({"provider": "direct", "owner": "o", "repo": "r", "page_url": "https://x/"});
        let normalized = normalize_source(&raw).unwrap();
        assert_eq!(normalized["supplemental_downloads"], json!([]));
    }

    // --- normalize_source: platform_overrides retention ----------------------

    #[test]
    fn platform_overrides_retained_when_non_empty_object_github() {
        let raw = json!({
            "provider": "github", "owner": "o", "repo": "r",
            "platform_overrides": {"linux": {"asset_patterns": ["x"]}}
        });
        let normalized = normalize_source(&raw).unwrap();
        assert!(normalized.contains_key("platform_overrides"));
    }

    #[test]
    fn platform_overrides_dropped_when_empty_object() {
        let raw =
            json!({"provider": "github", "owner": "o", "repo": "r", "platform_overrides": {}});
        let normalized = normalize_source(&raw).unwrap();
        assert!(!normalized.contains_key("platform_overrides"));
    }

    #[test]
    fn platform_overrides_dropped_when_not_an_object() {
        let raw = json!({"provider": "github", "owner": "o", "repo": "r", "platform_overrides": ["linux"]});
        let normalized = normalize_source(&raw).unwrap();
        assert!(!normalized.contains_key("platform_overrides"));
    }

    #[test]
    fn platform_overrides_retained_for_direct_provider_too() {
        let raw = json!({
            "provider": "direct", "owner": "o", "repo": "r", "page_url": "https://x/",
            "platform_overrides": {"linux": {"page_url": "https://y/"}}
        });
        let normalized = normalize_source(&raw).unwrap();
        assert!(normalized.contains_key("platform_overrides"));
    }

    // --- merge_platform_override ----------------------------------------------

    #[test]
    fn merge_platform_override_applies_linux_entry() {
        let raw = json!({
            "provider": "github", "owner": "o", "repo": "r",
            "platform_overrides": {"win32": {"asset_patterns": ["win.zip"]}, "linux": {"asset_patterns": ["lin.AppImage"]}}
        });
        let mut normalized = normalize_source(&raw).unwrap();
        merge_platform_override(&mut normalized);
        assert_eq!(normalized["asset_patterns"], json!(["lin.AppImage"]));
    }

    #[test]
    fn merge_platform_override_skips_win32_on_linux() {
        let raw = json!({
            "provider": "github", "owner": "o", "repo": "r",
            "platform_overrides": {"win32": {"owner": "should-not-apply"}}
        });
        let mut normalized = normalize_source(&raw).unwrap();
        merge_platform_override(&mut normalized);
        assert_eq!(normalized.get("owner").and_then(Value::as_str), Some("o"));
    }

    #[test]
    fn merge_platform_override_applies_first_matching_entry_only() {
        // Both "l" and "linux" are prefixes of HOST_PLATFORM ("linux"); the
        // entry appearing first in JSON object order wins even though a
        // later entry is a more specific match.
        let mut source = map_of(json!({
            "provider": "github", "owner": "o", "repo": "r",
            "platform_overrides": {"l": {"owner": "first"}, "linux": {"owner": "second"}}
        }));
        merge_platform_override(&mut source);
        assert_eq!(source.get("owner").and_then(Value::as_str), Some("first"));
    }

    #[test]
    fn merge_platform_override_is_a_noop_without_platform_overrides() {
        let mut source = map_of(json!({"provider": "github", "owner": "o", "repo": "r"}));
        let before = source.clone();
        merge_platform_override(&mut source);
        assert_eq!(source, before);
    }

    // --- select_release: shapes and extraction ---------------------------------

    #[test]
    fn select_release_object_with_assets_shape() {
        let source = map_of(
            json!({"owner": "o", "repo": "r", "release_tag": "", "allow_prerelease": false}),
        );
        let releases = json!({"tag_name": "v1", "assets": []});
        let release = select_release(&source, &releases).unwrap();
        assert_eq!(release.get("tag_name").and_then(Value::as_str), Some("v1"));
    }

    #[test]
    fn select_release_object_with_releases_shape() {
        let source = map_of(
            json!({"owner": "o", "repo": "r", "release_tag": "", "allow_prerelease": false}),
        );
        let releases = json!({"releases": [{"tag_name": "v1"}, {"tag_name": "v2"}]});
        let release = select_release(&source, &releases).unwrap();
        assert_eq!(release.get("tag_name").and_then(Value::as_str), Some("v1"));
    }

    #[test]
    fn select_release_invalid_shape_errors_verbatim() {
        let source = map_of(
            json!({"owner": "o", "repo": "r", "release_tag": "", "allow_prerelease": false}),
        );
        let releases = json!({"foo": "bar"});
        let err = select_release(&source, &releases).unwrap_err();
        assert_eq!(
            err.0,
            "GitHub release metadata must be a release object, a list of release objects, or a dictionary with 'releases'."
        );
    }

    #[test]
    fn select_release_empty_list_errors_verbatim() {
        let source = map_of(
            json!({"owner": "acme", "repo": "widget", "release_tag": "", "allow_prerelease": false}),
        );
        let releases = json!([]);
        let err = select_release(&source, &releases).unwrap_err();
        assert_eq!(err.0, "No GitHub releases were provided for 'acme/widget'.");
    }

    #[test]
    fn select_release_skips_drafts() {
        let source = map_of(
            json!({"owner": "o", "repo": "r", "release_tag": "", "allow_prerelease": false}),
        );
        let releases = json!([
            {"draft": true, "tag_name": "v1"},
            {"tag_name": "v2"}
        ]);
        let release = select_release(&source, &releases).unwrap();
        assert_eq!(release.get("tag_name").and_then(Value::as_str), Some("v2"));
    }

    #[test]
    fn select_release_skips_prerelease_unless_allowed() {
        let source = map_of(
            json!({"owner": "o", "repo": "r", "release_tag": "", "allow_prerelease": false}),
        );
        let releases = json!([
            {"prerelease": true, "tag_name": "v1"},
            {"tag_name": "v2"}
        ]);
        let release = select_release(&source, &releases).unwrap();
        assert_eq!(release.get("tag_name").and_then(Value::as_str), Some("v2"));

        let source_allowed =
            map_of(json!({"owner": "o", "repo": "r", "release_tag": "", "allow_prerelease": true}));
        let release_allowed = select_release(&source_allowed, &releases).unwrap();
        assert_eq!(
            release_allowed.get("tag_name").and_then(Value::as_str),
            Some("v1")
        );
    }

    #[test]
    fn select_release_matches_tag_case_insensitively() {
        let source = map_of(
            json!({"owner": "o", "repo": "r", "release_tag": "V1.0", "allow_prerelease": false}),
        );
        let releases = json!([{"tag_name": "v1.0"}]);
        let release = select_release(&source, &releases).unwrap();
        assert_eq!(
            release.get("tag_name").and_then(Value::as_str),
            Some("v1.0")
        );
    }

    #[test]
    fn select_release_latest_tag_is_treated_as_unset() {
        let source = map_of(
            json!({"owner": "o", "repo": "r", "release_tag": "latest", "allow_prerelease": false}),
        );
        let releases = json!([{"tag_name": "v1"}, {"tag_name": "v2"}]);
        let release = select_release(&source, &releases).unwrap();
        assert_eq!(release.get("tag_name").and_then(Value::as_str), Some("v1"));
    }

    #[test]
    fn select_release_first_in_order_wins_when_no_tag_set() {
        let source = map_of(
            json!({"owner": "o", "repo": "r", "release_tag": "", "allow_prerelease": false}),
        );
        let releases = json!([{"tag_name": "v2"}, {"tag_name": "v1"}]);
        let release = select_release(&source, &releases).unwrap();
        assert_eq!(release.get("tag_name").and_then(Value::as_str), Some("v2"));
    }

    #[test]
    fn select_release_no_match_for_tag_lists_available_tags() {
        let source = map_of(
            json!({"owner": "acme", "repo": "widget", "release_tag": "vX", "allow_prerelease": false}),
        );
        let releases = json!([{"tag_name": "v1"}, {"tag_name": "v2"}]);
        let err = select_release(&source, &releases).unwrap_err();
        assert_eq!(
            err.0,
            "No matching GitHub release was found for tag 'vX' in 'acme/widget'. Available tags: v1, v2."
        );
    }

    #[test]
    fn select_release_no_match_for_tag_with_no_visible_tags_says_none() {
        let source = map_of(
            json!({"owner": "acme", "repo": "widget", "release_tag": "vX", "allow_prerelease": false}),
        );
        let releases = json!([{"tag_name": ""}, {}]);
        let err = select_release(&source, &releases).unwrap_err();
        assert_eq!(
            err.0,
            "No matching GitHub release was found for tag 'vX' in 'acme/widget'. Available tags: none."
        );
    }

    #[test]
    fn select_release_all_drafts_or_prereleases_without_tag_errors_verbatim() {
        let source = map_of(
            json!({"owner": "acme", "repo": "widget", "release_tag": "", "allow_prerelease": false}),
        );
        let releases =
            json!([{"draft": true, "tag_name": "v1"}, {"prerelease": true, "tag_name": "v2"}]);
        let err = select_release(&source, &releases).unwrap_err();
        assert_eq!(
            err.0,
            "No usable GitHub release was found for 'acme/widget'. All releases were drafts or prereleases."
        );
    }

    // --- select_asset -----------------------------------------------------------

    #[test]
    fn select_asset_no_assets_errors_verbatim() {
        let source = map_of(
            json!({"asset_patterns": ["*"], "asset_exclude_patterns": [], "asset_preferred_patterns": []}),
        );
        let release = map_of(json!({"tag_name": "v1", "assets": []}));
        let err = select_asset(&source, &release).unwrap_err();
        assert_eq!(
            err.0,
            "Selected GitHub release has no assets. release_tag='v1'"
        );
    }

    #[test]
    fn select_asset_non_list_assets_errors_verbatim() {
        let source = map_of(
            json!({"asset_patterns": ["*"], "asset_exclude_patterns": [], "asset_preferred_patterns": []}),
        );
        let release = map_of(json!({"tag_name": "v1", "assets": "nope"}));
        let err = select_asset(&source, &release).unwrap_err();
        assert_eq!(
            err.0,
            "Selected GitHub release has no assets. release_tag='v1'"
        );
    }

    #[test]
    fn select_asset_missing_url_is_skipped_but_counted_in_available_names() {
        let source = map_of(
            json!({"asset_patterns": ["*.nomatch"], "asset_exclude_patterns": [], "asset_preferred_patterns": []}),
        );
        let release = map_of(json!({"tag_name": "v1", "assets": [
            {"name": "a.zip"},
            {"name": "b.zip", "browser_download_url": "https://x/b.zip"}
        ]}));
        let err = select_asset(&source, &release).unwrap_err();
        assert_eq!(
            err.0,
            "No release asset matched configured patterns. include=['*.nomatch'], exclude=[], available_assets=['a.zip', 'b.zip']"
        );
    }

    #[test]
    fn select_asset_include_index_orders_before_preferred() {
        let source = map_of(json!({
            "asset_patterns": ["*.appimage", "*.zip"],
            "asset_exclude_patterns": [],
            "asset_preferred_patterns": []
        }));
        let release = map_of(json!({"tag_name": "v1", "assets": [
            {"name": "a.zip", "browser_download_url": "https://x/a.zip"},
            {"name": "b.AppImage", "browser_download_url": "https://x/b.AppImage"}
        ]}));
        let asset = select_asset(&source, &release).unwrap();
        assert_eq!(
            asset.get("name").and_then(Value::as_str),
            Some("b.AppImage")
        );
    }

    #[test]
    fn select_asset_excludes_matching_exclude_pattern() {
        let source = map_of(json!({
            "asset_patterns": ["*"],
            "asset_exclude_patterns": ["*.sha256"],
            "asset_preferred_patterns": []
        }));
        let release = map_of(json!({"tag_name": "v1", "assets": [
            {"name": "a.zip.sha256", "browser_download_url": "https://x/a.zip.sha256"},
            {"name": "a.zip", "browser_download_url": "https://x/a.zip"}
        ]}));
        let asset = select_asset(&source, &release).unwrap();
        assert_eq!(asset.get("name").and_then(Value::as_str), Some("a.zip"));
    }

    #[test]
    fn select_asset_preferred_index_unmatched_is_patterns_len() {
        let source = map_of(json!({
            "asset_patterns": ["*"],
            "asset_exclude_patterns": [],
            "asset_preferred_patterns": ["*.AppImage"]
        }));
        let release = map_of(json!({"tag_name": "v1", "assets": [
            {"name": "a.zip", "browser_download_url": "https://x/a.zip"},
            {"name": "b.AppImage", "browser_download_url": "https://x/b.AppImage"}
        ]}));
        let asset = select_asset(&source, &release).unwrap();
        assert_eq!(
            asset.get("name").and_then(Value::as_str),
            Some("b.AppImage")
        );
    }

    #[test]
    fn select_asset_state_penalty_prefers_uploaded() {
        let source = map_of(
            json!({"asset_patterns": ["*"], "asset_exclude_patterns": [], "asset_preferred_patterns": []}),
        );
        let release = map_of(json!({"tag_name": "v1", "assets": [
            {"name": "a.zip", "browser_download_url": "https://x/a.zip", "state": "removed"},
            {"name": "b.zip", "browser_download_url": "https://x/b.zip", "state": "uploaded"}
        ]}));
        let asset = select_asset(&source, &release).unwrap();
        assert_eq!(asset.get("name").and_then(Value::as_str), Some("b.zip"));
    }

    #[test]
    fn select_asset_missing_state_defaults_to_uploaded_penalty() {
        let source = map_of(
            json!({"asset_patterns": ["*"], "asset_exclude_patterns": [], "asset_preferred_patterns": []}),
        );
        let release = map_of(json!({"tag_name": "v1", "assets": [
            {"name": "a.zip", "browser_download_url": "https://x/a.zip", "state": "removed"},
            {"name": "b.zip", "browser_download_url": "https://x/b.zip"}
        ]}));
        let asset = select_asset(&source, &release).unwrap();
        assert_eq!(asset.get("name").and_then(Value::as_str), Some("b.zip"));
    }

    #[test]
    fn select_asset_ties_break_on_casefolded_name() {
        let source = map_of(
            json!({"asset_patterns": ["*"], "asset_exclude_patterns": [], "asset_preferred_patterns": []}),
        );
        let release = map_of(json!({"tag_name": "v1", "assets": [
            {"name": "Beta.zip", "browser_download_url": "https://x/b.zip"},
            {"name": "alpha.zip", "browser_download_url": "https://x/a.zip"}
        ]}));
        let asset = select_asset(&source, &release).unwrap();
        assert_eq!(asset.get("name").and_then(Value::as_str), Some("alpha.zip"));
    }

    // --- fnmatch_case ------------------------------------------------------------

    #[test]
    fn fnmatch_case_star_matches_any_run_including_none() {
        assert!(fnmatch_case("a*c", "abc"));
        assert!(fnmatch_case("a*c", "ac"));
        assert!(!fnmatch_case("a*c", "abcd"));
    }

    #[test]
    fn fnmatch_case_question_mark_matches_exactly_one() {
        assert!(fnmatch_case("a?c", "abc"));
        assert!(!fnmatch_case("a?c", "ac"));
    }

    #[test]
    fn fnmatch_case_bracket_class() {
        assert!(fnmatch_case("[abc]", "a"));
        assert!(!fnmatch_case("[abc]", "d"));
    }

    #[test]
    fn fnmatch_case_negated_bracket_class() {
        assert!(fnmatch_case("[!abc]", "d"));
        assert!(!fnmatch_case("[!abc]", "a"));
    }

    #[test]
    fn fnmatch_case_bracket_range() {
        assert!(fnmatch_case("[a-z]", "m"));
        assert!(!fnmatch_case("[a-z]", "M"));
    }

    #[test]
    fn fnmatch_case_unclosed_bracket_is_a_literal() {
        assert!(fnmatch_case("[abc", "[abc"));
        assert!(!fnmatch_case("[abc", "abc"));
    }

    #[test]
    fn fnmatch_case_real_catalog_patterns() {
        assert!(fnmatch_case(
            "pcsx2-v*-linux-appimage-x64-qt.appimage",
            "pcsx2-v2.1.0-linux-appimage-x64-qt.appimage"
        ));
        assert!(!fnmatch_case(
            "pcsx2-v*-linux-appimage-x64-qt.appimage",
            "pcsx2-v2.1.0-windows-x64-qt.zip"
        ));
        assert!(fnmatch_case(
            "eden-linux-*-amd64-clang-pgo.appimage",
            "eden-linux-0.0.5-amd64-clang-pgo.appimage"
        ));
        assert!(!fnmatch_case(
            "eden-linux-*-amd64-clang-pgo.appimage",
            "eden-linux-0.0.5-amd64-clang-pgo.zip"
        ));
    }

    // --- whole-catalog normalization ---------------------------------------------

    #[test]
    fn every_catalog_source_block_with_a_recognized_provider_normalizes() {
        let entries: Vec<Value> = serde_json::from_str(AUTOPROFILES_JSON).unwrap();
        let mut checked = 0;
        for entry in &entries {
            let Some(source) = entry.get("source") else {
                continue;
            };
            if !source.is_object() {
                continue;
            }
            checked += 1;
            let result = normalize_source(source);
            assert!(
                result.is_ok(),
                "entry {:?} failed to normalize: {:?}",
                entry.get("name"),
                result.err()
            );
        }
        assert!(
            checked > 0,
            "expected at least one catalog entry with a source block"
        );
    }
}
