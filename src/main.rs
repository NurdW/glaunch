mod cli;
mod profile;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use glaunch::{config, hardware, history, launch};
use hardware::resolve_wayland_display_index;

fn main() {
    if let Err(e) = run() {
        eprintln!("glaunch: {e:#}");
        let code = if e.downcast_ref::<HardwareError>().is_some() {
            2
        } else {
            1
        };
        std::process::exit(code);
    }
}

#[derive(Debug)]
struct HardwareError;
impl std::fmt::Display for HardwareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hardware detection failure")
    }
}
impl std::error::Error for HardwareError {}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run(args) => {
            let global = config::load_global_config()?;

            // Load profile (explicit flag, or auto-detect from Steam app ID)
            let profile = if let Some(ref profile_name) = args.profile {
                config::load_profile(profile_name)?
            } else if let Some(app_id) = config::extract_steam_app_id(&args.command) {
                if args.verbose {
                    eprintln!("glaunch: detected Steam app ID {app_id}");
                }
                config::find_profile_by_app_id(app_id)?.map(|(_, p)| p)
            } else {
                None
            };

            let cli_overrides = cli_overrides_from_run_args(&args);
            let settings = config::resolve_settings_from_layers(
                Some(&cli_overrides),
                profile.as_ref(),
                &global,
            );

            if args.verbose {
                log_settings(&settings, &cli_overrides, profile.as_ref());
            }

            // Validate config files before launching
            if settings.vkbasalt
                && let Some(profile) = &settings.vkbasalt_profile
                && !profile.is_empty()
            {
                let home = dirs::home_dir().expect("could not determine home directory");
                let config_path = home
                    .join(".config")
                    .join("vkBasalt")
                    .join(format!("{profile}.conf"));
                if !config_path.exists() {
                    anyhow::bail!("vkBasalt profile not found: {}", config_path.display());
                }
            }

            if settings.mangohud
                && let Some(config) = &settings.mangohud_config
                && !config.is_empty()
            {
                let home = dirs::home_dir().expect("could not determine home directory");
                let config_path = home
                    .join(".config")
                    .join("MangoHud")
                    .join(format!("{config}.conf"));
                if !config_path.exists() {
                    anyhow::bail!("MangoHud config not found: {}", config_path.display());
                }
            }

            // V-Cache detection (only if settings say vcache is enabled)
            let vcache = if settings.vcache {
                match hardware::detect_vcache() {
                    Ok(Some(info)) => {
                        if args.verbose {
                            eprintln!(
                                "glaunch: V-Cache detected — pinning to CPUs {} (L3: {}MB)",
                                info.cpus,
                                info.l3_size_kb / 1024
                            );
                        }
                        Some(info)
                    }
                    Ok(None) => {
                        eprintln!(
                            "glaunch: No X3D V-Cache topology detected — all CCDs have equal L3"
                        );
                        None
                    }
                    Err(e) => {
                        eprintln!("glaunch: V-Cache detection failed: {e}");
                        None
                    }
                }
            } else {
                None
            };

