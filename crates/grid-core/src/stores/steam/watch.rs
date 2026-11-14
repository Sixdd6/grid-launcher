//! Watching what Steam does: whether a game started through Steam is
//! running (the watched session's [`AliveCheck`]), and how far an install
//! Steam runs has come (`docs/stores-phase4-plan.md` "Is the game running"
//! and "Install progress and completion").
//!
//! Both decisions are pure step functions with the clock passed in
//! ([`AliveMachine::step`], [`InstallWatch::step`]), so tests drive them
//! without waiting; the app drives the polling.
//!
//! Is it running:
//! - Linux and macOS: `registry.vdf` (`RunningAppID`, or
//!   `apps/<appid>/Running` = `"1"`), read again only when its mtime or
//!   size changed;
//! - Linux, only when no `registry.vdf` exists: one pass over
//!   `/proc/*/cmdline` for Steam's `SteamLaunch AppId=<appid>` wrapper;
//! - Windows: `HKCU\Software\Valve\Steam` `RunningAppID`, or
//!   `Apps\<appid>\Running` = 1.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use super::filter;
use super::local::{self, ManifestRead, RunningApps};
use super::AppPhase;
use crate::launch::AliveCheck;

/// How long a watched session waits for Steam to start the game (Steam
/// may first have to start, sign in or finish an update).
pub const STEAM_START_GRACE: Duration = Duration::from_secs(180);

/// Misses in a row that end a running session (5 s at the 2.5 s reap).
pub const STEAM_END_MISSES: u8 = 2;

/// How long an install watch waits for Steam's manifest to appear (the
/// user may have closed Steam's install dialog). 5 minutes since user
/// decision 4 (`docs/stores-phase4-plan.md`); the row's words say so.
pub const STEAM_INSTALL_START_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// How often the app polls a live install watch.
pub const STEAM_INSTALL_POLL: Duration = Duration::from_secs(2);

/// Where a watched session is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlivePhase {
    /// Not seen running yet; alive until [`STEAM_START_GRACE`].
    Starting,
    /// Seen running; `misses` polls in a row have not seen it since.
    Running { misses: u8 },
    /// Over for good.
    Ended,
}

/// The watched session's state machine: Starting → Running → Ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AliveMachine {
    pub phase: AlivePhase,
}

impl Default for AliveMachine {
    fn default() -> Self {
        AliveMachine {
            phase: AlivePhase::Starting,
        }
    }
}

impl AliveMachine {
    /// One poll: `seen` is whether the game is running now, `elapsed` the
    /// time since the launch. Answers whether the session is still alive.
    /// A game never seen within the grace ends silently; a running one
    /// ends after [`STEAM_END_MISSES`] misses in a row.
    pub fn step(&mut self, seen: bool, elapsed: Duration) -> bool {
        self.phase = match self.phase {
            AlivePhase::Starting if seen => AlivePhase::Running { misses: 0 },
            AlivePhase::Starting if elapsed >= STEAM_START_GRACE => AlivePhase::Ended,
            AlivePhase::Starting => AlivePhase::Starting,
            AlivePhase::Running { .. } if seen => AlivePhase::Running { misses: 0 },
            AlivePhase::Running { misses } if misses + 1 >= STEAM_END_MISSES => AlivePhase::Ended,
            AlivePhase::Running { misses } => AlivePhase::Running { misses: misses + 1 },
            AlivePhase::Ended => AlivePhase::Ended,
        };
        self.phase != AlivePhase::Ended
    }
}

/// Where `registry.vdf` may be for a root, in order: `<base>/.steam/` for
/// a root under `<base>/.local/share/Steam` (native, Flatpak, Snap), the
/// root's parent (`~/.steam/` for `~/.steam/steam`), then the root itself
/// (macOS keeps it there; a test root may too).
pub fn registry_vdf_candidates(root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let parts: Vec<&std::ffi::OsStr> = root.iter().collect();
    if parts.len() >= 3 && parts[parts.len() - 3..] == [".local", "share", "Steam"] {
        if let Some(base) = root.ancestors().nth(3) {
            candidates.push(base.join(".steam").join("registry.vdf"));
        }
    }
    if let Some(parent) = root.parent() {
        candidates.push(parent.join("registry.vdf"));
    }
    candidates.push(root.join("registry.vdf"));
    candidates.dedup();
    candidates
}

