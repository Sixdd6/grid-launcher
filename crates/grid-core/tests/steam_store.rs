//! The Steam store's grid-core half (`docs/stores-phase4-plan.md` package
//! A) against real-format fixtures and a mock Web API:
//! - a Steam root copied from `tests/fixtures/steam/` (two libraries, a
//!   library whose path has a backslash and a quote, an unplugged library,
//!   tools Steam installs beside games, a manifest cut in half, and
//!   `StateFlags` 4/6/1026/1542/2048) read through `SteamService`;
//! - `loginusers.vdf` read for persona names only (its `AccountName` is
//!   `FAKE-ACCOUNT-NAME-not-shown` and must never come back);
//! - every Web API outcome against wiremock, checking that the key reaches
//!   the server only as the `key` query parameter of the key-carrying
//!   calls, never on `GetItems`, and never in any result's or error's
//!   `Display`/`Debug`;
//! - on unix, a fake `steam` that records its argv.
//!
//! Ids are invented (`91000xx` games, `76561198000000001`/`…0002`
//! accounts) except the real tool ids the filter must drop. The only key
//! is `FAKE-STEAM-KEY-not-real`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use grid_core::stores::steam::{
    filter, AppPhase, ClientExe, InstallWatch, ManifestRead, SteamApiError, SteamEndpoints,
    SteamId, SteamService, WatchEvent,
};
use secrecy::SecretString;
use url::Url;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "FAKE-STEAM-KEY-not-real";
const ACCOUNT_NAME: &str = "FAKE-ACCOUNT-NAME-not-shown";
const SAM: &str = "76561198000000001";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/steam")
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A KeyValues string's escaped form.
fn vdf_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The odd library's folder name: a quote and, off Windows, a backslash
/// (Windows forbids both in names, so it gets a plain name there).
fn odd_name() -> &'static str {
    if cfg!(windows) {
        "My Games Lib"
    } else {
        "My \"Games\"\\Lib"
    }
}

struct Fake {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    lib2: PathBuf,
    odd: PathBuf,
    missing: PathBuf,
}

/// The fixture root and libraries in a temp folder, with
/// `libraryfolders.vdf`'s paths filled in.
fn fake_steam() -> Fake {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Steam");
    let lib2 = tmp.path().join("SSD Library");
    let odd = tmp.path().join(odd_name());
    let missing = tmp.path().join("unplugged");
    copy_tree(&fixtures().join("root"), &root);
    copy_tree(&fixtures().join("lib2"), &lib2);
    fs::create_dir_all(odd.join("steamapps")).unwrap();
    fs::write(
        odd.join("steamapps/appmanifest_9100008.acf"),
        "\"AppState\"\n{\n\t\"appid\"\t\t\"9100008\"\n\t\"name\"\t\t\"Odd Path Game\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"Odd\"\n}\n",
    )
    .unwrap();
    let vdf = root.join("steamapps/libraryfolders.vdf");
    let text = fs::read_to_string(&vdf)
        .unwrap()
        .replace("@ROOT@", &vdf_escape(&root.display().to_string()))
        .replace("@LIB2@", &vdf_escape(&lib2.display().to_string()))
        .replace("@ODD@", &vdf_escape(&odd.display().to_string()))
        .replace("@MISSING@", &vdf_escape(&missing.display().to_string()));
    fs::write(&vdf, text).unwrap();
    Fake {
        _tmp: tmp,
        root,
        lib2,
        odd,
        missing,
    }
}

/// A loopback address nothing answers on: no test here ever reaches Steam.
fn dead_api() -> SteamEndpoints {
    SteamEndpoints::e2e(Url::parse("http://127.0.0.1:9/").unwrap())
}

fn service_for(fake: &Fake, exe: Option<PathBuf>) -> SteamService {
    SteamService::new(reqwest::Client::new(), dead_api())
        .with_roots_override(vec![fake.root.clone()], exe)
}

