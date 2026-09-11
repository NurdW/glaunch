use std::fs;
use std::path::Path;

use glaunch::hardware::{detect_vcache_from_path, resolve_wayland_display_index_from_output};

fn create_mock_sysfs(base: &Path, cpus: &[(u32, u64, &str)]) {
    for (cpu_id, size_kb, shared_list) in cpus {
        let cache_dir = base
            .join(format!("cpu{cpu_id}"))
            .join("cache")
            .join("index3");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("size"), format!("{size_kb}K")).unwrap();
        fs::write(cache_dir.join("shared_cpu_list"), shared_list).unwrap();
    }
}

#[test]
fn test_vcache_asymmetric_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("cpu");
    // CCD0: 96MB L3 (V-Cache), CCD1: 32MB L3
    create_mock_sysfs(
        &base,
        &[
            (0, 98304, "0-7"), // 96MB
            (1, 98304, "0-7"),
            (8, 32768, "8-15"), // 32MB
            (9, 32768, "8-15"),
        ],
    );

    let result = detect_vcache_from_path(&base).unwrap();
    assert!(result.is_some());
    let info = result.unwrap();
    assert_eq!(info.cpus, "0-7");
    assert_eq!(info.l3_size_kb, 98304);
}

#[test]
fn test_vcache_symmetric_not_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("cpu");
    // Both CCDs 32MB — no X3D
    create_mock_sysfs(&base, &[(0, 32768, "0-7"), (8, 32768, "8-15")]);

    let result = detect_vcache_from_path(&base).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_vcache_single_ccd_not_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("cpu");
    // Single CCD — can't be asymmetric
    create_mock_sysfs(&base, &[(0, 32768, "0-7"), (1, 32768, "0-7")]);

    let result = detect_vcache_from_path(&base).unwrap();
    assert!(result.is_none());
}

const WAYLAND_INFO_SAMPLE: &str = r#"
interface: 'wl_output',                                  version:  4, name: 66
	name: DP-1
	description: ASUSTek COMPUTER INC PG27UCDM
	x: 3072, y: 0, scale: 2,
	flags: none
	width: 3840 px, height: 2160 px, refresh: 120.000 Hz
interface: 'wl_output',                                  version:  4, name: 67
	name: DP-2
	description: Advanced Micro Peripherals Ltd ES07D03
	x: 0, y: 0, scale: 2,
	flags: none
	width: 3840 px, height: 2160 px, refresh: 120.000 Hz
interface: 'wl_output',                                  version:  4, name: 68
	name: HDMI-A-1
	description: Sony SONY TV  *30
	x: 6144, y: 0, scale: 2,
	flags: none
	width: 3840 px, height: 2160 px, refresh: 60.000 Hz
"#;

#[test]
fn test_resolve_wayland_display_index_first() {
    let result =
        resolve_wayland_display_index_from_output(WAYLAND_INFO_SAMPLE.as_bytes(), "DP-1").unwrap();
    assert_eq!(result, Some(0));
}

#[test]
fn test_resolve_wayland_display_index_middle() {
    let result =
        resolve_wayland_display_index_from_output(WAYLAND_INFO_SAMPLE.as_bytes(), "DP-2").unwrap();
    assert_eq!(result, Some(1));
}

#[test]
fn test_resolve_wayland_display_index_last() {
    let result =
        resolve_wayland_display_index_from_output(WAYLAND_INFO_SAMPLE.as_bytes(), "HDMI-A-1")
            .unwrap();
    assert_eq!(result, Some(2));
}

#[test]
fn test_resolve_wayland_display_index_not_found() {
    let result =
        resolve_wayland_display_index_from_output(WAYLAND_INFO_SAMPLE.as_bytes(), "DP-99")
            .unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_resolve_wayland_display_index_empty() {
    let result = resolve_wayland_display_index_from_output(b"", "DP-1").unwrap();
    assert_eq!(result, None);
}
