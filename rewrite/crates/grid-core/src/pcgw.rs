//! PCGamingWiki API client for resolving Windows save file locations.
//!
//! Ports `grid_launcher/server/pcgamingwiki.py` in full: the path-variable
//! table (`_PCGW_PATH_VARS`, :15-41), `_expand_pcgw_path_var` (:52),
//! `_expand_pcgw_path` (:56), `_extract_template_block` (:76),
//! `parse_windows_save_paths` (:96), `_fetch_json`/`_fetch_json_value`
//! (:128-150), `_split_template_args` (:153), `_build_page_id_url` (:185),
//! `_extract_page_id_from_query` (:190), `_extract_title_from_url` (:208),
//! `fetch_page_id_by_title` (:218), `fetch_page_wikitext` (:250),
//! `fetch_windows_save_paths` (:264).
//!
//! Token secrecy: [`build_http_client`] builds a plain client with a
//! `User-Agent` default header and nothing else — no Authorization header
//! is ever set on it, and it must never share a client with `RommClient`
//! (different host; the RomM token must never reach PCGamingWiki).
//!
//! **Brief-vs-Python discrepancy (Python wins, per task ruling):** the task
//! brief describes `_split_template_args`'s `|`-split as respecting nested
//! `{{}}` AND `[[...]]` links. The actual pinned Python function (:153-182)
//! only tracks `{{`/`}}` depth — it has no `[[`/`]]` handling at all, so a
//! `[[link|label]]` embedded in a path argument DOES get split on its
//! internal `|` like any other top-level pipe. This port matches the real
//! Python behavior exactly (not the brief's description of it): the two
//! resulting fragments simply fail `_expand_pcgw_path`'s leading-`{{p|...}}`
//! match and get dropped, which is why a stray link inside a save-path
//! template still parses harmlessly. See
//! [`tests::parse_extracts_paths_from_a_realistic_game_data_saves_block`]
//! for the fixture proving this.

use std::sync::LazyLock;

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use regex::Regex;
use serde_json::Value;

const PCGW_API_BASE: &str = "https://www.pcgamingwiki.com/w/api.php";

/// `Request(url, headers={"User-Agent": ...})` (pcgamingwiki.py:137).
const USER_AGENT: &str = "grid-launcher/1.0 (pcgamingwiki-client)";

/// `quote(title, safe="")` (pcgamingwiki.py:186,228): only the RFC 3986
/// unreserved characters survive unescaped — even `/`. Same set
/// `romm/cloud.rs`'s `ID_ENCODE_SET` uses, duplicated locally rather than
/// exported (matching that module's own precedent).
static TITLE_ENCODE_SET: LazyLock<AsciiSet> = LazyLock::new(|| {
    NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~')
});

fn encode_title(title: &str) -> String {
    utf8_percent_encode(title, &TITLE_ENCODE_SET).to_string()
}

// ---------------------------------------------------------------------
// Path-variable expansion — pcgamingwiki.py:15-73
// ---------------------------------------------------------------------

/// `_PATH_VAR_RE` (pcgamingwiki.py:43): a `{{p|...}}` / `{{P|...}}` token
/// anchored at the start of the (already-trimmed) text, capturing
/// everything up to the first `}`.
static PATH_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\{\{[Pp]\|([^}]+)\}\}").unwrap());

/// `_TRAILING_WILDCARD_RE` (pcgamingwiki.py:44): a trailing `\*.ext`-style
/// wildcard suffix on the final path segment.
static TRAILING_WILDCARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\\/][^\\/|\n\r]*\*[^|\n\r]*$").unwrap());

/// The inline `re.sub(r'\{\{[^{}]*\}\}', '', path)` at pcgamingwiki.py:71 —
/// strips any remaining non-nested template artifact (e.g. `{{note|...}}`)
/// left over after the leading `{{p|...}}` token has already been expanded.
static TEMPLATE_ARTIFACT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{[^{}]*\}\}").unwrap());