#[test]
fn the_override_client_and_its_accounts() {
    let fake = fake_steam();
    let exe = fake.root.join("fake-steam");
    let service = service_for(&fake, Some(exe.clone()));
    let client = service.find_client().unwrap();
    assert_eq!(client.root, fake.root);
    assert_eq!(client.exe, ClientExe::Path(exe));

    let accounts = service.local_accounts(&client);
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].steam_id.to_string(), SAM);
    assert_eq!(accounts[0].persona, "SamPlays");
    assert!(accounts[0].most_recent);
    assert_eq!(accounts[1].persona, "couch \"guest\"");
    assert!(!accounts[1].most_recent);
    assert!(!format!("{accounts:?}").contains(ACCOUNT_NAME));

    // No root given: no client, whatever this machine has.
    let none = SteamService::new(reqwest::Client::new(), dead_api())
        .with_roots_override(vec![fake.missing.clone()], None);
    assert!(none.find_client().is_none());
}

#[test]
fn every_library_and_every_manifest() {
    let fake = fake_steam();
    let service = service_for(&fake, None);
    let client = service.find_client().unwrap();

    let libraries = service.libraries(&client);
    assert_eq!(
        libraries,
        vec![
            fake.root.clone(),
            fake.lib2.clone(),
            fake.odd.clone(),
            fake.missing.clone()
        ],
        "the odd path reads back exactly"
    );

    let report = service.scan_report(&client);
    let ids: Vec<u32> = report.manifests.iter().map(|m| m.appid).collect();
    assert_eq!(
        ids,
        [228980, 1628350, 2805730, 9100001, 9100004, 9100005, 9100006, 9100002, 9100003, 9100008]
    );
    assert_eq!(report.unreadable, [9100007], "the half-written manifest");
    assert_eq!(
        report.missing_libraries,
        std::slice::from_ref(&fake.missing)
    );
    assert_eq!(service.scan(&client).len(), report.manifests.len());

    let by_id = |id: u32| report.manifests.iter().find(|m| m.appid == id).unwrap();
    let ferry = by_id(9100001);
    assert_eq!(ferry.name, "Moonlit Ferry");
    assert_eq!(ferry.library, fake.root);
    assert_eq!(ferry.install_dir, "Moonlit Ferry");
    assert_eq!(ferry.flags, 4);
    assert_eq!(ferry.size_on_disk, 2_147_483_648);
    assert_eq!(ferry.build_id, 15_550_001);
    assert_eq!(ferry.last_updated, 1_730_000_000);
    assert_eq!(ferry.last_played, 1_731_200_000);

    let lantern = by_id(9100004);
    assert_eq!(filter::phase(lantern.flags), AppPhase::Installing);
    assert_eq!(filter::progress(lantern), (1_000_000_000, 4_000_000_000));
    assert_eq!(lantern.last_played, 0);

    assert_eq!(
        filter::phase(by_id(9100005).flags),
        AppPhase::Installed,
        "1542"
    );
    assert_eq!(filter::phase(by_id(9100006).flags), AppPhase::Gone, "2048");
    let radio = by_id(9100002);
    assert_eq!(radio.library, fake.lib2);
    assert_eq!(filter::phase(radio.flags), AppPhase::Installed, "6");
    assert_eq!(radio.last_played, 1_730_900_000);
    assert_eq!(
        by_id(9100003).name,
        "Old Brick Road",
        "unknown keys ignored"
    );
    assert_eq!(by_id(9100008).library, fake.odd);

    // Games only: the tools never pass the filter.
    let games: Vec<u32> = report
        .manifests
        .iter()
        .filter(|m| filter::is_game(m.appid, &m.name, &m.install_dir, None))
        .filter(|m| filter::phase(m.flags) == AppPhase::Installed)
        .map(|m| m.appid)
        .collect();
    assert_eq!(games, [9100001, 9100005, 9100002, 9100003, 9100008]);

    // One library on its own.
    let lib2 = service.scan_library(&fake.lib2).unwrap();
    assert_eq!(lib2.manifests.len(), 2);
    assert!(service.scan_library(&fake.missing).is_none());
}

#[test]
fn a_root_without_libraryfolders_is_its_own_library() {
    let fake = fake_steam();
    fs::write(
        fake.root.join("steamapps/libraryfolders.vdf"),
        "\"libraryfolders\"\n{\n\t\"0\"",
    )
    .unwrap();
    let service = service_for(&fake, None);
    let client = service.find_client().unwrap();
    assert_eq!(service.libraries(&client), vec![fake.root.clone()]);
    assert_eq!(service.scan(&client).len(), 7);
}

