//! The GRID-managed store tools (heroic-gogdl for GOG, legendary for Epic,
//! nile for Amazon):
//! their pinned releases and SHA-256s, the install that checks them, and
//! the command that starts one.
//!
//! The gogdl pins come from GitHub's published asset digests for
//! heroic-gogdl v1.4.0 (`docs/stores-phase1-plan.md` "Spike results"); the
//! legendary pins from the 0.21.1 assets themselves, which match GitHub's
//! digests (`docs/stores-phase2-plan.md` "Research"); the nile pins from
//! the v1.2.0 assets, which match GitHub's digests
//! (`docs/stores-phase3-plan.md` "Research"). gogdl's and legendary's
//! Linux assets are Python zipapps, run as `python3 <file>` with the same
//! Python 3.10 floor as GRID's umu-launcher
//! ([`crate::launch::umu::MIN_PYTHON`]); every nile asset is a frozen
//! PyInstaller binary, run as itself.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use futures_util::StreamExt as _;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::paths;
use super::runner::ToolCommand;
use super::StoreError;

/// One store tool GRID downloads and runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreTool {
    Gogdl,
    Legendary,
    Nile,
}

impl StoreTool {
    /// The lowercase name: its folder under `<data_dir>/stores/` and its
    /// serde form.
    pub fn name(self) -> &'static str {
        match self {
            StoreTool::Gogdl => "gogdl",
            StoreTool::Legendary => "legendary",
            StoreTool::Nile => "nile",
        }
    }

    /// The e2e override variable naming a fake tool
    /// (`GRID_LAUNCHER_E2E_TOOL_GOGDL`, `GRID_LAUNCHER_E2E_TOOL_LEGENDARY`,
    /// `GRID_LAUNCHER_E2E_TOOL_NILE`), read only with the `e2e` feature.
    #[cfg(feature = "e2e")]
    pub fn e2e_env(self) -> &'static str {
        match self {
            StoreTool::Gogdl => "GRID_LAUNCHER_E2E_TOOL_GOGDL",
            StoreTool::Legendary => "GRID_LAUNCHER_E2E_TOOL_LEGENDARY",
            StoreTool::Nile => "GRID_LAUNCHER_E2E_TOOL_NILE",
        }
    }
}

/// How a downloaded asset is started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetLauncher {
    /// A native executable, run as itself.
    Direct,
    /// A Python zipapp, run as `python3 <file>` (gogdl and legendary on
    /// Linux).
    Python3,
}

/// One upstream release asset, for one OS and architecture.
#[derive(Debug, PartialEq, Eq)]
pub struct PinnedAsset {
    pub os: &'static str,
    pub arch: &'static str,
    pub url: &'static str,
    pub file_name: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    pub launcher: AssetLauncher,
}

/// A tool's pinned release.
#[derive(Debug, PartialEq, Eq)]
pub struct ToolPin {
    pub tool: StoreTool,
    pub version: &'static str,
    pub license: &'static str,
    pub source_url: &'static str,
    pub assets: &'static [PinnedAsset],
}

macro_rules! gogdl_asset {
    ($os:literal, $arch:literal, $file:literal, $sha:literal, $size:literal, $launcher:expr) => {
        PinnedAsset {
            os: $os,
            arch: $arch,
            url: concat!(
                "https://github.com/Heroic-Games-Launcher/heroic-gogdl/releases/download/v1.4.0/",
                $file
            ),
            file_name: $file,
            sha256: $sha,
            size: $size,
            launcher: $launcher,
        }
    };
}