/// `_SAVE_TEMPLATE_START_RE` (pcgamingwiki.py:45): the opening of a
/// `{{Game data/saves|...` template, case-insensitive, tolerant of stray
/// whitespace around `game`/`data`/`/`/`saves`.
static SAVE_TEMPLATE_START_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\{\{\s*game\s*data\s*/\s*saves\s*\|").unwrap());

/// `_expand_pcgw_path_var` (pcgamingwiki.py:52-53): the `_PCGW_PATH_VARS`
/// table (:15-41) as a match — trimmed, case-folded lookup. A key mapped to
/// `None` (DRM/launcher-relative roots with no filesystem equivalent —
/// `steam`, `uplay`, `epicgames`, `gog`, `origin`, `battlenet`, `itchapp`,
/// `registry`) and an unknown key both resolve to `None`, exactly like
/// Python's `dict.get` returning `None` either way.
fn expand_pcgw_path_var(var_name: &str) -> Option<&'static str> {
    match var_name.trim().to_lowercase().as_str() {
        "userprofile\\documents" => Some("%USERPROFILE%\\Documents"),
        "userdocuments" => Some("%USERPROFILE%\\Documents"),
        "savedgames" => Some("%USERPROFILE%\\Documents"),
        "userprofile" => Some("%USERPROFILE%"),
        "appdata" => Some("%APPDATA%"),
        "localappdata" => Some("%LOCALAPPDATA%"),
        "local appdata" => Some("%LOCALAPPDATA%"),
        "applocaldata" => Some("%LOCALAPPDATA%"),
        "programdata" => Some("%PROGRAMDATA%"),
        "allusersappdata" => Some("%PROGRAMDATA%"),
        "public\\documents" => Some("%PUBLIC%\\Documents"),
        "publicdocuments" => Some("%PUBLIC%\\Documents"),
        "public" => Some("%PUBLIC%"),
        "windir" => Some("%WINDIR%"),
        "syswow64" => Some("%WINDIR%"),
        "system" => Some("%WINDIR%"),
        "game" => Some("%GAME_DIR%"),
        _ => None,
    }
}

/// `_expand_pcgw_path` (pcgamingwiki.py:56-73): expand a single raw
/// template argument into a Windows env-var path, or `None` when it isn't
/// (or doesn't expand to) a recognized `{{p|...}}` path.
fn expand_pcgw_path(raw_path: &str) -> Option<String> {
    let text = raw_path.trim();
    if text.is_empty() {
        return None;
    }

    let caps = PATH_VAR_RE.captures(text)?;
    let whole = caps.get(0).unwrap();
    let var_name = caps.get(1).unwrap().as_str();

    let expanded_var = expand_pcgw_path_var(var_name)?;
    if expanded_var.is_empty() {
        return None;
    }

    // Replace the anchored `{{p|...}}` match (and any leading whitespace it
    // consumed) with the expanded variable, keeping everything after it.
    let mut path = format!("{expanded_var}{}", &text[whole.end()..]);
    path = TEMPLATE_ARTIFACT_RE
        .replace_all(&path, "")
        .trim()
        .to_string();
    path = TRAILING_WILDCARD_RE.replace(&path, "").into_owned();
    let path = path.trim_end_matches(['\\', '/']).trim();
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

// ---------------------------------------------------------------------
// Template scanning — pcgamingwiki.py:76-125,153-182
// ---------------------------------------------------------------------

/// `_extract_template_block` (pcgamingwiki.py:76-93): brace-balances a
/// `{{...}}` template starting at `start_index` (which must point at an
/// opening `{{`), returning the full `{{...}}` text and the byte offset
/// just past its closing `}}`. `None` on unbalanced input (braces that
/// never return to depth 0 before the text ends).
///
/// Scans by raw byte pairs rather than `char`s — safe here because `{`/`}`
/// are single-byte ASCII code points that can never appear as a
/// continuation byte of a different UTF-8 sequence, so a byte-level `"{{"`/
/// `"}}"` match can never produce a false positive, and every returned
/// slice boundary (`start_index`, and the final `i`) always lands exactly
/// on one of those ASCII brace bytes — always a valid `str` boundary.
fn extract_template_block(wikitext: &str, start_index: usize) -> Option<(String, usize)> {
    let bytes = wikitext.as_bytes();
    let len = bytes.len();
    let mut i = start_index;
    let mut depth: i32 = 0;
    while i + 1 < len {
        if &bytes[i..i + 2] == b"{{" {
            depth += 1;
            i += 2;
            continue;
        }
        if &bytes[i..i + 2] == b"}}" {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Some((wikitext[start_index..i].to_string(), i));
            }
            continue;
        }
        i += 1;
    }
    None
}

/// `_split_template_args` (pcgamingwiki.py:153-182): splits `inner_template`
/// on top-level `|` characters, treating a `{{`/`}}` pair as one opaque
/// unit so a pipe inside a nested template doesn't split the outer
/// argument list. See this module's doc comment for why this does NOT
/// also protect `[[...]]` links, matching the real (not the brief's
/// described) Python behavior.
fn split_template_args(inner_template: &str) -> Vec<String> {
    let chars: Vec<char> = inner_template.chars().collect();
    let len = chars.len();
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth: i32 = 0;
    let mut i = 0usize;
    while i < len {
        if i + 1 < len && chars[i] == '{' && chars[i + 1] == '{' {
            depth += 1;
            current.push('{');
            current.push('{');
            i += 2;
            continue;
        }
        if depth > 0 && i + 1 < len && chars[i] == '}' && chars[i + 1] == '}' {
            depth -= 1;
            current.push('}');
            current.push('}');
            i += 2;
            continue;
        }
        let ch = chars[i];
        if ch == '|' && depth == 0 {
            args.push(std::mem::take(&mut current));
            i += 1;
            continue;
        }
        current.push(ch);
        i += 1;
    }
    args.push(current);
    args
}

/// `parse_windows_save_paths` (pcgamingwiki.py:96-125): scans `wikitext`
/// for every `{{Game data/saves|...}}` template, keeps only rows whose
/// second argument is (case-insensitively) `Windows`, expands each
/// remaining argument via [`expand_pcgw_path`], and returns the expanded
/// paths in first-seen order with duplicates dropped. A row that never
/// expands (unrecognized/DRM-relative variable, or not a `{{p|...}}` token
/// at all — e.g. a `[[link|label]]` fragment produced by the `|`-split
/// discrepancy noted in this module's doc comment) contributes nothing.
///
/// An unbalanced template (brace extraction failing) stops the whole scan
/// early — no further templates after it are considered — matching
/// Python's bare `break` at :109 exactly (not a `continue`).
pub fn parse_windows_save_paths(wikitext: &str) -> Vec<String> {
    let mut found_paths: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut pos = 0usize;
    while let Some(m) = SAVE_TEMPLATE_START_RE.find_at(wikitext, pos) {
        let Some((block, next_pos)) = extract_template_block(wikitext, m.start()) else {
            break;
        };
        pos = next_pos;

        // Trim the outer `{{`/`}}` before splitting template arguments.
        let inner = &block[2..block.len() - 2];
        let parts: Vec<String> = split_template_args(inner)
            .into_iter()
            .map(|p| p.trim().to_string())
            .collect();
        if parts.len() < 3 {
            continue;
        }
        if parts[1].trim().to_lowercase() != "windows" {
            continue;
        }

        for arg in &parts[2..] {
            if let Some(expanded) = expand_pcgw_path(arg) {
                if seen.insert(expanded.clone()) {
                    found_paths.push(expanded);
                }
            }
        }
    }

    found_paths
}

// ---------------------------------------------------------------------
// HTTP: page-id lookup + wikitext fetch — pcgamingwiki.py:128-269
// ---------------------------------------------------------------------

/// Builds the plain (no auth header) client every PCGamingWiki request
/// uses: a static `User-Agent` matching `_fetch_json_value`'s
/// `Request(..., headers={"User-Agent": ...})` (pcgamingwiki.py:137) and a
/// 10-second timeout matching `urlopen(req, timeout=10)` (:138). Deliberately
/// separate from `RommClient`'s client (different host; never carries the
/// RomM Authorization header).
pub fn build_http_client() -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(USER_AGENT),
    );
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .default_headers(headers)
        .build()
        .expect("pcgw http client: static header/timeout config always builds")
}

