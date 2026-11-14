//! Store tool progress lines, parsed into [`ToolProgress`], and legendary's
//! and nile's known lines, read as a [`ToolSignal`] ([`legendary_signal`],
//! [`nile_signal`]). A line that is neither is dropped, never logged;
//! legendary's format is under [`parse_legendary_line`], nile's under
//! [`parse_nile_line`].
//!
//! gogdl (v1.4.0) reports on stderr about once a second, through Python
//! logging (`[PROGRESS] INFO: <msg>`):
//!
//! ```text
//! [PROGRESS] INFO: = Progress: 12.34 123456/1000000, Running for: 00:01:02, ETA: 00:07:30
//! [PROGRESS] INFO: = Downloaded: 1.00 MiB, Written: 2.00 MiB
//! [PROGRESS] INFO:  + Download    - 3.50 MiB/s (raw) / 7.00 MiB/s (decompressed)
//! [PROGRESS] INFO:  + Disk    - 7.00 MiB/s (write) / 0.00 MiB/s (read)
//! ```
//!
//! The `Progress` line carries bytes written and the total; the
//! `+ Download` line carries the raw download speed. They come as separate
//! reports: a speed report has `done == total == 0` and a non-zero
//! `speed_bps` ([`ToolProgress::is_speed_only`]); [`ProgressMerger`] folds
//! the two into one running picture.

use super::runner::ToolSignal;
use crate::library::queue::JobPhase;

/// One progress report from a tool.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ToolProgress {
    pub done: u64,
    pub total: u64,
    pub speed_bps: f64,
    pub phase: JobPhase,
}

impl ToolProgress {
    /// A report that carries only a speed (gogdl's `+ Download` line).
    pub fn is_speed_only(&self) -> bool {
        self.done == 0 && self.total == 0 && self.speed_bps > 0.0
    }
}

const MIB: f64 = 1024.0 * 1024.0;

/// The progress a gogdl output line reports, `None` for any other line.
pub fn parse_gogdl_line(line: &str) -> Option<ToolProgress> {
    let line = line.trim_end_matches(['\r', '\n']);
    if let Some(rest) = after(line, "= Progress:") {
        // "12.34 123456/1000000, Running for: ..."
        let mut parts = rest.split_whitespace();
        let _percent = parts.next()?;
        let counts = parts.next()?.trim_end_matches(',');
        let (done, total) = counts.split_once('/')?;
        let done: u64 = done.trim().parse().ok()?;
        let total: u64 = total.trim().parse().ok()?;
        return Some(ToolProgress {
            done,
            total,
            speed_bps: 0.0,
            phase: JobPhase::Downloading,
        });
    }
    if let Some(rest) = after(line, "+ Download") {
        // "\t- 3.50 MiB/s (raw) / ..."
        let rest = rest.trim_start().trim_start_matches('-').trim_start();
        let number = rest.split_whitespace().next()?;
        let mib: f64 = number.parse().ok()?;
        if !mib.is_finite() || mib <= 0.0 {
            return None;
        }
        return Some(ToolProgress {
            done: 0,
            total: 0,
            speed_bps: mib * MIB,
            phase: JobPhase::Downloading,
        });
    }
    None
}

/// The text after `marker`, when the line is a gogdl progress-logger line
/// that holds it.
fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    if !line.contains("[PROGRESS]") {
        return None;
    }
    let at = line.find(marker)?;
    Some(&line[at + marker.len()..])
}

/// The `total` of a legendary progress report: legendary's `done` is a
/// fraction in hundredths of a percent (`12.34%` is `1234` of `10_000`),
/// because its counts are chunks or files, not bytes. The job turns it into
/// bytes with the size `info` (or [`super::runner::ToolSignal::DownloadSize`])
/// gave.
pub const LEGENDARY_PROGRESS_TOTAL: u64 = 10_000;

