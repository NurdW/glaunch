use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::App;
use super::View;
use crate::config::{self, MangoHudConfig, Profile, ProfileSettings, VkBasaltConfig};

#[derive(Debug, Clone)]
pub struct EditState {
    pub is_new: bool,
    pub fields: Vec<Field>,
    pub selected: usize,
    pub editing_text: bool,
}

#[derive(Debug, Clone)]
pub enum Field {
    Text { label: String, value: String },
    Toggle { label: String, value: Option<bool> },
}

impl EditState {
    pub fn new_profile() -> Self {
        Self {
            is_new: true,
            fields: vec![
                Field::Text {
                    label: "Slug".into(),
                    value: String::new(),
                },
                Field::Text {
                    label: "Name".into(),
                    value: String::new(),
                },
                Field::Text {
                    label: "Steam App ID".into(),
                    value: String::new(),
                },
                Field::Text {
                    label: "Width".into(),
                    value: "3840".into(),
                },
                Field::Text {
                    label: "Height".into(),
                    value: "2160".into(),
                },
                Field::Text {
                    label: "Refresh Rate".into(),
                    value: String::new(),
                },
                Field::Toggle {
                    label: "HDR".into(),
                    value: None,
                },
                Field::Toggle {
                    label: "ITM".into(),
                    value: None,
                },
                Field::Toggle {
                    label: "VRR".into(),
                    value: None,
                },
                Field::Toggle {
                    label: "FSR4".into(),
                    value: None,
                },
                Field::Toggle {
                    label: "Gamescope".into(),
                    value: None,
                },
                Field::Text {
                    label: "Output".into(),
                    value: String::new(),
                },
                Field::Toggle {
                    label: "V-Cache".into(),
                    value: None,
                },
                Field::Toggle {
                    label: "Fix Mouse".into(),
                    value: None,
                },
                Field::Toggle {
                    label: "MangoHud".into(),
                    value: None,
                },
                Field::Toggle {
                    label: "vkBasalt".into(),
                    value: None,
                },
                Field::Text {
                    label: "vkBasalt Profile".into(),
                    value: String::new(),
                },
            ],
            selected: 0,
            editing_text: false,
        }
    }

    pub fn from_profile(slug: String, profile: Profile, is_new: bool) -> Self {
        let s = profile.settings.as_ref();
        let m = profile.mangohud.as_ref();
        let v = profile.vkbasalt.as_ref();

        Self {
            is_new,
            fields: vec![
                Field::Text {
                    label: "Slug".into(),
                    value: slug,
                },
                Field::Text {
                    label: "Name".into(),
                    value: profile.name.unwrap_or_default(),
                },
                Field::Text {
                    label: "Steam App ID".into(),
                    value: profile
                        .steam_app_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                },
                Field::Text {
                    label: "Width".into(),
                    value: s
                        .and_then(|s| s.width)
                        .map(|w| w.to_string())
                        .unwrap_or_default(),
                },
                Field::Text {
                    label: "Height".into(),
                    value: s
                        .and_then(|s| s.height)
                        .map(|h| h.to_string())
                        .unwrap_or_default(),
                },
                Field::Text {
                    label: "Refresh Rate".into(),
                    value: s
                        .and_then(|s| s.refresh_rate)
                        .map(|r| r.to_string())
                        .unwrap_or_default(),
                },
                Field::Toggle {
                    label: "HDR".into(),
                    value: s.and_then(|s| s.hdr),
                },
                Field::Toggle {
                    label: "ITM".into(),
                    value: s.and_then(|s| s.itm),
                },
                Field::Toggle {
                    label: "VRR".into(),
                    value: s.and_then(|s| s.vrr),
                },
                Field::Toggle {
                    label: "FSR4".into(),
                    value: s.and_then(|s| s.fsr4),
                },
                Field::Toggle {
                    label: "Gamescope".into(),
                    value: s.and_then(|s| s.gamescope),
                },
                Field::Text {
                    label: "Output".into(),
                    value: s.and_then(|s| s.output.clone()).unwrap_or_default(),
                },
                Field::Toggle {
                    label: "V-Cache".into(),
                    value: s.and_then(|s| s.vcache),
                },
                Field::Toggle {
                    label: "Fix Mouse".into(),
                    value: s.and_then(|s| s.fix_mouse),
                },
                Field::Toggle {
                    label: "MangoHud".into(),
                    value: m.and_then(|m| m.enabled),
                },
                Field::Toggle {
                    label: "vkBasalt".into(),
                    value: v.and_then(|v| v.enabled),
                },
                Field::Text {
                    label: "vkBasalt Profile".into(),
                    value: v.and_then(|v| v.profile.clone()).unwrap_or_default(),
                },
            ],
            selected: 0,
            editing_text: false,
        }
    }

