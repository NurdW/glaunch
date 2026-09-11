use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::config_dir;

const CACHE_MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60; // 30 days

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcacheInfo {
    pub cpus: String,
    pub l3_size_kb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub driver: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub vcache: Option<VcacheInfo>,
    pub gpus: Vec<GpuInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HardwareCache {
    #[serde(default)]
    timestamp: u64,
    #[serde(flatten)]
    info: HardwareInfo,
}

/// Resolve a Wayland connector name (e.g. "DP-1") to its `wl_output` index.
///
/// Runs `wayland-info` and finds the position of the named output in the ordered
/// list.  This index is what gamescope's `--display-index` expects in nested mode.
/// Returns `None` if the connector isn't found or `wayland-info` isn't available.
pub fn resolve_wayland_display_index(connector: &str) -> Result<Option<u32>> {
    resolve_wayland_display_index_from_output(
        &std::process::Command::new("wayland-info")
            .output()
            .context("failed to run wayland-info — is it installed?")?
            .stdout,
        connector,
    )
}

/// Parse `wayland-info` stdout to find the index of a named connector.
///
/// `wayland-info` lists interfaces sequentially.  We track whether we're inside
/// a `wl_output` block and only consider `name:` lines that belong to one.
pub fn resolve_wayland_display_index_from_output(
    stdout: &[u8],
    connector: &str,
) -> Result<Option<u32>> {
    let text = String::from_utf8_lossy(stdout);
    let mut index: u32 = 0;
    let mut in_wl_output = false;

    for line in text.lines() {
        // Interface header lines start at column 0 (no leading whitespace).
        if line.starts_with("interface: ") {
            in_wl_output = line.contains("'wl_output'");
            continue;
        }
        // Property lines inside a block are indented (tab or spaces).
        if in_wl_output {
            let trimmed = line.trim();
            if let Some(name) = trimmed.strip_prefix("name: ") {
                if name.trim() == connector {
                    return Ok(Some(index));
                }
                index += 1;
            }
        }
    }
    Ok(None)
}

pub fn detect_vcache() -> Result<Option<VcacheInfo>> {
    detect_vcache_from_path(Path::new("/sys/devices/system/cpu"))
}

pub fn detect_vcache_from_path(cpu_base: &Path) -> Result<Option<VcacheInfo>> {
    let mut ccds: HashMap<String, u64> = HashMap::new();

    let entries = match fs::read_dir(cpu_base) {
        Ok(entries) => entries,
        Err(_) => return Ok(None),
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let cache_dir = entry.path().join("cache").join("index3");
        if !cache_dir.exists() {
            continue;
        }
        let size_str = match fs::read_to_string(cache_dir.join("size")) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let size_kb: u64 = size_str.trim().trim_end_matches('K').parse().unwrap_or(0);
        let cpus = match fs::read_to_string(cache_dir.join("shared_cpu_list")) {
            Ok(s) => s.trim().to_string(),
            Err(_) => continue,
        };
        ccds.entry(cpus).or_insert(size_kb);
    }

    if ccds.len() < 2 {
        return Ok(None);
    }

    let max_size = ccds.values().copied().max().unwrap_or(0);
    let min_size = ccds.values().copied().min().unwrap_or(0);

    if max_size == min_size {
        return Ok(None);
    }

    let (cpus, size) = ccds.into_iter().max_by_key(|(_, size)| *size).unwrap();

    Ok(Some(VcacheInfo {
        cpus,
        l3_size_kb: size,
    }))
}

pub fn detect_gpus() -> Vec<GpuInfo> {
    let drm_dir = Path::new("/sys/class/drm");
    let mut gpus = Vec::new();
    if !drm_dir.exists() {
        return gpus;
    }

    for entry in fs::read_dir(drm_dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let device_dir = path.join("device");
        let gpu_name = fs::read_to_string(device_dir.join("label"))
            .or_else(|_| fs::read_to_string(device_dir.join("product_name")))
            .unwrap_or_else(|_| name.to_string())
            .trim()
            .to_string();
        let driver = device_dir
            .join("driver")
            .read_link()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_default();
        gpus.push(GpuInfo {
            name: gpu_name,
            driver,
        });
    }

    gpus
}

pub fn detect_hardware(force_refresh: bool) -> Result<HardwareInfo> {
    if !force_refresh && let Some(cached) = load_hardware_cache()? {
        return Ok(cached);
    }

    let info = HardwareInfo {
        vcache: detect_vcache()?,
        gpus: detect_gpus(),
    };

    save_hardware_cache(&info)?;
    Ok(info)
}

fn cache_path() -> PathBuf {
    config_dir().join("hardware_cache.toml")
}

fn load_hardware_cache() -> Result<Option<HardwareInfo>> {
    let path = cache_path();
    if !path.exists() {
        return Ok(None);
    }

    let metadata = fs::metadata(&path)?;
    let age = metadata.modified()?.elapsed().unwrap_or_default().as_secs();

    if age > CACHE_MAX_AGE_SECS {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)?;
    let cache: HardwareCache = toml::from_str(&contents)?;
    Ok(Some(cache.info))
}

fn save_hardware_cache(info: &HardwareInfo) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let cache = HardwareCache {
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        info: info.clone(),
    };
    let contents = toml::to_string_pretty(&cache)?;
    fs::write(cache_path(), contents)?;
    Ok(())
}