/// The progress a legendary output line reports, `None` for any other line.
///
/// legendary 0.21.1 logs its download progress on stderr about once a
/// second (`downloader/mp/manager.py`), and writes the verify step's
/// progress to stdout with `\r` (`cli.py` `verify_game`):
///
/// ```text
/// [DLManager] INFO: = Progress: 12.34% (123/1000), Running for 00:01:02, ETA: 00:08:10
/// [DLManager] INFO:  - Downloaded: 512.00 MiB, Written: 1024.00 MiB
/// [DLManager] INFO:  + Download<TAB>- 20.00 MiB/s (raw) / 40.00 MiB/s (decompressed)
/// Verification progress: 12/100 (12.0%) [3.4 MiB/s]
/// ```
///
/// (`<TAB>` is a tab.) `= Progress` gives a [`JobPhase::Downloading`] fraction (of
/// [`LEGENDARY_PROGRESS_TOTAL`]); `+ Download` a speed-only report;
/// `Verification progress` a [`JobPhase::Verifying`] fraction with its
/// speed. The `Downloaded` and `Written` lines are not used: the fraction
/// times the known download size is the byte count.
pub fn parse_legendary_line(line: &str) -> Option<ToolProgress> {
    let line = line.trim_end_matches(['\r', '\n', '\t', ' ']);
    if line.contains("[DLManager]") {
        if let Some(at) = line.find("= Progress:") {
            let rest = &line[at + "= Progress:".len()..];
            let percent = rest.trim_start().split('%').next()?.trim();
            return Some(ToolProgress {
                done: permyriad(percent)?,
                total: LEGENDARY_PROGRESS_TOTAL,
                speed_bps: 0.0,
                phase: JobPhase::Downloading,
            });
        }
        if let Some(at) = line.find("+ Download") {
            let rest = line[at + "+ Download".len()..]
                .trim_start()
                .trim_start_matches('-')
                .trim_start();
            let mib = rest.split_whitespace().next()?;
            let speed_bps = mib_per_second(mib)?;
            return Some(ToolProgress {
                done: 0,
                total: 0,
                speed_bps,
                phase: JobPhase::Downloading,
            });
        }
        return None;
    }
    let rest = line.trim_start().strip_prefix("Verification progress:")?;
    // " 12/100 (12.0%) [3.4 MiB/s]"
    let open = rest.find('(')?;
    let close = rest[open..].find('%')? + open;
    let done = permyriad(rest[open + 1..close].trim())?;
    let speed_bps = rest
        .find('[')
        .and_then(|at| rest[at + 1..].split_whitespace().next())
        .and_then(|mib| mib.parse::<f64>().ok())
        .filter(|mib| mib.is_finite() && *mib > 0.0)
        .map_or(0.0, |mib| mib * MIB);
    Some(ToolProgress {
        done,
        total: LEGENDARY_PROGRESS_TOTAL,
        speed_bps,
        phase: JobPhase::Verifying,
    })
}

/// `"12.34"` (a percentage) as hundredths of a percent, clamped to
/// `0..=10_000`; `None` when it is not a number.
fn permyriad(percent: &str) -> Option<u64> {
    let value: f64 = percent.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    Some(
        (value * 100.0)
            .round()
            .clamp(0.0, LEGENDARY_PROGRESS_TOTAL as f64) as u64,
    )
}

/// `"20.00"` MiB/s as bytes per second, `None` for zero or not a number.
fn mib_per_second(text: &str) -> Option<f64> {
    let mib: f64 = text.parse().ok()?;
    (mib.is_finite() && mib > 0.0).then_some(mib * MIB)
}

