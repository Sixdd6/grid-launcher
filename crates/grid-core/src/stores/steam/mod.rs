//! Steam (`docs/stores-phase4-plan.md`). The first store with no tool, no
//! login window and no GRID-run job: GRID reads Steam's own files for what
//! is installed, asks the Steam Web API (with the user's own key) for what
//! is owned, and hands `steam://` links to the Steam client for install,
//! uninstall, verify and play.
//!
//! `meta` (the `store_meta` JSON), `row` (the registry row) and `filter`
//! (games vs tools, and the `StateFlags` reading) are Step 0's shared
//! contracts, implemented. Package A adds `api` (the Web API), `paths` and
//! `local` (roots, executable, `loginusers.vdf`, libraries and manifests),
//! `client` (the `steam://` opener) and `watch` (the alive check and the
//! install watch), and fills in [`SteamService`]'s bodies; until then they
//! answer "nothing found" or `not implemented`.
//!
//! Secrets: the Web API key arrives as a [`SecretString`]. Only `api.rs`
//! (package A) may read it, and only to put it in a request's query at send
//! time (`RequestBuilder::query`): no `Url` carrying it is ever kept,
//! returned, formatted or logged, every `reqwest::Error` goes through
//! `without_url()`, and the HTTP client follows no redirect. Spike 4 found
//! that reqwest, hyper, hyper-util and h2 log no request URL at any level,
//! but a `reqwest::Error`'s `Display`/`Debug` carries the full URL.
//! `loginusers.vdf`'s `AccountName` (the Steam login name) is never read
//! into any struct. Nothing the Steam client prints is read or logged.

pub mod filter;
pub mod meta;
pub mod row;

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;

use secrecy::SecretString;
use url::Url;

use super::NOT_IMPLEMENTED;
use crate::launch::AliveCheck;

/// The Steam Web API.
pub const STEAM_API: &str = "https://api.steampowered.com";

/// Where Steam's store art lives (`GetItems`' `asset_url_format` is
/// relative to it).
pub const STEAM_ART_BASE: &str = "https://shared.akamai.steamstatic.com/store_item_assets/";

/// The Steam addresses GRID uses. Tests and e2e point them at a mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamEndpoints {
    /// The Web API base (`IPlayerService/GetOwnedGames/v1/` and friends are
    /// joined to it).
    pub api: Url,
    /// The art base covers are built on.
    pub art_base: Url,
    /// Origins besides Steam's image hosts that may serve art (the e2e
    /// mock).
    pub art_origins: Vec<Url>,
}

impl SteamEndpoints {
    /// Steam's own addresses.
    pub fn real() -> SteamEndpoints {
        let url = |text: &str| Url::parse(text).unwrap_or_else(|_| unreachable!("{text}"));
        SteamEndpoints {
            api: url(STEAM_API),
            art_base: url(STEAM_ART_BASE),
            art_origins: Vec::new(),
        }
    }

    /// Everything at one mock server (e2e, tests): the API at `<base>/`,
    /// art at `<base>/art/`, and the base's origin may serve art.
    pub fn e2e(base: Url) -> SteamEndpoints {
        let root = base.join("/").unwrap_or_else(|_| base.clone());
        SteamEndpoints {
            api: root.clone(),
            art_base: root.join("art/").unwrap_or_else(|_| root.clone()),
            art_origins: vec![root],
        }
    }
}

/// GRID's words for a SteamID64 that isn't one.
pub const NOT_A_STEAM_ID: &str =
    "That isn\u{2019}t a SteamID64. It has 17 digits and starts with 7656.";

/// A SteamID64: the public profile number (17 digits, starting `7656`).
/// Not a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SteamId(u64);

impl SteamId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// [`SteamId`]'s parse error; its `Display` is [`NOT_A_STEAM_ID`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadSteamId;

impl fmt::Display for BadSteamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(NOT_A_STEAM_ID)
    }
}

impl std::error::Error for BadSteamId {}

impl FromStr for SteamId {
    type Err = BadSteamId;

    /// 17 ASCII digits starting `7656`, surrounding blanks ignored.
    fn from_str(text: &str) -> Result<SteamId, BadSteamId> {
        let text = text.trim();
        if text.len() != 17
            || !text.starts_with("7656")
            || !text.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(BadSteamId);
        }
        text.parse().map(SteamId).map_err(|_| BadSteamId)
    }
}

impl fmt::Display for SteamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Why a Web API call failed. `Display` is GRID's words and never carries a
/// URL, a status body or the key. A rejected key's words name no status
/// code (`docs/stores-phase4-plan.md` "User decisions" 1).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SteamApiError {
    /// 401 or 403: Steam refused the key.
    #[error("Steam rejected the key. Check that you copied all 32 characters, or make a new key.")]
    Rejected { status: u16 },
    /// `GetOwnedGames` answered `{"response": {}}`: the profile's game
    /// details are private.
    #[error("The key works, but this profile\u{2019}s game details are private, so Steam lists no games.")]
    Private,
    /// `GetPlayerSummaries` listed no player for the SteamID64.
    #[error("That isn\u{2019}t a SteamID64. It has 17 digits and starts with 7656.")]
    NoSuchAccount,
    /// No answer, a timeout, a redirect, 429, 5xx or an unreadable body.
    #[error("Couldn\u{2019}t reach api.steampowered.com. Check your connection and try again.")]
    Network,
}

