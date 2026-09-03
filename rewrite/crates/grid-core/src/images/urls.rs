//! Verbatim port of `grid_launcher/cover/utils.py`: Python `urllib.parse`
//! semantics are reproduced by hand (`urlsplit`, `urlunsplit`, `quote`,
//! `parse_qsl`/`urlencode`) because the `url` crate normalizes differently
//! (lowercases hosts, adds a trailing slash to bare origins).

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

/// Python `quote(path, safe="/%._-~")`: letters, digits, `_.-~` always safe.
const PATH_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'%')
    .remove(b'.')
    .remove(b'_')
    .remove(b'-')
    .remove(b'~');
/// Python `quote_plus(s, safe="")`.
const QUERY_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'.')
    .remove(b'_')
    .remove(b'-')
    .remove(b'~');
const USES_NETLOC: [&str; 6] = ["http", "https", "ftp", "file", "ws", "wss"];

static SCREENSHOT_HINT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[^a-z0-9])(?:screenshot|screen[_-]?shot|gameplay|in[_-]?game|title[_-]?screen|titlescreen)(?:[^a-z0-9]|$)",
    )
    .expect("static regex")
});
static NON_SCREENSHOT_ART_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[^a-z0-9])(?:box(?:[_-]?art)?|cover(?:[_-]?art)?|fan(?:[_-]?art)?|logo|clear[_-]?logo|clear[_-]?art|banner|poster|marquee|cartridge|disc)(?:[^a-z0-9]|$)",
    )
    .expect("static regex")
});

const LAUNCHBOX_SCREENSHOT_TYPE_TOKENS: [&str; 6] = [
    "screenshot",
    "title screen",
    "titlescreen",
    "gameplay",
    "in-game",
    "ingame",
];

/// Every extension `extension_for` can return; the cache probes these.
pub const LOOKUP_EXTENSIONS: [&str; 14] = [
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "ico", "svg", "avif", "heic",
    "heif", "img",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SplitUrl {
    pub scheme: String,
    pub netloc: String,
    pub path: String,
    pub query: String,
    pub fragment: String,
}

fn is_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
}

/// Python `urllib.parse.urlsplit`.
pub fn urlsplit(value: &str) -> SplitUrl {
    let (rest, fragment) = match value.split_once('#') {
        Some((a, b)) => (a, b.to_string()),
        None => (value, String::new()),
    };
    let (rest, query) = match rest.split_once('?') {
        Some((a, b)) => (a, b.to_string()),
        None => (rest, String::new()),
    };
    let (scheme, rest) = match rest.split_once(':') {
        Some((s, r)) if is_scheme(s) => (s.to_ascii_lowercase(), r),
        _ => (String::new(), rest),
    };
    let (netloc, path) = match rest.strip_prefix("//") {
        Some(after) => {
            let end = after.find(['/', '?', '#']).unwrap_or(after.len());
            (after[..end].to_string(), after[end..].to_string())
        }
        None => (String::new(), rest.to_string()),
    };
    SplitUrl {
        scheme,
        netloc,
        path,
        query,
        fragment,
    }
}

/// Python `urllib.parse.urlunsplit`.
pub fn urlunsplit(s: &SplitUrl) -> String {
    let mut url = s.path.clone();
    if !s.netloc.is_empty()
        || (!s.scheme.is_empty()
            && USES_NETLOC.contains(&s.scheme.as_str())
            && !url.starts_with("//"))
    {
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        url = format!("//{}{url}", s.netloc);
    }
    if !s.scheme.is_empty() {
        url = format!("{}:{url}", s.scheme);
    }
    if !s.query.is_empty() {
        url = format!("{url}?{}", s.query);
    }
    if !s.fragment.is_empty() {
        url = format!("{url}#{}", s.fragment);
    }
    url
}

fn unquote_plus(s: &str) -> String {
    percent_decode_str(&s.replace('+', " "))
        .decode_utf8_lossy()
        .into_owned()
}

fn quote_plus(s: &str) -> String {
    utf8_percent_encode(s, QUERY_SAFE)
        .to_string()
        .replace("%20", "+")
}