/// The [`ToolSignal`] a legendary output line (stderr or stdout) means,
/// `None` for a line GRID has no word for. The raw line is dropped either
/// way; only the signal (GRID's own words) travels.
///
/// legendary's exit codes are unreliable (many failures log `ERROR:` and
/// exit 0, a missing login is an uncaught `ValueError` and exit 1), so a
/// job reads these. Specific signals win over [`ToolSignal::OtherError`],
/// which also comes for any other `ERROR`/`CRITICAL` log line and for a
/// Python traceback; a caller checks the specific ones first. Lines and
/// sources: `docs/stores-phase2-plan.md` "Spike results".
pub fn legendary_signal(line: &str) -> Option<ToolSignal> {
    let line = line.trim_end_matches(['\r', '\n', '\t', ' ']).trim_start();
    if line.is_empty() {
        return None;
    }
    // The verify step names each bad file at ERROR; the counts come in its
    // result line, so the per-file lines say nothing more.
    for per_file in [
        "File does not match hash:",
        "File is missing:",
        "Other failure (see log), treating file as missing:",
    ] {
        if line.contains(per_file) {
            return None;
        }
    }
    if line.contains("Verification finished successfully.") {
        return Some(ToolSignal::VerifyOk);
    }
    if let Some(at) = line.find("Verification failed, ") {
        return verify_failed(&line[at + "Verification failed, ".len()..]);
    }
    if let Some(at) = line.find("INFO: Download size: ") {
        let rest = &line[at + "INFO: Download size: ".len()..];
        let mib: f64 = rest.split_whitespace().next()?.parse().ok()?;
        if !mib.is_finite() || mib < 0.0 {
            return None;
        }
        return Some(ToolSignal::DownloadSize((mib * MIB).round() as u64));
    }
    if line.contains("INFO: Download size is 0,") {
        return Some(ToolSignal::DownloadSize(0));
    }
    if line.contains("Logging in failed with ") {
        return Some(if NETWORK_ERRORS.iter().any(|name| line.contains(name)) {
            ToolSignal::Network
        } else {
            ToolSignal::LoginFailed
        });
    }
    if line.contains("Login attempt failed")
        || line.contains("Login to EGS API failed")
        || line.contains("ERROR: Login failed")
    {
        return Some(ToolSignal::LoginFailed);
    }
    if line.contains("No saved credentials")
        || line.contains("Stored credentials are no longer valid")
        || line.contains("Stored credentials were found but were no longer valid")
    {
        return Some(ToolSignal::NotLoggedIn);
    }
    if line.contains("HTTP request for login failed") {
        return Some(ToolSignal::Network);
    }
    if line.starts_with("requests.exceptions.")
        && NETWORK_ERRORS
            .iter()
            .any(|name| line["requests.exceptions.".len()..].starts_with(name))
    {
        return Some(ToolSignal::Network);
    }
    if line.starts_with("MemoryError")
        || line.contains("Current shared memory cache is smaller than required")
    {
        return Some(ToolSignal::MemoryError);
    }
    // Case matters: "Potentially not enough available disk space!" is only
    // a warning.
    if line.contains("Not enough available disk space") {
        return Some(ToolSignal::NotEnoughSpace);
    }
    if line.contains("has to be installed via a third-party store") {
        return Some(ToolSignal::ThirdPartyManaged);
    }
    if line.contains("is not currently installed")
        || line.contains("not installed, cannot uninstall")
        || line.contains("but app not installed")
        || line.contains("Game not installed:")
        || line.contains("Game is not installed")
        || (line.contains("ERROR: ") && line.contains(" is not installed"))
        || (line.contains("CRITICAL: ") && line.contains(" is not installed"))
    {
        return Some(ToolSignal::NotInstalled);
    }
    if line.starts_with("Traceback (most recent call last)")
        || line.contains("] ERROR: ")
        || line.contains("] CRITICAL: ")
    {
        return Some(ToolSignal::OtherError);
    }
    None
}

/// The `requests` errors that mean the network, not the request, failed.
const NETWORK_ERRORS: [&str; 7] = [
    "ConnectionError",
    "ProxyError",
    "ConnectTimeout",
    "ReadTimeout",
    "Timeout",
    "SSLError",
    "ChunkedEncodingError",
];

/// `"2 file(s) corrupted, 1 file(s) are missing."` as the counts.
fn verify_failed(rest: &str) -> Option<ToolSignal> {
    let mut numbers = rest
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u32>().ok());
    let corrupted = numbers.next()??;
    let missing = numbers.next()??;
    Some(ToolSignal::VerifyFailed { corrupted, missing })
}