/// `_fetch_json_value` (pcgamingwiki.py:135-150): GET `url`, decode as JSON.
/// A non-2xx status maps to the `"PCGamingWiki HTTP {code}: {detail}"` text
/// (:147, `detail` a <=300-char body excerpt, or the status line when the
/// body is empty); a transport or decode failure maps to
/// `"PCGamingWiki request failed: {exc}"` (:149).
async fn fetch_json_value(http: &reqwest::Client, url: &str) -> Result<Value, String> {
    let response = http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("PCGamingWiki request failed: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let excerpt: String = body.chars().take(300).collect();
        let detail = if excerpt.is_empty() {
            status.to_string()
        } else {
            excerpt
        };
        return Err(format!("PCGamingWiki HTTP {}: {detail}", status.as_u16()));
    }
    let text = response
        .text()
        .await
        .map_err(|e| format!("PCGamingWiki request failed: {e}"))?;
    serde_json::from_str::<Value>(&text).map_err(|e| format!("PCGamingWiki request failed: {e}"))
}

/// `_fetch_json` (pcgamingwiki.py:128-132): [`fetch_json_value`], requiring
/// the decoded payload to be a JSON object.
async fn fetch_json(http: &reqwest::Client, url: &str) -> Result<Value, String> {
    let value = fetch_json_value(http, url).await?;
    if value.is_object() {
        Ok(value)
    } else {
        Err("PCGamingWiki response must be a JSON object".to_string())
    }
}

