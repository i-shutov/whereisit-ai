//! Windows-only, read-only pin for a CTF source while its captured snapshot is used.
//!
//! The open handle denies new write/delete sharing. The snapshot is read through
//! that exact handle, while full-width file identity detects path substitution.

use crate::ctf_sidecar::Snapshot;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Component, Path, Prefix};

const FILE_SHARE_READ: u32 = 0x0000_0001;
const MAX_SOURCE_BYTES: u64 = 2_000_000_000;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdInfo {
    volume_serial: u64,
    file_id: [u8; 16],
}

#[link(name = "kernel32")]
extern "system" {
    fn GetFileInformationByHandleEx(
        file: *mut std::ffi::c_void,
        information_class: i32,
        information: *mut std::ffi::c_void,
        buffer_size: u32,
    ) -> i32;
    fn GetDriveTypeW(root_path_name: *const u16) -> u32;
    fn GetFinalPathNameByHandleW(
        file: *mut std::ffi::c_void,
        file_path: *mut u16,
        buffer_length: u32,
        volume_name_flags: u32,
    ) -> u32;
    fn GetVolumeInformationByHandleW(
        file: *mut std::ffi::c_void,
        volume_name_buffer: *mut u16,
        volume_name_size: u32,
        volume_serial_number: *mut u32,
        maximum_component_length: *mut u32,
        file_system_flags: *mut u32,
        file_system_name_buffer: *mut u16,
        file_system_name_size: u32,
    ) -> i32;
}

pub(super) struct SourcePin {
    file: File,
    identity: FileIdInfo,
    size_bytes: u64,
}

pub(super) fn capture(path: &Path, max_bytes: u64) -> io::Result<(SourcePin, Snapshot)> {
    if max_bytes > MAX_SOURCE_BYTES {
        return Err(invalid("Byte limit exceeds the supported source limit"));
    }
    validate_local_path_syntax(path)?;

    // Omitting FILE_SHARE_WRITE and FILE_SHARE_DELETE prevents mutation or
    // replacement while this handle remains open. FILE_SHARE_READ preserves
    // ordinary readers.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)?;
    ensure_supported_filesystem(&file)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() > max_bytes {
        return Err(invalid("Not a regular file or size limit exceeded"));
    }

    let identity = file_id_info(&file)?;
    ensure_path_is_pinned(identity, path)?;

    // The protected length is known. Reserve one extra byte for the EOF probe
    // instead of geometrically growing a large read buffer and retaining its
    // freed allocations in the Windows heap while the parsed catalog is built.
    let capacity = usize::try_from(before.len() + 1)
        .map_err(|_| invalid("Source size cannot fit in memory"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| io::Error::other("Unable to allocate bounded source buffer"))?;
    file.try_clone()?
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(invalid("Size limit exceeded while reading"));
    }
    let after = file.metadata()?;
    if !after.is_file() || after.len() != before.len() || after.len() != bytes.len() as u64 {
        return Err(invalid("Source changed while being captured"));
    }
    ensure_path_is_pinned(identity, path)?;

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("source.ctf")
        .to_owned();
    let snapshot = Snapshot::from_bytes(name, bytes);
    Ok((
        SourcePin {
            file,
            identity,
            size_bytes: after.len(),
        },
        snapshot,
    ))
}

fn validate_local_path_syntax(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(invalid("Source path must be absolute"));
    }
    let prefix = match path.components().next() {
        Some(Component::Prefix(prefix)) => prefix,
        _ => return Err(invalid("Source path must have a local disk prefix")),
    };
    match prefix.kind() {
        Prefix::Disk(_) | Prefix::VerbatimDisk(_) => Ok(()),
        _ => Err(invalid("Network and device source paths are unsupported")),
    }
}

