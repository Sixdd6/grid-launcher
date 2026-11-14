//! Steam's own files: `config/loginusers.vdf` (the accounts on this
//! computer), `steamapps/libraryfolders.vdf` (the libraries) and every
//! `appmanifest_<appid>.acf` (`docs/stores-phase4-plan.md` "Steam's local
//! files" and Spike 1).
//!
//! steamlocate 2.1.1 reads the libraries and the manifests;
//! keyvalues-parser reads `loginusers.vdf`, `registry.vdf` and a manifest's
//! `LastPlayed` (steamlocate has no field for it).
//!
//! Secrets and privacy:
//! - only `PersonaName` and `MostRecent` are read from a user in
//!   `loginusers.vdf`: `AccountName` (the Steam login name) is never looked
//!   up, kept or returned, and neither is `registry.vdf`'s `AutoLoginUser`;
//! - a parse error's `Display`/`Debug` quotes the file's text at the error
//!   (which can be an account name), so no parse error is ever formatted,
//!   logged or returned: a file that doesn't parse reads as "nothing".

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use keyvalues_parser::{Obj, Value};

use super::{LocalAccount, ManifestState, SteamId};

/// A KeyValues object's first value for `name`, matched without regard to
/// ASCII case (Steam writes `Apps` and `apps`, `MostRecent` and
/// `mostrecent`).
fn field<'o, 't>(obj: &'o Obj<'t>, name: &str) -> Option<&'o Value<'t>> {
    obj.0
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .and_then(|(_, values)| values.first())
}

fn str_field<'o>(obj: &'o Obj<'_>, name: &str) -> Option<&'o str> {
    field(obj, name).and_then(Value::get_str)
}

fn obj_field<'o>(obj: &'o Obj<'_>, name: &str) -> Option<&'o Obj<'o>> {
    field(obj, name).and_then(Value::get_obj)
}

/// The accounts in `loginusers.vdf`'s text: each SteamID64 with its
/// `PersonaName` and whether it is `MostRecent`, the signed-in account
/// first. `None` when the text doesn't parse. Never `AccountName`.
pub fn parse_login_users(text: &str) -> Option<Vec<LocalAccount>> {
    let vdf = keyvalues_parser::parse(text).ok()?;
    let users = vdf.value.get_obj()?;
    let mut accounts: Vec<LocalAccount> = users
        .0
        .iter()
        .filter_map(|(id, values)| {
            let steam_id = id.parse::<SteamId>().ok()?;
            let user = values.first()?.get_obj()?;
            Some(LocalAccount {
                steam_id,
                persona: str_field(user, "PersonaName")
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                most_recent: str_field(user, "MostRecent").map(str::trim) == Some("1"),
            })
        })
        .collect();
    accounts.sort_by_key(|account| !account.most_recent);
    Some(accounts)
}

/// `<root>/config/loginusers.vdf`'s accounts; none when it is missing or
/// doesn't parse.
pub fn local_accounts(root: &Path) -> Vec<LocalAccount> {
    let path = root.join("config").join("loginusers.vdf");
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_login_users(&text).unwrap_or_else(|| {
        tracing::debug!("Steam: couldn't read loginusers.vdf");
        Vec::new()
    })
}

/// What `registry.vdf` says is running: `RunningAppID` and every
/// `apps/<appid>` whose `Running` is `"1"`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunningApps {
    pub running_app_id: u32,
    pub running: Vec<u32>,
}

impl RunningApps {
    pub fn has(&self, appid: u32) -> bool {
        appid != 0 && (self.running_app_id == appid || self.running.contains(&appid))
    }
}

/// `registry.vdf`'s text (`Registry/HKCU/Software/Valve/Steam`): the
/// running app. `None` when it doesn't parse. Nothing else is read
/// (never `AutoLoginUser`).
pub fn parse_registry(text: &str) -> Option<RunningApps> {
    let vdf = keyvalues_parser::parse(text).ok()?;
    let mut node = vdf.value.get_obj()?;
    for name in ["HKCU", "Software", "Valve", "Steam"] {
        let Some(next) = obj_field(node, name) else {
            return Some(RunningApps::default());
        };
        node = next;
    }
    let running_app_id = str_field(node, "RunningAppID")
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0);
    let running = obj_field(node, "apps")
        .map(|apps| {
            apps.0
                .iter()
                .filter_map(|(id, values)| {
                    let app = values.first()?.get_obj()?;
                    (str_field(app, "Running").map(str::trim) == Some("1"))
                        .then(|| id.parse().ok())
                        .flatten()
                })
                .collect()
        })
        .unwrap_or_default();
    Some(RunningApps {
        running_app_id,
        running,
    })
}

