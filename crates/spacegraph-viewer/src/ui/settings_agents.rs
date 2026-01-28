use bevy_egui::egui;

use crate::graph::state::NetCommand;
use crate::graph::GraphState;
use crate::ui::UiLayout;
use crate::util::agent_command::build_agent_command;
use crate::util::config::{AgentEndpoint, AgentEndpointKind, AgentMode, PathPolicyConfig};

pub fn agent_editor_window(ctx: &egui::Context, st: &mut GraphState, layout: &UiLayout) {
    if !st.ui.show_agent_editor {
        return;
    }

    let mut content_rect = layout.content_rect;
    if content_rect == egui::Rect::NOTHING {
        content_rect = ctx.screen_rect();
    }

    let default_size = egui::vec2(
        content_rect.width().clamp(360.0, 520.0),
        content_rect.height().clamp(220.0, 320.0),
    );
    let mut default_pos = content_rect.center() - default_size / 2.0;
    default_pos.x = default_pos
        .x
        .clamp(content_rect.min.x, content_rect.max.x - default_size.x);
    default_pos.y = default_pos
        .y
        .clamp(content_rect.min.y, content_rect.max.y - default_size.y);

    let mut open = st.ui.show_agent_editor;
    let mut close_requested = false;
    egui::Window::new("Add Agent")
        .collapsible(false)
        .resizable(true)
        .default_size(default_size)
        .default_pos(default_pos)
        .constrain_to(content_rect)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Register a new agent endpoint (UDS only for now).");
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(
                    &mut st.ui.agent_editor.name_input,
                ));
            });
            ui.horizontal(|ui| {
                ui.label("UDS path");
                ui.add(
                    egui::TextEdit::singleline(&mut st.ui.agent_editor.uds_input)
                        .desired_width(260.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Mode override");
                agent_mode_override_combo(ui, &mut st.ui.agent_editor.mode_override);
            });
            ui.checkbox(
                &mut st.ui.agent_editor.auto_connect,
                "Auto-connect on startup",
            );

            if let Some(msg) = st.ui.agent_editor.notice.as_ref() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(msg).color(egui::Color32::LIGHT_RED));
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close_requested = true;
                }
                if ui.button("Add").clicked() {
                    let name = st.ui.agent_editor.name_input.trim();
                    let uds = st.ui.agent_editor.uds_input.trim();
                    if name.is_empty() {
                        st.ui.agent_editor.notice =
                            Some("Please enter a name for this agent.".to_string());
                    } else if uds.is_empty() {
                        st.ui.agent_editor.notice = Some("Please enter a UDS path.".to_string());
                    } else if st.net.endpoints.iter().any(|e| e.name == name) {
                        st.ui.agent_editor.notice =
                            Some("An agent with this name already exists.".to_string());
                    } else {
                        let endpoint = AgentEndpoint {
                            name: name.to_string(),
                            kind: AgentEndpointKind::UdsPath(uds.to_string()),
                            auto_connect: st.ui.agent_editor.auto_connect,
                            mode_override: st.ui.agent_editor.mode_override,
                        };
                        st.net.endpoints.push(endpoint);
                        st.net.ensure_stream(name);
                        if st.ui.agent_editor.auto_connect {
                            st.net.commands.push(NetCommand::Connect(name.to_string()));
                        }
                        st.ui.agent_editor.notice = None;
                        close_requested = true;
                    }
                }
            });
        });

    if close_requested {
        open = false;
    }
    st.ui.show_agent_editor = open;
}

pub fn agent_command_window(ctx: &egui::Context, st: &mut GraphState, layout: &UiLayout) {
    if !st.ui.agent_command.open {
        return;
    }

    let Some(target) = st.ui.agent_command.target.clone() else {
        st.ui.agent_command.open = false;
        return;
    };
    let Some(endpoint) = st.net.endpoints.iter().find(|e| e.name == target) else {
        st.ui.agent_command.open = false;
        return;
    };

    let mut content_rect = layout.content_rect;
    if content_rect == egui::Rect::NOTHING {
        content_rect = ctx.screen_rect();
    }

    let default_size = egui::vec2(
        content_rect.width().clamp(420.0, 720.0),
        content_rect.height().clamp(180.0, 260.0),
    );
    let mut default_pos = content_rect.center() - default_size / 2.0;
    default_pos.x = default_pos
        .x
        .clamp(content_rect.min.x, content_rect.max.x - default_size.x);
    default_pos.y = default_pos
        .y
        .clamp(content_rect.min.y, content_rect.max.y - default_size.y);

    let mut open = st.ui.agent_command.open;
    let policy = PathPolicyConfig {
        includes: st.cfg.path_includes.clone(),
        excludes: st.cfg.path_excludes.clone(),
    };
    let mode = endpoint.mode_override.unwrap_or(st.cfg.agent_default_mode);
    let command = build_agent_command(endpoint, &policy, mode);

    let mut close_requested = false;
    egui::Window::new(format!("Agent Command: {}", endpoint.name))
        .collapsible(false)
        .resizable(true)
        .default_size(default_size)
        .default_pos(default_pos)
        .constrain_to(content_rect)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Run in terminal on the target host.");
            ui.add_space(6.0);

            ui.label(egui::RichText::new("Command").strong());
            ui.add(egui::Label::new(egui::RichText::new(&command).monospace()).selectable(true));

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Copy").clicked() {
                    ui.output_mut(|o| o.copied_text = command.clone());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        open = false;
    }
    st.ui.agent_command.open = open;
}

fn agent_mode_override_combo(ui: &mut egui::Ui, value: &mut Option<AgentMode>) {
    let label = match value {
        Some(mode) => mode.as_str(),
        None => "default",
    };
    egui::ComboBox::from_id_source("agent_mode_override")
        .selected_text(label)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "default");
            ui.selectable_value(value, Some(AgentMode::User), "user");
            ui.selectable_value(value, Some(AgentMode::Privileged), "privileged");
        });
}