#[test]
fn one_manifest_found_unreadable_or_missing() {
    let fake = fake_steam();
    let service = service_for(&fake, None);
    let client = service.find_client().unwrap();
    match service.manifest_read(&client, 9100002) {
        ManifestRead::Found(m) => assert_eq!(m.library, fake.lib2),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        service.manifest_read(&client, 9100007),
        ManifestRead::Unreadable
    );
    assert_eq!(
        service.manifest_read(&client, 9199999),
        ManifestRead::Missing
    );
    assert_eq!(service.manifest(&client, 9100007), None);
    assert_eq!(service.manifest(&client, 9100003).unwrap().appid, 9100003);
}

#[test]
fn an_install_watched_through_the_files() {
    let fake = fake_steam();
    let service = service_for(&fake, None);
    let client = service.find_client().unwrap();
    let appid = 9100010;
    let manifest = fake.lib2.join(format!("steamapps/appmanifest_{appid}.acf"));
    let write = |flags: u32, done: u64| {
        fs::write(
            &manifest,
            format!(
                "\"AppState\"\n{{\n\t\"appid\"\t\t\"{appid}\"\n\t\"name\"\t\t\"New Game\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\t\"installdir\"\t\t\"New Game\"\n\t\"BytesToDownload\"\t\t\"100\"\n\t\"BytesDownloaded\"\t\t\"{done}\"\n}}\n"
            ),
        )
        .unwrap();
    };
    let mut watch = InstallWatch::default();
    let s = Duration::from_secs;
    assert_eq!(
        watch.step(&service.manifest_read(&client, appid), s(2)),
        WatchEvent::Waiting
    );
    write(1026, 25);
    assert_eq!(
        watch.step(&service.manifest_read(&client, appid), s(4)),
        WatchEvent::Progress(25, 100)
    );
    fs::write(&manifest, "\"AppState\"\n{\n\t\"appid\"").unwrap();
    assert_eq!(
        watch.step(&service.manifest_read(&client, appid), s(6)),
        WatchEvent::Progress(25, 100),
        "being written: unchanged"
    );
    write(4, 0);
    assert_eq!(
        watch.step(&service.manifest_read(&client, appid), s(8)),
        WatchEvent::Installed
    );
}

// ---- The Web API ----

fn secret() -> SecretString {
    SecretString::from(KEY.to_string())
}

fn sam() -> SteamId {
    SAM.parse().unwrap()
}

/// A client like the app's: no redirects.
fn api_service(server: &MockServer) -> SteamService {
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    SteamService::new(
        http,
        SteamEndpoints::e2e(Url::parse(&server.uri()).unwrap()),
    )
}

/// No key in anything the result shows.
fn assert_clean<T: std::fmt::Debug>(result: &T) {
    let shown = format!("{result:?}");
    assert!(!shown.contains(KEY), "{shown}");
    assert!(!shown.contains("key="), "{shown}");
}

fn assert_error_clean(error: &SteamApiError) {
    let shown = format!("{error} {error:?}");
    assert!(!shown.contains(KEY), "{shown}");
    assert!(!shown.contains("http"), "{shown}");
}

const OWNED_PATH: &str = "/IPlayerService/GetOwnedGames/v1/";

async fn owned_answer(
    response: ResponseTemplate,
) -> Result<grid_core::stores::steam::OwnedGames, SteamApiError> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(OWNED_PATH))
        .and(query_param("key", KEY))
        .and(query_param("steamid", SAM))
        .and(query_param("include_appinfo", "1"))
        .and(query_param("include_played_free_games", "1"))
        .respond_with(response)
        .expect(1)
        .mount(&server)
        .await;
    let result = api_service(&server).owned(&secret(), sam()).await;
    match &result {
        Ok(owned) => assert_clean(owned),
        Err(error) => assert_error_clean(error),
    }
    result
}

const STEAM_HTML_401: &str = "<html><head><title>Unauthorized</title></head><body><h1>Unauthorized</h1>Access is denied. Retrying will not help. Please verify your <pre>key=</pre> parameter.</body></html>";