/// A manifest's `LastPlayed` (Unix seconds), 0 when it has none or the
/// text doesn't parse.
pub fn parse_last_played(text: &str) -> i64 {
    keyvalues_parser::parse(text)
        .ok()
        .and_then(|vdf| {
            vdf.value
                .get_obj()
                .and_then(|app| str_field(app, "LastPlayed"))
                .and_then(|text| text.trim().parse().ok())
        })
        .unwrap_or(0)
}

/// The root's libraries: every `path` in `steamapps/libraryfolders.vdf`
/// plus the root itself, each folder once (the vdf's spelling first).
/// When the vdf is missing or doesn't parse, the root alone.
pub fn library_paths(root: &Path) -> Vec<PathBuf> {
    let listed = steamlocate::SteamDir::from_dir(root)
        .and_then(|dir| dir.library_paths())
        .unwrap_or_else(|_| {
            tracing::debug!("Steam: couldn't read libraryfolders.vdf; scanning the root only");
            Vec::new()
        });
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut libraries = Vec::new();
    for path in listed
        .into_iter()
        .chain(std::iter::once(root.to_path_buf()))
    {
        if path.as_os_str().is_empty() {
            continue;
        }
        let resolved = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.contains(&resolved) {
            continue;
        }
        seen.push(resolved);
        libraries.push(path);
    }
    libraries
}

/// Unix seconds of a `SystemTime`, 0 before 1970.
fn unix_seconds(time: std::time::SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|age| i64::try_from(age.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn manifest_path(library: &Path, appid: u32) -> PathBuf {
    library
        .join("steamapps")
        .join(format!("appmanifest_{appid}.acf"))
}

/// GRID's reading of one steamlocate app.
fn manifest_state(library: &Path, app: steamlocate::App) -> ManifestState {
    let last_played = fs::read_to_string(manifest_path(library, app.app_id))
        .map(|text| parse_last_played(&text))
        .unwrap_or(0);
    ManifestState {
        appid: app.app_id,
        name: app.name.unwrap_or_default(),
        library: library.to_path_buf(),
        install_dir: app.install_dir,
        flags: app
            .state_flags
            .map(|flags| (flags.0 & u64::from(u32::MAX)) as u32)
            .unwrap_or(0),
        size_on_disk: app.size_on_disk.unwrap_or(0),
        build_id: app.build_id.unwrap_or(0),
        last_updated: app.last_updated.map(unix_seconds).unwrap_or(0),
        last_played,
        bytes_to_download: app.bytes_to_download.unwrap_or(0),
        bytes_downloaded: app.bytes_downloaded.unwrap_or(0),
        bytes_to_stage: app.bytes_to_stage.unwrap_or(0),
        bytes_staged: app.bytes_staged.unwrap_or(0),
    }
}

/// One library's manifests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryScan {
    /// Every manifest that parsed.
    pub manifests: Vec<ManifestState>,
    /// App ids whose manifest is there but didn't parse (Steam may be
    /// writing it): the app is skipped, never the library, and a caller
    /// treats it as unchanged since its last scan.
    pub unreadable: Vec<u32>,
}

/// Every `appmanifest_<appid>.acf` in `library/steamapps/`. `None` when
/// that folder can't be read (a missing or unmounted library).
pub fn scan_library(library: &Path) -> Option<LibraryScan> {
    let Ok(lib) = steamlocate::Library::from_dir(library) else {
        tracing::debug!("Steam: a library folder couldn't be read");
        return None;
    };
    let mut scan = LibraryScan::default();
    for &appid in lib.app_ids() {
        match lib.app(appid) {
            Some(Ok(app)) => scan.manifests.push(manifest_state(library, app)),
            Some(Err(_)) => scan.unreadable.push(appid),
            None => {}
        }
    }
    scan.manifests.sort_by_key(|m| m.appid);
    scan.unreadable.sort_unstable();
    Some(scan)
}

/// Every library of a root, scanned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteamScan {
    /// Every manifest that parsed, in every library.
    pub manifests: Vec<ManifestState>,
    /// App ids whose manifest didn't parse ([`LibraryScan::unreadable`]).
    pub unreadable: Vec<u32>,
    /// Libraries whose `steamapps/` couldn't be read (an unplugged drive):
    /// a caller keeps what it knew of their games rather than call them
    /// uninstalled.
    pub missing_libraries: Vec<PathBuf>,
}

/// Every manifest in every library of `root`.
pub fn scan_root(root: &Path) -> SteamScan {
    let mut scan = SteamScan::default();
    for library in library_paths(root) {
        match scan_library(&library) {
            Some(found) => {
                scan.manifests.extend(found.manifests);
                scan.unreadable.extend(found.unreadable);
            }
            None => scan.missing_libraries.push(library),
        }
    }
    scan
}

/// One app's manifest, looked for in every library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestRead {
    Found(ManifestState),
    /// The file is there but didn't parse (Steam may be writing it).
    Unreadable,
    /// No library has it.
    Missing,
}

