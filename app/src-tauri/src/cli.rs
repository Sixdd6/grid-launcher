//! Command-line options. Pure. GRID reads one flag: `--tv` starts it in TV
//! mode (TV mode README § Decided "Entering and leaving"). Anything else is
//! ignored, so a desktop file or launcher that passes its own arguments
//! never stops GRID from starting. The mode is never persisted: without
//! `--tv`, GRID starts in desktop mode.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CliOptions {
    pub tv: bool,
}

/// Parses the process arguments, the program name first (as
/// `std::env::args()` yields them).
pub fn parse_cli<I, S>(args: I) -> CliOptions
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut options = CliOptions::default();
    for arg in args.into_iter().skip(1) {
        if arg.as_ref() == "--tv" {
            options.tv = true;
        }
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_is_desktop_mode() {
        assert_eq!(parse_cli(["grid-launcher"]), CliOptions { tv: false });
        assert_eq!(parse_cli(Vec::<String>::new()), CliOptions { tv: false });
    }

    #[test]
    fn tv_starts_tv_mode() {
        assert!(parse_cli(["grid-launcher", "--tv"]).tv);
        assert!(parse_cli(["grid-launcher", "--foo", "--tv", "bar"]).tv);
    }

    #[test]
    fn unknown_and_near_miss_arguments_are_ignored() {
        for arg in ["--TV", "-tv", "tv", "--tv=1", "--television", "--"] {
            assert!(!parse_cli(["grid-launcher", arg]).tv, "{arg}");
        }
    }

    #[test]
    fn the_program_name_is_never_read_as_a_flag() {
        assert!(!parse_cli(["--tv"]).tv);
    }
}