/// The progress a nile output line reports, `None` for any other line.
///
/// nile 1.2.0 logs on stderr through Python logging as
/// `LEVEL [Name]:<TAB> message`. Its download progress
/// (`nile/downloading/progress.py`) comes about once a second:
///
/// ```text
/// INFO [PROGRESS]:<TAB> = Progress: 11.45 424234858/3705895264, Running for: 02:52:48, ETA: 22:16:42
/// INFO [PROGRESS]:<TAB> = Downloaded: 404.57 MiB, Written: 404.57 MiB
/// INFO [PROGRESS]:<TAB>  + Download<TAB>- 12.34 MiB/s
/// INFO [PROGRESS]:<TAB>  + Disk<TAB>- 12.34 MiB/s
/// ```
///
/// `= Progress` carries real bytes done and the total (the files are
/// stored as they download, so it is also the disk size); `+ Download`
/// gives a speed-only report. `verify` and `update` print the same lines,
/// so the phase is the job's to set.
pub fn parse_nile_line(line: &str) -> Option<ToolProgress> {
    let line = line.trim_end_matches(['\r', '\n', '\t', ' ']);
    if !line.contains("[PROGRESS]") {
        return None;
    }
    if let Some(at) = line.find("= Progress:") {
        let mut parts = line[at + "= Progress:".len()..].split_whitespace();
        let _percent = parts.next()?;
        let counts = parts.next()?.trim_end_matches(',');
        let (done, total) = counts.split_once('/')?;
        return Some(ToolProgress {
            done: done.trim().parse().ok()?,
            total: total.trim().parse().ok()?,
            speed_bps: 0.0,
            phase: JobPhase::Downloading,
        });
    }
    if let Some(at) = line.find("+ Download") {
        let rest = line[at + "+ Download".len()..]
            .trim_start()
            .trim_start_matches('-')
            .trim_start();
        let speed_bps = mib_per_second(rest.split_whitespace().next()?)?;
        return Some(ToolProgress {
            done: 0,
            total: 0,
            speed_bps,
            phase: JobPhase::Downloading,
        });
    }
    None
}

/// The [`ToolSignal`] a nile output line (stderr or stdout) means, `None`
/// for a line GRID has no word for. The raw line is dropped either way.
///
/// nile exits 0 for many failures, so a job reads these. Specific signals
/// win over [`ToolSignal::OtherError`], which also comes for any other
/// `ERROR [`/`CRITICAL [` line and for a Python traceback. Lines and
/// sources: `docs/stores-phase3-plan.md` "Research" and "Spike results".
///
/// - `Failed to refresh the token <Response [4xx]>`: Amazon refused the
///   saved login ([`ToolSignal::NotLoggedIn`]); the same prefix followed by
///   anything else is a `requests` connection error's text
///   ([`ToolSignal::Network`]).
/// - `Game is up to date` is [`ToolSignal::DownloadSize`] `(0)`: nothing
///   to download (as legendary's "Download size is 0").
/// - `No games installed` (`list-updates` with nothing installed) is no
///   signal: its `[]` on stdout is the answer.
/// - The `WARNING` that the FuelSDK download failed is no signal either.
/// - A loader error naming a missing glibc version (the bootloader's, or
///   PyInstaller's `[PYI-…:ERROR] Failed to load Python shared library`
///   line) is [`ToolSignal::SystemTooOld`]: the frozen build needs glibc
///   2.35.
pub fn nile_signal(line: &str) -> Option<ToolSignal> {
    let line = line.trim_end_matches(['\r', '\n', '\t', ' ']).trim_start();
    if line.is_empty() || line.contains("No games installed") {
        return None;
    }
    if line.contains("GLIBC_") && line.contains("not found") {
        return Some(ToolSignal::SystemTooOld);
    }
    if let Some(at) = line.find("Failed to refresh the token") {
        let rest = line[at + "Failed to refresh the token".len()..].trim_start();
        return Some(if rest.starts_with("<Response") {
            ToolSignal::NotLoggedIn
        } else {
            ToolSignal::Network
        });
    }
    if line.contains("User not logged in") || line.contains("You are not logged in") {
        return Some(ToolSignal::NotLoggedIn);
    }
    if line.contains("Failed to register a device") {
        return Some(ToolSignal::LoginFailed);
    }
    if line.contains("Not enough space available") {
        return Some(ToolSignal::NotEnoughSpace);
    }
    if line.contains("Game is not installed")
        || line.contains("No game is installed")
        || line.contains("Game isn't installed")
    {
        return Some(ToolSignal::NotInstalled);
    }
    if line.contains("Game is up to date") {
        return Some(ToolSignal::DownloadSize(0));
    }
    if line.starts_with("requests.exceptions.")
        && NETWORK_ERRORS
            .iter()
            .any(|name| line["requests.exceptions.".len()..].starts_with(name))
    {
        return Some(ToolSignal::Network);
    }
    if line.starts_with("Checksum error for")
        || line.starts_with("Traceback (most recent call last)")
        || line.starts_with("ERROR [")
        || line.starts_with("CRITICAL [")
    {
        return Some(ToolSignal::OtherError);
    }
    None
}

