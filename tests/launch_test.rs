use glaunch::config::ResolvedSettings;
use glaunch::hardware::VcacheInfo;
use glaunch::launch::build_launch_plan;

fn default_settings() -> ResolvedSettings {
    ResolvedSettings {
        width: 3840,
        height: 2160,
        refresh_rate: 240,
        hdr: true,
        vrr: true,
        gamescope: true,
        itm: false,
        fsr4: false,
        vcache: false,
        fix_mouse: false,
        mangohud: false,
        mangohud_config: None,
        vkbasalt: false,
        vkbasalt_profile: None,
        output: None,
    }
}

#[test]
fn test_basic_gamescope_command() {
    let settings = default_settings();
    let game_cmd = vec!["wine".to_string(), "game.exe".to_string()];

    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    assert_eq!(plan.command[0], "gamemoderun");
    assert_eq!(plan.command[1], "gamescope");
    assert!(plan.command.contains(&"-W".to_string()));
    assert!(plan.command.contains(&"3840".to_string()));
    assert!(plan.command.contains(&"-H".to_string()));
    assert!(plan.command.contains(&"2160".to_string()));
    assert!(plan.command.contains(&"--fullscreen".to_string()));
    assert!(plan.command.contains(&"-r".to_string()));
    assert!(plan.command.contains(&"240".to_string()));
    assert!(plan.command.contains(&"--hdr-enabled".to_string()));
    assert!(plan.command.contains(&"--adaptive-sync".to_string()));
    // game command should be after the --
    let separator_pos = plan.command.iter().position(|s| s == "--").unwrap();
    assert_eq!(plan.command[separator_pos + 1], "wine");
    assert_eq!(plan.command[separator_pos + 2], "game.exe");
    // no --display-index without explicit output
    assert!(!plan.command.contains(&"--display-index".to_string()));
}

#[test]
fn test_no_gamescope() {
    let mut settings = default_settings();
    settings.gamescope = false;

    let game_cmd = vec!["wine".to_string(), "game.exe".to_string()];
    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    assert_eq!(plan.command[0], "gamemoderun");
    assert_eq!(plan.command[1], "wine");
    assert_eq!(plan.command[2], "game.exe");
    assert!(!plan.command.contains(&"gamescope".to_string()));
}

#[test]
fn test_hdr_disabled() {
    let mut settings = default_settings();
    settings.hdr = false;

    let game_cmd = vec!["game".to_string()];
    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    assert!(!plan.command.contains(&"--hdr-enabled".to_string()));
}

#[test]
fn test_itm_adds_flag() {
    let mut settings = default_settings();
    settings.itm = true;

    let game_cmd = vec!["game".to_string()];
    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    assert!(plan.command.contains(&"--hdr-itm-enable".to_string()));
}

#[test]
fn test_fsr4_env_var() {
    let mut settings = default_settings();
    settings.fsr4 = true;

    let game_cmd = vec!["game".to_string()];
    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    assert!(
        plan.env_vars
            .contains(&("PROTON_FSR4_RDNA3_UPGRADE".to_string(), "1".to_string()))
    );
}

#[test]
fn test_vkbasalt_env_vars() {
    let mut settings = default_settings();
    settings.vkbasalt = true;
    settings.vkbasalt_profile = Some("reshade-film".to_string());

    let game_cmd = vec!["game".to_string()];
    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    assert!(
        plan.env_vars
            .contains(&("ENABLE_VKBASALT".to_string(), "1".to_string()))
    );
    let vkbasalt_config = plan
        .env_vars
        .iter()
        .find(|(k, _)| k == "VKBASALT_CONFIG_FILE")
        .map(|(_, v)| v.as_str());
    assert!(vkbasalt_config.is_some());
    assert!(vkbasalt_config.unwrap().contains("reshade-film.conf"));
}

#[test]
fn test_mangohud_env_vars() {
    let mut settings = default_settings();
    settings.mangohud = true;
    settings.mangohud_config = Some("fps-only".to_string());

    let game_cmd = vec!["game".to_string()];
    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    assert!(
        plan.env_vars
            .contains(&("MANGOHUD".to_string(), "1".to_string()))
    );
    let mangohud_config = plan
        .env_vars
        .iter()
        .find(|(k, _)| k == "MANGOHUD_CONFIGFILE")
        .map(|(_, v)| v.as_str());
    assert!(mangohud_config.is_some());
    assert!(mangohud_config.unwrap().contains("fps-only.conf"));
}

#[test]
fn test_fix_mouse_ld_preload() {
    let mut settings = default_settings();
    settings.fix_mouse = true;

    let game_cmd = vec!["game".to_string()];
    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    assert!(plan.env_vars.contains(&(
        "LD_PRELOAD".to_string(),
        "/usr/lib/libgamemodeauto.so.0".to_string()
    )));
}

#[test]
fn test_vcache_pinning_wraps_command() {
    let settings = default_settings();
    let vcache = VcacheInfo {
        cpus: "0-7".to_string(),
        l3_size_kb: 98304,
    };

    let game_cmd = vec!["game".to_string()];
    let plan = build_launch_plan(&settings, Some(&vcache), None, &game_cmd).unwrap();

    assert_eq!(plan.command[0], "systemd-run");
    assert!(plan.command.contains(&"--user".to_string()));
    assert!(plan.command.contains(&"--scope".to_string()));
    assert!(plan.command.contains(&"AllowedCPUs=0-7".to_string()));
}

#[test]
fn test_display_index_added_when_present() {
    let settings = default_settings();
    let game_cmd = vec!["game".to_string()];
    let plan = build_launch_plan(&settings, None, Some(2), &game_cmd).unwrap();

    let idx_pos = plan
        .command
        .iter()
        .position(|s| s == "--display-index")
        .expect("--display-index should be present");
    assert_eq!(plan.command[idx_pos + 1], "2");

    // --display-index should come before the -- separator
    let separator_pos = plan.command.iter().position(|s| s == "--").unwrap();
    assert!(idx_pos < separator_pos);
}

#[test]
fn test_display_index_not_added_without_gamescope() {
    let mut settings = default_settings();
    settings.gamescope = false;

    let game_cmd = vec!["game".to_string()];
    // Even if display_index is passed, it shouldn't appear without gamescope
    let plan = build_launch_plan(&settings, None, Some(0), &game_cmd).unwrap();
    assert!(!plan.command.contains(&"--display-index".to_string()));
}

#[test]
fn test_dry_run_output_format() {
    let mut settings = default_settings();
    settings.fsr4 = true;
    settings.gamescope = false;

    let game_cmd = vec!["wine".to_string(), "game.exe".to_string()];
    let plan = build_launch_plan(&settings, None, None, &game_cmd).unwrap();

    let output = glaunch::launch::dry_run_output(&plan);
    assert!(output.contains("PROTON_FSR4_RDNA3_UPGRADE=1"));
    assert!(output.contains("gamemoderun wine game.exe"));
}
