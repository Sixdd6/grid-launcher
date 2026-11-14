//! An Amazon row's `store_meta` JSON. Never a token: nothing in here is a
//! secret, and the column is never serialized to IPC.
//!
//! Every field defaults, so a row written by an older or newer build reads
//! back; unknown keys are dropped on the next write.

use serde::{Deserialize, Serialize};

use super::row::FuelStep;

/// What GRID keeps about one installed Amazon game.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct AmazonMeta {
    /// nile's product id (`amzn1.adg.product.<uuid>`, the row's
    /// `store_id` too).
    pub product_id: String,
    /// The installed version id from nile's `installed.json`. It is an
    /// opaque id, not a readable number, so the UI never shows it
    /// (`docs/stores-phase3-plan.md` "User decisions" 1).
    pub version: String,
    /// The user picked the executable in Game Settings; an update keeps it.
    pub exe_user_pinned: bool,
    /// The fuel.json `PostInstall` list that last ran to the end with
    /// every step working (`post_install`). An update or a verify runs the
    /// steps again only when the game's list differs from this one; empty
    /// for a row from before GRID ran them.
    pub post_install_ran: Vec<FuelStep>,
}

impl AmazonMeta {
    /// The meta stored in a row's `store_meta`; a blank or unreadable value
    /// reads as the default.
    pub fn from_json(text: &str) -> AmazonMeta {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// The JSON for a row's `store_meta`.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_round_trips_and_tolerates_old_or_blank_values() {
        let meta = AmazonMeta {
            product_id: "amzn1.adg.product.11111111-2222-3333-4444-555555555555".into(),
            version: "a1b2c3".into(),
            exe_user_pinned: true,
            post_install_ran: vec![FuelStep {
                id: String::new(),
                command: "oalinst.exe".into(),
                args: vec!["/silent".into()],
                valid_returns: vec![],
                in_exe_folder: false,
                root: String::new(),
            }],
        };
        assert_eq!(AmazonMeta::from_json(&meta.to_json()), meta);
        assert_eq!(AmazonMeta::from_json(""), AmazonMeta::default());
        assert_eq!(AmazonMeta::from_json("not json"), AmazonMeta::default());
        let partial = AmazonMeta::from_json(r#"{"version": "v9", "future": 1}"#);
        assert_eq!(partial.version, "v9");
        assert_eq!(partial.product_id, "");
        assert!(!partial.exe_user_pinned);
        assert!(partial.post_install_ran.is_empty(), "a row from before");
        assert!(!meta.to_json().to_lowercase().contains("token"));
    }
}
