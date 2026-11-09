//! Windows: removable drives are the drive letters `GetDriveTypeW` calls
//! removable, plus fixed drives on an external bus (USB, SD, MMC, 1394),
//! which is how Windows reports a USB SSD such as a Samsung T7. The system
//! drive is never one.

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows_sys::Win32::Storage::FileSystem::{
    BusType1394, BusTypeMmc, BusTypeSd, BusTypeUsb, GetDriveTypeW, GetLogicalDrives,
    GetVolumeInformationW, FILE_SHARE_READ, FILE_SHARE_WRITE,
};
use windows_sys::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY,
    STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use super::DetectedDrive;

// From Win32_System_WindowsProgramming and Win32_System_SystemServices,
// which grid-core does not enable for three constants.
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const FILE_READ_ONLY_VOLUME: u32 = 0x0008_0000;

fn wide(text: &str) -> Vec<u16> {
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(Some(0))
        .collect()
}

fn from_wide(buffer: &[u16]) -> String {
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

/// `%SystemDrive%` as `C:`.
fn system_drive() -> String {
    std::env::var("SystemDrive")
        .unwrap_or_else(|_| "C:".to_string())
        .trim_end_matches(['\\', '/'])
        .to_ascii_uppercase()
}

/// Whether the volume `letter:` sits on a USB, SD, MMC or 1394 bus.
fn on_external_bus(letter: char) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;

    // Access 0: a device query needs no read or write rights.
    let Ok(device) = std::fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(format!(r"\\.\{letter}:"))
    else {
        return false;
    };
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut buffer = [0u8; 1024];
    let mut returned = 0u32;
    // SAFETY: the handle belongs to `device`, which outlives the call; the
    // input and output buffers are valid for the sizes passed, and the call
    // is synchronous (no OVERLAPPED).
    let ok = unsafe {
        DeviceIoControl(
            device.as_raw_handle(),
            IOCTL_STORAGE_QUERY_PROPERTY,
            std::ptr::from_ref(&query).cast(),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    let needed =
        std::mem::offset_of!(STORAGE_DEVICE_DESCRIPTOR, BusType) + std::mem::size_of::<i32>();
    if ok == 0 || (returned as usize) < needed {
        return false;
    }
    // SAFETY: the buffer is larger than the descriptor and was filled by the
    // call; `read_unaligned` copes with the byte array's alignment.
    let descriptor: STORAGE_DEVICE_DESCRIPTOR =
        unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast()) };
    matches!(
        descriptor.BusType,
        BusTypeUsb | BusTypeSd | BusTypeMmc | BusType1394
    )
}

/// The drive at `letter:`, when it is a removable drive with media in it.
fn probe(letter: char) -> Option<DetectedDrive> {
    let root = format!("{letter}:\\");
    let root_wide = wide(&root);
    // SAFETY: `root_wide` is NUL-terminated and outlives the call.
    let kind = unsafe { GetDriveTypeW(root_wide.as_ptr()) };
    match kind {
        DRIVE_REMOVABLE => {}
        DRIVE_FIXED if on_external_bus(letter) => {}
        _ => return None,
    }
    let mut label = [0u16; 261];
    let mut fs_name = [0u16; 261];
    let mut flags = 0u32;
    // SAFETY: `root_wide` is NUL-terminated; both buffers are valid for the
    // lengths passed; the unused out-pointers may be null.
    let ok = unsafe {
        GetVolumeInformationW(
            root_wide.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut flags,
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    };
    // An empty card reader has no volume.
    if ok == 0 {
        return None;
    }
    let label = match from_wide(&label) {
        name if name.trim().is_empty() => format!("{letter}:"),
        name => name,
    };
    Some(DetectedDrive::new(
        PathBuf::from(root),
        label,
        from_wide(&fs_name),
        flags & FILE_READ_ONLY_VOLUME != 0,
    ))
}

pub(super) fn scan() -> Vec<DetectedDrive> {
    let system = system_drive();
    // SAFETY: no arguments; returns a bit mask.
    let mask = unsafe { GetLogicalDrives() };
    (0u8..26)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| char::from(b'A' + bit))
        .filter(|letter| format!("{letter}:") != system)
        .filter_map(probe)
        .collect()
}