            // Resolve Wayland display index for --output / output config
            let display_index = if settings.gamescope {
                if let Some(ref connector) = settings.output {
                    match resolve_wayland_display_index(connector) {
                        Ok(Some(idx)) => {
                            if args.verbose {
                                eprintln!(
                                    "glaunch: Resolved output {connector} → display index {idx}"
                                );
                            }
                            Some(idx)
                        }
                        Ok(None) => {
                            eprintln!(
                                "glaunch: warning: output '{connector}' not found in Wayland outputs"
                            );
                            None
                        }
                        Err(e) => {
                            eprintln!("glaunch: warning: failed to resolve output: {e}");
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let plan =
                launch::build_launch_plan(&settings, vcache.as_ref(), display_index, &args.command)?;

            if args.dry_run {
                println!("{}", launch::dry_run_output(&plan));
                return Ok(());
            }

            // Record launch history (non-fatal)
            if history::has_overrides(&cli_overrides) {
                let slug = history::derive_app_slug(&args.command);
                let app_id = config::extract_steam_app_id(&args.command);
                match history::load_history() {
                    Ok(mut hist) => {
                        history::record_launch(
                            &mut hist,
                            &slug,
                            app_id,
                            args.command.clone(),
                            cli_overrides,
                        );
                        if let Err(e) = history::save_history(&hist) {
                            eprintln!("glaunch: warning: failed to save launch history: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("glaunch: warning: failed to load launch history: {e}");
                    }
                }
            }

            launch::execute(&plan)?;
            unreachable!()
        }
        Command::Profile(args) => {
            match args.command {
                cli::ProfileCommand::List => profile::list()?,
                cli::ProfileCommand::Show { name } => profile::show(&name)?,
                cli::ProfileCommand::Create { name } => profile::create(&name)?,
                cli::ProfileCommand::Edit { name } => profile::edit(&name)?,
                cli::ProfileCommand::Delete { name } => profile::delete(&name)?,
            }
            Ok(())
        }
        Command::Tui => {
            glaunch::tui::run_tui()?;
            Ok(())
        }
        Command::Info(args) => {
            let info = hardware::detect_hardware(args.refresh)
                .map_err(|e| anyhow::anyhow!(HardwareError).context(e))?;

            println!("=== glaunch hardware info ===\n");

            match &info.vcache {
                Some(vc) => {
                    println!("V-Cache: detected");
                    println!("  CPUs: {}", vc.cpus);
                    println!("  L3 cache: {} MB", vc.l3_size_kb / 1024);
                }
                None => println!("V-Cache: not detected (symmetric L3 or single CCD)"),
            }

            if info.gpus.is_empty() {
                println!("\nGPU: not detected");
            } else {
                for (i, gpu) in info.gpus.iter().enumerate() {
                    println!("\nGPU {i}: {}", gpu.name);
                    if !gpu.driver.is_empty() {
                        println!("  Driver: {}", gpu.driver);
                    }
                }
            }

            Ok(())
        }
        Command::History(args) => {
            match args.command {
                cli::HistoryCommand::List => glaunch::history::list_history()?,
                cli::HistoryCommand::Show { slug } => glaunch::history::show_history(&slug)?,
                cli::HistoryCommand::Promote { slug, launch, name } => {
                    glaunch::history::promote_to_profile(&slug, launch, name.as_deref())?;
                }
                cli::HistoryCommand::Clear { slug } => {
                    glaunch::history::clear_history(slug.as_deref())?;
                }
            }
            Ok(())
        }
    }
}

fn cli_overrides_from_run_args(args: &cli::RunArgs) -> config::CliOverrides {
    config::CliOverrides {
        width: args.width,
        height: args.height,
        refresh_rate: args.refresh_rate,
        hdr: if args.no_hdr { Some(false) } else { None },
        vrr: if args.no_vrr { Some(false) } else { None },
        gamescope: if args.no_gamescope { Some(false) } else { None },
        output: args.output.clone(),
        itm: if args.itm { Some(true) } else { None },
        fsr4: if args.fsr4 { Some(true) } else { None },
        vcache: if args.vcache {
            Some(true)
        } else if args.no_vcache {
            Some(false)
        } else {
            None
        },
        fix_mouse: if args.fix_mouse { Some(true) } else { None },
        mangohud: args.mangohud.as_ref().map(|_| true),
        mangohud_config: args.mangohud.clone().filter(|s| !s.is_empty()),
        vkbasalt: args.vkbasalt.as_ref().map(|_| true),
        vkbasalt_profile: args.vkbasalt.clone().filter(|s| !s.is_empty()),
    }
}

fn log_settings(
    settings: &config::ResolvedSettings,
    cli: &config::CliOverrides,
    profile: Option<&config::Profile>,
) {
    let profile_name = profile.and_then(|p| p.name.as_deref());
    let prof_settings = profile.and_then(|p| p.settings.as_ref());

    let source_bool = |cli_val: Option<bool>, prof_val: Option<bool>| -> &'static str {
        if cli_val.is_some() {
            " (CLI)"
        } else if prof_val.is_some() {
            " (profile)"
        } else {
            ""
        }
    };

    if let Some(name) = profile_name {
        eprintln!("glaunch: Using profile '{name}'");
    }
    eprintln!(
        "glaunch: Resolution: {}x{} @ {}Hz",
        settings.width, settings.height, settings.refresh_rate
    );
    eprintln!(
        "glaunch: HDR: {}{}",
        if settings.hdr { "enabled" } else { "disabled" },
        source_bool(cli.hdr, prof_settings.and_then(|s| s.hdr))
    );
    eprintln!(
        "glaunch: VRR: {}{}",
        if settings.vrr { "enabled" } else { "disabled" },
        source_bool(cli.vrr, prof_settings.and_then(|s| s.vrr))
    );
    eprintln!(
        "glaunch: Gamescope: {}{}",
        if settings.gamescope {
            "enabled"
        } else {
            "disabled"
        },
        source_bool(cli.gamescope, prof_settings.and_then(|s| s.gamescope))
    );
    if settings.itm {
        eprintln!(
            "glaunch: ITM: enabled{}",
            source_bool(cli.itm, prof_settings.and_then(|s| s.itm))
        );
    }
    if settings.fsr4 {
        eprintln!("glaunch: FSR4: enabled");
    }
    if settings.mangohud {
        eprintln!("glaunch: MangoHud: enabled");
    }
    if settings.vkbasalt {
        let vk_profile = settings.vkbasalt_profile.as_deref().unwrap_or("default");
        eprintln!("glaunch: vkBasalt: enabled (profile: {vk_profile})");
    }
    if settings.fix_mouse {
        eprintln!("glaunch: Mouse fix: enabled");
    }
    if settings.vcache {
        eprintln!("glaunch: V-Cache pinning: requested");
    }
    if let Some(ref output) = settings.output {
        eprintln!("glaunch: Output: {output}");
    }
}