/// `_build_page_id_url` (pcgamingwiki.py:185-187).
fn build_page_id_url(base: &str, title: &str) -> String {
    format!(
        "{base}?action=query&titles={}&prop=info&format=json",
        encode_title(title)
    )
}

/// `_extract_page_id_from_query` (pcgamingwiki.py:190-205): the first
/// `query.pages` entry that isn't the sentinel `"-1"` key and isn't marked
/// `"missing"`, parsed as an integer page id. Object iteration order
/// matches Python's `dict.items()` insertion order because `serde_json`'s
/// `preserve_order` feature is enabled crate-wide.
fn extract_page_id_from_query(payload: &Value) -> Option<i64> {
    let pages = payload.get("query")?.get("pages")?.as_object()?;
    if pages.is_empty() {
        return None;
    }
    for (page_id_raw, page_data) in pages {
        if page_id_raw == "-1" {
            continue;
        }
        let Some(page_data) = page_data.as_object() else {
            continue;
        };
        if page_data.contains_key("missing") {
            continue;
        }
        if let Ok(id) = page_id_raw.parse::<i64>() {
            return Some(id);
        }
    }
    None
}

/// `_extract_title_from_url` (pcgamingwiki.py:208-215): the `/wiki/<title>`
/// segment of a PCGamingWiki page URL, percent-decoded with underscores
/// turned back into spaces. Uses a lossy UTF-8 decode (never fails) to
/// match `unquote`'s leniency on malformed byte sequences.
fn extract_title_from_url(raw_url: &str) -> Option<String> {
    let parsed = url::Url::parse(raw_url).ok()?;
    let path = parsed.path();
    let idx = path.find("/wiki/")?;
    let after = &path[idx + "/wiki/".len()..];
    let decoded = percent_encoding::percent_decode_str(after)
        .decode_utf8_lossy()
        .replace('_', " ");
    let title = decoded.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_string())
    }
}