fn ensure_supported_filesystem(file: &File) -> io::Result<()> {
    let root = volume_guid_root(file)?;
    let mut root_wide: Vec<u16> = root.encode_utf16().collect();
    root_wide.push(0);
    // The GUID root comes from the opened handle, so mapped drive aliases,
    // junctions, and mount points are judged by the volume actually opened.
    // SAFETY: root_wide is a live, NUL-terminated volume root.
    if unsafe { GetDriveTypeW(root_wide.as_ptr()) } != 3 {
        // DRIVE_FIXED
        return Err(invalid("Source must reside on a fixed local drive"));
    }

    let mut filesystem = [0u16; 261];
    // SAFETY: null optional output buffers are permitted; filesystem is a
    // writable 261-WCHAR buffer, and the handle remains valid for the call.
    let ok = unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle().cast(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let end = filesystem
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(filesystem.len());
    let name = String::from_utf16_lossy(&filesystem[..end]);
    if !name.eq_ignore_ascii_case("NTFS") && !name.eq_ignore_ascii_case("ReFS") {
        return Err(invalid(
            "Source filesystem is unsupported for write pinning",
        ));
    }
    Ok(())
}

fn volume_guid_root(file: &File) -> io::Result<String> {
    let mut path = vec![0u16; 32_768];
    // SAFETY: the buffer is writable, its capacity is passed, and the handle
    // stays alive during the synchronous call. VOLUME_NAME_GUID requests the
    // final path rooted at the opened volume's GUID.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle().cast(),
            path.as_mut_ptr(),
            path.len() as u32,
            0x1,
        )
    } as usize;
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    if length >= path.len() {
        return Err(invalid("Final source path exceeds the supported limit"));
    }
    let path = String::from_utf16(&path[..length])
        .map_err(|_| invalid("Filesystem returned an invalid final path"))?;
    let prefix = r"\\?\Volume{";
    if !path.starts_with(prefix) {
        return Err(invalid("Filesystem did not provide a local volume GUID"));
    }
    let closing = path[prefix.len()..]
        .find("}\\")
        .map(|offset| prefix.len() + offset + 2)
        .ok_or_else(|| invalid("Filesystem returned an invalid volume GUID path"))?;
    let guid = &path[prefix.len()..closing - 2];
    let guid_valid = guid.len() == 36
        && guid.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        });
    if !guid_valid {
        return Err(invalid("Filesystem returned an invalid volume GUID"));
    }
    Ok(path[..closing].to_owned())
}

impl SourcePin {
    pub(super) fn verify(&self, path: &Path) -> io::Result<()> {
        let held = self.file.metadata()?;
        if !held.is_file() || held.len() != self.size_bytes {
            return Err(invalid("Pinned source metadata changed"));
        }
        let current_file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(path)?;
        let current = current_file.metadata()?;
        if !current.is_file() || current.len() != self.size_bytes {
            return Err(invalid("Source path no longer names the captured file"));
        }
        if file_id_info(&current_file)? != self.identity {
            return Err(invalid("Source path identity changed"));
        }
        Ok(())
    }
}

fn file_id_info(file: &File) -> io::Result<FileIdInfo> {
    let mut info = FileIdInfo {
        volume_serial: 0,
        file_id: [0; 16],
    };
    // FileIdInfo is information class 18. Unlike same-file's Windows helper,
    // this preserves the full 128-bit ID on ReFS.
    // SAFETY: the handle stays owned by File; info is a writable repr(C)
    // FILE_ID_INFO buffer of exactly the size passed to the synchronous API.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle().cast(),
            18,
            (&mut info as *mut FileIdInfo).cast(),
            std::mem::size_of::<FileIdInfo>() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.file_id == [0; 16] {
        return Err(invalid("Filesystem returned an unsupported zero file ID"));
    }
    Ok(info)
}