#[tokio::test]
async fn owned_games_every_outcome() {
    let owned = owned_answer(ResponseTemplate::new(200).set_body_raw(
        r#"{"response":{"game_count":3,"games":[
            {"appid":9100001,"name":"Moonlit Ferry","playtime_forever":120,"img_icon_url":"abc","rtime_last_played":1731200000},
            {"appid":9100011,"name":" Quiet Comet ","playtime_forever":0,"rtime_last_played":0},
            {"appid":431960,"name":"Wallpaper Engine","playtime_forever":5}
        ]}}"#,
        "application/json",
    ))
    .await
    .unwrap();
    assert_eq!(owned.games.len(), 3);
    assert_eq!(owned.games[0].appid, 9100001);
    assert_eq!(owned.games[0].last_played, 1_731_200_000);
    assert_eq!(owned.games[1].name, "Quiet Comet");
    assert_eq!(owned.games[2].last_played, 0);

    let none = owned_answer(
        ResponseTemplate::new(200)
            .set_body_raw(r#"{"response":{"game_count":0}}"#, "application/json"),
    )
    .await
    .unwrap();
    assert!(none.games.is_empty(), "a public profile that owns nothing");

    assert_eq!(
        owned_answer(
            ResponseTemplate::new(200).set_body_raw(r#"{"response":{}}"#, "application/json")
        )
        .await,
        Err(SteamApiError::Private)
    );
    assert_eq!(
        owned_answer(ResponseTemplate::new(401).set_body_raw(STEAM_HTML_401, "text/html")).await,
        Err(SteamApiError::Rejected { status: 401 })
    );
    assert_eq!(
        owned_answer(ResponseTemplate::new(403).set_body_raw(STEAM_HTML_401, "text/html")).await,
        Err(SteamApiError::Rejected { status: 403 })
    );
    for status in [429, 500, 503] {
        assert_eq!(
            owned_answer(ResponseTemplate::new(status)).await,
            Err(SteamApiError::Network),
            "{status}"
        );
    }
    assert_eq!(
        owned_answer(
            ResponseTemplate::new(302).insert_header("location", "https://example.invalid/")
        )
        .await,
        Err(SteamApiError::Network),
        "a redirect is not followed"
    );
    assert_eq!(
        owned_answer(ResponseTemplate::new(200).set_body_raw("<html>not json</html>", "text/html"))
            .await,
        Err(SteamApiError::Network)
    );
}

#[tokio::test]
async fn an_unreachable_api_is_network_with_no_url() {
    // A port nothing listens on.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    drop(listener);
    let service = SteamService::new(
        reqwest::Client::new(),
        SteamEndpoints::e2e(Url::parse(&base).unwrap()),
    );
    let error = service.owned(&secret(), sam()).await.unwrap_err();
    assert_eq!(error, SteamApiError::Network);
    assert_error_clean(&error);
    let error = service.persona(&secret(), sam()).await.unwrap_err();
    assert_eq!(error, SteamApiError::Network);
    assert!(service.app_info(&[620]).await.is_empty());
}

#[tokio::test]
async fn persona_found_or_no_such_account() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ISteamUser/GetPlayerSummaries/v2/"))
        .and(query_param("key", KEY))
        .and(query_param("steamids", SAM))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"response":{"players":[{"steamid":"76561198000000001","communityvisibilitystate":3,"personaname":"SamPlays"}]}}"#,
            "application/json",
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/ISteamUser/GetPlayerSummaries/v2/"))
        .and(query_param("steamids", "76561198000000009"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(r#"{"response":{"players":[]}}"#, "application/json"),
        )
        .mount(&server)
        .await;
    let service = api_service(&server);
    assert_eq!(service.persona(&secret(), sam()).await.unwrap(), "SamPlays");
    let nobody: SteamId = "76561198000000009".parse().unwrap();
    let error = service.persona(&secret(), nobody).await.unwrap_err();
    assert_eq!(error, SteamApiError::NoSuchAccount);
    assert_error_clean(&error);
}

#[tokio::test]
async fn get_items_sends_no_key_and_batches_by_100() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IStoreBrowseService/GetItems/v1/"))
        .respond_with(|request: &wiremock::Request| {
            let input = request
                .url
                .query_pairs()
                .find(|(name, _)| name == "input_json")
                .map(|(_, value)| value.into_owned())
                .unwrap_or_default();
            let input: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
            let items: Vec<serde_json::Value> = input["ids"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|id| {
                    let appid = id["appid"].as_u64().unwrap_or(0);
                    if appid == 2805730 {
                        serde_json::json!({"item_type":0,"id":appid,"success":15,"visible":false,"name":"","appid":0})
                    } else {
                        serde_json::json!({"item_type":0,"id":appid,"success":1,"appid":appid,"type":0,
                            "name": format!("Game {appid}"),
                            "assets":{"asset_url_format":"${FILENAME}","library_capsule":format!("{appid}.jpg")}})
                    }
                })
                .collect();
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"response":{"store_items":items}}))
        })
        .mount(&server)
        .await;
    let service = api_service(&server);
    let mut ids: Vec<u32> = (9_200_000..9_200_149).collect();
    ids.push(2805730);
    ids.push(9_200_000); // asked twice, answered once
    let infos = service.app_info(&ids).await;
    assert_eq!(infos.len(), 150);
    assert_eq!(infos[0].appid, 9_200_000);
    assert_eq!(infos[0].name, "Game 9200000");
    assert_eq!(infos[0].kind, Some(0));
    assert_eq!(infos[0].cover, format!("{}/art/9200000.jpg", server.uri()));
    let proton = infos.iter().find(|info| info.appid == 2805730).unwrap();
    assert_eq!(proton.kind, None);
    assert!(proton
        .cover
        .ends_with("/art/steam/apps/2805730/library_600x900.jpg"));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "100 ids per request");
    for request in &requests {
        assert!(
            !request.url.query_pairs().any(|(name, _)| name == "key"),
            "GetItems carries no key"
        );
        assert!(!request.url.as_str().contains(KEY));
    }
}

