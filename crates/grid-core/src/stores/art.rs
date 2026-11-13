//! Store art: the image URLs GOG gives (protocol-relative links, gamesdb
//! `url_format` templates, api.gog.com `formatter_template_url`s) made
//! into plain `https` URLs, and a small on-disk cache under
//! `<data_dir>/stores/art/` keyed by the URL's SHA-256, like the RomM art
//! cache. Only `http(s)` URLs are fetched.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::paths::STORES_DIR;

/// The api.gog.com screenshot size GRID asks for.
pub const SCREENSHOT_FORMATTER: &str = "ggvgl_2x";

/// The art cache folder: `<data_dir>/stores/art`.
pub fn art_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(STORES_DIR).join("art")
}

/// A protocol-relative URL (`//images.gog.com/x.jpg`) as `https`; any other
/// value is trimmed and passed through.
pub fn absolute_url(url: &str) -> String {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

/// A GOG URL template with `{formatter}` and `{ext}` filled in. A
/// formatter is joined with the `_` GOG puts before it when the template
/// has none (`..._{formatter}.jpg` vs `...{formatter}.{ext}`).
pub fn fill_template(template: &str, formatter: &str, ext: &str) -> String {
    let template = absolute_url(template);
    if template.is_empty() {
        return String::new();
    }
    let formatter = if formatter.is_empty() || template.contains("_{formatter}") {
        formatter.to_string()
    } else {
        format!("_{formatter}")
    };
    template
        .replace("{formatter}", &formatter)
        .replace("{ext}", ext)
}

/// The cache file for `url`: `<art_dir>/<sha256(url)>.<ext>` (the URL's
/// extension when it has a short alphanumeric one, else `img`).
pub fn cache_path(data_dir: &Path, url: &str) -> PathBuf {
    let digest: String = Sha256::digest(url.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let ext = url
        .split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit('/').next())
        .and_then(|name| {
            name.rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
        })
        .filter(|ext| {
            (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())
        })
        .unwrap_or_else(|| "img".to_string());
    art_dir(data_dir).join(format!("{digest}.{ext}"))
}

/// The cached copy of `url`, downloading it on a miss. `None` for a blank
/// or non-http URL, or when the download fails.
pub async fn cached(http: &reqwest::Client, data_dir: &Path, url: &str) -> Option<PathBuf> {
    let url = absolute_url(url);
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    let path = cache_path(data_dir, &url);
    if path.is_file() {
        return Some(path);
    }
    let resp = http.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    if bytes.is_empty() {
        return None;
    }
    fs::create_dir_all(path.parent()?).ok()?;
    let part = path.with_extension("part");
    fs::write(&part, &bytes).ok()?;
    fs::rename(&part, &path).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn urls_are_made_absolute_and_templates_filled() {
        assert_eq!(
            absolute_url("//images.gog.com/a.jpg"),
            "https://images.gog.com/a.jpg"
        );
        assert_eq!(absolute_url(" https://x/a.png "), "https://x/a.png");
        assert_eq!(
            fill_template("https://images.gog.com/v{formatter}.{ext}", "", "jpg"),
            "https://images.gog.com/v.jpg"
        );
        assert_eq!(
            fill_template(
                "https://images.gog.com/v{formatter}.{ext}",
                "glx_vertical_cover",
                "webp"
            ),
            "https://images.gog.com/v_glx_vertical_cover.webp"
        );
        assert_eq!(
            fill_template(
                "//images.gog-statics.com/s_{formatter}.jpg",
                "ggvgl_2x",
                "jpg"
            ),
            "https://images.gog-statics.com/s_ggvgl_2x.jpg"
        );
        assert_eq!(fill_template("", "x", "jpg"), "");
    }

    #[test]
    fn cache_paths_are_stable_and_keep_the_extension() {
        let data = Path::new("/data");
        let a = cache_path(data, "https://x/a.JPG?size=2");
        assert_eq!(a, cache_path(data, "https://x/a.JPG?size=2"));
        assert!(a.starts_with("/data/stores/art"));
        assert_eq!(a.extension().unwrap(), "jpg");
        assert_eq!(
            cache_path(data, "https://x/noext").extension().unwrap(),
            "img"
        );
    }

    #[tokio::test]
    async fn a_miss_downloads_and_a_hit_does_not() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cover.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"jpeg".to_vec()))
            .expect(1)
            .mount(&server)
            .await;
        let data = tempfile::tempdir().unwrap();
        let http = reqwest::Client::new();
        let url = format!("{}/cover.jpg", server.uri());
        let first = cached(&http, data.path(), &url).await.unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"jpeg");
        let second = cached(&http, data.path(), &url).await.unwrap();
        assert_eq!(first, second);
        assert!(cached(&http, data.path(), "file:///etc/passwd")
            .await
            .is_none());
    }
}