/// The owned list. Built only from a `GetOwnedGames` answer that has
/// `game_count` (one without it is [`SteamApiError::Private`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedGames {
    pub games: Vec<OwnedGame>,
}

/// One owned game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedGame {
    pub appid: u32,
    pub name: String,
    /// `rtime_last_played` (Unix seconds), 0 for never.
    pub last_played: i64,
}

/// What `IStoreBrowseService/GetItems` says about one app (no key needed).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppInfo {
    pub appid: u32,
    /// The store type (0 game, 1 demo, 6 software, …); `None` when the app
    /// has no store page (`success` 15) or wasn't asked yet.
    pub kind: Option<u8>,
    pub name: String,
    pub description: String,
    pub developers: Vec<String>,
    pub year: String,
    /// The cover URL (the plain `library_600x900.jpg` path when the store
    /// named no capsule).
    pub cover: String,
    pub screenshots: Vec<String>,
}

/// How GRID runs the Steam client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientExe {
    /// An executable GRID runs with the link as its argument (`/usr/bin/steam`,
    /// `~/.steam/steam.sh`, `/snap/bin/steam`, `steam.exe`).
    Path(PathBuf),
    /// `flatpak run com.valvesoftware.Steam` (or its exported launcher).
    Flatpak,
    /// No executable, but a root: `xdg-open <link>`.
    XdgOpen,
}

/// The Steam client on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamClient {
    /// The Steam root (the folder holding `steamapps/` and `config/`).
    pub root: PathBuf,
    pub exe: ClientExe,
    /// What Settings shows: the executable's path, "Flatpak
    /// (com.valvesoftware.Steam)", the Snap's name, or the root.
    pub display: String,
}

/// An account that signed in to Steam on this computer
/// (`config/loginusers.vdf`). Never the login name (`AccountName`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalAccount {
    pub steam_id: SteamId,
    pub persona: String,
    /// `MostRecent "1"`: signed in to Steam on this computer now.
    pub most_recent: bool,
}

/// One app manifest (`steamapps/appmanifest_<appid>.acf`), as GRID reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestState {
    pub appid: u32,
    pub name: String,
    /// The library folder holding this manifest's `steamapps/`.
    pub library: PathBuf,
    /// `installdir`, under `steamapps/common/`.
    pub install_dir: String,
    /// `StateFlags` ([`filter::phase`]).
    pub flags: u32,
    pub size_on_disk: u64,
    pub build_id: u64,
    /// `LastUpdated`, Unix seconds.
    pub last_updated: i64,
    /// `LastPlayed`, Unix seconds.
    pub last_played: i64,
    pub bytes_to_download: u64,
    pub bytes_downloaded: u64,
    pub bytes_to_stage: u64,
    pub bytes_staged: u64,
}

/// What a manifest's `StateFlags` say ([`filter::phase`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPhase {
    Installed,
    Installing,
    Gone,
}

/// A `steam://` link GRID hands to the Steam client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamLink {
    Install(u32),
    Uninstall(u32),
    Validate(u32),
    Run(u32),
}

impl SteamLink {
    /// Whether the client gets `-silent` before the link: only for
    /// [`SteamLink::Run`] (the other links must show Steam's dialog).
    pub fn silent(self) -> bool {
        matches!(self, SteamLink::Run(_))
    }
}

impl fmt::Display for SteamLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SteamLink::Install(id) => write!(f, "steam://install/{id}"),
            SteamLink::Uninstall(id) => write!(f, "steam://uninstall/{id}"),
            SteamLink::Validate(id) => write!(f, "steam://validate/{id}"),
            SteamLink::Run(id) => write!(f, "steam://rungameid/{id}"),
        }
    }
}

/// Steam's local files, the Web API and the client. Cheap to clone.
#[derive(Clone)]
pub struct SteamService {
    // Package A's `api.rs` sends with it (built with no redirects).
    #[allow(dead_code)]
    http: reqwest::Client,
    endpoints: SteamEndpoints,
    // Package A's root and executable finders read the override first.
    #[allow(dead_code)]
    roots_override: Option<Vec<PathBuf>>,
    #[allow(dead_code)]
    exe_override: Option<PathBuf>,
}

impl SteamService {
    /// `http` must follow no redirect (`redirect::Policy::none()`).
    pub fn new(http: reqwest::Client, endpoints: SteamEndpoints) -> Self {
        SteamService {
            http,
            endpoints,
            roots_override: None,
            exe_override: None,
        }
    }