/// `appid`'s manifest in the first of `libraries` that has the file.
pub fn read_manifest(libraries: &[PathBuf], appid: u32) -> ManifestRead {
    for library in libraries {
        if !manifest_path(library, appid).is_file() {
            continue;
        }
        let found = steamlocate::Library::from_dir(library)
            .ok()
            .and_then(|lib| lib.app(appid));
        return match found {
            Some(Ok(app)) => ManifestRead::Found(manifest_state(library, app)),
            // The file went away between the check and the read.
            None if !manifest_path(library, appid).is_file() => continue,
            _ => ManifestRead::Unreadable,
        };
    }
    ManifestRead::Missing
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOGIN_USERS: &str = r#""users"
{
	"76561198000000001"
	{
		"AccountName"		"FAKE-ACCOUNT-NAME-not-shown"
		"PersonaName"		"Sam \"the\" Player"
		"RememberPassword"		"1"
		"WantsOfflineMode"		"0"
		"SkipOfflineModeWarning"		"0"
		"AllowAutoLogin"		"1"
		"MostRecent"		"1"
		"Timestamp"		"1731500000"
	}
	"76561198000000002"
	{
		"AccountName"		"FAKE-ACCOUNT-NAME-not-shown"
		"PersonaName"		"couchguest"
		"RememberPassword"		"1"
		"mostrecent"		"0"
		"Timestamp"		"1731400000"
	}
	"12345"
	{
		"PersonaName"		"not a SteamID64"
	}
}
"#;

    #[test]
    fn login_users_give_personas_never_account_names() {
        let accounts = parse_login_users(LOGIN_USERS).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].steam_id.to_string(), "76561198000000001");
        assert_eq!(accounts[0].persona, "Sam \"the\" Player");
        assert!(accounts[0].most_recent);
        assert_eq!(accounts[1].persona, "couchguest");
        assert!(!accounts[1].most_recent);
        assert!(!format!("{accounts:?}").contains("FAKE-ACCOUNT-NAME"));
    }

    #[test]
    fn the_signed_in_account_comes_first() {
        let text = LOGIN_USERS
            .replace("\"MostRecent\"\t\t\"1\"", "\"MostRecent\"\t\t\"0\"")
            .replace("\"mostrecent\"\t\t\"0\"", "\"mostrecent\"\t\t\"1\"");
        let accounts = parse_login_users(&text).unwrap();
        assert_eq!(accounts[0].persona, "couchguest");
        assert!(accounts[0].most_recent);
    }

    #[test]
    fn a_broken_login_users_reads_as_nothing() {
        let half = &LOGIN_USERS[..LOGIN_USERS.len() / 2];
        assert_eq!(parse_login_users(half), None);
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("config")).unwrap();
        fs::write(tmp.path().join("config/loginusers.vdf"), half).unwrap();
        assert!(local_accounts(tmp.path()).is_empty());
        assert!(local_accounts(&tmp.path().join("missing")).is_empty());
    }

    const REGISTRY: &str = r#""Registry"
{
	"HKCU"
	{
		"Software"
		{
			"Valve"
			{
				"Steam"
				{
					"AutoLoginUser"		"FAKE-ACCOUNT-NAME-not-shown"
					"RunningAppID"		"620"
					"Apps"
					{
						"620"
						{
							"Running"		"1"
							"Installed"		"1"
						}
						"440"
						{
							"Running"		"0"
						}
						"1245620"
						{
							"running"		"1"
						}
					}
				}
			}
		}
	}
}
"#;

    #[test]
    fn registry_names_the_running_app() {
        let running = parse_registry(REGISTRY).unwrap();
        assert_eq!(running.running_app_id, 620);
        assert_eq!(running.running, vec![1245620, 620]);
        assert!(running.has(620));
        assert!(running.has(1245620));
        assert!(!running.has(440));
        assert!(!running.has(0));
        assert!(!format!("{running:?}").contains("FAKE-ACCOUNT-NAME"));

        let idle = parse_registry(
            &REGISTRY.replace("\"RunningAppID\"\t\t\"620\"", "\"RunningAppID\"\t\t\"0\""),
        )
        .unwrap();
        assert_eq!(idle.running_app_id, 0);

        assert_eq!(
            parse_registry("\"Registry\"\n{\n}\n"),
            Some(RunningApps::default())
        );
        assert_eq!(parse_registry(&REGISTRY[..REGISTRY.len() / 2]), None);
    }

    #[test]
    fn last_played_from_a_manifest() {
        let text =
            "\"AppState\"\n{\n\t\"appid\"\t\t\"620\"\n\t\"LastPlayed\"\t\t\"1731000000\"\n}\n";
        assert_eq!(parse_last_played(text), 1_731_000_000);
        assert_eq!(parse_last_played("\"AppState\"\n{\n}\n"), 0);
        assert_eq!(parse_last_played("\"AppState\"\n{\n\t\"Last"), 0);
    }
}