#[tokio::test]
async fn a_failed_get_items_batch_gives_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/IStoreBrowseService/GetItems/v1/"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert!(api_service(&server).app_info(&[620]).await.is_empty());
}

// ---- The client ----

#[cfg(unix)]
#[test]
fn links_reach_a_fake_steam_with_its_argv() {
    use std::os::unix::fs::PermissionsExt;
    let fake = fake_steam();
    let argv_file = fake.root.join("argv.txt");
    let script = fake.root.join("fake-steam");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\necho {KEY}\necho {KEY} >&2\nprintf '%s\\n' \"$@\" >> '{}.tmp'\nmv '{}.tmp' '{}'\n",
            argv_file.display(),
            argv_file.display(),
            argv_file.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let service = service_for(&fake, Some(script));
    let client = service.find_client().unwrap();

    let opened = |link| {
        let _ = fs::remove_file(&argv_file);
        service.open(&client, link).unwrap();
        for _ in 0..200 {
            if let Ok(text) = fs::read_to_string(&argv_file) {
                return text;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the fake steam never ran");
    };
    use grid_core::stores::steam::SteamLink;
    assert_eq!(
        opened(SteamLink::Run(9100001)),
        "-silent\nsteam://rungameid/9100001\n"
    );
    assert_eq!(
        opened(SteamLink::Install(9100004)),
        "steam://install/9100004\n"
    );
    assert_eq!(
        opened(SteamLink::Uninstall(9100001)),
        "steam://uninstall/9100001\n"
    );
    assert_eq!(
        opened(SteamLink::Validate(9100001)),
        "steam://validate/9100001\n"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_alive_check_through_the_service() {
    let fake = fake_steam();
    let service = service_for(&fake, None);
    let client = service.find_client().unwrap();
    let registry = |running: u32| {
        format!(
            "\"Registry\"\n{{\n\t\"HKCU\"\n\t{{\n\t\t\"Software\"\n\t\t{{\n\t\t\t\"Valve\"\n\t\t\t{{\n\t\t\t\t\"Steam\"\n\t\t\t\t{{\n\t\t\t\t\t\"AutoLoginUser\"\t\t\"{ACCOUNT_NAME}\"\n\t\t\t\t\t\"RunningAppID\"\t\t\"{running}\"\n\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n"
        )
    };
    let file = fake.root.join("registry.vdf");
    fs::write(&file, registry(0)).unwrap();
    let mut alive = service.alive_check(&client, 9100001);
    assert!(alive());
    fs::write(&file, registry(9100001)).unwrap();
    assert!(alive());
    fs::write(&file, registry(0)).unwrap();
    assert!(alive());
    assert!(!alive());
}