/// Folds a tool's separate progress and speed reports into one running
/// report.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProgressMerger {
    current: ToolProgress,
}

impl ProgressMerger {
    /// Takes one report and returns the merged picture.
    pub fn push(&mut self, report: ToolProgress) -> ToolProgress {
        if report.is_speed_only() {
            self.current.speed_bps = report.speed_bps;
        } else {
            self.current.done = report.done;
            self.current.total = report.total;
            if report.speed_bps > 0.0 {
                self.current.speed_bps = report.speed_bps;
            }
        }
        if report.phase != JobPhase::None {
            self.current.phase = report.phase;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_progress_line_gives_bytes_and_total() {
        let line = "[PROGRESS] INFO: = Progress: 12.34 123456/1000000, Running for: 00:01:02, ETA: 00:07:30";
        let report = parse_gogdl_line(line).unwrap();
        assert_eq!(report.done, 123_456);
        assert_eq!(report.total, 1_000_000);
        assert_eq!(report.phase, JobPhase::Downloading);
        assert!(!report.is_speed_only());
    }

    #[test]
    fn a_download_line_gives_the_raw_speed() {
        let line = "[PROGRESS] INFO:  + Download\t- 3.50 MiB/s (raw) / 7.00 MiB/s (decompressed)";
        let report = parse_gogdl_line(line).unwrap();
        assert!(report.is_speed_only());
        assert!((report.speed_bps - 3.5 * MIB).abs() < 1.0);
    }

    #[test]
    fn other_lines_are_not_progress() {
        for line in [
            "",
            "[PROGRESS] INFO: = Downloaded: 1.00 MiB, Written: 2.00 MiB",
            "[PROGRESS] INFO:  + Disk\t- 7.00 MiB/s (write) / 0.00 MiB/s (read)",
            "[PROGRESS] INFO:  + Download\t- 0.00 MiB/s (raw) / 0.00 MiB/s (decompressed)",
            "[AUTH] INFO: = Progress: 1.0 1/2,",
            "[PROGRESS] INFO: = Progress: x y/z",
            "{\"access_token\": \"FAKE-STORE-TOKEN-not-real\"}",
            "New/changed files 3",
        ] {
            assert_eq!(parse_gogdl_line(line), None, "{line:?}");
        }
    }

    #[test]
    fn legendary_progress_lines() {
        let p = parse_legendary_line(
            "[DLManager] INFO: = Progress: 12.34% (123/1000), Running for 00:01:02, ETA: 00:08:10\n",
        )
        .unwrap();
        assert_eq!((p.done, p.total), (1234, LEGENDARY_PROGRESS_TOTAL));
        assert_eq!(p.phase, JobPhase::Downloading);
        assert!(!p.is_speed_only());
        let full = parse_legendary_line(
            "[DLManager] INFO: = Progress: 100.00% (1000/1000), Running for 00:10:00, ETA: 00:00:00",
        )
        .unwrap();
        assert_eq!(full.done, LEGENDARY_PROGRESS_TOTAL);

        let s = parse_legendary_line(
            "[DLManager] INFO:  + Download\t- 20.00 MiB/s (raw) / 40.00 MiB/s (decompressed)",
        )
        .unwrap();
        assert!(s.is_speed_only());
        assert!((s.speed_bps - 20.0 * MIB).abs() < 1.0);

        let v =
            parse_legendary_line("Verification progress: 12/100 (12.5%) [3.4 MiB/s]\t").unwrap();
        assert_eq!((v.done, v.total), (1250, LEGENDARY_PROGRESS_TOTAL));
        assert_eq!(v.phase, JobPhase::Verifying);
        assert!((v.speed_bps - 3.4 * MIB).abs() < 1.0);
        let v0 = parse_legendary_line("Verification progress: 1/100 (0.0%) [0.0 MiB/s]\t").unwrap();
        assert_eq!(v0.done, 0);
        assert_eq!(v0.speed_bps, 0.0);
    }

    #[test]
    fn other_lines_are_not_legendary_progress() {
        for line in [
            "",
            "[DLManager] INFO:  - Downloaded: 512.00 MiB, Written: 1024.00 MiB",
            "[DLManager] INFO:  - Cache usage: 10.00 MiB, active tasks: 4",
            "[DLManager] INFO:  + Disk\t- 7.00 MiB/s (write) / 0.00 MiB/s (read)",
            "[DLManager] INFO:  + Download\t- 0.00 MiB/s (raw) / 0.00 MiB/s (decompressed)",
            "[cli] INFO: = Progress: 12.34% (1/2)",
            "[PROGRESS] INFO: = Progress: 12.34 123456/1000000, Running for: 00:01:02, ETA: 00:07:30",
            "{\"egl_parameters\": [\"-AUTH_PASSWORD=FAKE-STORE-TOKEN-not-real\"]}",
            "Verification progress: nope",
        ] {
            assert_eq!(parse_legendary_line(line), None, "{line:?}");
        }
        // gogdl's parser leaves legendary's lines alone too.
        assert_eq!(
            parse_gogdl_line(
                "[DLManager] INFO: = Progress: 12.34% (123/1000), Running for 00:01:02"
            ),
            None
        );
    }

    #[test]
    fn legendary_signals_from_recorded_lines() {
        use ToolSignal::*;
        let cases: &[(&str, Option<ToolSignal>)] = &[
            // Spike 2: a code legendary could not redeem (Epic unreachable).
            (
                "[Core] ERROR: Logging in failed with ConnectionError(MaxRetryError(\"HTTPSConnectionPool(host='oauth.unreachable.invalid', port=443)\")), please try again.",
                Some(Network),
            ),
            (
                "[Core] ERROR: Logging in failed with ProxyError(MaxRetryError('x')), please try again.",
                Some(Network),
            ),
            (
                "[Core] ERROR: Logging in failed with InvalidCredentialsError('errors.com.epicgames.account.oauth.authorization_code_not_found'), please try again.",
                Some(LoginFailed),
            ),
            ("[cli] ERROR: Login attempt failed, please see log for details.", Some(LoginFailed)),
            ("[EPCAPI] ERROR: Login to EGS API failed with errorCode: errors.com.epicgames.x", Some(LoginFailed)),
            ("[cli] ERROR: Login failed, cannot continue!", Some(LoginFailed)),
            ("[cli] ERROR: Login failed! Cannot continue with download process.", Some(LoginFailed)),
            // Spike 1: signed out, a command that needs the login.
            ("ValueError: No saved credentials", Some(NotLoggedIn)),
            ("[Core] ERROR: Stored credentials are no longer valid! Please login again.", Some(NotLoggedIn)),
            (
                "[Core] ERROR: HTTP request for login failed: ConnectionError(x), please try again later.",
                Some(Network),
            ),
            (
                "requests.exceptions.ProxyError: HTTPSConnectionPool(host='api.legendary.gl', port=443): Max retries exceeded",
                Some(Network),
            ),
            ("requests.exceptions.ConnectionError: x", Some(Network)),
            ("requests.exceptions.HTTPError: 404", None),
            ("Traceback (most recent call last):", Some(OtherError)),
            ("MemoryError: Current shared memory cache is smaller than required: 1024 < 2048.", Some(MemoryError)),
            (" ! Failure: Not enough available disk space! 1.00 GiB < 2.00 GiB", Some(NotEnoughSpace)),
            (" - Warning: Potentially not enough available disk space! x", None),
            (
                "[cli] ERROR: The selected title has to be installed via a third-party store: Origin",
                Some(ThirdPartyManaged),
            ),
            ("[cli] ERROR: Game Fortnite is not currently installed!", Some(NotInstalled)),
            ("[cli] ERROR: Game Fortnite not installed, cannot uninstall!", Some(NotInstalled)),
            ("[cli] CRITICAL: Base game \"Fortnite\" is not installed!", Some(NotInstalled)),
            ("[cli] INFO: Verification finished successfully.", Some(VerifyOk)),
            (
                "[cli] ERROR: Verification failed, 2 file(s) corrupted, 1 file(s) are missing.",
                Some(VerifyFailed { corrupted: 2, missing: 1 }),
            ),
            ("[cli] ERROR: File does not match hash: \"Game/x.pak\"", None),
            ("[cli] ERROR: File is missing: \"Game/y.pak\"", None),
            ("[cli] INFO: Download size: 1.50 MiB (Compression savings: 12.3%)", Some(DownloadSize(1_572_864))),
            (
                "[cli] INFO: Download size is 0, the game is either already up to date or has not changed. Exiting...",
                Some(DownloadSize(0)),
            ),
            ("[cli] CRITICAL: Installation cannot proceed, exiting.", Some(OtherError)),
            ("[Core] ERROR: Getting exchange code failed: {}", Some(OtherError)),
            // Lines GRID has no word for.
            ("[Core] WARNING: Getting locale failed: ValueError('x'), falling back to using en-US.", None),
            ("[cli] INFO: Testing existing login data if present...", None),
            ("[cli] INFO: Successfully logged in as \"SamPlays\"", None),
            ("Please enter the \"authorizationCode\" value from the JSON response: ", None),
            ("{\"access_token\": \"FAKE-STORE-TOKEN-not-real\"}", None),
            ("", None),
        ];
        for (line, want) in cases {
            assert_eq!(&legendary_signal(line), want, "{line:?}");
        }
    }

    #[test]
    fn nile_progress_lines() {
        let p = parse_nile_line(
            "INFO [PROGRESS]:\t = Progress: 11.45 424234858/3705895264, Running for: 02:52:48, ETA: 22:16:42\n",
        )
        .unwrap();
        assert_eq!((p.done, p.total), (424_234_858, 3_705_895_264));
        assert_eq!(p.phase, JobPhase::Downloading);
        assert!(!p.is_speed_only());
        let s = parse_nile_line("INFO [PROGRESS]:\t  + Download\t- 12.34 MiB/s").unwrap();
        assert!(s.is_speed_only());
        assert!((s.speed_bps - 12.34 * MIB).abs() < 1.0);
        for line in [
            "",
            "INFO [PROGRESS]:\t = Downloaded: 404.57 MiB, Written: 404.57 MiB",
            "INFO [PROGRESS]:\t  + Disk\t- 12.34 MiB/s",
            "INFO [PROGRESS]:\t  + Download\t- 0.00 MiB/s",
            "INFO [CLI]:\t = Progress: 1.00 1/2,",
            "INFO [PROGRESS]:\t = Progress: x y/z",
            "INFO [CLI]:\t Download complete",
            "{\"fake_session\": \"FAKE-STORE-TOKEN-not-real\"}",
        ] {
            assert_eq!(parse_nile_line(line), None, "{line:?}");
        }
    }

    #[test]
    fn nile_signals_from_recorded_lines() {
        use ToolSignal::*;
        let cases: &[(&str, Option<ToolSignal>)] = &[
            // Spike 1: signed out, offline.
            ("ERROR [CLI]:\t User not logged in", Some(NotLoggedIn)),
            ("ERROR [AUTH_MANAGER]:\t You are not logged in", Some(NotLoggedIn)),
            ("WARNING [CLI]:\t There was an error getting sdk", None),
            ("ERROR [CLI]:\t No games installed", None),
            ("ERROR [UNINSTALL]:\t Game isn't installed", Some(NotInstalled)),
            ("ERROR [CLI]:\t No game is installed", Some(NotInstalled)),
            ("ERROR [CLI]:\t Game is not installed", Some(NotInstalled)),
            ("ERROR [DOWNLOAD]:\t Unable to load manifest", Some(OtherError)),
            ("ERROR [CLI]:\t No game match", Some(OtherError)),
            ("Traceback (most recent call last):", Some(OtherError)),
            ("AttributeError: 'NoneType' object has no attribute 'encode'", None),
            (
                "[PYI-186:ERROR] Failed to execute script 'cli' due to unhandled exception!",
                None,
            ),
            // Spike 2: register with Amazon unreachable.
            (
                "requests.exceptions.ProxyError: HTTPSConnectionPool(host='api.amazon.com', port=443): Max retries exceeded with url: /auth/register",
                Some(Network),
            ),
            ("requests.exceptions.ConnectionError: x", Some(Network)),
            ("requests.exceptions.HTTPError: 404", None),
            (
                "urllib3.exceptions.MaxRetryError: HTTPSConnectionPool(host='api.amazon.com', port=443)",
                None,
            ),
            (
                "INFO [AUTH_MANAGER]:\t Registerring a device. ID: FAKE-STORE-TOKEN-not-real",
                None,
            ),
            // From the 1.2.0 source.
            ("ERROR [AUTH_MANAGER]:\t Failed to register a device", Some(LoginFailed)),
            ("INFO [AUTH_MANAGER]:\t Succesfully registered a device", None),
            (
                "ERROR [AUTH_MANAGER]:\t Failed to refresh the token <Response [400]>",
                Some(NotLoggedIn),
            ),
            (
                "ERROR [AUTH_MANAGER]:\t Failed to refresh the token HTTPSConnectionPool(host='api.amazon.com', port=443): Max retries exceeded with url: /auth/token (Caused by ProxyError('Unable to connect to proxy'))",
                Some(Network),
            ),
            ("ERROR [LIBRARY]:\t There was an error syncing library", Some(OtherError)),
            ("ERROR [DOWNLOAD]:\t Not enough space available", Some(NotEnoughSpace)),
            ("INFO [DOWNLOAD]:\t Game is up to date", Some(DownloadSize(0))),
            ("Checksum error for /games/Fogline/bin/Fogline.exe", Some(OtherError)),
            ("INFO [CLI]:\t Download complete", None),
            ("INFO [LIBRARY]:\t Successfully synced the library", None),
            ("{\"Username\": \"<not logged in>\", \"LoggedIn\": false}", None),
            ("FAKE-STORE-TOKEN-not-real", None),
            ("", None),
            // An old glibc: the bootloader's and PyInstaller's loader errors.
            (
                "[PYI-4242:ERROR] Failed to load Python shared library '/tmp/_MEIabc/libpython3.8.so.1.0': /lib64/libc.so.6: version `GLIBC_2.35' not found (required by /tmp/_MEIabc/libpython3.8.so.1.0)",
                Some(SystemTooOld),
            ),
            (
                "./nile_linux_x86_64: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.14' not found (required by ./nile_linux_x86_64)",
                Some(SystemTooOld),
            ),
        ];
        for (line, want) in cases {
            assert_eq!(&nile_signal(line), want, "{line:?}");
        }
    }

    #[test]
    fn the_merger_keeps_bytes_and_speed_together() {
        let mut merger = ProgressMerger::default();
        let a = merger.push(
            parse_gogdl_line(
                "[PROGRESS] INFO: = Progress: 50.00 50/100, Running for: 00:00:01, ETA: 00:00:01",
            )
            .unwrap(),
        );
        assert_eq!((a.done, a.total), (50, 100));
        let b = merger.push(
            parse_gogdl_line(
                "[PROGRESS] INFO:  + Download\t- 1.00 MiB/s (raw) / 1.00 MiB/s (decompressed)",
            )
            .unwrap(),
        );
        assert_eq!((b.done, b.total), (50, 100));
        assert!((b.speed_bps - MIB).abs() < 1.0);
    }
}
