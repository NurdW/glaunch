use std::os::unix::process::CommandExt;
use std::process::Command as StdCommand;

use anyhow::{Context, Result, bail};

use crate::config::ResolvedSettings;
use crate::hardware::VcacheInfo;

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub env_vars: Vec<(String, String)>,
    pub command: Vec<String>,
}

pub fn build_launch_plan(
    settings: &ResolvedSettings,
    vcache: Option<&VcacheInfo>,
    display_index: Option<u32>,
    game_command: &[String],
) -> Result<LaunchPlan> {
    if game_command.is_empty() {
        bail!("no game command provided");
    }

    let mut env_vars: Vec<(String, String)> = Vec::new();
    // Accumulate LD_PRELOAD entries so multiple features can contribute
    let mut ld_preload_entries: Vec<String> = Vec::new();

    if settings.fsr4 {
        env_vars.push(("PROTON_FSR4_RDNA3_UPGRADE".into(), "1".into()));
    }

    if settings.vkbasalt {
        env_vars.push(("ENABLE_VKBASALT".into(), "1".into()));
        if let Some(profile) = &settings.vkbasalt_profile
            && !profile.is_empty()
        {
            let home = dirs::home_dir().expect("could not determine home directory");
            let config_path = home
                .join(".config")
                .join("vkBasalt")
                .join(format!("{profile}.conf"));
            env_vars.push((
                "VKBASALT_CONFIG_FILE".into(),
                config_path.to_string_lossy().to_string(),
            ));
        }
    }

    if settings.mangohud {
        env_vars.push(("MANGOHUD".into(), "1".into()));
        // Preload the MangoHud shim so OpenGL games get the overlay too.
        // $LIB is resolved by the dynamic linker to lib64 or lib at runtime.
        ld_preload_entries.push("/usr/$LIB/mangohud/libMangoHud_shim.so".into());
        if let Some(config) = &settings.mangohud_config
            && !config.is_empty()
        {
            let home = dirs::home_dir().expect("could not determine home directory");
            let config_path = home
                .join(".config")
                .join("MangoHud")
                .join(format!("{config}.conf"));
            env_vars.push((
                "MANGOHUD_CONFIGFILE".into(),
                config_path.to_string_lossy().to_string(),
            ));
        }
    }

    if settings.fix_mouse {
        ld_preload_entries.push("/usr/lib/libgamemodeauto.so.0".into());
    }

    // Merge all LD_PRELOAD entries with any existing value from the environment
    if !ld_preload_entries.is_empty() {
        if let Ok(existing) = std::env::var("LD_PRELOAD")
            && !existing.is_empty()
        {
            ld_preload_entries.push(existing);
        }
        env_vars.push(("LD_PRELOAD".into(), ld_preload_entries.join(":")));
    }

    let mut command: Vec<String> = Vec::new();

    // Gamescope wrapping
    if settings.gamescope {
        command.push("gamemoderun".into());
        command.push("gamescope".into());
        command.extend(["-W".into(), settings.width.to_string()]);
        command.extend(["-H".into(), settings.height.to_string()]);
        command.push("--fullscreen".into());
        command.extend(["-r".into(), settings.refresh_rate.to_string()]);

        if let Some(idx) = display_index {
            command.extend(["--display-index".into(), idx.to_string()]);
        }

        if settings.vrr {
            command.push("--adaptive-sync".into());
        }
        if settings.hdr {
            command.push("--hdr-enabled".into());
        }
        if settings.itm {
            command.push("--hdr-itm-enable".into());
        }
        if settings.mangohud {
            command.push("--mangoapp".into());
        }

        command.push("--".into());
        command.extend_from_slice(game_command);
    } else {
        command.push("gamemoderun".into());
        command.extend_from_slice(game_command);
    }

    // V-Cache pinning wraps the whole thing
    if let Some(vcache) = vcache {
        let mut wrapped = vec![
            "systemd-run".into(),
            "--user".into(),
            "--scope".into(),
            "-p".into(),
            format!("AllowedCPUs={}", vcache.cpus),
            "--".into(),
        ];
        wrapped.append(&mut command);
        command = wrapped;
    }

    Ok(LaunchPlan { env_vars, command })
}

fn shell_quote(s: &str) -> String {
    if s.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '\\' || c == '$') {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

pub fn dry_run_output(plan: &LaunchPlan) -> String {
    let mut parts = Vec::new();
    for (key, value) in &plan.env_vars {
        parts.push(format!("{key}={}", shell_quote(value)));
    }
    for arg in &plan.command {
        parts.push(shell_quote(arg));
    }
    parts.join(" ")
}

pub fn execute(plan: &LaunchPlan) -> Result<()> {
    if plan.command.is_empty() {
        bail!("empty command");
    }

    let mut cmd = StdCommand::new(&plan.command[0]);
    cmd.args(&plan.command[1..]);
    for (key, value) in &plan.env_vars {
        cmd.env(key, value);
    }

    let err = cmd.exec();
    Err(err).with_context(|| format!("failed to exec {}", plan.command[0]))
}
