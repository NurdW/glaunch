//! glaunch-gui — a small desktop window for building `glaunch run` commands
//! for Steam's Launch Options box, and for saving/loading glaunch profiles.
//!
//! Why this exists: glaunch itself is a terminal tool, but the command you
//! paste into Steam is fiddly to type by hand. This window turns the options
//! into checkboxes and shows the finished command live.
//!
//! It links against the real glaunch library (path dependency) so profile
//! files written here are guaranteed to use the exact same format glaunch
//! reads — the two can never drift apart.

use std::process::Command;

use eframe::egui::{self, ComboBox, ScrollArea, TextEdit, Ui, Widget};
use glaunch::config::{
    self, MangoHudConfig, Profile, ProfileSettings, VkBasaltConfig,
};

// glaunch's own hardcoded fallbacks (see glaunch/src/config.rs).
// We only emit a flag when a value differs from these, keeping the
// generated command short and letting glaunch stay the source of truth.
const DEFAULT_WIDTH: u32 = 3840;
const DEFAULT_HEIGHT: u32 = 2160;
const DEFAULT_REFRESH: u32 = 240;

/// Everything the user can tweak in the form.
/// Numbers are kept as raw text so half-typed values ("1920", "192", "")
/// never crash the app; we parse them lazily with a fallback.
#[derive(Clone)]
struct Form {
    width: String,
    height: String,
    refresh: String,
    gamescope: bool,
    hdr: bool,
    vrr: bool,
    itm: bool,
    fsr4: bool,
    vcache: bool,
    fix_mouse: bool,
    mangohud: bool,
    mangohud_config: String,
    vkbasalt: bool,
    vkbasalt_profile: String,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH.to_string(),
            height: DEFAULT_HEIGHT.to_string(),
            refresh: DEFAULT_REFRESH.to_string(),
            gamescope: true,
            hdr: true,
            vrr: true,
            itm: false,
            fsr4: false,
            vcache: false,
            fix_mouse: false,
            mangohud: false,
            mangohud_config: String::new(),
            vkbasalt: false,
            vkbasalt_profile: String::new(),
        }
    }
}

impl Form {
    fn width_num(&self) -> u32 {
        self.width.trim().parse().unwrap_or(DEFAULT_WIDTH)
    }
    fn height_num(&self) -> u32 {
        self.height.trim().parse().unwrap_or(DEFAULT_HEIGHT)
    }
    fn refresh_num(&self) -> u32 {
        self.refresh.trim().parse().unwrap_or(DEFAULT_REFRESH)
    }

    /// The `glaunch run` flags implied by the form.
    /// Only non-default values get a flag, mirroring glaunch's defaults.
    fn flags(&self) -> Vec<String> {
        let mut f = Vec::new();
        let (w, h, r) = (self.width_num(), self.height_num(), self.refresh_num());
        if w != DEFAULT_WIDTH {
            f.extend(["-w".into(), w.to_string()]);
        }
        if h != DEFAULT_HEIGHT {
            f.extend(["-H".into(), h.to_string()]);
        }
        if r != DEFAULT_REFRESH {
            f.extend(["-r".into(), r.to_string()]);
        }
        // glaunch turns gamescope/hdr/vrr ON by default, so we only ever
        // need "off" flags for them.
        if !self.gamescope {
            f.push("--no-gamescope".into());
        }
        if !self.hdr {
            f.push("--no-hdr".into());
        }
        if !self.vrr {
            f.push("--no-vrr".into());
        }
        if self.itm {
            f.push("--itm".into());
        }
        if self.fsr4 {
            f.push("--fsr4".into());
        }
        if self.vcache {
            f.push("--vcache".into());
        }
        if self.fix_mouse {
            f.push("--fix-mouse".into());
        }
        if self.mangohud {
            f.push("--mangohud".into());
            let cfg = self.mangohud_config.trim();
            if !cfg.is_empty() {
                f.push(cfg.to_string());
            }
        }
        if self.vkbasalt {
            f.push("--vkbasalt".into());
            let p = self.vkbasalt_profile.trim();
            if !p.is_empty() {
                f.push(p.to_string());
            }
        }
        f
    }

