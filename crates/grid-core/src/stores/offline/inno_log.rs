//! Why a silent Inno Setup run failed, read from its `/LOG` file.
//!
//! The log is matched against a few known patterns only. Its text is never
//! logged, never put in an error and never sent to the UI: what leaves this
//! module is an [`InstallerReason`], which GRID words itself.

use std::fs;
use std::io::Read;
use std::path::Path;

/// The most of a log GRID reads. Inno's logs are a few hundred KB at most;
/// the reason is near the end.
pub const LOG_READ_MAX: u64 = 4 * 1024 * 1024;

/// Why a silent install failed, in GRID's own words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallerReason {
    /// The disk ran out of space.
    DiskFull,
    /// A file was in use, or access was denied.
    FileInUse,
    /// The installer's own files are missing or damaged (a missing or
    /// broken `.bin` part, a failed checksum).
    Damaged,
    /// The installer cancelled itself (a suppressed message box answered
    /// Cancel or Abort).
    Cancelled,
    /// Nothing in the log says why.
    Unknown,
}

impl InstallerReason {
    /// The reason as the end of a sentence ("Couldn't run the installer
    /// silently: <words>.").
    pub fn words(self) -> &'static str {
        match self {
            InstallerReason::DiskFull => "there isn't enough disk space",
            InstallerReason::FileInUse => "a file it needed was in use or access was denied",
            InstallerReason::Damaged => "the installer's files are missing or damaged",
            InstallerReason::Cancelled => "the installer cancelled itself",
            InstallerReason::Unknown => "the installer stopped without saying why",
        }
    }
}

/// Patterns, lower-case, in the order they are tried: a specific cause
/// first, then a cancel (which an earlier error often ends in).
const DAMAGED: [&str; 9] = [
    "setup files are corrupted",
    "corrupted",
    "is corrupt",
    "crc error",
    "checksum",
    "setup needs the next disk",
    "please insert disk",
    "insert the disk",
    "decompression",
];

const DISK_FULL: [&str; 6] = [
    "not enough space on the disk",
    "not enough free disk space",
    "there is not enough space",
    "disk is full",
    "error_disk_full",
    "code 112",
];

const FILE_IN_USE: [&str; 7] = [
    "being used by another process",
    "access is denied",
    "access denied",
    "error_sharing_violation",
    "error_access_denied",
    "code 5.",
    "code 32.",
];

const CANCELLED: [&str; 6] = [
    "user canceled",
    "user cancelled",
    "installation process canceled",
    "installation process cancelled",
    "setup was canceled",
    "got eabort exception",
];

/// Lines that say a run failed even when the process exited 0.
const FAILED: [&str; 3] = [
    "rolling back changes",
    "fatal exception",
    "setup was canceled",
];

/// The reason a log gives, `None` when it names none of the known causes.
pub fn reason(log: &str) -> Option<InstallerReason> {
    let lower = log.to_lowercase();
    let has = |patterns: &[&str]| patterns.iter().any(|p| lower.contains(p));
    if has(&DAMAGED) {
        Some(InstallerReason::Damaged)
    } else if has(&DISK_FULL) {
        Some(InstallerReason::DiskFull)
    } else if has(&FILE_IN_USE) {
        Some(InstallerReason::FileInUse)
    } else if has(&CANCELLED) {
        Some(InstallerReason::Cancelled)
    } else {
        None
    }
}

/// Whether a log says the run failed (rolled back, a fatal exception), for
/// a run that exited 0.
pub fn says_failed(log: &str) -> bool {
    let lower = log.to_lowercase();
    FAILED.iter().any(|p| lower.contains(p))
}

/// The log at `path` as text (`""` when missing): its last
/// [`LOG_READ_MAX`] bytes, UTF-16 when it has a byte-order mark, else
/// UTF-8 with anything else replaced.
pub fn read(path: &Path) -> String {
    let Ok(mut file) = fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    if len > LOG_READ_MAX {
        use std::io::{Seek, SeekFrom};
        if file.seek(SeekFrom::Start(len - LOG_READ_MAX)).is_err() {
            return String::new();
        }
    }
    let mut bytes = Vec::new();
    if file.take(LOG_READ_MAX).read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    decode(&bytes)
}

fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shaped like Inno Setup 6's logs; the paths are invented.
    const HEAD: &str = "2026-11-14 10:00:01.000   Log opened. (Time zone: UTC+00:00)\n\
        2026-11-14 10:00:01.000   Setup version: Inno Setup version 6.2.2\n\
        2026-11-14 10:00:01.000   Original Setup EXE: Z:\\fake\\Downloads\\setup_quarry_nine_1.4_(69001).exe\n\
        2026-11-14 10:00:01.000   Setup command line: /SL5=\"$1;$2\" /SP- /VERYSILENT /DIR=\"Z:\\fake\\GOG\\Quarry Nine\"\n\
        2026-11-14 10:00:02.000   Starting the installation process.\n";

    fn log(tail: &str) -> String {
        format!("{HEAD}{tail}")
    }

    #[test]
    fn disk_full() {
        let text = log("2026-11-14 10:00:09.000   Exception message:\n\
            2026-11-14 10:00:09.000   Message box (Abort/Retry/Ignore):\n\
            An error occurred while trying to copy a file:\n\
            There is not enough space on the disk.\n\
            2026-11-14 10:00:09.000   Defaulting to Abort for suppressed message box (Abort/Retry/Ignore):\n\
            2026-11-14 10:00:09.000   User chose Abort.\n\
            2026-11-14 10:00:09.000   Rolling back changes.\n");
        assert_eq!(reason(&text), Some(InstallerReason::DiskFull));
        assert!(says_failed(&text));
    }

    #[test]
    fn file_in_use_and_access_denied() {
        let in_use = log("2026-11-14 10:00:09.000   -- File entry --\n\
            2026-11-14 10:00:09.000   Dest filename: Z:\\fake\\GOG\\Quarry Nine\\game.exe\n\
            2026-11-14 10:00:09.000   DeleteFile: The process cannot access the file because it is being used by another process.\n");
        assert_eq!(reason(&in_use), Some(InstallerReason::FileInUse));
        let denied = log("2026-11-14 10:00:09.000   MoveFile failed; code 5.\n\
            2026-11-14 10:00:09.000   Access is denied.\n");
        assert_eq!(reason(&denied), Some(InstallerReason::FileInUse));
    }

    #[test]
    fn missing_or_damaged_parts() {
        let next_disk = log("2026-11-14 10:00:09.000   Setup needs the next disk: setup_quarry_nine_1.4_(69001)-2.bin\n\
            2026-11-14 10:00:09.000   Defaulting to Cancel for suppressed message box.\n\
            2026-11-14 10:00:09.000   User canceled the installation.\n");
        assert_eq!(
            reason(&next_disk),
            Some(InstallerReason::Damaged),
            "the cause wins over the cancel it ended in"
        );
        let corrupt = log("2026-11-14 10:00:09.000   The setup files are corrupted. Please obtain a new copy of the program.\n");
        assert_eq!(reason(&corrupt), Some(InstallerReason::Damaged));
    }

    #[test]
    fn cancelled() {
        let text = log("2026-11-14 10:00:09.000   Got EAbort exception.\n\
            2026-11-14 10:00:09.000   Deinitializing Setup.\n");
        assert_eq!(reason(&text), Some(InstallerReason::Cancelled));
    }

    #[test]
    fn a_clean_or_empty_log_names_no_reason() {
        let ok = log(
            "2026-11-14 10:00:30.000   Installation process succeeded.\n\
            2026-11-14 10:00:30.000   Log closed.\n",
        );
        assert_eq!(reason(&ok), None);
        assert!(!says_failed(&ok));
        assert_eq!(reason(""), None);
    }

    #[test]
    fn every_reason_has_grids_words() {
        assert_eq!(
            InstallerReason::Unknown.words(),
            "the installer stopped without saying why"
        );
        for r in [
            InstallerReason::DiskFull,
            InstallerReason::FileInUse,
            InstallerReason::Damaged,
            InstallerReason::Cancelled,
            InstallerReason::Unknown,
        ] {
            assert!(!r.words().to_lowercase().contains("desktop"));
        }
    }

    #[test]
    fn reads_utf8_utf16_and_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(&dir.path().join("missing.log")), "");
        let utf8 = dir.path().join("a.log");
        fs::write(&utf8, b"\xEF\xBB\xBFUser canceled.\n").unwrap();
        assert_eq!(read(&utf8), "User canceled.\n");
        let utf16 = dir.path().join("b.log");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "Disk is full".encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        fs::write(&utf16, bytes).unwrap();
        assert_eq!(reason(&read(&utf16)), Some(InstallerReason::DiskFull));
    }
}