static GOGDL_ASSETS: [PinnedAsset; 6] = [
    gogdl_asset!(
        "linux",
        "x86_64",
        "gogdl_linux_x86_64",
        "2aa2b59c83d3bfbe7b60ead91c6297b263b5d1e6c233a04b7409bafe26738f02",
        983218,
        AssetLauncher::Python3
    ),
    gogdl_asset!(
        "linux",
        "aarch64",
        "gogdl_linux_arm64",
        "3df71cb07603b5d17fd914c96d17e4b381fad5a5705dc7dd3e3ff7a3856c6cc3",
        975558,
        AssetLauncher::Python3
    ),
    gogdl_asset!(
        "windows",
        "x86_64",
        "gogdl_windows_x86_64.exe",
        "291b653e6dbf7a394a3342ed0ebcea2cbc83e5f66d030777d65ccdd60d18ab02",
        12471749,
        AssetLauncher::Direct
    ),
    gogdl_asset!(
        "windows",
        "aarch64",
        "gogdl_windows_arm64.exe",
        "f3885d51c823d2faaf3b49647c63e799339afdc51fdbd0515e399ca08954630c",
        12296193,
        AssetLauncher::Direct
    ),
    gogdl_asset!(
        "macos",
        "x86_64",
        "gogdl_macos_x86_64",
        "5b7e68497907025549f83f960a6d325e53cf48f5cac63a4f1313318e50dd2de8",
        11720560,
        AssetLauncher::Direct
    ),
    gogdl_asset!(
        "macos",
        "aarch64",
        "gogdl_macos_arm64",
        "af167d29ca822f495ff677d03c40bb9d6dc5535dac1f30d71775c91fe844149c",
        11098928,
        AssetLauncher::Direct
    ),
];

static GOGDL_PIN: ToolPin = ToolPin {
    tool: StoreTool::Gogdl,
    version: "1.4.0",
    license: "GPL-3.0",
    source_url: "https://github.com/Heroic-Games-Launcher/heroic-gogdl",
    assets: &GOGDL_ASSETS,
};

macro_rules! legendary_asset {
    ($os:literal, $arch:literal, $file:literal, $sha:literal, $size:literal, $launcher:expr) => {
        PinnedAsset {
            os: $os,
            arch: $arch,
            url: concat!(
                "https://github.com/legendary-gl/legendary/releases/download/0.21.1/",
                $file
            ),
            file_name: $file,
            sha256: $sha,
            size: $size,
            launcher: $launcher,
        }
    };
}

/// legendary 0.21.1 "Lowlife" (2026-09-08). The project publishes no
/// SHA256SUMS: these were computed from the downloaded assets and match
/// GitHub's per-asset digests (the Windows arm64 one is the API's digest).
static LEGENDARY_ASSETS: [PinnedAsset; 6] = [
    legendary_asset!(
        "linux",
        "x86_64",
        "legendary_linux_x64",
        "dbed33bbe96031e65858233e4badf0a1d401bb6cb0cc995d1237cf3a0c826a45",
        3093622,
        AssetLauncher::Python3
    ),
    legendary_asset!(
        "linux",
        "aarch64",
        "legendary_linux_arm64",
        "e901cb52ada10d5cef6ffb2a0985099958124457bfb455a571023d1e042143d9",
        2999455,
        AssetLauncher::Python3
    ),
    legendary_asset!(
        "windows",
        "x86_64",
        "legendary_windows_x64.exe",
        "6be77857dc0a6dd33ea7442ce8b5291944ec6da53757f81ca95b57ce794baf5b",
        17654404,
        AssetLauncher::Direct
    ),
    legendary_asset!(
        "windows",
        "aarch64",
        "legendary_windows_arm64.exe",
        "5ca178d75713cb92ee24daa5df1905c75f05b8b403383669ca7aa32a5809fba3",
        17147114,
        AssetLauncher::Direct
    ),
    legendary_asset!(
        "macos",
        "x86_64",
        "legendary_macOS_x64",
        "3dfab50277284b5bb0104f8710e4ff1d9ff00c992e44f5843fd0c977c5cc99c4",
        15566864,
        AssetLauncher::Direct
    ),
    legendary_asset!(
        "macos",
        "aarch64",
        "legendary_macOS_arm64",
        "d87978321dba9cb731fab40c72f0a30bed55baca8b5341ab21024c5733cd837e",
        15275984,
        AssetLauncher::Direct
    ),
];

static LEGENDARY_PIN: ToolPin = ToolPin {
    tool: StoreTool::Legendary,
    version: "0.21.1",
    license: "GPL-3.0",
    source_url: "https://github.com/legendary-gl/legendary",
    assets: &LEGENDARY_ASSETS,
};

macro_rules! nile_asset {
    ($os:literal, $arch:literal, $file:literal, $sha:literal, $size:literal) => {
        PinnedAsset {
            os: $os,
            arch: $arch,
            url: concat!(
                "https://github.com/imLinguin/nile/releases/download/v1.2.0/",
                $file
            ),
            file_name: $file,
            sha256: $sha,
            size: $size,
            launcher: AssetLauncher::Direct,
        }
    };
}