    /// The one-line command to paste into Steam's Launch Options field.
    fn steam_command(&self) -> String {
        let mut parts = vec!["glaunch".to_string(), "run".to_string()];
        parts.extend(self.flags());
        parts.push("--".into());
        parts.push("%command%".into());
        parts.join(" ")
    }

    /// Fill the form from a saved profile. Missing (None) fields fall back
    /// to glaunch's defaults, matching how glaunch itself resolves layers.
    fn from_profile(profile: &Profile) -> Self {
        let s = profile.settings.clone().unwrap_or_default();
        let pick = |v: Option<bool>, fallback: bool| v.unwrap_or(fallback);
        let num = |v: Option<u32>, fallback: u32| v.unwrap_or(fallback).to_string();
        Self {
            width: num(s.width, DEFAULT_WIDTH),
            height: num(s.height, DEFAULT_HEIGHT),
            refresh: num(s.refresh_rate, DEFAULT_REFRESH),
            gamescope: pick(s.gamescope, true),
            hdr: pick(s.hdr, true),
            vrr: pick(s.vrr, true),
            itm: pick(s.itm, false),
            fsr4: pick(s.fsr4, false),
            vcache: pick(s.vcache, false),
            fix_mouse: pick(s.fix_mouse, false),
            mangohud: profile
                .mangohud
                .as_ref()
                .and_then(|m| m.enabled)
                .unwrap_or(false),
            mangohud_config: profile
                .mangohud
                .as_ref()
                .and_then(|m| m.config.clone())
                .unwrap_or_default(),
            vkbasalt: profile
                .vkbasalt
                .as_ref()
                .and_then(|v| v.enabled)
                .unwrap_or(false),
            vkbasalt_profile: profile
                .vkbasalt
                .as_ref()
                .and_then(|v| v.profile.clone())
                .unwrap_or_default(),
        }
    }

    /// Build the profile file structure. Every setting is written explicitly
    /// so a GUI-saved profile fully describes the game (no hidden inheritance).
    fn to_profile(&self, slug: &str, app_id: Option<u64>) -> Profile {
        Profile {
            name: Some(slug.to_string()),
            steam_app_id: app_id,
            settings: Some(ProfileSettings {
                width: Some(self.width_num()),
                height: Some(self.height_num()),
                refresh_rate: Some(self.refresh_num()),
                hdr: Some(self.hdr),
                vrr: Some(self.vrr),
                gamescope: Some(self.gamescope),
                itm: Some(self.itm),
                fsr4: Some(self.fsr4),
                vcache: Some(self.vcache),
                fix_mouse: Some(self.fix_mouse),
            }),
            mangohud: Some(MangoHudConfig {
                enabled: Some(self.mangohud),
                config: nonempty(&self.mangohud_config),
            }),
            vkbasalt: Some(VkBasaltConfig {
                enabled: Some(self.vkbasalt),
                profile: nonempty(&self.vkbasalt_profile),
            }),
        }
    }
}

fn nonempty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

