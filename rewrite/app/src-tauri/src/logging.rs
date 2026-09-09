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

fn current_rust_log() -> Option<String> {
    std::env::var("RUST_LOG").ok()
}

/// Installs the global subscriber with a reloadable filter. Called once, at
/// startup, before anything else can log.
pub fn init(debug_prints: bool) {
    let directive = filter_directive(current_rust_log().as_deref(), debug_prints);
    let (layer, handle) = reload::Layer::new(EnvFilter::new(directive));
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
    let directive = filter_directive(current_rust_log().as_deref(), enabled);
    if let Some(handle) = RELOAD_HANDLE.get() {
        let _ = handle.reload(EnvFilter::new(directive));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