fn ensure_path_is_pinned(identity: FileIdInfo, path: &Path) -> io::Result<()> {
    let current = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)?;
    if file_id_info(&current)? != identity {
        return Err(invalid("Source path identity changed"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[cfg(windows)]
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileMappingW(
            file: *mut std::ffi::c_void,
            attributes: *mut std::ffi::c_void,
            protection: u32,
            maximum_size_high: u32,
            maximum_size_low: u32,
            name: *const u16,
        ) -> *mut std::ffi::c_void;
        fn MapViewOfFile(
            mapping: *mut std::ffi::c_void,
            desired_access: u32,
            file_offset_high: u32,
            file_offset_low: u32,
            bytes_to_map: usize,
        ) -> *mut std::ffi::c_void;
        fn UnmapViewOfFile(address: *const std::ffi::c_void) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }

    #[cfg(windows)]
    struct Mapping(*mut std::ffi::c_void);

    #[cfg(windows)]
    impl Drop for Mapping {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    #[cfg(windows)]
    struct MappedView(*mut std::ffi::c_void);

    #[cfg(windows)]
    impl Drop for MappedView {
        fn drop(&mut self) {
            unsafe {
                UnmapViewOfFile(self.0);
            }
        }
    }

    #[test]
    fn preexisting_writable_view_without_file_or_mapping_handles_prevents_pin() {
        use std::ffi::c_void;
        #[link(name = "kernel32")]
        extern "system" {
            fn CreateFileMappingW(
                file: *mut c_void,
                attrs: *mut c_void,
                protect: u32,
                high: u32,
                low: u32,
                name: *const u16,
            ) -> *mut c_void;
            fn MapViewOfFile(
                mapping: *mut c_void,
                access: u32,
                high: u32,
                low: u32,
                bytes: usize,
            ) -> *mut c_void;
            fn UnmapViewOfFile(view: *const c_void) -> i32;
            fn CloseHandle(handle: *mut c_void) -> i32;
        }
        struct View(*mut c_void);
        impl Drop for View {
            fn drop(&mut self) {
                // SAFETY: this view was returned by MapViewOfFile and is owned
                // solely by this guard, including when an assertion unwinds.
                unsafe {
                    UnmapViewOfFile(self.0);
                }
            }
        }
        let (_dir, path) = fixture();
        let writer = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        // SAFETY: valid live file handle; null optional security/name pointers;
        // PAGE_READWRITE mapping and FILE_MAP_WRITE view cover the generated file.
        let view = unsafe {
            let mapping = CreateFileMappingW(
                writer.as_raw_handle().cast(),
                std::ptr::null_mut(),
                4,
                0,
                0,
                std::ptr::null(),
            );
            assert!(!mapping.is_null());
            let view = MapViewOfFile(mapping, 2, 0, 0, 0);
            assert_ne!(CloseHandle(mapping), 0);
            assert!(!view.is_null());
            View(view)
        };
        drop(writer);
        assert!(capture(&path, 100).is_err());
        drop(view);
        let (pin, _) = capture(&path, 100).unwrap();
        pin.verify(&path).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("source.ctf");
        fs::write(&path, b"fixture bytes").unwrap();
        (dir, path)
    }

    #[test]
    fn readers_allowed_and_snapshot_comes_from_pinned_handle() {
        let (_dir, path) = fixture();
        let (pin, snapshot) = capture(&path, 100).unwrap();
        assert_eq!(fs::read(&path).unwrap(), snapshot.bytes());
        assert_eq!(snapshot.bytes(), b"fixture bytes");
        pin.verify(&path).unwrap();
    }

    #[test]
    fn write_truncate_rename_and_delete_are_denied_while_pinned() {
        let (dir, path) = fixture();
        let (pin, _) = capture(&path, 100).unwrap();
        assert!(OpenOptions::new().write(true).open(&path).is_err());
        assert!(OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)
            .is_err());
        assert!(fs::rename(&path, dir.path().join("renamed.ctf")).is_err());
        assert!(fs::remove_file(&path).is_err());
        pin.verify(&path).unwrap();
    }

    #[test]
    fn hardlink_writer_is_denied_and_preexisting_writer_blocks_capture() {
        let (dir, path) = fixture();
        let link = dir.path().join("linked.ctf");
        fs::hard_link(&path, &link).unwrap();
        let (pin, _) = capture(&path, 100).unwrap();
        assert!(OpenOptions::new().write(true).open(&link).is_err());
        drop(pin);
        let mut writer = OpenOptions::new().append(true).open(&path).unwrap();
        assert!(capture(&path, 100).is_err());
        writer.write_all(b"x").unwrap();
    }

    #[test]
    fn readonly_attribute_is_accepted_and_drop_releases_lock() {
        let (_dir, path) = fixture();
        let original_perms = fs::metadata(&path).unwrap().permissions();
        let mut perms = original_perms.clone();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).unwrap();
        let (pin, _) = capture(&path, 100).unwrap();
        pin.verify(&path).unwrap();
        drop(pin);
        fs::set_permissions(&path, original_perms).unwrap();
        OpenOptions::new().append(true).open(&path).unwrap();
    }

    #[test]
    fn verify_rejects_a_different_path() {
        let (dir, path) = fixture();
        let other = dir.path().join("other.ctf");
        fs::write(&other, b"fixture bytes").unwrap();
        let (pin, _) = capture(&path, 100).unwrap();
        assert!(pin.verify(&other).is_err());
    }

    #[test]
    fn zero_limit_accepts_empty_file_and_rejects_nonempty_file() {
        let (dir, path) = fixture();
        assert!(capture(&path, 0).is_err());
        fs::write(&path, b"").unwrap();
        let (pin, snapshot) = capture(&path, 0).unwrap();
        assert!(snapshot.bytes().is_empty());
        pin.verify(&path).unwrap();
        drop(pin);
        fs::write(dir.path().join("second.ctf"), b"x").unwrap();
    }

    #[test]
    fn too_small_limit_rejects_before_snapshot_publication() {
        let (_dir, path) = fixture();
        assert!(capture(&path, b"fixture bytes".len() as u64 - 1).is_err());
        let (pin, snapshot) = capture(&path, b"fixture bytes".len() as u64).unwrap();
        assert_eq!(snapshot.bytes(), b"fixture bytes");
        pin.verify(&path).unwrap();
    }

    #[test]
    fn overflow_and_overmaximum_limits_fail_closed() {
        let (_dir, path) = fixture();
        assert!(capture(&path, u64::MAX).is_err());
        assert!(capture(&path, MAX_SOURCE_BYTES + 1).is_err());
    }

    #[test]
    fn parent_directory_cannot_be_renamed_until_pin_is_dropped() {
        let dir = tempdir().unwrap();
        let parent = dir.path().join("catalog");
        let moved = dir.path().join("catalog-moved");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("source.ctf");
        fs::write(&path, b"fixture bytes").unwrap();
        let (pin, _) = capture(&path, 100).unwrap();
        assert!(fs::rename(&parent, &moved).is_err());
        pin.verify(&path).unwrap();
        drop(pin);
        fs::rename(&parent, &moved).unwrap();
    }

    #[test]
    fn dropping_pin_allows_write_and_parent_directory_rename() {
        let dir = tempdir().unwrap();
        let parent = dir.path().join("catalog");
        let moved = dir.path().join("catalog-moved");
        fs::create_dir(&parent).unwrap();
        let path = parent.join("source.ctf");
        fs::write(&path, b"fixture bytes").unwrap();
        let (pin, _) = capture(&path, 100).unwrap();
        drop(pin);
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"!")
            .unwrap();
        fs::rename(&parent, &moved).unwrap();
    }

    #[test]
    fn write_capable_mapping_open_is_denied_while_pinned() {
        let (_dir, path) = fixture();
        let (pin, _) = capture(&path, 100).unwrap();
        // Windows write mappings require a write-capable file handle; ensure
        // that acquiring such a handle fails while the source pin is alive.
        assert!(OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .is_err());
        pin.verify(&path).unwrap();
    }

    #[test]
    fn rejects_unc_device_and_relative_paths_before_opening() {
        for path in [
            Path::new(r"\\server\share\source.ctf"),
            Path::new(r"\\.\C:\source.ctf"),
            Path::new(r"C:relative.ctf"),
            Path::new("relative.ctf"),
        ] {
            let error = capture(path, 100)
                .err()
                .expect("unexpected path acceptance");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(
                error.raw_os_error(),
                None,
                "must reject before filesystem I/O"
            );
        }
    }

    #[test]
    fn canonical_local_temp_file_is_eligible() {
        let (_dir, path) = fixture();
        let canonical = fs::canonicalize(path).unwrap();
        validate_local_path_syntax(&canonical).unwrap();
        let (pin, snapshot) = capture(&canonical, 100).unwrap();
        assert_eq!(snapshot.bytes(), b"fixture bytes");
        pin.verify(&canonical).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn preexisting_writable_section_and_view_prevent_pin_capture() {
        let (_dir, path) = fixture();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let raw_mapping = unsafe {
            CreateFileMappingW(
                file.as_raw_handle().cast(),
                std::ptr::null_mut(),
                0x04, // PAGE_READWRITE
                0,
                0,
                std::ptr::null(),
            )
        };
        assert!(
            !raw_mapping.is_null(),
            "CreateFileMappingW failed: {}",
            io::Error::last_os_error()
        );
        let mapping = Mapping(raw_mapping);

        // The section keeps write-capable access after the original file handle
        // is closed, so a read-only source pin must fail before publication.
        drop(file);
        assert!(capture(&path, 100).is_err());

        let raw_view = unsafe { MapViewOfFile(mapping.0, 0x0002, 0, 0, 0) }; // FILE_MAP_WRITE
        assert!(
            !raw_view.is_null(),
            "MapViewOfFile failed: {}",
            io::Error::last_os_error()
        );
        let view = MappedView(raw_view);
        drop(mapping);

        // The writable view remains usable after closing the section handle.
        unsafe {
            *(view.0.cast::<u8>()) = b'F';
        }
        assert!(capture(&path, 100).is_err());
        drop(view);
    }
}