/// `fetch_page_id_by_title` (pcgamingwiki.py:218-247): an exact
/// `action=query` lookup, falling back to `action=opensearch`'s first
/// result URL (re-resolved through a second `action=query`) when the exact
/// title doesn't resolve.
async fn fetch_page_id_by_title(
    http: &reqwest::Client,
    base: &str,
    title: &str,
) -> Result<Option<i64>, String> {
    let query_title = title.trim();
    if query_title.is_empty() {
        return Ok(None);
    }

    let payload = fetch_json(http, &build_page_id_url(base, query_title)).await?;
    if let Some(id) = extract_page_id_from_query(&payload) {
        return Ok(Some(id));
    }

    let opensearch_url = format!(
        "{base}?action=opensearch&search={}&namespace=0&limit=3&format=json",
        encode_title(query_title)
    );
    let search_payload = fetch_json_value(http, &opensearch_url).await?;

    let search_urls: Vec<String> = search_payload
        .as_array()
        .filter(|arr| arr.len() > 3)
        .and_then(|arr| arr[3].as_array())
        .map(|inner| {
            inner
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    let Some(first_url) = search_urls.first() else {
        return Ok(None);
    };
    let Some(resolved_title) = extract_title_from_url(first_url) else {
        return Ok(None);
    };

    let fallback_payload = fetch_json(http, &build_page_id_url(base, &resolved_title)).await?;
    Ok(extract_page_id_from_query(&fallback_payload))
}

/// `fetch_page_wikitext` (pcgamingwiki.py:250-261).
async fn fetch_page_wikitext(
    http: &reqwest::Client,
    base: &str,
    page_id: i64,
) -> Result<String, String> {
    let url = format!("{base}?action=parse&pageid={page_id}&prop=wikitext&format=json");
    let payload = fetch_json(http, &url).await?;
    let wikitext = payload
        .get("parse")
        .and_then(|v| v.get("wikitext"))
        .and_then(|v| v.get("*"))
        .ok_or_else(|| "PCGamingWiki parse response missing wikitext".to_string())?;
    wikitext
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "PCGamingWiki wikitext payload must be a string".to_string())
}

/// `fetch_windows_save_paths` (pcgamingwiki.py:264-269): the page-id lookup
/// then wikitext fetch then parse, against the real PCGamingWiki API.
/// `title` not resolving to a page id is not an error — it degrades to an
/// empty list, exactly like Python's `if page_id is None: return []`. Any
/// other failure (HTTP, decode, missing wikitext) surfaces as `Err`; the
/// caller (the app-layer cache in `cloud_service.rs`) is the one that
/// degrades THAT to an empty list, never an error dialog.
pub async fn fetch_windows_save_paths(
    http: &reqwest::Client,
    title: &str,
) -> Result<Vec<String>, String> {
    fetch_windows_save_paths_with_base(http, PCGW_API_BASE, title).await
}

/// [`fetch_windows_save_paths`] with an overridable API base, so tests can
/// point it at a `wiremock` server instead of the real PCGamingWiki host.
async fn fetch_windows_save_paths_with_base(
    http: &reqwest::Client,
    base: &str,
    title: &str,
) -> Result<Vec<String>, String> {
    let Some(page_id) = fetch_page_id_by_title(http, base, title).await? else {
        return Ok(Vec::new());
    };
    let wikitext = fetch_page_wikitext(http, base, page_id).await?;
    Ok(parse_windows_save_paths(&wikitext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // -------------------------------------------------------------
    // parse_windows_save_paths — pure parsing, no network
    // -------------------------------------------------------------

    /// Fixture with a nested `{{note|...}}` template AND a `[[link|label]]`
    /// fragment sharing an argument list with a real path row. Proves both
    /// that nested `{{}}` templates are stripped after expansion and that
    /// the `[[...]]` link's internal `|` — NOT protected by
    /// `_split_template_args` (see this module's doc comment) — splits
    /// into two junk fragments that simply fail to expand and vanish,
    /// leaving the real path intact.
    #[test]
    fn parse_extracts_paths_from_a_realistic_game_data_saves_block() {
        let wikitext = "Some prose before the template. \
            {{Game data/saves|Windows|{{p|appdata}}\\MyGame\\saves\\{{note|name=Steam release}}|\
            See [[Special:MyGame|save notes]] for details}} \
            Some prose after.";

        let paths = parse_windows_save_paths(wikitext);

        assert_eq!(paths, vec!["%APPDATA%\\MyGame\\saves".to_string()]);
    }

    /// One assert per `_PCGW_PATH_VARS` entry (pcgamingwiki.py:15-41),
    /// Some and None alike, driven through the public parse entry point.
    #[test]
    fn parse_expands_the_path_variable_table() {
        let some_cases: &[(&str, &str)] = &[
            ("userprofile\\documents", "%USERPROFILE%\\Documents"),
            ("userdocuments", "%USERPROFILE%\\Documents"),
            ("savedgames", "%USERPROFILE%\\Documents"),
            ("userprofile", "%USERPROFILE%"),
            ("appdata", "%APPDATA%"),
            ("localappdata", "%LOCALAPPDATA%"),
            ("local appdata", "%LOCALAPPDATA%"),
            ("applocaldata", "%LOCALAPPDATA%"),
            ("programdata", "%PROGRAMDATA%"),
            ("allusersappdata", "%PROGRAMDATA%"),
            ("public\\documents", "%PUBLIC%\\Documents"),
            ("publicdocuments", "%PUBLIC%\\Documents"),
            ("public", "%PUBLIC%"),
            ("windir", "%WINDIR%"),
            ("syswow64", "%WINDIR%"),
            ("system", "%WINDIR%"),
            ("game", "%GAME_DIR%"),
        ];
        for (key, expected_var) in some_cases {
            let wikitext = format!("{{{{Game data/saves|Windows|{{{{p|{key}}}}}\\Sub}}}}");
            let paths = parse_windows_save_paths(&wikitext);
            assert_eq!(
                paths,
                vec![format!("{expected_var}\\Sub")],
                "path-var key {key:?} did not expand as expected"
            );
        }

        let none_cases: &[&str] = &[
            "steam",
            "uplay",
            "epicgames",
            "gog",
            "origin",
            "battlenet",
            "itchapp",
            "registry",
        ];
        for key in none_cases {
            let wikitext = format!("{{{{Game data/saves|Windows|{{{{p|{key}}}}}\\Sub}}}}");
            let paths = parse_windows_save_paths(&wikitext);
            assert_eq!(
                paths,
                Vec::<String>::new(),
                "path-var key {key:?} must expand to nothing (DRM/launcher-relative root)"
            );
        }
    }

    #[test]
    fn parse_drops_unexpandable_rows_and_dedupes() {
        let wikitext = "{{Game data/saves|Windows|\
            {{p|appdata}}\\Game\\saves|\
            {{p|appdata}}\\Game\\saves|\
            Just some literal text, not a path var|\
            {{p|steam}}\\userdata\\saves}}";

        let paths = parse_windows_save_paths(wikitext);

        // The duplicate {{p|appdata}} row collapses to one entry (in-order
        // dedupe); the literal-text row (no {{p|...}} token) and the
        // {{p|steam}} row (maps to None) both drop out entirely.
        assert_eq!(paths, vec!["%APPDATA%\\Game\\saves".to_string()]);
    }

    #[test]
    fn parse_windows_save_paths_no_windows_entry() {
        let wikitext = "{{Game data/saves|Linux|{{p|userprofile}}/.local/share/MyGame/saves}}";
        assert_eq!(parse_windows_save_paths(wikitext), Vec::<String>::new());
    }

    #[test]
    fn parse_windows_save_paths_wildcard_stripped() {
        let wikitext = "{{Game data/saves|Windows|{{p|userprofile}}\\Documents\\Game\\*.sav}}";
        assert_eq!(
            parse_windows_save_paths(wikitext),
            vec!["%USERPROFILE%\\Documents\\Game".to_string()]
        );
    }

    #[test]
    fn parse_windows_save_paths_batman_arkham_asylum() {
        let wikitext = "{{Game data/saves|Windows|\
            {{p|userprofile\\Documents}}\\Eidos\\Batman Arkham Asylum\\SaveData\\|\
            {{p|userprofile\\Documents}}\\Square Enix\\Batman Arkham Asylum GOTY\\SaveData\\{{note|name=Game of the Year Edition}}}}";

        let paths = parse_windows_save_paths(wikitext);

        assert!(paths.contains(
            &"%USERPROFILE%\\Documents\\Eidos\\Batman Arkham Asylum\\SaveData".to_string()
        ));
        assert!(paths.contains(
            &"%USERPROFILE%\\Documents\\Square Enix\\Batman Arkham Asylum GOTY\\SaveData"
                .to_string()
        ));
    }

    // -------------------------------------------------------------
    // extract_template_block — brace balancing
    // -------------------------------------------------------------

    #[test]
    fn template_block_extraction_balances_braces() {
        let wikitext = "prefix {{outer|{{inner|value}}|more}} suffix";
        let start = wikitext.find("{{outer").unwrap();

        let (block, next_pos) = extract_template_block(wikitext, start).unwrap();

        assert_eq!(block, "{{outer|{{inner|value}}|more}}");
        assert_eq!(&wikitext[next_pos..], " suffix");
    }

    #[test]
    fn template_block_extraction_returns_none_when_unbalanced() {
        let wikitext = "{{outer|{{inner|value}}|more";
        assert_eq!(extract_template_block(wikitext, 0), None);
    }

    // -------------------------------------------------------------
    // split_template_args
    // -------------------------------------------------------------

    #[test]
    fn split_template_args_respects_nested_braces_not_links() {
        let parts = split_template_args("Windows|{{p|appdata}}\\Game|[[link|label]]");
        assert_eq!(
            parts,
            vec![
                "Windows".to_string(),
                "{{p|appdata}}\\Game".to_string(),
                "[[link".to_string(),
                "label]]".to_string(),
            ]
        );
    }

    // -------------------------------------------------------------
    // HTTP: two-step page-id -> wikitext flow (wiremock)
    // -------------------------------------------------------------

    #[tokio::test]
    async fn fetch_windows_save_paths_full_round_trip() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .and(query_param("titles", "My Game"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "batchcomplete": "",
                "query": {
                    "pages": {
                        "12345": {"pageid": 12345, "ns": 0, "title": "My Game"}
                    }
                }
            })))
            .mount(&mock_server)
            .await;
        Mock::given(method("GET"))
            .and(query_param("action", "parse"))
            .and(query_param("pageid", "12345"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "parse": {
                    "wikitext": {
                        "*": "{{Game data/saves|Windows|{{p|appdata}}\\MyGame\\saves}}"
                    }
                }
            })))
            .mount(&mock_server)
            .await;

        let http = build_http_client();
        let paths = fetch_windows_save_paths_with_base(&http, &mock_server.uri(), "My Game")
            .await
            .unwrap();

        assert_eq!(paths, vec!["%APPDATA%\\MyGame\\saves".to_string()]);
    }

    #[tokio::test]
    async fn fetch_windows_save_paths_opensearch_fallback() {
        let mock_server = MockServer::start().await;
        let base = mock_server.uri();

        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .and(query_param("titles", "Stardew Valley"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "batchcomplete": "",
                "query": {"pages": {"-1": {"ns": 0, "title": "Stardew Valley", "missing": ""}}}
            })))
            // Only the FIRST exact-title query must see the "missing" page —
            // the resolved title happens to be identical text here (as in
            // the Python oracle this ports), so without this cap the same
            // mock would also answer the second (fallback) request.
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;
        Mock::given(method("GET"))
            .and(query_param("action", "opensearch"))
            .and(query_param("search", "Stardew Valley"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                "Stardew Valley",
                ["Stardew Valley"],
                [],
                [format!("{base}/wiki/Stardew_Valley")]
            ])))
            .mount(&mock_server)
            .await;
        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .and(query_param("titles", "Stardew Valley"))
            .and(query_param("format", "json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "batchcomplete": "",
                "query": {"pages": {"54321": {"pageid": 54321, "ns": 0, "title": "Stardew Valley"}}}
            })))
            .mount(&mock_server)
            .await;

        let page_id = fetch_page_id_by_title(&build_http_client(), &base, "Stardew Valley")
            .await
            .unwrap();

        assert_eq!(page_id, Some(54321));
    }

    #[tokio::test]
    async fn fetch_windows_save_paths_missing_page_degrades_to_empty_list() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "batchcomplete": "",
                "query": {"pages": {"-1": {"ns": 0, "title": "Nonexistent Game", "missing": ""}}}
            })))
            .mount(&mock_server)
            .await;
        Mock::given(method("GET"))
            .and(query_param("action", "opensearch"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                "Nonexistent Game",
                [],
                [],
                []
            ])))
            .mount(&mock_server)
            .await;

        let http = build_http_client();
        let paths =
            fetch_windows_save_paths_with_base(&http, &mock_server.uri(), "Nonexistent Game")
                .await
                .unwrap();

        assert_eq!(paths, Vec::<String>::new());
    }

    #[tokio::test]
    async fn fetch_windows_save_paths_http_error_raises() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .respond_with(ResponseTemplate::new(500).set_body_string("server exploded"))
            .mount(&mock_server)
            .await;

        let http = build_http_client();
        let err = fetch_windows_save_paths_with_base(&http, &mock_server.uri(), "Any Game")
            .await
            .unwrap_err();

        assert!(err.contains("PCGamingWiki HTTP 500"), "got: {err}");
    }

    #[tokio::test]
    async fn build_http_client_sends_user_agent_and_never_an_authorization_header() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&mock_server)
            .await;

        let http = build_http_client();
        http.get(mock_server.uri()).send().await.unwrap();

        let received = mock_server.received_requests().await.unwrap();
        assert_eq!(received.len(), 1);
        let headers = &received[0].headers;
        assert_eq!(headers.get("user-agent").unwrap(), USER_AGENT);
        assert!(
            headers.get("authorization").is_none(),
            "the PCGW client must never carry an Authorization header"
        );
    }
}