struct App {
    form: Form,
    profiles: Vec<(String, Profile)>,
    profile_idx: Option<usize>,
    slug: String,
    app_id: String,
    status: String,
    status_is_error: bool,
    /// Output of the last "preview full command" dry run.
    preview: Option<String>,
    /// Two-step delete guard: Some(slug) means we're asking "really?".
    pending_delete: Option<String>,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            form: Form::default(),
            profiles: Vec::new(),
            profile_idx: None,
            slug: String::new(),
            app_id: String::new(),
            status: String::from("Ready. Pick settings, then copy the command."),
            status_is_error: false,
            preview: None,
            pending_delete: None,
        };
        app.refresh_profiles();
        app
    }

    fn refresh_profiles(&mut self) {
        self.profiles = config::list_profiles().unwrap_or_default();
        if let Some(idx) = self.profile_idx
            && idx >= self.profiles.len()
        {
            self.profile_idx = None;
        }
    }

    fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status = msg.into();
        self.status_is_error = is_error;
    }

    fn copy_command(&mut self) {
        let cmd = self.form.steam_command();
        match arboard::Clipboard::new() {
            Ok(mut cb) => match cb.set_text(cmd.clone()) {
                Ok(()) => self.set_status(format!("Copied to clipboard: {cmd}"), false),
                Err(e) => self.set_status(format!("Clipboard failed: {e}"), true),
            },
            Err(e) => self.set_status(format!("Clipboard unavailable: {e}"), true),
        }
    }

    /// Ask the real glaunch binary (installed on PATH) to print the fully
    /// resolved launch command without running anything (--dry-run).
    fn preview_full(&mut self) {
        let mut args = self.form.flags();
        args.push("-n".into());
        args.extend(["--".into(), "%command%".into()]);
        match Command::new("glaunch").args(&args).output() {
            Ok(out) if out.status.success() => {
                self.preview = Some(String::from_utf8_lossy(&out.stdout).trim().to_string());
                self.set_status("Preview generated by the real glaunch binary.", false);
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                self.preview = None;
                self.set_status(format!("glaunch said: {err}"), true);
            }
            Err(e) => {
                self.preview = None;
                self.set_status(format!("Could not run 'glaunch' (is it on PATH?): {e}"), true);
            }
        }
    }

    fn save_profile(&mut self) {
        let slug = self.slug.trim().to_string();
        if slug.is_empty() {
            self.set_status("Give the profile a name first (letters, digits, - _ .)", true);
            return;
        }
        let app_id = self.app_id.trim().parse::<u64>().ok();
        if !self.app_id.trim().is_empty() && app_id.is_none() {
            self.set_status("Steam App ID must be a number (or leave it empty).", true);
            return;
        }
        let profile = self.form.to_profile(&slug, app_id);
        match config::save_profile(&slug, &profile) {
            Ok(()) => {
                self.refresh_profiles();
                self.set_status(format!("Saved profile '{slug}' to ~/.config/glaunch/profiles/"), false);
            }
            Err(e) => self.set_status(format!("Could not save: {e}"), true),
        }
    }

    fn load_profile(&mut self) {
        let Some(idx) = self.profile_idx else { return };
        let Some((slug, profile)) = self.profiles.get(idx).cloned() else {
            return;
        };
        self.form = Form::from_profile(&profile);
        self.slug = slug.clone();
        self.app_id = profile
            .steam_app_id
            .map(|id| id.to_string())
            .unwrap_or_default();
        self.preview = None;
        self.set_status(format!("Loaded profile '{slug}'."), false);
    }

    fn delete_profile(&mut self) {
        let Some(slug) = self.pending_delete.take() else { return };
        match config::delete_profile(&slug) {
            Ok(true) => {
                self.refresh_profiles();
                self.set_status(format!("Deleted profile '{slug}'."), false);
            }
            Ok(false) => self.set_status(format!("Profile '{slug}' was already gone."), true),
            Err(e) => self.set_status(format!("Could not delete: {e}"), true),
        }
    }

    // ---- UI sections ----

    fn draw_settings(&mut self, ui: &mut Ui) {
        let f = &mut self.form;

        ui.group(|ui| {
            ui.strong("Display");
            ui.horizontal(|ui| {
                ui.label("Width");
                TextEdit::singleline(&mut f.width).hint_text("3840").desired_width(64.0).ui(ui);
                ui.label("Height");
                TextEdit::singleline(&mut f.height).hint_text("2160").desired_width(64.0).ui(ui);
                ui.label("Refresh");
                TextEdit::singleline(&mut f.refresh).hint_text("240").desired_width(52.0).ui(ui);
            });
            ui.checkbox(&mut f.gamescope, "Use gamescope (window/composition wrapper)");
        });

        ui.group(|ui| {
            ui.strong("Picture & system");
            ui.checkbox(&mut f.hdr, "HDR output");
            ui.checkbox(&mut f.vrr, "Adaptive sync (VRR)");
            ui.checkbox(&mut f.itm, "HDR inverse tone mapping for SDR games (--itm)");
            ui.checkbox(&mut f.fsr4, "FSR4 upscaling for RDNA3 GPUs (--fsr4)");
            ui.checkbox(&mut f.vcache, "Pin to AMD V-Cache CCD (--vcache)");
            ui.checkbox(&mut f.fix_mouse, "Fix Steam mouse stutter via gamemode LD_PRELOAD");
        });

        ui.group(|ui| {
            ui.strong("Overlays & post-processing");
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.mangohud, "MangoHud (FPS/perf overlay)");
                TextEdit::singleline(&mut f.mangohud_config)
                    .hint_text("config preset name (optional)")
                    .desired_width(170.0)
                    .ui(ui);
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.vkbasalt, "vkBasalt (post-processing shaders)");
                TextEdit::singleline(&mut f.vkbasalt_profile)
                    .hint_text("profile name (optional)")
                    .desired_width(170.0)
                    .ui(ui);
            });
        });
    }

    fn draw_output(&mut self, ui: &mut Ui) {
        let cmd = self.form.steam_command();

        ui.group(|ui| {
            ui.strong("Steam Launch Options command");
            ui.label("Paste this into Steam → game → Properties → Launch Options:");
            ui.horizontal_wrapped(|ui| {
                ui.monospace(cmd.clone());
            });
            ui.horizontal(|ui| {
                if ui.button("📋 Copy command").clicked() {
                    self.copy_command();
                }
                if ui.button("Preview fully resolved command").clicked() {
                    self.preview_full();
                }
            });
            if let Some(preview) = &self.preview {
                ui.separator();
                ui.label("What glaunch will actually execute (dry run):");
                ui.monospace(preview.clone());
            }
        });

        ui.group(|ui| {
            ui.strong("Saved profiles");
            ui.label("Profiles live in ~/.config/glaunch/profiles and can be auto-applied per game.");
            ui.horizontal(|ui| {
                ui.label("Name");
                TextEdit::singleline(&mut self.slug)
                    .hint_text("e.g. cyberpunk")
                    .desired_width(140.0)
                    .ui(ui);
                ui.label("Steam App ID");
                TextEdit::singleline(&mut self.app_id)
                    .hint_text("optional")
                    .desired_width(80.0)
                    .ui(ui);
            });
            ui.horizontal(|ui| {
                if ui.button("💾 Save current settings as profile").clicked() {
                    self.save_profile();
                }
                if ui.button("↻ Refresh list").clicked() {
                    self.refresh_profiles();
                }
            });

            let selected_text = self
                .profile_idx
                .and_then(|i| self.profiles.get(i))
                .map(|(slug, _)| slug.clone())
                .unwrap_or_else(|| "choose a profile…".to_string());
            let count = self.profiles.len();
            let mut idx = self.profile_idx.unwrap_or(usize::MAX);
            ComboBox::from_id_salt("profile_picker")
                .selected_text(selected_text)
                .width(160.0)
                .show_index(ui, &mut idx, count, |i| {
                    // `i` can be usize::MAX when nothing is selected yet,
                    // so look it up safely instead of indexing directly.
                    self.profiles
                        .get(i)
                        .map(|(slug, _)| slug.clone())
                        .unwrap_or_default()
                });
            if idx != usize::MAX {
                self.profile_idx = Some(idx);
            }
            ui.horizontal(|ui| {
                if ui.button("Load selected").clicked() {
                    self.load_profile();
                }
                match &self.pending_delete {
                    None => {
                        if ui.button("Delete selected").clicked() {
                            if let Some(i) = self.profile_idx
                                && let Some((slug, _)) = self.profiles.get(i)
                            {
                                let slug = slug.clone();
                                self.pending_delete = Some(slug);
                            }
                        }
                    }
                    Some(slug) => {
                        ui.label(format!("Really delete '{slug}'?"));
                        if ui.button("Yes, delete").clicked() {
                            self.delete_profile();
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_delete = None;
                        }
                    }
                }
            });

            if self.profiles.is_empty() {
                ui.label("(no profiles saved yet)");
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let color = if self.status_is_error {
                egui::Color32::from_rgb(240, 130, 130)
            } else {
                egui::Color32::from_rgb(140, 220, 160)
            };
            ui.colored_label(color, &self.status);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                ui.columns(2, |cols| {
                    // One closure owns the whole `self` borrow, so we can
                    // draw the left column first and the right column second.
                    self.draw_settings(&mut cols[0]);
                    self.draw_output(&mut cols[1]);
                });
            });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([860.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "glaunch command builder",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
