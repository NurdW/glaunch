use std::collections::BTreeMap;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{self, CliOverrides};

const MAX_LAUNCHES_PER_APP: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRecord {
    pub timestamp: DateTime<Utc>,
    pub command: Vec<String>,
    pub overrides: CliOverrides,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppHistory {
    pub steam_app_id: Option<u64>,
    pub launches: Vec<LaunchRecord>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct History(BTreeMap<String, AppHistory>);

impl Deref for History {
    type Target = BTreeMap<String, AppHistory>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for History {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub fn history_path() -> PathBuf {
    config::config_dir().join("history.json")
}

pub fn load_history() -> Result<History> {
    let path = history_path();
    if !path.exists() {
        return Ok(History::default());
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn save_history(history: &History) -> Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let contents = serde_json::to_string_pretty(history).context("failed to serialize history")?;
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
}

pub fn record_launch(
    history: &mut History,
    slug: &str,
    steam_app_id: Option<u64>,
    command: Vec<String>,
    overrides: CliOverrides,
) {
    let record = LaunchRecord {
        timestamp: Utc::now(),
        command,
        overrides,
    };

    let app = history
        .entry(slug.to_string())
        .or_insert_with(|| AppHistory {
            steam_app_id,
            launches: Vec::new(),
        });

    if let Some(id) = steam_app_id {
        app.steam_app_id = Some(id);
    }

    app.launches.insert(0, record);
    app.launches.truncate(MAX_LAUNCHES_PER_APP);
}

const STEAM_INFRA_PREFIXES: &[&str] = &[
    "steamlinuxruntime",
    "proton ",
    "proton-",
    "steam linux runtime",
];

fn is_steam_infrastructure(folder: &str) -> bool {
    let lower = folder.to_lowercase();
    STEAM_INFRA_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

pub fn derive_app_slug(command: &[String]) -> String {
    // Collect all steamapps/common/ matches, preferring non-infrastructure folders
    let mut candidates: Vec<(String, bool)> = Vec::new();
    for arg in command {
        if let Some((folder, slug)) = extract_slug_from_steamapps_path(arg) {
            let is_infra = is_steam_infrastructure(&folder);
            candidates.push((slug, is_infra));
        }
    }

    // Prefer the first non-infrastructure match
    if let Some((slug, _)) = candidates.iter().find(|(_, is_infra)| !is_infra) {
        return slug.clone();
    }
    // Fall back to the first match even if it's infrastructure
    if let Some((slug, _)) = candidates.first() {
        return slug.clone();
    }

    // Fallback: use executable basename
    if let Some(first) = command.first() {
        let path = std::path::Path::new(first);
        if let Some(stem) = path.file_stem() {
            let slug = normalize_to_slug(&stem.to_string_lossy());
            if !slug.is_empty() {
                return slug;
            }
        }
    }

    "unknown".to_string()
}

fn extract_slug_from_steamapps_path(arg: &str) -> Option<(String, String)> {
    let marker = "steamapps/common/";
    // Case-insensitive search by scanning for the marker in the lowered string,
    // then slicing the original to preserve case for camelCase splitting.
    // Safe because Steam paths are ASCII before the marker.
    let lower = arg.to_lowercase();
    let idx = lower.find(marker)?;
    let after = &arg[idx + marker.len()..];
    let folder = after.split('/').next().unwrap_or("");
    if folder.is_empty() {
        return None;
    }
    let slug = normalize_to_slug(folder);
    if slug.is_empty() {
        None
    } else {
        Some((folder.to_string(), slug))
    }
}

fn normalize_to_slug(name: &str) -> String {
    // Insert hyphens at camelCase boundaries (e.g. MarvelRivals → Marvel-Rivals)
    let mut expanded = String::with_capacity(name.len() + 4);
    let mut prev_lower = false;
    for c in name.chars() {
        if c.is_uppercase() && prev_lower {
            expanded.push('-');
        }
        expanded.push(c);
        prev_lower = c.is_lowercase();
    }

    expanded
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == ' ' || c == '_' || c == '-' {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|&c| c != '\0')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn has_overrides(cli: &CliOverrides) -> bool {
    cli.width.is_some()
        || cli.height.is_some()
        || cli.refresh_rate.is_some()
        || cli.hdr.is_some()
        || cli.vrr.is_some()
        || cli.gamescope.is_some()
        || cli.output.is_some()
        || cli.itm.is_some()
        || cli.fsr4.is_some()
        || cli.vcache.is_some()
        || cli.fix_mouse.is_some()
        || cli.mangohud.is_some()
        || cli.mangohud_config.is_some()
        || cli.vkbasalt.is_some()
        || cli.vkbasalt_profile.is_some()
}

use crate::config::{MangoHudConfig, Profile, ProfileSettings, VkBasaltConfig};

pub fn build_profile_from_overrides(
    overrides: &CliOverrides,
    steam_app_id: Option<u64>,
    display_name: &str,
) -> Profile {
    let has_settings = overrides.width.is_some()
        || overrides.height.is_some()
        || overrides.refresh_rate.is_some()
        || overrides.hdr.is_some()
        || overrides.vrr.is_some()
        || overrides.gamescope.is_some()
        || overrides.output.is_some()
        || overrides.itm.is_some()
        || overrides.fsr4.is_some()
        || overrides.vcache.is_some()
        || overrides.fix_mouse.is_some();

    let settings = if has_settings {
        Some(ProfileSettings {
            width: overrides.width,
            height: overrides.height,
            refresh_rate: overrides.refresh_rate,
            hdr: overrides.hdr,
            vrr: overrides.vrr,
            gamescope: overrides.gamescope,
            output: overrides.output.clone(),
            itm: overrides.itm,
            fsr4: overrides.fsr4,
            vcache: overrides.vcache,
            fix_mouse: overrides.fix_mouse,
        })
    } else {
        None
    };

    let mangohud = if overrides.mangohud.is_some() || overrides.mangohud_config.is_some() {
        Some(MangoHudConfig {
            enabled: overrides.mangohud,
            config: overrides.mangohud_config.clone(),
        })
    } else {
        None
    };

    let vkbasalt = if overrides.vkbasalt.is_some() || overrides.vkbasalt_profile.is_some() {
        Some(VkBasaltConfig {
            enabled: overrides.vkbasalt,
            profile: overrides.vkbasalt_profile.clone(),
        })
    } else {
        None
    };

    Profile {
        name: Some(display_name.to_string()),
        steam_app_id,
        settings,
        mangohud,
        vkbasalt,
    }
}

pub fn list_history() -> Result<()> {
    let history = load_history()?;
    if history.is_empty() {
        println!("No launch history. Run a game with CLI flags to start recording.");
        return Ok(());
    }

    println!(
        "{:<25} {:<12} {:<10} LAST LAUNCHED",
        "SLUG", "APP ID", "LAUNCHES"
    );
    println!("{}", "-".repeat(65));

    for (slug, app) in history.iter() {
        let app_id = app
            .steam_app_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "-".to_string());
        let last = app
            .launches
            .first()
            .map(|l| l.timestamp.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<25} {:<12} {:<10} {}",
            slug,
            app_id,
            app.launches.len(),
            last
        );
    }

    Ok(())
}

pub fn show_history(slug: &str) -> Result<()> {
    let history = load_history()?;
    let app = history
        .get(slug)
        .ok_or_else(|| anyhow::anyhow!("no history for '{slug}'"))?;

    if let Some(id) = app.steam_app_id {
        println!("App: {slug} (Steam ID: {id})");
    } else {
        println!("App: {slug}");
    }
    println!();

    for (i, launch) in app.launches.iter().enumerate() {
        println!(
            "  Launch {} — {}",
            i + 1,
            launch.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!("    Command: {}", launch.command.join(" "));
        let json = serde_json::to_string(&launch.overrides).unwrap_or_else(|_| "{}".to_string());
        println!("    Overrides: {json}");
        println!();
    }

    Ok(())
}

pub fn promote_to_profile(
    slug: &str,
    launch_index: usize,
    profile_name: Option<&str>,
) -> Result<()> {
    let history = load_history()?;
    let app = history
        .get(slug)
        .ok_or_else(|| anyhow::anyhow!("no history for '{slug}'"))?;

    if launch_index == 0 || launch_index > app.launches.len() {
        anyhow::bail!(
            "launch index {launch_index} out of range (1-{})",
            app.launches.len()
        );
    }

    let launch = &app.launches[launch_index - 1];
    let target_slug = profile_name.unwrap_or(slug);
    let profile_path = config::profile_path(target_slug);

    if profile_path.exists() {
        anyhow::bail!(
            "profile '{target_slug}' already exists. Use --name to choose a different name."
        );
    }

    let display_name = profile_name.unwrap_or(slug).replace('-', " ");
    let profile = build_profile_from_overrides(&launch.overrides, app.steam_app_id, &display_name);
    config::save_profile(target_slug, &profile)?;

    println!("Created profile: {}", profile_path.display());
    println!("Edit it with: glaunch profile edit {target_slug}");
    Ok(())
}

pub fn clear_history(slug: Option<&str>) -> Result<()> {
    match slug {
        Some(slug) => {
            let mut history = load_history()?;
            if history.remove(slug).is_some() {
                save_history(&history)?;
                println!("Cleared history for '{slug}'.");
            } else {
                anyhow::bail!("no history for '{slug}'");
            }
        }
        None => {
            let history = History::default();
            save_history(&history)?;
            println!("Cleared all launch history.");
        }
    }
    Ok(())
}