    fn to_profile(&self) -> (String, Profile) {
        let field_val = |label: &str| -> String {
            self.fields
                .iter()
                .find_map(|f| match f {
                    Field::Text { label: l, value } if l == label => Some(value.clone()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        let field_opt_bool = |label: &str| -> Option<bool> {
            self.fields
                .iter()
                .find_map(|f| match f {
                    Field::Toggle { label: l, value } if l == label => Some(*value),
                    _ => None,
                })
                .unwrap_or(None)
        };

        let slug = field_val("Slug");
        let name = field_val("Name");
        let app_id_str = field_val("Steam App ID");
        let width_str = field_val("Width");
        let height_str = field_val("Height");
        let refresh_rate_str = field_val("Refresh Rate");
        let vkbasalt_profile = field_val("vkBasalt Profile");

        let output_str = field_val("Output");
        let output = if output_str.is_empty() {
            None
        } else {
            Some(output_str)
        };

        let settings = ProfileSettings {
            width: width_str.parse().ok(),
            height: height_str.parse().ok(),
            refresh_rate: refresh_rate_str.parse().ok(),
            hdr: field_opt_bool("HDR"),
            vrr: field_opt_bool("VRR"),
            gamescope: field_opt_bool("Gamescope"),
            output,
            itm: field_opt_bool("ITM"),
            fsr4: field_opt_bool("FSR4"),
            vcache: field_opt_bool("V-Cache"),
            fix_mouse: field_opt_bool("Fix Mouse"),
        };
        let has_settings = settings.width.is_some()
            || settings.height.is_some()
            || settings.refresh_rate.is_some()
            || settings.hdr.is_some()
            || settings.vrr.is_some()
            || settings.gamescope.is_some()
            || settings.output.is_some()
            || settings.itm.is_some()
            || settings.fsr4.is_some()
            || settings.vcache.is_some()
            || settings.fix_mouse.is_some();

        let mangohud_enabled = field_opt_bool("MangoHud");
        let vkbasalt_enabled = field_opt_bool("vkBasalt");
        let vkbasalt_prof = if vkbasalt_profile.is_empty() {
            None
        } else {
            Some(vkbasalt_profile)
        };

        let profile = Profile {
            name: if name.is_empty() { None } else { Some(name) },
            steam_app_id: app_id_str.parse().ok(),
            settings: if has_settings { Some(settings) } else { None },
            mangohud: mangohud_enabled.map(|enabled| MangoHudConfig {
                enabled: Some(enabled),
                config: None,
            }),
            vkbasalt: if vkbasalt_enabled.is_some() || vkbasalt_prof.is_some() {
                Some(VkBasaltConfig {
                    enabled: vkbasalt_enabled,
                    profile: vkbasalt_prof,
                })
            } else {
                None
            },
        };

        (slug, profile)
    }
}

pub fn render(frame: &mut Frame, state: &EditState) {
    let title = if state.is_new {
        " New Profile "
    } else {
        " Edit Profile "
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    let field_count = state.fields.len();
    let constraints: Vec<Constraint> = (0..field_count + 1)
        .map(|_| Constraint::Length(1))
        .collect();

    let chunks = Layout::vertical(constraints).split(inner);

    for (i, field) in state.fields.iter().enumerate() {
        let is_selected = i == state.selected;
        let style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let text = match field {
            Field::Text { label, value } => {
                let cursor = if is_selected && state.editing_text {
                    "|"
                } else {
                    ""
                };
                format!("  {label:<18} {value}{cursor}")
            }
            Field::Toggle { label, value } => {
                let indicator = match value {
                    Some(true) => "[x]",
                    Some(false) => "[ ]",
                    None => "[-]",
                };
                format!("  {label:<18} {indicator}")
            }
        };

        if i < chunks.len() {
            frame.render_widget(Paragraph::new(text).style(style), chunks[i]);
        }
    }

    // Help text at the bottom
    let help_idx = field_count;
    if help_idx < chunks.len() {
        let help = if state.editing_text {
            "  Enter: confirm | Esc: cancel"
        } else {
            "  j/k: navigate | Enter/Space: edit/toggle ([x]/[ ]/[-]=inherit) | s: save | Esc: cancel"
        };
        frame.render_widget(
            Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
            chunks[help_idx],
        );
    }
}

pub fn handle_input(app: &mut App, key: KeyEvent) {
    let edit_state = match &mut app.edit_state {
        Some(s) => s,
        None => return,
    };

    if edit_state.editing_text {
        match key.code {
            KeyCode::Esc => {
                edit_state.editing_text = false;
            }
            KeyCode::Enter => {
                edit_state.editing_text = false;
            }
            KeyCode::Backspace => {
                if let Field::Text { value, .. } = &mut edit_state.fields[edit_state.selected] {
                    value.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Field::Text { value, .. } = &mut edit_state.fields[edit_state.selected] {
                    value.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.edit_state = None;
            app.view = View::List;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let count = edit_state.fields.len();
            edit_state.selected = (edit_state.selected + 1) % count;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            let count = edit_state.fields.len();
            edit_state.selected = if edit_state.selected == 0 {
                count - 1
            } else {
                edit_state.selected - 1
            };
        }
        KeyCode::Enter | KeyCode::Char(' ') => match &mut edit_state.fields[edit_state.selected] {
            Field::Toggle { value, .. } => {
                *value = match *value {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
            }
            Field::Text { .. } => edit_state.editing_text = true,
        },
        KeyCode::Char('s') => {
            let (slug, profile) = edit_state.to_profile();
            if slug.is_empty() {
                app.status_message = Some("Slug cannot be empty".to_string());
                return;
            }
            match config::save_profile(&slug, &profile) {
                Ok(()) => {
                    app.status_message = Some(format!("Saved profile '{slug}'"));
                    let _ = app.reload_profiles();
                    app.edit_state = None;
                    app.view = View::List;
                }
                Err(e) => {
                    app.status_message = Some(format!("Save failed: {e}"));
                }
            }
        }
        _ => {}
    }
}
