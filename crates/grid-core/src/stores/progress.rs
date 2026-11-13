//! Store tool progress lines, parsed into [`ToolProgress`]. A line that is
//! not progress is dropped, never logged. Package A fills the parser.

use crate::library::queue::JobPhase;

/// One progress report from a tool.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ToolProgress {
    pub done: u64,
    pub total: u64,
    pub speed_bps: f64,
    pub phase: JobPhase,
}

/// The progress a gogdl output line reports, `None` for any other line.
pub fn parse_gogdl_line(line: &str) -> Option<ToolProgress> {
    let _ = line;
    None
}
