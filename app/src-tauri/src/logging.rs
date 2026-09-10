//! The tracing filter and the runtime switch behind the `debug_prints`
//! setting (docs/porting/02-config-and-secrets.md).
//!
//! Logging policy (spec, normative) is unchanged by the toggle: raising the
//! level to `debug` must never surface a header, token, or credentialed URL
//! — secrets are structurally unloggable (`SecretString`).

use std::sync::OnceLock;

use tracing_subscriber::prelude::*;
use tracing_subscriber::{reload, EnvFilter, Registry};

static RELOAD_HANDLE: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

/// The filter directive to run with. `RUST_LOG` wins whenever it is set and
/// non-blank — the toggle never overrides an explicit environment choice.
pub fn filter_directive(rust_log: Option<&str>, debug_prints: bool) -> String {
    match rust_log {
        Some(value) if !value.trim().is_empty() => value.to_string(),
        _ if debug_prints => "debug".to_string(),
        _ => "info".to_string(),
    }
}

/// The [`EnvFilter`] to run with. `EnvFilter::new` is LOSSY: it drops the
/// directives it cannot parse, so a malformed `RUST_LOG` would leave a
/// near-silent filter instead of the level the user expects. A parse failure
/// therefore falls back to the directive we would have used with no
/// `RUST_LOG` set at all.
fn env_filter(rust_log: Option<&str>, debug_prints: bool) -> EnvFilter {
    let directive = filter_directive(rust_log, debug_prints);
    EnvFilter::try_new(&directive)
        .unwrap_or_else(|_| EnvFilter::new(filter_directive(None, debug_prints)))
}

fn current_rust_log() -> Option<String> {
    std::env::var("RUST_LOG").ok()
}

/// Installs the global subscriber with a reloadable filter. Called once, at
/// startup, before anything else can log.
pub fn init(debug_prints: bool) {
    let (layer, handle) =
        reload::Layer::new(env_filter(current_rust_log().as_deref(), debug_prints));
    let _ = RELOAD_HANDLE.set(handle);
    tracing_subscriber::registry()
        .with(layer)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Recomputes the directive against the CURRENT `RUST_LOG` and reloads the
/// live filter. A missing handle (never initialized, e.g. in tests) or a
/// reload error is ignored: a settings toggle must not fail the command.
pub fn apply_debug_prints(enabled: bool) {
    if let Some(handle) = RELOAD_HANDLE.get() {
        let _ = handle.reload(env_filter(current_rust_log().as_deref(), enabled));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_filter_falls_back_to_info_when_rust_log_is_unparseable() {
        assert_eq!(env_filter(Some("=,,="), false).to_string(), "info");
    }

    #[test]
    fn env_filter_falls_back_to_debug_when_the_toggle_is_on() {
        assert_eq!(env_filter(Some("=,,="), true).to_string(), "debug");
    }

    #[test]
    fn env_filter_keeps_a_valid_rust_log() {
        assert_eq!(env_filter(Some("warn"), true).to_string(), "warn");
    }

    #[test]
    fn rust_log_wins_over_the_toggle() {
        assert_eq!(filter_directive(Some("warn"), true), "warn");
        assert_eq!(filter_directive(Some("warn"), false), "warn");
    }

    #[test]
    fn without_rust_log_the_toggle_picks_the_level() {
        assert_eq!(filter_directive(None, true), "debug");
        assert_eq!(filter_directive(None, false), "info");
    }

    #[test]
    fn a_blank_rust_log_is_treated_as_unset() {
        assert_eq!(filter_directive(Some("  "), true), "debug");
        assert_eq!(filter_directive(Some(""), false), "info");
    }
}
