//! Store art: the image URLs GOG gives (protocol-relative links, gamesdb
//! `url_format` templates, api.gog.com `formatter_template_url`s) made
//! into plain `https` URLs, the store image hosts GRID fetches from (GOG's,
//! Epic's, Amazon's and Steam's, [`is_store_art_url`]), and a small on-disk cache under
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

/// The stores' image hosts: these domains and their subdomains. GOG's,
/// then Epic's (keyImages come from `cdn1.epicgames.com`,
/// `cdn2.unrealengine.com` and `media-cdn.epicgames.com`;
/// `docs/stores-phase2-plan.md` "Spike results", spike 4), then Amazon's
/// (`m.media-amazon.com`, and the older `images-na.ssl-images-amazon.com`
/// and `images-amazon.com` hosts; `docs/stores-phase3-plan.md` spike 4),
/// then Steam's (`shared.akamai.steamstatic.com`,
/// `cdn.cloudflare.steamstatic.com` and the other `steamstatic.com` CDNs;
/// `docs/stores-phase4-plan.md` "Art hosts").
const STORE_ART_DOMAINS: [&str; 8] = [
    "gog-statics.com",
    "gog.com",
    "epicgames.com",
    "unrealengine.com",
    "media-amazon.com",
    "ssl-images-amazon.com",
    "images-amazon.com",
    "steamstatic.com",
];

/// Store image hosts admitted as the exact host only, never their domain:
/// Steam's old `steamcdn-a.akamaihd.net`. `akamaihd.net` itself serves every
/// Akamai customer, so it must never be a whole domain.
const STORE_ART_HOSTS: [&str; 1] = ["steamcdn-a.akamaihd.net"];

