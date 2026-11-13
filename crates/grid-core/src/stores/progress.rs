//! Store tool progress lines, parsed into [`ToolProgress`]. A line that is
//! not progress is dropped, never logged.
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

/// Folds gogdl's separate byte and speed reports into one running report.
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