/// `registry.vdf`, parsed again only when its mtime or size changed. A
/// file that doesn't parse (Steam writing it) keeps the last answer.
#[derive(Debug, Default)]
pub struct RegistryProbe {
    candidates: Vec<PathBuf>,
    cached: Option<(PathBuf, SystemTime, u64, RunningApps)>,
}

impl RegistryProbe {
    pub fn new(root: &Path) -> RegistryProbe {
        RegistryProbe {
            candidates: registry_vdf_candidates(root),
            cached: None,
        }
    }

    /// What is running, `None` when no `registry.vdf` exists.
    pub fn running(&mut self) -> Option<RunningApps> {
        let (path, meta) = self.candidates.iter().find_map(|path| {
            std::fs::metadata(path)
                .ok()
                .filter(|meta| meta.is_file())
                .map(|meta| (path.clone(), meta))
        })?;
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let size = meta.len();
        if let Some((cached_path, cached_time, cached_size, apps)) = &self.cached {
            if *cached_path == path && *cached_time == modified && *cached_size == size {
                return Some(apps.clone());
            }
        }
        let parsed = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| local::parse_registry(&text));
        match parsed {
            Some(apps) => {
                self.cached = Some((path, modified, size, apps.clone()));
                Some(apps)
            }
            // Unreadable now: the last answer stands (and is read again
            // next time, since the cache keeps the old mtime).
            None => Some(
                self.cached
                    .as_ref()
                    .map(|(_, _, _, apps)| apps.clone())
                    .unwrap_or_default(),
            ),
        }
    }
}

/// Whether a process's `cmdline` (NUL-separated) is Steam's launch
/// wrapper for `appid`: the two adjacent arguments `SteamLaunch` and
/// `AppId=<appid>`.
pub fn cmdline_launches(cmdline: &[u8], appid: u32) -> bool {
    let want = format!("AppId={appid}");
    let args: Vec<&[u8]> = cmdline.split(|b| *b == 0).collect();
    args.windows(2)
        .any(|pair| pair[0] == b"SteamLaunch" && pair[1] == want.as_bytes())
}

/// One pass over `/proc/*/cmdline` for Steam's wrapper of `appid`.
#[cfg(target_os = "linux")]
pub fn proc_has_launch(appid: u32) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name();
        let is_pid = name
            .to_str()
            .is_some_and(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()));
        is_pid
            && std::fs::read(entry.path().join("cmdline"))
                .is_ok_and(|cmdline| cmdline_launches(&cmdline, appid))
    })
}

/// Whether `appid` runs, by `HKCU\Software\Valve\Steam`.
#[cfg(windows)]
fn windows_running(appid: u32) -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;
    let Ok(steam) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Software\\Valve\\Steam", KEY_READ)
    else {
        return false;
    };
    if steam.get_value::<u32, _>("RunningAppID").ok() == Some(appid) {
        return true;
    }
    steam
        .open_subkey_with_flags(format!("Apps\\{appid}"), KEY_READ)
        .and_then(|app| app.get_value::<u32, _>("Running"))
        .is_ok_and(|running| running == 1)
}

/// Whether `appid` runs now, by this OS's rule (module docs).
fn seen_running(probe: &mut RegistryProbe, appid: u32) -> bool {
    #[cfg(windows)]
    {
        let _ = probe;
        windows_running(appid)
    }
    #[cfg(not(windows))]
    {
        match probe.running() {
            Some(apps) => apps.has(appid),
            #[cfg(target_os = "linux")]
            None => proc_has_launch(appid),
            #[cfg(not(target_os = "linux"))]
            None => false,
        }
    }
}

/// The watched session's check for `appid` under `root`: cheap (a stat,
/// and a small read only when `registry.vdf` changed) and never blocks.
pub fn alive_check(root: &Path, appid: u32) -> AliveCheck {
    let mut probe = RegistryProbe::new(root);
    let mut machine = AliveMachine::default();
    let started = Instant::now();
    Box::new(move || {
        if machine.phase == AlivePhase::Ended {
            return false;
        }
        let seen = seen_running(&mut probe, appid);
        machine.step(seen, started.elapsed())
    })
}

/// What one poll of an install watch found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEvent {
    /// Installing: `(done, total)` bytes.
    Progress(u64, u64),
    /// Steam finished the install.
    Installed,
    /// The install went away before it finished.
    Cancelled,
    /// No manifest within the start timeout: Steam never started (the user
    /// closed Steam's dialog, or left it open).
    TimedOut,
    /// No manifest yet, or one poll missed it after it was seen.
    Waiting,
}