/// nile v1.2.0 "Robert Speedwagon" (2026-03-03). The project publishes no
/// SHA256SUMS: these were computed from the downloaded assets and match
/// GitHub's per-asset digests. Every asset is a frozen PyInstaller
/// `--onefile` binary (Python 3.8 inside), so all are `Direct`. There is no
/// Windows arm64 asset. The Linux x86_64 binary's bundled libraries need
/// glibc 2.35 (`docs/stores-phase3-plan.md` "Spike results").
static NILE_ASSETS: [PinnedAsset; 5] = [
    nile_asset!(
        "linux",
        "x86_64",
        "nile_linux_x86_64",
        "d73519514df9d52c50c6feec629373cc3a0d9504e087b6c7372a8ca8a95e7bfe",
        11113264
    ),
    nile_asset!(
        "linux",
        "aarch64",
        "nile_linux_arm64",
        "197859c629c47e4e4d7ed1d2a290b7bef1e778f891ff8ca42ba65dbb020ba751",
        10847400
    ),
    nile_asset!(
        "windows",
        "x86_64",
        "nile_windows_x86_64.exe",
        "6531790c59f78cea4a8743bf0582d5afda7fb887f5c143391d7339ad0f42ab88",
        12386046
    ),
    nile_asset!(
        "macos",
        "x86_64",
        "nile_macOS_x86_64",
        "93ef4a5d29a378901569e3a854c289d356a0be3503700ebf7ee70586d401b0b0",
        10002608
    ),
    nile_asset!(
        "macos",
        "aarch64",
        "nile_macOS_arm64",
        "6cc74eb19117c8c309b0b94657e1e267642a3b4840d7386a133dcf6f3bffeab0",
        8348560
    ),
];

static NILE_PIN: ToolPin = ToolPin {
    tool: StoreTool::Nile,
    version: "1.2.0",
    license: "GPL-3.0",
    source_url: "https://github.com/imLinguin/nile",
    assets: &NILE_ASSETS,
};

/// The tool's pinned release.
pub fn pin(tool: StoreTool) -> &'static ToolPin {
    match tool {
        StoreTool::Gogdl => &GOGDL_PIN,
        StoreTool::Legendary => &LEGENDARY_PIN,
        StoreTool::Nile => &NILE_PIN,
    }
}

/// The asset for `os`/`arch` (`std::env::consts` names).
pub fn asset_for(tool: StoreTool, os: &str, arch: &str) -> Option<&'static PinnedAsset> {
    pin(tool)
        .assets
        .iter()
        .find(|asset| asset.os == os && asset.arch == arch)
}

/// The asset for this computer's OS and architecture, `None` when the tool
/// has none for it.
pub fn host_asset(tool: StoreTool) -> Option<&'static PinnedAsset> {
    asset_for(tool, std::env::consts::OS, std::env::consts::ARCH)
}

/// The fake tool the e2e harness names, when the variable is set and the
/// file exists. Always `None` without the `e2e` feature.
fn e2e_override(tool: StoreTool) -> Option<PathBuf> {
    #[cfg(feature = "e2e")]
    {
        let raw = std::env::var_os(tool.e2e_env())?;
        let path = PathBuf::from(raw);
        if path.as_os_str().is_empty() || !path.is_file() {
            return None;
        }
        Some(path)
    }
    #[cfg(not(feature = "e2e"))]
    {
        let _ = tool;
        None
    }
}

/// The installed tool's executable, `None` when it is not installed (or
/// the file there is not the pinned size). With the `e2e` feature,
/// `GRID_LAUNCHER_E2E_TOOL_<TOOL>` names a fake tool.
pub fn installed_path(data_dir: &Path, tool: StoreTool) -> Option<PathBuf> {
    if let Some(fake) = e2e_override(tool) {
        return Some(fake);
    }
    let asset = host_asset(tool)?;
    let path = paths::tool_dir(data_dir, tool).join(asset.file_name);
    let meta = fs::metadata(&path).ok()?;
    (meta.is_file() && meta.len() == asset.size).then_some(path)
}