    /// Tests and e2e only: the Steam roots (and executable) to use instead
    /// of the real ones. The ONE override; an e2e build always sets it, so
    /// it never reads a real Steam root.
    #[doc(hidden)]
    pub fn with_roots_override(mut self, roots: Vec<PathBuf>, exe: Option<PathBuf>) -> Self {
        self.roots_override = Some(roots);
        self.exe_override = exe;
        self
    }

    pub fn endpoints(&self) -> &SteamEndpoints {
        &self.endpoints
    }

    /// The Steam root and executable, `None` when Steam isn't installed.
    pub fn find_client(&self) -> Option<SteamClient> {
        None
    }

    /// The accounts in `loginusers.vdf`.
    pub fn local_accounts(&self, _client: &SteamClient) -> Vec<LocalAccount> {
        Vec::new()
    }

    /// Every manifest in every library.
    pub fn scan(&self, _client: &SteamClient) -> Vec<ManifestState> {
        Vec::new()
    }

    /// One app's manifest, in whichever library holds it.
    pub fn manifest(&self, _client: &SteamClient, _appid: u32) -> Option<ManifestState> {
        None
    }

    /// Hands `link` to the Steam client: detached, null stdio, never waited
    /// on in the caller's thread.
    pub fn open(&self, _client: &SteamClient, _link: SteamLink) -> io::Result<()> {
        Err(io::Error::other(NOT_IMPLEMENTED))
    }

    /// The watched session's check for `appid` (cheap, never blocks).
    pub fn alive_check(&self, _client: &SteamClient, _appid: u32) -> AliveCheck {
        Box::new(|| false)
    }

    /// `IPlayerService/GetOwnedGames`.
    pub async fn owned(
        &self,
        _key: &SecretString,
        _id: SteamId,
    ) -> Result<OwnedGames, SteamApiError> {
        Err(SteamApiError::Network)
    }

    /// `ISteamUser/GetPlayerSummaries`: the persona name.
    pub async fn persona(
        &self,
        _key: &SecretString,
        _id: SteamId,
    ) -> Result<String, SteamApiError> {
        Err(SteamApiError::Network)
    }

    /// `IStoreBrowseService/GetItems` (no key), 100 ids per request;
    /// failures give no entry.
    pub async fn app_info(&self, _ids: &[u32]) -> Vec<AppInfo> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_ids_are_17_digits_starting_7656() {
        let id: SteamId = " 76561198000000001 ".parse().unwrap();
        assert_eq!(id.get(), 76_561_198_000_000_001);
        assert_eq!(id.to_string(), "76561198000000001");
        for bad in [
            "",
            "7656119800000000",
            "765611980000000012",
            "86561198000000001",
            "7656119800000000a",
            "+6561198000000001",
            "https://steamcommunity.com/profiles/76561198000000001",
        ] {
            assert_eq!(bad.parse::<SteamId>(), Err(BadSteamId), "{bad}");
        }
        assert_eq!(BadSteamId.to_string(), NOT_A_STEAM_ID);
    }

    #[test]
    fn links_and_silence() {
        assert_eq!(SteamLink::Install(620).to_string(), "steam://install/620");
        assert_eq!(
            SteamLink::Uninstall(620).to_string(),
            "steam://uninstall/620"
        );
        assert_eq!(SteamLink::Validate(620).to_string(), "steam://validate/620");
        assert_eq!(SteamLink::Run(620).to_string(), "steam://rungameid/620");
        assert!(SteamLink::Run(620).silent());
        assert!(!SteamLink::Install(620).silent());
        assert!(!SteamLink::Uninstall(620).silent());
        assert!(!SteamLink::Validate(620).silent());
    }

    #[test]
    fn api_errors_are_grids_words_with_no_status_or_url() {
        for error in [
            SteamApiError::Rejected { status: 401 },
            SteamApiError::Rejected { status: 403 },
            SteamApiError::Private,
            SteamApiError::NoSuchAccount,
            SteamApiError::Network,
        ] {
            let shown = format!("{error} {error:?}");
            assert!(!shown.contains("http"), "{shown}");
            assert!(!shown.contains("key="), "{shown}");
            let words = error.to_string();
            assert!(!words.contains("401") && !words.contains("403"), "{words}");
        }
    }

    #[test]
    fn endpoints_real_and_e2e() {
        let real = SteamEndpoints::real();
        assert_eq!(real.api.as_str(), "https://api.steampowered.com/");
        assert_eq!(real.art_base.as_str(), STEAM_ART_BASE);
        assert!(real.art_origins.is_empty());
        let e2e = SteamEndpoints::e2e(Url::parse("http://127.0.0.1:4010/x").unwrap());
        assert_eq!(e2e.api.as_str(), "http://127.0.0.1:4010/");
        assert_eq!(e2e.art_base.as_str(), "http://127.0.0.1:4010/art/");
        assert_eq!(e2e.art_origins.len(), 1);
    }

    #[test]
    fn the_stub_finds_nothing() {
        let service = SteamService::new(reqwest::Client::new(), SteamEndpoints::real())
            .with_roots_override(Vec::new(), None);
        assert!(service.find_client().is_none());
    }
}
