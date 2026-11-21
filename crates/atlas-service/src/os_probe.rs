//! Read-only enumeration of local mounted filesystem volume identities.
//!
//! This probe is deliberately independent of catalog paths and CTF contents.
//! A failed, incomplete, or malformed enumeration never reports an empty
//! successful snapshot, because that could incorrectly mark prior bindings
//! offline.

use serde_json::{json, Value};
#[cfg(any(windows, target_os = "macos", test))]
use std::collections::HashMap;

#[cfg(any(windows, target_os = "macos", test))]
const MAX_VOLUMES: usize = 256;
#[cfg(any(windows, target_os = "macos", test))]
const MAX_LABEL_CHARS: usize = 256;
#[cfg(any(windows, target_os = "macos", test))]
const MAX_MOUNT_CHARS: usize = 4096;

#[cfg(any(windows, target_os = "macos", test))]
fn valid_uuid_body(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

#[cfg(any(windows, target_os = "macos", test))]
fn valid_windows_guid(guid: &str) -> bool {
    guid.strip_prefix('{')
        .and_then(|body| body.strip_suffix('}'))
        .is_some_and(valid_uuid_body)
}

#[cfg(any(windows, test))]
fn windows_volume_guid(path: &str) -> Option<String> {
    let guid = path.strip_prefix(r"\\?\Volume")?.strip_suffix('\\')?;
    valid_windows_guid(guid).then(|| guid.to_ascii_lowercase())
}

#[cfg(any(windows, test))]
fn windows_physical_id(guid: &str, serial: u32) -> String {
    format!("win:{guid}:{serial:08x}")
}

#[cfg(any(windows, target_os = "macos", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct Volume {
    physical_id: String,
    label: String,
    mount_root: String,
    system: bool,
}

fn failed(platform: &str, message: &str) -> Value {
    json!({
        "platform": platform,
        "probe_ok": false,
        "observed_ids": [],
        "items": [],
        "error": message,
    })
}

#[cfg(any(windows, target_os = "macos", test))]
fn normalize_id(value: &str, platform: &str) -> Option<String> {
    let raw = value.trim();
    let id = match platform {
        "windows" => {
            let (guid, serial) = raw.strip_prefix("win:")?.split_once(':')?;
            if !valid_windows_guid(guid)
                || serial.len() != 8
                || !serial.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return None;
            }
            format!("win:{guid}:{serial}")
        }
        "macos" => {
            let uuid = raw.strip_prefix("mac:").unwrap_or(raw);
            if !valid_uuid_body(uuid) {
                return None;
            }
            format!("mac:{}", uuid.to_ascii_lowercase())
        }
        _ => return None,
    };
    Some(id.to_ascii_lowercase())
}

#[cfg(any(windows, target_os = "macos", test))]
fn bounded_text(value: &str, limit: usize) -> Option<String> {
    if value.chars().count() > limit || value.chars().any(char::is_control) {
        None
    } else {
        Some(value.to_owned())
    }
}

/// Validate and convert a complete synthetic/native enumeration into the
/// service contract. Duplicate rows are retained in both arrays so callers can
/// detect cloned UUID conflicts. Only unique, non-system, mounted rows can be
/// selected for explicit pairing.
#[cfg(any(windows, target_os = "macos", test))]
fn response(platform: &str, volumes: Vec<Volume>) -> Value {
    if volumes.len() > MAX_VOLUMES {
        return failed(platform, "Too many mounted local volumes");
    }
    let mut counts = HashMap::<String, usize>::new();
    for volume in &volumes {
        *counts.entry(volume.physical_id.clone()).or_default() += 1;
    }
    let observed_ids: Vec<String> = volumes
        .iter()
        .map(|volume| volume.physical_id.clone())
        .collect();
    let items: Vec<Value> = volumes
        .into_iter()
        .map(|volume| {
            let unique = counts.get(&volume.physical_id) == Some(&1);
            json!({
                "physical_id": volume.physical_id,
                "label": volume.label,
                "mount_root": volume.mount_root,
                "system": volume.system,
                "eligible": unique && !volume.system && !volume.mount_root.is_empty(),
            })
        })
        .collect();
    json!({"platform":platform,"probe_ok":true,"observed_ids":observed_ids,"items":items})
}

