//! Who a controller is. Pure: the poll thread reads the fields from gilrs.

use serde::{Deserialize, Serialize};

/// One connected controller, as the UI sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadInfo {
    /// This connection's id (gilrs' `GamepadId`): stable while the
    /// controller stays connected; every event carries it.
    pub id: u32,
    /// The controller model's key: its gilrs UUID as 32 lowercase hex
    /// digits (SDL's GUID). Settings › TV mode › Button assignments are
    /// stored under it, so two controllers of one model share an assignment.
    pub key: String,
    /// The name the OS or the SDL mapping gives it, trimmed; "Controller"
    /// when it has none.
    pub name: String,
    /// USB vendor and product ids, when the platform reports them.
    pub vendor: Option<u16>,
    pub product: Option<u16>,
}

impl PadInfo {
    pub fn new(
        id: u32,
        uuid: [u8; 16],
        name: &str,
        vendor: Option<u16>,
        product: Option<u16>,
    ) -> Self {
        let name = name.trim();
        Self {
            id,
            key: pad_key(uuid),
            name: if name.is_empty() {
                "Controller".to_string()
            } else {
                name.to_string()
            },
            vendor,
            product,
        }
    }
}

/// The UUID as lowercase hex, in byte order.
pub fn pad_key(uuid: [u8; 16]) -> String {
    uuid.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_is_the_uuid_in_hex() {
        let mut uuid = [0u8; 16];
        uuid[0] = 0x03;
        uuid[4] = 0x5e;
        uuid[5] = 0x04;
        uuid[15] = 0xff;
        assert_eq!(pad_key(uuid), "030000005e04000000000000000000ff");
        assert_eq!(pad_key([0; 16]).len(), 32);
    }

    #[test]
    fn two_pads_of_one_model_share_a_key_but_not_an_id() {
        let uuid = [7u8; 16];
        let a = PadInfo::new(
            0,
            uuid,
            "Xbox Wireless Controller",
            Some(0x045e),
            Some(0x0b13),
        );
        let b = PadInfo::new(
            1,
            uuid,
            "Xbox Wireless Controller",
            Some(0x045e),
            Some(0x0b13),
        );
        assert_eq!(a.key, b.key);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn the_name_is_trimmed_and_never_empty() {
        assert_eq!(
            PadInfo::new(0, [0; 16], "  8BitDo Pro 2 ", None, None).name,
            "8BitDo Pro 2"
        );
        assert_eq!(
            PadInfo::new(0, [0; 16], "   ", None, None).name,
            "Controller"
        );
    }

    #[test]
    fn it_serializes_with_the_fields_the_ui_reads() {
        let pad = PadInfo::new(2, [0; 16], "Pad", Some(1), None);
        assert_eq!(
            serde_json::to_value(&pad).unwrap(),
            serde_json::json!({
                "id": 2,
                "key": "00000000000000000000000000000000",
                "name": "Pad",
                "vendor": 1,
                "product": null
            })
        );
    }
}