/// Polls in a row without an installing manifest, once the install was
/// seen, that end an install watch Cancelled (one poll that catches Steam
/// between writes does not).
pub const STEAM_INSTALL_END_MISSES: u8 = 2;

/// An install Steam runs, as GRID follows it after `steam://install`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallWatch {
    /// The install was seen under way.
    seen: bool,
    /// The last progress seen.
    last: Option<(u64, u64)>,
    /// Polls in a row, since it was seen, without an installing manifest.
    misses: u8,
    /// How long to wait for the manifest to appear.
    start_timeout: Duration,
}

impl Default for InstallWatch {
    fn default() -> InstallWatch {
        InstallWatch {
            seen: false,
            last: None,
            misses: 0,
            start_timeout: STEAM_INSTALL_START_TIMEOUT,
        }
    }
}

impl InstallWatch {
    /// A watch for an install already under way (found at startup, or one
    /// Steam started by itself).
    pub fn resumed() -> InstallWatch {
        InstallWatch {
            seen: true,
            ..InstallWatch::default()
        }
    }

    /// The same watch with another start timeout (tests).
    pub fn with_start_timeout(self, start_timeout: Duration) -> InstallWatch {
        InstallWatch {
            start_timeout,
            ..self
        }
    }

    /// One poll: what the manifest says now, `elapsed` since the link was
    /// opened.
    /// - installed: [`WatchEvent::Installed`];
    /// - installing: its progress;
    /// - a manifest Steam is writing (unreadable): unchanged since the last
    ///   poll;
    /// - missing or gone after it was seen installing: waiting once, then
    ///   cancelled after [`STEAM_INSTALL_END_MISSES`] polls in a row;
    /// - missing or gone before that: waiting, until the start timeout
    ///   ([`STEAM_INSTALL_START_TIMEOUT`]), then timed out.
    pub fn step(&mut self, read: &ManifestRead, elapsed: Duration) -> WatchEvent {
        match read {
            ManifestRead::Found(m) => match filter::phase(m.flags) {
                AppPhase::Installed => WatchEvent::Installed,
                AppPhase::Installing => {
                    self.seen = true;
                    self.misses = 0;
                    let (done, total) = filter::progress(m);
                    self.last = Some((done, total));
                    WatchEvent::Progress(done, total)
                }
                AppPhase::Gone => self.not_there(elapsed),
            },
            ManifestRead::Unreadable => match self.last {
                Some((done, total)) => WatchEvent::Progress(done, total),
                None if self.seen => WatchEvent::Progress(0, 0),
                None => WatchEvent::Waiting,
            },
            ManifestRead::Missing => self.not_there(elapsed),
        }
    }