#[cfg(any(windows, target_os = "macos", test))]
fn validate_volume(platform: &str, mut volume: Volume) -> Option<Volume> {
    volume.physical_id = normalize_id(&volume.physical_id, platform)?;
    volume.label = bounded_text(&volume.label, MAX_LABEL_CHARS)?;
    volume.mount_root = bounded_text(&volume.mount_root, MAX_MOUNT_CHARS)?;
    Some(volume)
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_info(info: &Value) -> Option<Option<Volume>> {
    let mounted = info.get("Mounted")?.as_bool()?;
    if !mounted {
        return Some(None);
    }
    let local = info.get("LocalVolume")?.as_bool()?;
    if !local {
        return Some(None);
    }
    let uuid = info.get("VolumeUUID")?.as_str()?;
    let label = info.get("VolumeName").and_then(Value::as_str).unwrap_or("");
    let mount = info.get("MountPoint")?.as_str()?;
    let volume = validate_volume(
        "macos",
        Volume {
            physical_id: format!("mac:{uuid}"),
            label: label.to_owned(),
            mount_root: mount.to_owned(),
            system: mount == "/",
        },
    )?;
    Some(Some(volume))
}

#[cfg(any(target_os = "macos", test))]
fn collect_macos_partition_ids(list: &Value, out: &mut Vec<String>) -> Option<()> {
    let disks = list.get("AllDisksAndPartitions")?.as_array()?;
    for disk in disks {
        let partitions = disk.get("Partitions")?.as_array()?;
        for part in partitions {
            let id = part.get("DeviceIdentifier")?.as_str()?;
            if id.len() > 32 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return None;
            }
            out.push(id.to_owned());
            if out.len() > MAX_VOLUMES {
                return None;
            }
        }
    }
    Some(())
}