/// Whether `url` is store art GRID may fetch for the UI: an `https` address
/// on a store's image hosts ([`STORE_ART_DOMAINS`] and their subdomains, or
/// exactly one of [`STORE_ART_HOSTS`]), or any address on one
/// of `extra_origins` (the e2e build's mock servers, which serve art over
/// plain http). Store art is fetched with no credential, so this never
/// admits the RomM server just because it is connected.
pub fn is_store_art_url(url: &str, extra_origins: &[url::Url]) -> bool {
    let Ok(parsed) = url::Url::parse(&absolute_url(url)) else {
        return false;
    };
    if !parsed.cannot_be_a_base()
        && extra_origins
            .iter()
            .any(|origin| parsed.origin() == origin.origin())
    {
        return true;
    }
    if parsed.scheme() != "https" {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    STORE_ART_HOSTS.contains(&host.as_str())
        || STORE_ART_DOMAINS
            .iter()
            .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
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

    #[test]
    fn store_art_is_gogs_image_hosts_or_the_gamesdb_origin() {
        let gamesdb = [url::Url::parse("https://gamesdb.gog.com/").unwrap()];
        assert!(is_store_art_url(
            "https://images.gog-statics.com/a_196.jpg",
            &gamesdb
        ));
        assert!(is_store_art_url(
            "//images-1.gog-statics.com/a.jpg",
            &gamesdb
        ));
        assert!(is_store_art_url(
            "https://gamesdb.gog.com/art.png",
            &gamesdb
        ));
        assert!(!is_store_art_url(
            "http://images.gog-statics.com/a.jpg",
            &gamesdb
        ));
        assert!(!is_store_art_url(
            "https://evilgog-statics.com/a.jpg",
            &gamesdb
        ));
        assert!(!is_store_art_url("https://romm.example/a.jpg", &gamesdb));
        assert!(!is_store_art_url("file:///etc/passwd", &gamesdb));
        let mock = [url::Url::parse("http://127.0.0.1:4100/").unwrap()];
        assert!(is_store_art_url(
            "http://127.0.0.1:4100/art/a_cover.png",
            &mock
        ));
        assert!(!is_store_art_url("http://127.0.0.1:4200/art/a.png", &mock));
        assert!(!is_store_art_url("http://127.0.0.1:4100/art/a.png", &[]));
    }

    #[test]
    fn store_art_includes_epics_cdns_over_https() {
        for url in [
            "https://cdn1.epicgames.com/df37f065c3f14eadbf011177396e2966/offer/EGS_x-1920x1080-1cf3.jpg",
            "https://cdn2.unrealengine.com/egs-x-g1a-01-1920x1080-b65e51b6fb2e.jpg",
            "https://media-cdn.epicgames.com/5e50/5e50-00001-thumb.png",
            "https://CDN1.EpicGames.com/a.jpg",
        ] {
            assert!(is_store_art_url(url, &[]), "{url}");
        }
        for url in [
            "http://cdn1.epicgames.com/a.jpg",
            "https://cdn1.epicgames.com.evil.example/a.jpg",
            "https://notepicgames.com/a.jpg",
            "com.epicgames.video://db158029-97be-45f7-9a3f-c89f95c9b532",
            "https://unrealengine.co/a.jpg",
        ] {
            assert!(!is_store_art_url(url, &[]), "{url}");
        }
        // Two e2e origins: the GOG mock and the Epic mock.
        let mocks = [
            url::Url::parse("http://127.0.0.1:4100/").unwrap(),
            url::Url::parse("http://127.0.0.1:4300/").unwrap(),
        ];
        assert!(is_store_art_url("http://127.0.0.1:4300/art/e1.jpg", &mocks));
        assert!(is_store_art_url("http://127.0.0.1:4100/art/g1.jpg", &mocks));
        assert!(!is_store_art_url("http://127.0.0.1:4400/art/x.jpg", &mocks));
    }

    #[test]
    fn store_art_includes_amazons_image_hosts_over_https() {
        for url in [
            "https://m.media-amazon.com/images/I/A1quZkgvrsL.jpg",
            "https://images-na.ssl-images-amazon.com/images/I/71VKoVJLPfL.jpg",
            "https://images-amazon.com/images/I/x.png",
        ] {
            assert!(is_store_art_url(url, &[]), "{url}");
        }
        for url in [
            "http://m.media-amazon.com/images/I/a.jpg",
            "https://m.media-amazon.com.evil.example/a.jpg",
            "https://notmedia-amazon.com/a.jpg",
            "https://www.amazon.com/a.jpg",
            "https://gaming.amazon.com/a.jpg",
        ] {
            assert!(!is_store_art_url(url, &[]), "{url}");
        }
    }

    #[test]
    fn store_art_includes_steams_hosts_but_never_all_of_akamaihd() {
        for url in [
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/620/library_600x900.jpg",
            "https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/620/library_600x900.jpg",
            "https://shared.steamstatic.com/store_item_assets/steam/apps/620/library_600x900.jpg",
            "https://cdn.cloudflare.steamstatic.com/steam/apps/620/library_600x900.jpg",
            "https://cdn.akamai.steamstatic.com/steam/apps/620/header.jpg",
            "https://steamcdn-a.akamaihd.net/steam/apps/620/library_600x900.jpg",
            "https://STEAMCDN-A.akamaihd.net/steam/apps/620/header.jpg",
        ] {
            assert!(is_store_art_url(url, &[]), "{url}");
        }
        for url in [
            "http://shared.akamai.steamstatic.com/a.jpg",
            "http://steamcdn-a.akamaihd.net/a.jpg",
            "https://akamaihd.net/a.jpg",
            "https://evil.akamaihd.net/a.jpg",
            "https://x.steamcdn-a.akamaihd.net/a.jpg",
            "https://steamcdn-b.akamaihd.net/a.jpg",
            "https://steamstatic.com.evil.example/a.jpg",
            "https://notsteamstatic.com/a.jpg",
            "https://api.steampowered.com/a.jpg",
            "https://store.steampowered.com/a.jpg",
        ] {
            assert!(!is_store_art_url(url, &[]), "{url}");
        }
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