    fn not_there(&mut self, elapsed: Duration) -> WatchEvent {
        if self.seen {
            self.misses = self.misses.saturating_add(1);
            if self.misses >= STEAM_INSTALL_END_MISSES {
                WatchEvent::Cancelled
            } else {
                WatchEvent::Waiting
            }
        } else if elapsed >= self.start_timeout {
            WatchEvent::TimedOut
        } else {
            WatchEvent::Waiting
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::steam::ManifestState;

    const S: fn(u64) -> Duration = Duration::from_secs;

    #[test]
    fn a_session_waits_for_the_grace_then_ends_silently() {
        let mut machine = AliveMachine::default();
        assert!(machine.step(false, S(0)));
        assert!(machine.step(false, S(179)));
        assert!(!machine.step(false, S(180)));
        assert_eq!(machine.phase, AlivePhase::Ended);
        // Ended is for good.
        assert!(!machine.step(true, S(181)));
    }

    #[test]
    fn a_running_session_ends_after_two_misses_in_a_row() {
        let mut machine = AliveMachine::default();
        assert!(machine.step(false, S(30)));
        assert!(machine.step(true, S(40)));
        assert!(machine.step(false, S(42)), "one miss");
        assert!(machine.step(true, S(45)), "seen again: misses reset");
        assert!(machine.step(false, S(47)));
        assert!(!machine.step(false, S(50)), "two misses");
        assert!(!machine.step(true, S(52)));
        // Seen past the grace still counts: the grace is only for starting.
        let mut late = AliveMachine::default();
        assert!(late.step(true, S(179)));
        assert!(late.step(true, S(1000)));
    }

    #[test]
    fn registry_vdf_places() {
        let shown = |root: &str| registry_vdf_candidates(Path::new(root));
        assert_eq!(
            shown("/home/u/.local/share/Steam"),
            [
                PathBuf::from("/home/u/.steam/registry.vdf"),
                PathBuf::from("/home/u/.local/share/registry.vdf"),
                PathBuf::from("/home/u/.local/share/Steam/registry.vdf"),
            ]
        );
        assert_eq!(
            shown("/home/u/.var/app/com.valvesoftware.Steam/.local/share/Steam")[0],
            PathBuf::from("/home/u/.var/app/com.valvesoftware.Steam/.steam/registry.vdf")
        );
        assert_eq!(
            shown("/home/u/.steam/steam")[0],
            PathBuf::from("/home/u/.steam/registry.vdf")
        );
        assert_eq!(
            shown("/tmp/fake-root").last().unwrap(),
            &PathBuf::from("/tmp/fake-root/registry.vdf")
        );
    }

    fn registry(running: u32) -> String {
        format!(
            "\"Registry\"\n{{\n\t\"HKCU\"\n\t{{\n\t\t\"Software\"\n\t\t{{\n\t\t\t\"Valve\"\n\t\t\t{{\n\t\t\t\t\"Steam\"\n\t\t\t\t{{\n\t\t\t\t\t\"RunningAppID\"\t\t\"{running}\"\n\t\t\t\t}}\n\t\t\t}}\n\t\t}}\n\t}}\n}}\n"
        )
    }

    #[test]
    fn the_probe_rereads_only_on_change_and_keeps_the_last_answer() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        let mut probe = RegistryProbe::new(&root);
        assert_eq!(probe.running(), None, "no registry.vdf");
        let file = root.join("registry.vdf");
        std::fs::write(&file, registry(620)).unwrap();
        assert!(probe.running().unwrap().has(620));
        // Half-written: the last answer stands.
        std::fs::write(&file, "\"Registry\"\n{\n\t\"HKCU").unwrap();
        assert!(probe.running().unwrap().has(620));
        std::fs::write(&file, registry(1234567)).unwrap();
        let now = probe.running().unwrap();
        assert!(now.has(1234567) && !now.has(620));
    }

    // Windows reads HKCU, not registry.vdf.
    #[cfg(not(windows))]
    #[test]
    fn the_alive_check_follows_registry_vdf() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("registry.vdf");
        std::fs::write(&file, registry(0)).unwrap();
        let mut alive = alive_check(&root, 7_000_001);
        assert!(alive(), "starting");
        std::fs::write(&file, registry(7_000_001)).unwrap();
        assert!(alive(), "running");
        std::fs::write(&file, registry(0)).unwrap();
        assert!(alive(), "one miss");
        assert!(!alive(), "two misses");
        assert!(!alive());
    }