/// Python `parse_qsl(query, keep_blank_values=True)`.
fn parse_qsl(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (unquote_plus(name), unquote_plus(value))
        })
        .collect()
}

/// Python `urlencode(pairs, doseq=True)` over string pairs.
fn urlencode(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `_looks_like_screenshot_url` (cover/utils.py:20).
pub fn looks_like_screenshot_url(value: &str) -> bool {
    let parsed = urlsplit(value);
    let haystack = if !parsed.path.is_empty() || !parsed.query.is_empty() {
        format!("{}?{}", parsed.path, parsed.query)
    } else {
        value.to_string()
    };
    if SCREENSHOT_HINT_RE.is_match(&haystack) {
        return true;
    }
    !NON_SCREENSHOT_ART_RE.is_match(&haystack)
}

/// `resolve_cover_url` (cover/utils.py:28).
pub fn resolve_image_url(value: &str, base_url: &str) -> String {
    let candidate = value.trim();
    if candidate.is_empty() {
        return String::new();
    }
    let candidate = if candidate.starts_with("http://") || candidate.starts_with("https://") {
        candidate.to_string()
    } else if base_url.is_empty() {
        return String::new();
    } else if candidate.starts_with('/') {
        format!("{base_url}{candidate}")
    } else {
        format!("{base_url}/{candidate}")
    };
    let split = urlsplit(&candidate);
    urlunsplit(&SplitUrl {
        scheme: split.scheme,
        netloc: split.netloc,
        path: utf8_percent_encode(&split.path, PATH_SAFE).to_string(),
        query: urlencode(&parse_qsl(&split.query)),
        fragment: split.fragment,
    })
}

/// `filter_to_server_host` (cover/utils.py:47). Permissive on an empty
/// `url`/`base_url` or a base with no netloc; whole-netloc comparison.
pub fn filter_to_server_host(url: &str, base_url: &str) -> String {
    if url.is_empty() || base_url.is_empty() {
        return url.to_string();
    }
    let base_netloc = urlsplit(base_url).netloc;
    if base_netloc.is_empty() {
        return url.to_string();
    }
    let candidate_netloc = urlsplit(url).netloc;
    if !candidate_netloc.is_empty() && candidate_netloc != base_netloc {
        return String::new();
    }
    url.to_string()
}

/// The desktop window's composition (grid-launcher.py:2894):
/// `filter_to_server_host(resolve_cover_url(value, base), base)`.
pub fn server_resolver(base_url: &str) -> impl Fn(&str) -> String {
    let base = base_url.to_string();
    move |value: &str| filter_to_server_host(&resolve_image_url(value, &base), &base)
}

fn resolve_cover_value(value: &Value, resolver: &dyn Fn(&str) -> String) -> String {
    match value {
        Value::String(s) => resolver(s),
        Value::Object(map) => {
            for key in [
                "url",
                "path",
                "image",
                "src",
                "download_path",
                "file_path",
                "full_path",
            ] {
                if let Some(Value::String(candidate)) = map.get(key) {
                    let resolved = resolver(candidate);
                    if !resolved.is_empty() {
                        return resolved;
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// `cover_url_from_rom_payload` (cover/utils.py:63).
pub fn cover_url_from_payload(payload: &Value, resolver: &dyn Fn(&str) -> String) -> String {
    for key in [
        "url_cover",
        "path_cover_large",
        "path_cover_small",
        "cover_url",
        "cover_image",
        "cover_path",
        "image_url",
    ] {
        if let Some(value) = payload.get(key) {
            let resolved = resolve_cover_value(value, resolver);
            if !resolved.is_empty() {
                return resolved;
            }
        }
    }
    String::new()
}

fn is_launchbox_screenshot_type(image_type: &str) -> bool {
    let normalized = image_type.trim().to_lowercase();
    LAUNCHBOX_SCREENSHOT_TYPE_TOKENS
        .iter()
        .any(|t| normalized.contains(t))
}

/// `screenshot_urls_from_rom_payload` (cover/utils.py:93), source order and
/// the per-append de-dup preserved exactly.
pub fn screenshot_urls_from_payload(
    payload: &Value,
    resolver: &dyn Fn(&str) -> String,
) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();

    fn append_url(
        urls: &mut Vec<String>,
        value: Option<&Value>,
        resolver: &dyn Fn(&str) -> String,
    ) {
        match value {
            Some(Value::String(s)) => {
                let resolved = resolver(s);
                if !resolved.is_empty() && !urls.contains(&resolved) {
                    urls.push(resolved);
                }
            }
            Some(Value::Object(map)) => {
                for key in ["url", "path", "image", "src"] {
                    if let Some(Value::String(candidate)) = map.get(key) {
                        let resolved = resolver(candidate);
                        if !resolved.is_empty() && !urls.contains(&resolved) {
                            urls.push(resolved);
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(Value::Array(items)) = payload.get("merged_screenshots") {
        for item in items {
            append_url(&mut urls, Some(item), resolver);
        }
    }
    if let Some(Value::Array(items)) = payload.get("user_screenshots") {
        for item in items {
            let Value::Object(map) = item else { continue };
            for key in ["download_path", "file_path", "full_path"] {
                append_url(&mut urls, map.get(key), resolver);
            }
        }
    }
    for block in ["gamelist_metadata", "ss_metadata"] {
        if let Some(Value::Object(map)) = payload.get(block) {
            for key in ["screenshot_url", "title_screen_url"] {
                append_url(&mut urls, map.get(key), resolver);
            }
        }
    }
    if let Some(Value::Object(launchbox)) = payload.get("launchbox_metadata") {
        if let Some(Value::Array(images)) = launchbox.get("images") {
            for image in images {
                let Value::Object(map) = image else { continue };
                let Some(Value::String(image_type)) = map.get("type") else {
                    continue;
                };
                if is_launchbox_screenshot_type(image_type) {
                    append_url(&mut urls, map.get("url"), resolver);
                }
            }
        }
    }
    for key in [
        "url_screenshots",
        "path_screenshots",
        "screenshots",
        "images",
    ] {
        match payload.get(key) {
            Some(Value::Array(items)) if key == "images" => {
                for item in items {
                    let Value::Object(map) = item else {
                        append_url(&mut urls, Some(item), resolver);
                        continue;
                    };
                    if let Some(Value::String(image_type)) = map.get("type") {
                        if is_launchbox_screenshot_type(image_type) {
                            append_url(&mut urls, Some(item), resolver);
                        }
                        continue;
                    }
                    append_url(&mut urls, Some(item), resolver);
                }
            }
            Some(Value::Array(items)) => {
                for item in items {
                    append_url(&mut urls, Some(item), resolver);
                }
            }
            other => append_url(&mut urls, other, resolver),
        }
    }
    for key in ["url_screenshot", "path_screenshot"] {
        append_url(&mut urls, payload.get(key), resolver);
    }

    urls.into_iter()
        .filter(|u| looks_like_screenshot_url(u))
        .collect()
}

/// `screenshot_urls_from_game` (cover/utils.py:183): re-filter on read.
pub fn screenshot_urls_from_stored(raw: &str) -> Vec<String> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    let mut unique: Vec<String> = Vec::new();
    for line in raw.lines() {
        let value = line.trim();
        if !value.is_empty()
            && looks_like_screenshot_url(value)
            && !unique.iter().any(|u| u == value)
        {
            unique.push(value.to_string());
        }
    }
    unique
}

/// Result of `extension_for`. `identified` is true when Content-Type,
/// magic bytes or the SVG sniff recognized an image (the content gate);
/// false when only the URL suffix or the `img` fallback chose it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sniff {
    pub ext: String,
    pub identified: bool,
}

fn sniff(ext: &str, identified: bool) -> Sniff {
    Sniff {
        ext: ext.to_string(),
        identified,
    }
}

/// `cover_cache_extension_from_payload` (cover/utils.py:220), without the
/// leading dot. D11: the SVG `<?xml` branch lowercases (Python's
/// `bytes.casefold()` does not exist).
pub fn extension_for(url: &str, body: &[u8], content_type: &str) -> Sniff {
    let normalized = content_type.trim().to_lowercase();
    let normalized = normalized.split(';').next().unwrap_or("");
    let mapped = match normalized {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/bmp" | "image/x-ms-bmp" => Some("bmp"),
        "image/tiff" => Some("tiff"),
        "image/x-icon" | "image/vnd.microsoft.icon" => Some("ico"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    };
    if let Some(ext) = mapped {
        return sniff(ext, true);
    }
    if body.starts_with(b"\x89PNG\r\n\x1a\n") {
        return sniff("png", true);
    }
    if body.starts_with(b"\xff\xd8\xff") {
        return sniff("jpg", true);
    }
    if body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a") {
        return sniff("gif", true);
    }
    if body.starts_with(b"BM") {
        return sniff("bmp", true);
    }
    if body.starts_with(b"II*\0") || body.starts_with(b"MM\0*") {
        return sniff("tiff", true);
    }
    if body.starts_with(b"\0\0\x01\0") {
        return sniff("ico", true);
    }
    if body.len() >= 12 && body.starts_with(b"RIFF") && &body[8..12] == b"WEBP" {
        return sniff("webp", true);
    }
    let preview = &body[..body.len().min(256)];
    let start = preview
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(preview.len());
    let preview = &preview[start..];
    let lowered = preview.to_ascii_lowercase();
    if preview.starts_with(b"<svg")
        || (preview.starts_with(b"<?xml") && lowered.windows(4).any(|w| w == b"<svg"))
    {
        return sniff("svg", true);
    }
    let path = urlsplit(url).path;
    let last = path.rsplit('/').next().unwrap_or("");
    if let Some(idx) = last.rfind('.') {
        if idx > 0 && idx + 1 < last.len() {
            let suffix = last[idx + 1..].to_lowercase();
            if [
                "jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff", "ico", "svg", "avif",
                "heic", "heif",
            ]
            .contains(&suffix.as_str())
            {
                return sniff(&suffix, false);
            }
        }
    }
    sniff("img", false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn identity(v: &str) -> String {
        v.to_string()
    }

    // tests/test_screenshot_urls.py:13
    #[test]
    fn launchbox_typed_images_keep_only_screenshot_types() {
        let payload = json!({"launchbox_metadata": {"images": [
            {"type": "Box - Front", "url": "https://img.example/box-front.jpg"},
            {"type": "Fanart - Background", "url": "https://img.example/fanart.jpg"},
            {"type": "Clear Logo", "url": "https://img.example/logo.png"},
            {"type": "Screenshot - Gameplay", "url": "https://img.example/shot-gameplay.jpg"},
            {"type": "Screenshot - Game Title", "url": "https://img.example/shot-title.jpg"}
        ]}});
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec![
                "https://img.example/shot-gameplay.jpg",
                "https://img.example/shot-title.jpg"
            ]
        );
    }

    // tests/test_screenshot_urls.py:36
    #[test]
    fn metadata_blocks_exclude_non_screenshot_fields() {
        let payload = json!({
            "gamelist_metadata": {"screenshot_url": "https://img.example/gamelist-shot.jpg",
                "title_screen_url": "https://img.example/gamelist-title.jpg",
                "image_url": "https://img.example/gamelist-box-art.jpg"},
            "ss_metadata": {"screenshot_url": "https://img.example/ss-shot.jpg",
                "title_screen_url": "https://img.example/ss-title.jpg",
                "fanart_url": "https://img.example/ss-fanart.jpg"}
        });
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec![
                "https://img.example/gamelist-shot.jpg",
                "https://img.example/gamelist-title.jpg",
                "https://img.example/ss-shot.jpg",
                "https://img.example/ss-title.jpg"
            ]
        );
    }

    // tests/test_screenshot_urls.py:62
    #[test]
    fn screenshot_source_order_and_images_block_filtering() {
        let payload = json!({
            "merged_screenshots": ["https://img.example/merged-shot-1.jpg", "https://img.example/merged-shot-2.jpg"],
            "url_screenshots": ["https://img.example/list-shot-1.jpg", "https://img.example/list-shot-2.jpg"],
            "images": [
                {"type": "Screenshot - Gameplay", "url": "https://img.example/images-shot-1.jpg"},
                {"type": "Box - Front", "url": "https://img.example/box-front.jpg"},
                {"type": "Fanart - Background", "url": "https://img.example/fanart.jpg"}
            ]
        });
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec![
                "https://img.example/merged-shot-1.jpg",
                "https://img.example/merged-shot-2.jpg",
                "https://img.example/list-shot-1.jpg",
                "https://img.example/list-shot-2.jpg",
                "https://img.example/images-shot-1.jpg"
            ]
        );
    }

    #[test]
    fn user_screenshots_take_every_path_key_and_dedupe() {
        let payload = json!({"user_screenshots": [
            {"download_path": "/assets/roms/1/screenshots/a.png", "file_path": "/assets/roms/1/screenshots/a.png", "full_path": "/x/b.png"},
            "not-a-dict"
        ]});
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec!["/assets/roms/1/screenshots/a.png", "/x/b.png"]
        );
    }

    #[test]
    fn non_list_screenshot_value_is_appended_as_single_item() {
        let payload = json!({"screenshots": "https://img.example/single-shot.jpg",
                             "url_screenshot": "https://img.example/only.jpg"});
        assert_eq!(
            screenshot_urls_from_payload(&payload, &identity),
            vec![
                "https://img.example/single-shot.jpg",
                "https://img.example/only.jpg"
            ]
        );
    }

    // tests/test_screenshot_urls.py:92
    #[test]
    fn stored_list_filters_stale_non_screenshot_lines() {
        let raw = "https://img.example/box-front.jpg\nhttps://img.example/screenshot-gameplay.jpg\n\
                   https://img.example/fanart-background.jpg\nhttps://img.example/title-screen.jpg\n\
                   https://img.example/screenshot-gameplay.jpg\n  \n";
        assert_eq!(
            screenshot_urls_from_stored(raw),
            vec![
                "https://img.example/screenshot-gameplay.jpg",
                "https://img.example/title-screen.jpg"
            ]
        );
        assert!(screenshot_urls_from_stored("   ").is_empty());
    }

    #[test]
    fn screenshot_heuristic_is_permissive_by_default() {
        assert!(looks_like_screenshot_url("https://h/assets/1234.png"));
        assert!(looks_like_screenshot_url("https://h/x/screenshot-1.jpg"));
        assert!(looks_like_screenshot_url(
            "https://h/x/a.jpg?kind=title_screen"
        ));
        assert!(!looks_like_screenshot_url("https://h/x/box-front.jpg"));
        assert!(!looks_like_screenshot_url("https://h/x/BoxArt.jpg"));
        assert!(!looks_like_screenshot_url("https://h/x/clear_logo.png"));
        // positive beats negative
        assert!(looks_like_screenshot_url(
            "https://h/x/cover-screenshot.jpg"
        ));
        // "boxes" is not "box" (token bounded by non-alphanumerics)
        assert!(looks_like_screenshot_url("https://h/x/boxes.jpg"));
    }

    // tests/test_screenshot_urls.py:114-150
    #[test]
    fn host_filter_cases() {
        let ext = "https://neoclone.screenscraper.fr/img/123.jpg";
        assert_eq!(filter_to_server_host(ext, "https://my-romm-server"), "");
        assert_eq!(filter_to_server_host(ext, ""), ext);
        assert_eq!(filter_to_server_host(ext, "not-a-url"), ext);
        assert_eq!(
            filter_to_server_host(
                "https://my-romm-server/api/roms/123/cover",
                "https://my-romm-server"
            ),
            "https://my-romm-server/api/roms/123/cover"
        );
        assert_eq!(
            filter_to_server_host(
                "https://my-romm-server:9090/img/cover.jpg",
                "https://my-romm-server:8080"
            ),
            ""
        );
        assert_eq!(filter_to_server_host("", "https://my-romm-server"), "");
    }

    #[test]
    fn resolve_relative_and_normalize() {
        assert_eq!(
            resolve_image_url("/api/roms/123/cover", "https://my-romm-server"),
            "https://my-romm-server/api/roms/123/cover"
        );
        assert_eq!(
            resolve_image_url("api/x.png", "https://h"),
            "https://h/api/x.png"
        );
        assert_eq!(resolve_image_url("/api/x.png", ""), "");
        assert_eq!(resolve_image_url("   ", "https://h"), "");
        assert_eq!(
            resolve_image_url("/assets/cover art.png", "https://h"),
            "https://h/assets/cover%20art.png"
        );
        // already-encoded stays encoded (% is safe)
        assert_eq!(
            resolve_image_url("/a%20b.png", "https://h"),
            "https://h/a%20b.png"
        );
        // query round-trip keeps blank values, encodes spaces as '+'
        assert_eq!(
            resolve_image_url("https://h/x.png?a=1&b=&c=x y#frag", ""),
            "https://h/x.png?a=1&b=&c=x+y#frag"
        );
        // absolute foreign URL untouched by resolve (filter drops it)
        let r = server_resolver("https://h");
        assert_eq!(r("https://other/x.png"), "");
        assert_eq!(r("/x.png"), "https://h/x.png");
    }

    #[test]
    fn cover_key_walk_and_dict_values() {
        let r = server_resolver("https://h");
        let p = json!({"path_cover_small": "/small.png", "path_cover_large": "/large.png"});
        assert_eq!(cover_url_from_payload(&p, &r), "https://h/large.png");
        let p = json!({"url_cover": "https://other/c.png", "path_cover_small": "/small.png"});
        assert_eq!(cover_url_from_payload(&p, &r), "https://h/small.png");
        let p = json!({"cover_image": {"src": "/dict.png"}});
        assert_eq!(cover_url_from_payload(&p, &r), "https://h/dict.png");
        assert_eq!(cover_url_from_payload(&json!({}), &r), "");
    }

    #[test]
    fn extension_for_precedence() {
        let png = b"\x89PNG\r\n\x1a\n....";
        assert_eq!(extension_for("/x", png, "").ext, "png");
        assert!(extension_for("/x", png, "").identified);
        assert_eq!(extension_for("/x", png, "image/jpeg; charset=x").ext, "jpg");
        assert_eq!(extension_for("/x", b"\xff\xd8\xff\xe0", "").ext, "jpg");
        assert_eq!(extension_for("/x", b"GIF89a", "").ext, "gif");
        assert_eq!(extension_for("/x", b"BM....", "").ext, "bmp");
        assert_eq!(extension_for("/x", b"II*\0", "").ext, "tiff");
        assert_eq!(extension_for("/x", b"\0\0\x01\0", "").ext, "ico");
        assert_eq!(extension_for("/x", b"RIFF\0\0\0\0WEBPVP8 ", "").ext, "webp");
        assert_eq!(extension_for("/x", b"RIFF\0\0\0\0WEB", "").ext, "img");
        assert_eq!(extension_for("/x", b"  <svg xmlns", "").ext, "svg");
        assert_eq!(
            extension_for("/x", b"<?xml version='1'?><SVG>", "").ext,
            "svg"
        );
        let s = extension_for("https://h/a/b.JPEG?x=1", b"zzzz", "text/html");
        assert_eq!(s.ext, "jpeg");
        assert!(!s.identified);
        assert_eq!(extension_for("https://h/a/b.exe", b"zzzz", "").ext, "img");
        assert_eq!(extension_for("https://h/a/.hidden", b"zzzz", "").ext, "img");
        assert_eq!(extension_for("/x", b"", "image/webp").ext, "webp");
    }

    #[test]
    fn urlsplit_matches_python_shapes() {
        let s = urlsplit("https://host:8080/p/a th?q=1#f");
        assert_eq!(s.scheme, "https");
        assert_eq!(s.netloc, "host:8080");
        assert_eq!(s.path, "/p/a th");
        assert_eq!(s.query, "q=1");
        assert_eq!(s.fragment, "f");
        assert_eq!(urlsplit("not-a-url").netloc, "");
        assert_eq!(urlunsplit(&urlsplit("https://host")), "https://host");
        assert_eq!(
            urlunsplit(&urlsplit("https://host/x?a=1#f")),
            "https://host/x?a=1#f"
        );
    }
}