/// Downloads the host asset into the tool's folder and checks its SHA-256
/// and size. A mismatch deletes the file and fails with
/// [`StoreError::Refused`]. `progress(done, total)`.
pub async fn install_tool(
    http: &reqwest::Client,
    data_dir: &Path,
    tool: StoreTool,
    progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<PathBuf, StoreError> {
    if let Some(fake) = e2e_override(tool) {
        progress(1, 1);
        return Ok(fake);
    }
    let asset = host_asset(tool).ok_or_else(|| {
        StoreError::Refused(format!(
            "{} has no build for this computer ({} {}).",
            tool.name(),
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    })?;
    let dir = paths::tool_dir(data_dir, tool);
    paths::ensure_private_dir(&dir).map_err(|e| io_refusal(tool, &e))?;
    let dest = dir.join(asset.file_name);
    download_verified(
        http,
        asset.url,
        &dest,
        asset.sha256,
        asset.size,
        tool,
        progress,
    )
    .await?;
    let made = match asset.launcher {
        AssetLauncher::Direct => paths::make_private_executable(&dest),
        AssetLauncher::Python3 => paths::make_private_file(&dest),
    };
    made.map_err(|e| io_refusal(tool, &e))?;
    Ok(dest)
}

fn io_refusal(tool: StoreTool, error: &std::io::Error) -> StoreError {
    StoreError::Refused(format!("Couldn't save {} ({}).", tool.name(), error.kind()))
}

/// Downloads `url` to `dest` through a `.part` file, checking the size and
/// the SHA-256 (lowercase hex). On any mismatch the partial file is
/// deleted and nothing is left at `dest`.
pub(crate) async fn download_verified(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    sha256: &str,
    size: u64,
    tool: StoreTool,
    progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<(), StoreError> {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "the download server".to_string());
    let part = dest.with_extension("part");
    let result = async {
        let resp = http
            .get(url)
            .send()
            .await
            .map_err(|_| StoreError::Network(host.clone()))?;
        if !resp.status().is_success() {
            return Err(StoreError::Network(host.clone()));
        }
        let mut file = fs::File::create(&part).map_err(|e| io_refusal(tool, &e))?;
        let mut hasher = Sha256::new();
        let mut done = 0u64;
        let mut stream = resp.bytes_stream();
        progress(0, size);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| StoreError::Network(host.clone()))?;
            done += chunk.len() as u64;
            if done > size {
                return Err(mismatch(tool));
            }
            hasher.update(&chunk);
            file.write_all(&chunk).map_err(|e| io_refusal(tool, &e))?;
            progress(done, size);
        }
        file.sync_all().map_err(|e| io_refusal(tool, &e))?;
        drop(file);
        let digest = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if done != size || !digest.eq_ignore_ascii_case(sha256) {
            return Err(mismatch(tool));
        }
        fs::rename(&part, dest).map_err(|e| io_refusal(tool, &e))
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&part);
        let _ = fs::remove_file(dest);
    }
    result
}

fn mismatch(tool: StoreTool) -> StoreError {
    StoreError::Refused(format!(
        "The {} download didn't match its checksum, so GRID deleted it.",
        tool.name()
    ))
}

/// The words for a Python that can't run `tool`'s Linux zipapp, `None`
/// when `found` (`(major, minor)`, `None` = no `python3`) meets
/// [`crate::launch::umu::MIN_PYTHON`].
pub fn python_problem(tool: StoreTool, found: Option<(u32, u32)>) -> Option<String> {
    let (major_min, minor_min) = crate::launch::umu::MIN_PYTHON;
    let name = tool.name();
    match found {
        Some(version) if version >= crate::launch::umu::MIN_PYTHON => None,
        Some((major, minor)) => Some(format!(
            "{name} needs Python {major_min}.{minor_min} or newer, but python3 is {major}.{minor}."
        )),
        None => Some(format!(
            "{name} needs Python {major_min}.{minor_min} or newer, and python3 was not found."
        )),
    }
}