#[cfg(windows)]
mod native {
    use super::{
        failed, response, validate_volume, windows_physical_id, windows_volume_guid, Volume,
        MAX_VOLUMES,
    };
    use serde_json::Value;
    type Handle = *mut core::ffi::c_void;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    const DRIVE_UNKNOWN: u32 = 0;
    const DRIVE_NO_ROOT_DIR: u32 = 1;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;
    const DRIVE_CDROM: u32 = 5;
    const DRIVE_RAMDISK: u32 = 6;
    const ERROR_NO_MORE_FILES: u32 = 18;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FindFirstVolumeW(name: *mut u16, length: u32) -> Handle;
        fn FindNextVolumeW(handle: Handle, name: *mut u16, length: u32) -> i32;
        fn FindVolumeClose(handle: Handle) -> i32;
        fn GetLastError() -> u32;
        fn GetDriveTypeW(root: *const u16) -> u32;
        fn GetVolumeInformationW(
            root: *const u16,
            name: *mut u16,
            name_len: u32,
            serial: *mut u32,
            max_component: *mut u32,
            flags: *mut u32,
            fs_name: *mut u16,
            fs_name_len: u32,
        ) -> i32;
        fn GetVolumePathNamesForVolumeNameW(
            volume_name: *const u16,
            paths: *mut u16,
            length: u32,
            needed: *mut u32,
        ) -> i32;
        fn GetWindowsDirectoryW(path: *mut u16, length: u32) -> u32;
        fn GetVolumeNameForVolumeMountPointW(
            mount_point: *const u16,
            volume_name: *mut u16,
            length: u32,
        ) -> i32;
    }

    fn drive_type(kind: u32) -> Option<bool> {
        match kind {
            DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR => None,
            DRIVE_FIXED | DRIVE_RAMDISK => Some(true),
            DRIVE_REMOTE | DRIVE_CDROM => Some(false),
            2 => Some(true), // DRIVE_REMOVABLE
            _ => None,
        }
    }

    unsafe fn system_volume_guid() -> Option<String> {
        let mut directory = [0u16; 32768];
        let length = GetWindowsDirectoryW(directory.as_mut_ptr(), directory.len() as u32) as usize;
        if length == 0 || length >= directory.len() || directory.get(length) != Some(&0) {
            return None;
        }
        let path = String::from_utf16(&directory[..length]).ok()?;
        let bytes = path.as_bytes();
        if bytes.len() < 3 || bytes[1] != b':' || !matches!(bytes[2], b'\\' | b'/') {
            return None;
        }
        let root = format!("{}:\\", bytes[0] as char);
        let mut wide: Vec<u16> = root.encode_utf16().collect();
        wide.push(0);
        let mut name = [0u16; 1024];
        if GetVolumeNameForVolumeMountPointW(wide.as_ptr(), name.as_mut_ptr(), name.len() as u32)
            == 0
        {
            return None;
        }
        let end = name.iter().position(|unit| *unit == 0)?;
        windows_volume_guid(&String::from_utf16(&name[..end]).ok()?)
    }

    unsafe fn enumerate() -> Option<Vec<Volume>> {
        let system_guid = system_volume_guid()?;
        let mut name = [0u16; 1024];
        let handle = FindFirstVolumeW(name.as_mut_ptr(), name.len() as u32);
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let result = (|| {
            let mut volumes = Vec::new();
            loop {
                let end = name.iter().position(|unit| *unit == 0)?;
                let path = String::from_utf16(&name[..end]).ok()?;
                let kind = GetDriveTypeW(name.as_ptr());
                match drive_type(kind) {
                    Some(false) => {}
                    None => return None,
                    Some(true) => {
                        let guid = windows_volume_guid(&path)?;
                        let mut label = [0u16; 512];
                        let mut serial = 0u32;
                        let mut max_component = 0u32;
                        let mut flags = 0u32;
                        let mut filesystem = [0u16; 64];
                        if GetVolumeInformationW(
                            name.as_ptr(),
                            label.as_mut_ptr(),
                            label.len() as u32,
                            &mut serial,
                            &mut max_component,
                            &mut flags,
                            filesystem.as_mut_ptr(),
                            filesystem.len() as u32,
                        ) == 0
                        {
                            return None;
                        }
                        let label_end = label.iter().position(|unit| *unit == 0)?;
                        let label = String::from_utf16(&label[..label_end]).ok()?;
                        let mut paths = vec![0u16; 32768];
                        let mut needed = 0u32;
                        if GetVolumePathNamesForVolumeNameW(
                            name.as_ptr(),
                            paths.as_mut_ptr(),
                            paths.len() as u32,
                            &mut needed,
                        ) == 0
                            || needed as usize > paths.len()
                        {
                            return None;
                        }
                        let mut roots = Vec::new();
                        for units in paths
                            .split(|unit| *unit == 0)
                            .filter(|units| !units.is_empty())
                        {
                            roots.push(String::from_utf16(units).ok()?);
                        }
                        let system = guid == system_guid;
                        // Preserve one row per OS volume, including a volume
                        // currently lacking a mount point. Its identity still
                        // matters for duplicate/conflict checks, but it cannot
                        // be selected for pairing without a current mount root.
                        let root = roots.into_iter().next().unwrap_or_default();
                        let volume = validate_volume(
                            "windows",
                            Volume {
                                physical_id: windows_physical_id(&guid, serial),
                                label,
                                mount_root: root,
                                system,
                            },
                        )?;
                        volumes.push(volume);
                        if volumes.len() > MAX_VOLUMES {
                            return None;
                        }
                    }
                }
                if FindNextVolumeW(handle, name.as_mut_ptr(), name.len() as u32) == 0 {
                    if GetLastError() == ERROR_NO_MORE_FILES {
                        break;
                    }
                    return None;
                }
            }
            Some(volumes)
        })();
        if FindVolumeClose(handle) == 0 {
            return None;
        }
        result
    }

    pub fn probe() -> Value {
        // All calls are fixed Win32 metadata APIs. No file contents are read.
        match unsafe { enumerate() } {
            Some(volumes) => response("windows", volumes),
            None => failed("windows", "Windows volume enumeration was incomplete"),
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::{failed, response, validate_volume, Volume, MAX_VOLUMES};
    use serde_json::Value;
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    const MAX_OUTPUT: usize = 2 * 1024 * 1024;
    const TIMEOUT: Duration = Duration::from_secs(4);

    fn run_fixed(path: &str, args: &[&str], input: Option<&[u8]>) -> Option<Vec<u8>> {
        let mut command = Command::new(path);
        command
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().ok()?;
        let mut stdout = child.stdout.take()?;
        let reader = thread::spawn(move || {
            let mut collected = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let count = match stdout.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let remaining = (MAX_OUTPUT + 1).saturating_sub(collected.len());
                collected.extend_from_slice(&buffer[..count.min(remaining)]);
            }
            collected
        });
        if let Some(input) = input {
            let mut stdin = child.stdin.take()?;
            if input.len() > MAX_OUTPUT || stdin.write_all(input).is_err() {
                let _ = child.kill();
            }
        }
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                _ => {
                    let _ = child.kill();
                    break None;
                }
            }
        };
        let bytes = reader.join().ok()?;
        status
            .filter(|s| s.success())
            .map(|_| bytes)
            .filter(|b| b.len() <= MAX_OUTPUT)
    }

    fn json_from_plist(bytes: &[u8]) -> Option<Value> {
        let plist = run_fixed(
            "/usr/bin/plutil",
            &["-convert", "json", "-o", "-", "-"],
            Some(bytes),
        )?;
        serde_json::from_slice(&plist).ok()
    }

    fn info_for(device: &str) -> Option<Value> {
        let path = format!("/dev/{device}");
        let plist = run_fixed("/usr/sbin/diskutil", &["info", "-plist", &path], None)?;
        json_from_plist(&plist)
    }

    fn enumerate() -> Option<Vec<Volume>> {
        let listing = run_fixed("/usr/sbin/diskutil", &["list", "-plist"], None)?;
        let list = json_from_plist(&listing)?;
        let mut ids = Vec::new();
        super::collect_macos_partition_ids(&list, &mut ids)?;
        let mut volumes = Vec::new();
        for device in ids {
            let info = info_for(&device)?;
            if let Some(volume) = super::parse_macos_info(&info)? {
                volumes.push(volume);
            }
            if volumes.len() > MAX_VOLUMES {
                return None;
            }
        }
        Some(volumes)
    }

    pub fn probe() -> Value {
        match enumerate() {
            Some(volumes) => response("macos", volumes),
            None => failed("macos", "macOS volume enumeration was incomplete"),
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod native {
    use super::failed;
    use serde_json::Value;
    pub fn probe() -> Value {
        failed(
            "unsupported",
            "Native volume probe is unsupported on this platform",
        )
    }
}

/// Enumerate mounted local filesystem volumes using only native OS metadata.
pub fn probe() -> Value {
    native::probe()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, label: &str, mount: &str, system: bool) -> Volume {
        Volume {
            physical_id: id.into(),
            label: label.into(),
            mount_root: mount.into(),
            system,
        }
    }

    #[test]
    fn duplicate_uuid_rows_remain_observed_and_neither_is_eligible() {
        let result = response(
            "macos",
            vec![
                row("mac:1234-abcd", "One", "/Volumes/one", false),
                row("mac:1234-abcd", "Clone", "/Volumes/clone", false),
            ],
        );
        assert_eq!(result["probe_ok"], true);
        assert_eq!(result["observed_ids"].as_array().unwrap().len(), 2);
        assert_eq!(result["items"][0]["eligible"], false);
        assert_eq!(result["items"][1]["eligible"], false);
    }

    #[test]
    fn system_volume_is_observed_but_never_pairable() {
        let guid =
            windows_volume_guid(r"\\?\Volume{12345678-1234-abcd-abcd-1234567890ab}\").unwrap();
        let id = windows_physical_id(&guid, 0x1234_abcd);
        let result = response("windows", vec![row(&id, "System", "C:\\", true)]);
        assert_eq!(result["observed_ids"], json!([id]));
        assert_eq!(result["items"][0]["system"], true);
        assert_eq!(result["items"][0]["eligible"], false);
    }

    #[test]
    fn empty_complete_enumeration_is_successful() {
        let result = response("macos", vec![]);
        assert_eq!(result["probe_ok"], true);
        assert_eq!(result["observed_ids"], json!([]));
        assert_eq!(result["items"], json!([]));
    }

    #[test]
    fn failed_or_malformed_enumeration_cannot_report_offline_evidence() {
        let result = failed("macos", "synthetic failure");
        assert_eq!(result["probe_ok"], false);
        assert_eq!(result["observed_ids"], json!([]));
        assert_eq!(normalize_id("/dev/disk9", "macos"), None);
        assert_eq!(normalize_id("not-a-guid", "windows"), None);
    }

    #[test]
    fn valid_ids_are_normalized_without_mount_letter_identity() {
        let guid =
            windows_volume_guid(r"\\?\Volume{12345678-1234-ABCD-ABCD-1234567890AB}\").unwrap();
        assert_eq!(guid, "{12345678-1234-abcd-abcd-1234567890ab}");
        assert_eq!(
            windows_physical_id(&guid, 1),
            "win:{12345678-1234-abcd-abcd-1234567890ab}:00000001",
        );
        assert_eq!(
            normalize_id(
                "win:{12345678-1234-ABCD-ABCD-1234567890AB}:1234ABCD",
                "windows"
            ),
            Some("win:{12345678-1234-abcd-abcd-1234567890ab}:1234abcd".into())
        );
        assert_eq!(windows_volume_guid("C:\\"), None);
        assert_eq!(windows_volume_guid(r"\\?\Volume{not-a-guid}\"), None);
        for invalid in [
            "win:{12345678-1234-abcd-abcd-1234567890ab}",
            "win:{12345678-1234-abcd-abcd-1234567890ab}:1234abc",
            "win:{12345678-1234-abcd-abcd-1234567890ab}:1234abcz",
            "win:12345678-1234-abcd-abcd-1234567890ab:1234abcd",
            "win:C:\\:1234abcd",
        ] {
            assert_eq!(normalize_id(invalid, "windows"), None, "{invalid}");
        }
        assert_eq!(
            normalize_id("mac:AABBCCDD-EEFF-0011-2233-445566778899", "macos"),
            Some("mac:aabbccdd-eeff-0011-2233-445566778899".into())
        );
    }

    #[test]
    fn windows_probe_matches_the_existing_win32_binding_format() {
        let temp = tempfile::tempdir().unwrap();
        let store = temp.path().join("fixture-bindings.sqlite");
        let existing = "win:{12345678-1234-abcd-abcd-1234567890ab}:1234abcd";
        let pair = json!({"user_store":store,"catalog_key":"sqlite:synthetic",
            "platform":"windows","volume_id":1,"physical_id":existing,
            "observed_ids":[existing],"probe_ok":true,"confirmed":true});
        assert_eq!(crate::physical_disks::pair(&pair).unwrap()["paired"], true);

        let guid =
            windows_volume_guid(r"\\?\Volume{12345678-1234-ABCD-ABCD-1234567890AB}\").unwrap();
        let old_id = windows_physical_id(&guid, 0x1234_abcd);
        let different_serial = windows_physical_id(&guid, 0x1234_abce);
        let probe = response(
            "windows",
            vec![
                row(&old_id, "Original volume", "R:\\", false),
                row(&different_serial, "Other serial", "S:\\", false),
            ],
        );
        assert_eq!(probe["observed_ids"], json!([existing, different_serial]));
        assert!(probe["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["eligible"] == true));
        let mut status = json!({"user_store":store,"catalog_key":"sqlite:synthetic",
            "platform":"windows","volume_ids":[1],"probe_ok":true,
            "observed_ids":probe["observed_ids"]});
        let matched = crate::physical_disks::status(&status).unwrap();
        assert_eq!(matched["items"][0]["state"], "connected");
        assert_eq!(matched["items"][0]["verified_physical_id"], existing);

        status["observed_ids"] = json!([different_serial]);
        let absent = crate::physical_disks::status(&status).unwrap();
        assert_eq!(absent["items"][0]["state"], "offline");
    }

    #[test]
    fn generated_diskutil_responses_require_complete_ids_and_keep_system_volume() {
        let listing = json!({"AllDisksAndPartitions":[{"Partitions":[
            {"DeviceIdentifier":"disk7s1"}, {"DeviceIdentifier":"disk7s2"}
        ]}]});
        let mut ids = Vec::new();
        collect_macos_partition_ids(&listing, &mut ids).unwrap();
        assert_eq!(ids, ["disk7s1", "disk7s2"]);
        let system = json!({"Mounted":true,"LocalVolume":true,"VolumeUUID":"AABBCCDD-EEFF-0011-2233-445566778899",
            "VolumeName":"System","MountPoint":"/"});
        let parsed = parse_macos_info(&system).unwrap().unwrap();
        assert_eq!(
            parsed.physical_id,
            "mac:aabbccdd-eeff-0011-2233-445566778899"
        );
        assert!(parsed.system);
        let remote_or_unmounted = json!({"Mounted":false,"LocalVolume":true});
        assert_eq!(parse_macos_info(&remote_or_unmounted).unwrap(), None);
        assert!(collect_macos_partition_ids(
            &json!({"AllDisksAndPartitions":[{}]}),
            &mut Vec::new()
        )
        .is_none());
    }
}
