//! Who a controller is. Pure: the poll thread reads the fields from gilrs.

#[cfg(any(target_os = "linux", test))]
use std::path::Path;

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

/// Where Linux lists USB devices, each with `idVendor`, `idProduct` and
/// `product` files.
#[cfg(target_os = "linux")]
pub const USB_DEVICES: &str = "/sys/bus/usb/devices";

/// The product name a USB device with these ids reports about itself, read
/// from `root` (normally [`USB_DEVICES`]). Linux input drivers name some pads
/// from a table keyed on the USB ids, and 8BitDo reuses ids across models:
/// xpad calls the Pro 3 receiver (2dc8:310b) "8BitDo Ultimate 2 Wireless
/// Controller", while the receiver itself says "8BitDo Pro 3 Receiver".
/// `None` when no device matches or it has no name (Bluetooth pads have no
/// USB device at all).
#[cfg(any(target_os = "linux", test))]
pub fn usb_product_name(root: &Path, vendor: u16, product: u16) -> Option<String> {
    usb_devices(root, vendor, product)
        .into_iter()
        .find_map(|dir| read_trimmed(&dir.join("product")).filter(|name| !name.is_empty()))
}

/// Whether a USB device with these ids has an interface bound to `driver`
/// (its `<device>:<config>.<interface>` entries in `root`, normally
/// [`USB_DEVICES`]). The 8BitDo Pro 3 receiver's pad interface is bound to
/// `xpad`, which always reports the triggers as `ABS_Z` / `ABS_RZ`.
#[cfg(any(target_os = "linux", all(test, unix)))]
pub fn usb_uses_driver(root: &Path, vendor: u16, product: u16, driver: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    let interfaces: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    usb_devices(root, vendor, product).iter().any(|dev| {
        let Some(name) = dev.file_name().and_then(|n| n.to_str()) else {
            return false;
        };
        let prefix = format!("{name}:");
        interfaces.iter().any(|iface| {
            iface
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
                && std::fs::read_link(iface.join("driver"))
                    .ok()
                    .and_then(|l| l.file_name().map(|n| n == driver))
                    .unwrap_or(false)
        })
    })
}

/// The USB device folders in `root` with these ids, sorted.
#[cfg(any(target_os = "linux", test))]
fn usb_devices(root: &Path, vendor: u16, product: u16) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|dir| {
            let id = |file: &str| {
                read_trimmed(&dir.join(file)).and_then(|s| u16::from_str_radix(&s, 16).ok())
            };
            id("idVendor") == Some(vendor) && id("idProduct") == Some(product)
        })
        .collect();
    dirs.sort();
    dirs
}

#[cfg(any(target_os = "linux", test))]
fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usb_device(root: &Path, dir: &str, vendor: &str, product: &str, name: Option<&str>) {
        let dir = root.join(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("idVendor"), format!("{vendor}\n")).unwrap();
        std::fs::write(dir.join("idProduct"), format!("{product}\n")).unwrap();
        if let Some(name) = name {
            std::fs::write(dir.join("product"), format!("{name}\n")).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_pad_bound_to_xpad_is_found_by_its_interfaces() {
        let root = tempfile::tempdir().unwrap();
        let drivers = root.path().join("drivers");
        std::fs::create_dir_all(drivers.join("xpad")).unwrap();
        std::fs::create_dir_all(drivers.join("usbhid")).unwrap();
        usb_device(
            root.path(),
            "5-3.3",
            "2dc8",
            "310b",
            Some("8BitDo Pro 3 Receiver"),
        );
        usb_device(root.path(), "5-3", "05e3", "0610", None);
        let iface = |name: &str, driver: Option<&str>| {
            let dir = root.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            if let Some(d) = driver {
                std::os::unix::fs::symlink(drivers.join(d), dir.join("driver")).unwrap();
            }
        };
        iface("5-3.3:1.0", Some("xpad"));
        iface("5-3.3:1.1", Some("usbhid"));
        iface("5-3:1.0", Some("usbhid"));
        iface("1-1:1.0", Some("xpad"));
        assert!(usb_uses_driver(root.path(), 0x2dc8, 0x310b, "xpad"));
        assert!(!usb_uses_driver(root.path(), 0x05e3, 0x0610, "xpad"));
        assert!(!usb_uses_driver(root.path(), 0x057e, 0x2009, "xpad"));
        assert!(!usb_uses_driver(&root.path().join("missing"), 1, 2, "xpad"));
    }

    #[test]
    fn the_usb_name_is_found_by_vendor_and_product() {
        let root = tempfile::tempdir().unwrap();
        usb_device(root.path(), "1-1", "2dc8", "3109", Some("IDLE"));
        usb_device(
            root.path(),
            "5-3.3",
            "2dc8",
            "310b",
            Some("8BitDo Pro 3 Receiver"),
        );
        usb_device(root.path(), "usb1", "1d6b", "0002", None);
        assert_eq!(
            usb_product_name(root.path(), 0x2dc8, 0x310b).as_deref(),
            Some("8BitDo Pro 3 Receiver")
        );
    }

    #[test]
    fn no_usb_name_without_a_match_or_a_name() {
        let root = tempfile::tempdir().unwrap();
        usb_device(root.path(), "1-1", "2dc8", "310b", Some("  "));
        usb_device(root.path(), "1-2", "045e", "0b13", None);
        assert_eq!(usb_product_name(root.path(), 0x2dc8, 0x310b), None);
        assert_eq!(usb_product_name(root.path(), 0x045e, 0x0b13), None);
        assert_eq!(usb_product_name(root.path(), 0x057e, 0x2009), None);
        assert_eq!(usb_product_name(&root.path().join("missing"), 1, 2), None);
    }

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