/// The command that runs the installed `tool` with `args` and `env`:
/// the file itself, or `python3 <file>` for a zipapp (after the Python
/// check). [`StoreError::ToolMissing`] when it is not installed.
pub fn tool_command(
    data_dir: &Path,
    tool: StoreTool,
    args: Vec<String>,
    env: Vec<(String, String)>,
) -> Result<ToolCommand, StoreError> {
    let path = installed_path(data_dir, tool).ok_or(StoreError::ToolMissing(tool.name()))?;
    let launcher = if e2e_override(tool).is_some() {
        AssetLauncher::Direct
    } else {
        host_asset(tool).map_or(AssetLauncher::Direct, |asset| asset.launcher)
    };
    match launcher {
        AssetLauncher::Direct => Ok(ToolCommand {
            program: path,
            args,
            env,
            ..Default::default()
        }),
        AssetLauncher::Python3 => {
            let child_env = crate::launch::spawn::clean_env();
            let python = child_env.get("PATH").and_then(|p| {
                crate::library::extract::which_in(std::ffi::OsStr::new(p), "python3")
            });
            let found = python
                .as_ref()
                .and_then(|_| crate::launch::umu::probe_python(&child_env));
            if let Some(problem) = python_problem(tool, found) {
                return Err(StoreError::Refused(problem));
            }
            let mut full = Vec::with_capacity(args.len() + 1);
            full.push(path.to_string_lossy().into_owned());
            full.extend(args);
            Ok(ToolCommand {
                program: python.unwrap_or_else(|| PathBuf::from("python3")),
                args: full,
                env,
                ..Default::default()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn every_asset_is_pinned_with_a_digest() {
        let pin = pin(StoreTool::Gogdl);
        assert_eq!(pin.version, "1.4.0");
        assert_eq!(pin.assets.len(), 6);
        for asset in pin.assets {
            assert_eq!(asset.sha256.len(), 64, "{}", asset.file_name);
            assert!(asset.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(asset.url.ends_with(asset.file_name));
            assert!(asset.url.contains("/v1.4.0/"));
            assert!(asset.size > 0);
        }
        let linux = asset_for(StoreTool::Gogdl, "linux", "x86_64").unwrap();
        assert_eq!(linux.launcher, AssetLauncher::Python3);
        let win = asset_for(StoreTool::Gogdl, "windows", "x86_64").unwrap();
        assert_eq!(win.launcher, AssetLauncher::Direct);
        assert_eq!(win.size, 12471749);
        assert!(asset_for(StoreTool::Gogdl, "freebsd", "x86_64").is_none());
    }

    #[test]
    fn legendary_is_pinned_to_0_21_1() {
        let pin = pin(StoreTool::Legendary);
        assert_eq!(pin.version, "0.21.1");
        assert_eq!(pin.license, "GPL-3.0");
        assert_eq!(pin.source_url, "https://github.com/legendary-gl/legendary");
        assert_eq!(pin.assets.len(), 6);
        for asset in pin.assets {
            assert_eq!(asset.sha256.len(), 64, "{}", asset.file_name);
            assert!(asset.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(asset.url.ends_with(asset.file_name));
            assert!(asset
                .url
                .contains("/legendary-gl/legendary/releases/download/0.21.1/"));
            assert!(asset.size > 0);
        }
        let linux = asset_for(StoreTool::Legendary, "linux", "x86_64").unwrap();
        assert_eq!(linux.file_name, "legendary_linux_x64");
        assert_eq!(linux.size, 3093622);
        assert_eq!(linux.launcher, AssetLauncher::Python3);
        let arm = asset_for(StoreTool::Legendary, "linux", "aarch64").unwrap();
        assert_eq!(arm.launcher, AssetLauncher::Python3);
        for (os, arch) in [
            ("windows", "x86_64"),
            ("windows", "aarch64"),
            ("macos", "x86_64"),
            ("macos", "aarch64"),
        ] {
            let asset = asset_for(StoreTool::Legendary, os, arch).unwrap();
            assert_eq!(asset.launcher, AssetLauncher::Direct, "{os} {arch}");
        }
        assert_eq!(StoreTool::Legendary.name(), "legendary");
        assert_eq!(
            serde_json::to_string(&StoreTool::Legendary).unwrap(),
            r#""legendary""#
        );
    }

    #[test]
    fn nile_is_pinned_to_1_2_0() {
        let pin = pin(StoreTool::Nile);
        assert_eq!(pin.version, "1.2.0");
        assert_eq!(pin.license, "GPL-3.0");
        assert_eq!(pin.source_url, "https://github.com/imLinguin/nile");
        assert_eq!(pin.assets.len(), 5);
        for asset in pin.assets {
            assert_eq!(asset.sha256.len(), 64, "{}", asset.file_name);
            assert!(asset.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(asset.url.ends_with(asset.file_name));
            assert!(asset
                .url
                .contains("/imLinguin/nile/releases/download/v1.2.0/"));
            assert!(asset.size > 0);
            assert_eq!(asset.launcher, AssetLauncher::Direct, "{}", asset.file_name);
        }
        let linux = asset_for(StoreTool::Nile, "linux", "x86_64").unwrap();
        assert_eq!(linux.file_name, "nile_linux_x86_64");
        assert_eq!(linux.size, 11113264);
        assert!(asset_for(StoreTool::Nile, "windows", "x86_64").is_some());
        assert!(
            asset_for(StoreTool::Nile, "windows", "aarch64").is_none(),
            "nile ships no Windows arm64 build"
        );
        assert_eq!(StoreTool::Nile.name(), "nile");
        assert_eq!(
            serde_json::to_string(&StoreTool::Nile).unwrap(),
            r#""nile""#
        );
    }

    #[test]
    fn python_floor_words() {
        assert_eq!(python_problem(StoreTool::Gogdl, Some((3, 10))), None);
        assert_eq!(python_problem(StoreTool::Gogdl, Some((3, 13))), None);
        assert!(python_problem(StoreTool::Gogdl, Some((3, 9)))
            .unwrap()
            .contains("python3 is 3.9"));
        assert!(python_problem(StoreTool::Gogdl, None)
            .unwrap()
            .contains("not found"));
        assert!(python_problem(StoreTool::Legendary, None)
            .unwrap()
            .starts_with("legendary needs Python 3.10"));
    }

    fn sha_hex(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[tokio::test]
    async fn a_good_download_lands_and_a_bad_one_is_deleted() {
        let server = MockServer::start().await;
        let body = b"#!/usr/bin/env python3\nfake gogdl\n".to_vec();
        Mock::given(method("GET"))
            .and(path("/gogdl"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("gogdl_linux_x86_64");
        let url = format!("{}/gogdl", server.uri());

        let mut seen = Vec::new();
        download_verified(
            &http,
            &url,
            &dest,
            &sha_hex(&body),
            body.len() as u64,
            StoreTool::Gogdl,
            &mut |d, t| seen.push((d, t)),
        )
        .await
        .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert_eq!(seen.last(), Some(&(body.len() as u64, body.len() as u64)));

        fs::remove_file(&dest).unwrap();
        let wrong = sha_hex(b"something else");
        let error = download_verified(
            &http,
            &url,
            &dest,
            &wrong,
            body.len() as u64,
            StoreTool::Gogdl,
            &mut |_, _| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(error, StoreError::Refused(_)), "{error}");
        assert!(error.to_string().contains("checksum"));
        assert!(!dest.exists(), "a mismatch deletes the file");
        assert!(!dest.with_extension("part").exists());

        let error = download_verified(
            &http,
            &url,
            &dest,
            &sha_hex(&body),
            body.len() as u64 - 1,
            StoreTool::Gogdl,
            &mut |_, _| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(error, StoreError::Refused(_)));
        assert!(!dest.exists(), "a size mismatch deletes the file");
    }

    #[test]
    fn an_absent_or_wrong_sized_file_is_not_installed() {
        let data = tempfile::tempdir().unwrap();
        let Some(asset) = host_asset(StoreTool::Gogdl) else {
            return;
        };
        #[cfg(feature = "e2e")]
        let _lock = crate::test_env::lock();
        assert_eq!(installed_path(data.path(), StoreTool::Gogdl), None);
        let dir = paths::tool_dir(data.path(), StoreTool::Gogdl);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(asset.file_name), b"short").unwrap();
        assert_eq!(installed_path(data.path(), StoreTool::Gogdl), None);
        assert!(matches!(
            tool_command(data.path(), StoreTool::Gogdl, Vec::new(), Vec::new()),
            Err(StoreError::ToolMissing("gogdl"))
        ));
    }
}