    #[test]
    fn steam_launch_wrapper_cmdlines() {
        let cmd =
            b"/home/u/.local/share/Steam/ubuntu12_32/reaper\0SteamLaunch\0AppId=620\0--\0/game\0";
        assert!(cmdline_launches(cmd, 620));
        assert!(!cmdline_launches(cmd, 62));
        assert!(!cmdline_launches(cmd, 6200));
        assert!(!cmdline_launches(b"reaper\0AppId=620\0SteamLaunch\0", 620));
        assert!(!cmdline_launches(b"sh\0-c\0SteamLaunch AppId=620\0", 620));
        assert!(!cmdline_launches(b"", 620));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_proc_pass_finds_a_live_wrapper() {
        let appid = 7_654_321;
        assert!(!proc_has_launch(appid));
        let mut child = std::process::Command::new("sh")
            .args([
                "-c",
                "sleep 30; true",
                "SteamLaunch",
                &format!("AppId={appid}"),
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut found = false;
        for _ in 0..50 {
            if proc_has_launch(appid) {
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(found);
        assert!(!proc_has_launch(appid));
    }

    fn manifest(flags: u32, done: u64, total: u64) -> ManifestRead {
        ManifestRead::Found(ManifestState {
            appid: 620,
            flags,
            bytes_to_download: total,
            bytes_downloaded: done,
            ..Default::default()
        })
    }

    #[test]
    fn an_install_runs_to_installed() {
        let mut watch = InstallWatch::default();
        assert_eq!(
            watch.step(&ManifestRead::Missing, S(2)),
            WatchEvent::Waiting
        );
        assert_eq!(watch.step(&manifest(1, 0, 0), S(4)), WatchEvent::Waiting);
        assert_eq!(
            watch.step(&manifest(1026, 10, 100), S(6)),
            WatchEvent::Progress(10, 100)
        );
        assert_eq!(
            watch.step(&ManifestRead::Unreadable, S(8)),
            WatchEvent::Progress(10, 100),
            "being written: unchanged"
        );
        assert_eq!(
            watch.step(&manifest(1026 | 512, 50, 100), S(10)),
            WatchEvent::Progress(50, 100),
            "paused stays installing"
        );
        assert_eq!(watch.step(&manifest(4, 0, 0), S(12)), WatchEvent::Installed);
    }

    #[test]
    fn an_install_that_never_starts_times_out_and_one_that_goes_away_is_cancelled() {
        // Five minutes since user decision 4 (was 15).
        assert_eq!(STEAM_INSTALL_START_TIMEOUT, S(5 * 60));
        let mut never = InstallWatch::default();
        assert_eq!(
            never.step(&ManifestRead::Unreadable, S(5)),
            WatchEvent::Waiting
        );
        assert_eq!(
            never.step(&ManifestRead::Missing, S(5 * 60 - 1)),
            WatchEvent::Waiting
        );
        assert_eq!(
            never.step(&ManifestRead::Missing, S(5 * 60)),
            WatchEvent::TimedOut
        );

        let mut short = InstallWatch::default().with_start_timeout(S(60));
        assert_eq!(
            short.step(&ManifestRead::Missing, S(1)),
            WatchEvent::Waiting
        );
        assert_eq!(
            short.step(&ManifestRead::Missing, S(61)),
            WatchEvent::TimedOut,
            "the dialog was closed"
        );

        // Seen under way before the timeout: the timeout no longer applies.
        let mut late = InstallWatch::default();
        assert_eq!(late.step(&ManifestRead::Missing, S(2)), WatchEvent::Waiting);
        assert_eq!(
            late.step(&manifest(1026, 0, 10), S(4 * 60)),
            WatchEvent::Progress(0, 10),
            "waiting, then progress"
        );
        assert_eq!(
            late.step(&manifest(1026, 5, 10), S(10 * 60)),
            WatchEvent::Progress(5, 10)
        );
        assert_eq!(
            late.step(&ManifestRead::Missing, S(11 * 60)),
            WatchEvent::Waiting
        );
        assert_eq!(
            late.step(&ManifestRead::Missing, S(11 * 60 + 2)),
            WatchEvent::Cancelled,
            "gone after it was seen is Cancelled, never TimedOut"
        );

        let mut gone = InstallWatch::default();
        assert_eq!(
            gone.step(&manifest(1026, 1, 10), S(3)),
            WatchEvent::Progress(1, 10)
        );
        assert_eq!(
            gone.step(&ManifestRead::Missing, S(5)),
            WatchEvent::Waiting,
            "one miss"
        );
        assert_eq!(
            gone.step(&manifest(1026, 2, 10), S(7)),
            WatchEvent::Progress(2, 10),
            "seen again: misses reset"
        );
        assert_eq!(gone.step(&ManifestRead::Missing, S(9)), WatchEvent::Waiting);
        assert_eq!(
            gone.step(&ManifestRead::Missing, S(11)),
            WatchEvent::Cancelled
        );

        let mut uninstalling = InstallWatch::default();
        uninstalling.step(&manifest(1026, 1, 10), S(3));
        assert_eq!(
            uninstalling.step(&manifest(2048, 0, 0), S(5)),
            WatchEvent::Waiting
        );
        assert_eq!(
            uninstalling.step(&manifest(2048, 0, 0), S(7)),
            WatchEvent::Cancelled
        );
    }

    #[test]
    fn a_resumed_watch_was_already_seen() {
        let mut watch = InstallWatch::resumed();
        assert_eq!(
            watch.step(&ManifestRead::Unreadable, S(0)),
            WatchEvent::Progress(0, 0)
        );
        assert_eq!(
            watch.step(&ManifestRead::Missing, S(2)),
            WatchEvent::Waiting
        );
        assert_eq!(
            watch.step(&ManifestRead::Missing, S(4)),
            WatchEvent::Cancelled
        );
        let mut done = InstallWatch::default();
        assert_eq!(
            done.step(&manifest(6, 0, 0), S(0)),
            WatchEvent::Installed,
            "an update queued still counts as installed"
        );
    }
}
