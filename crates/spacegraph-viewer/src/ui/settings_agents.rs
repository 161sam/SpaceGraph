use bevy_egui::egui;
use std::time::Instant;

use crate::graph::state::{NetCommand, NetStreamStatus};
use crate::graph::GraphState;
use crate::ui::UiLayout;
use crate::util::agent_command::build_agent_command;
use crate::util::config::{AgentEndpoint, AgentEndpointKind, AgentMode, PathPolicyConfig};

pub fn agent_manager_window(ctx: &egui::Context, st: &mut GraphState, layout: &UiLayout) {
    if !st.ui.show_agent_manager {
        return;
    }

    let mut content_rect = layout.content_rect;
    if content_rect == egui::Rect::NOTHING {
        content_rect = ctx.screen_rect();
    }

    let default_size = egui::vec2(
        content_rect.width().clamp(520.0, 900.0),
        content_rect.height().clamp(320.0, 620.0),
    );
    let mut default_pos = content_rect.center() - default_size / 2.0;
    default_pos.x = default_pos
        .x
        .clamp(content_rect.min.x, content_rect.max.x - default_size.x);
    default_pos.y = default_pos
        .y
        .clamp(content_rect.min.y, content_rect.max.y - default_size.y);

    let mut open = st.ui.show_agent_manager;
    let mut remove_index: Option<usize> = None;
    egui::Window::new("Manage Agents")
        .collapsible(false)
        .resizable(true)
        .default_size(default_size)
        .default_pos(default_pos)
        .constrain_to(content_rect)
        .open(&mut open)
        .show(ctx, |ui| {
            let now = Instant::now();
            let mut rows: Vec<usize> = (0..st.net.endpoints.len()).collect();
            rows.sort_by(|a, b| st.net.endpoints[*a].name.cmp(&st.net.endpoints[*b].name));

            egui::Grid::new("agents_table")
                .striped(true)
                .spacing(egui::vec2(8.0, 4.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Name").strong());
                    ui.label(egui::RichText::new("Status").strong());
                    ui.label(egui::RichText::new("Msgs/s").strong());
                    ui.label(egui::RichText::new("Last seen").strong());
                    ui.label(egui::RichText::new("Mode").strong());
                    ui.label(egui::RichText::new("Actions").strong());
                    ui.end_row();

                    for idx in rows {
                        let (endpoint_name, endpoint_mode_override) = {
                            let endpoint = &st.net.endpoints[idx];
                            (endpoint.name.clone(), endpoint.mode_override)
                        };
                        let stream = st.net.streams.get(&endpoint_name);
                        let status = stream
                            .map(|s| s.status)
                            .unwrap_or(NetStreamStatus::Disconnected);
                        let msg_rate = stream.map(|s| s.msg_rate).unwrap_or(0.0);
                        let last_seen = stream
                            .and_then(|s| s.last_seen)
                            .map(|ts| now.duration_since(ts));
                        let last_error = stream.and_then(|s| s.last_error.as_ref());

                        ui.label(&endpoint_name);
                        let status_text = match status {
                            NetStreamStatus::Disconnected => "disconnected",
                            NetStreamStatus::Connecting => "connecting",
                            NetStreamStatus::Connected => "connected",
                        };
                        let mut status_label = egui::RichText::new(status_text);
                        if last_error.is_some() {
                            status_label = status_label.color(egui::Color32::LIGHT_RED);
                        }
                        ui.vertical(|ui| {
                            let status_resp = ui.label(status_label);
                            if let Some(err) = last_error {
                                status_resp.on_hover_text(err);
                                ui.label(
                                    egui::RichText::new(err)
                                        .small()
                                        .color(egui::Color32::LIGHT_RED),
                                );
                            }
                        });
                        ui.label(format!("{msg_rate:.1}"));
                        let last_seen_label = match last_seen {
                            Some(delta) => format!("{:.1}s", delta.as_secs_f32()),
                            None => "—".to_string(),
                        };
                        ui.label(last_seen_label);

                        let mut mode_override = endpoint_mode_override;
                        let mode_label = match mode_override {
                            Some(mode) => mode.as_str(),
                            None => "default",
                        };
                        egui::ComboBox::from_id_source(format!(
                            "agent_mode_override_{endpoint_name}"
                        ))
                        .selected_text(mode_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut mode_override, None, "default");
                            ui.selectable_value(&mut mode_override, Some(AgentMode::User), "user");
                            ui.selectable_value(
                                &mut mode_override,
                                Some(AgentMode::Privileged),
                                "privileged",
                            );
                        });
                        if mode_override != endpoint_mode_override {
                            st.net.endpoints[idx].mode_override = mode_override;
                        }

                        ui.horizontal(|ui| {
                            let has_connection = st.net.connections.contains_key(&endpoint_name);
                            if ui
                                .add_enabled(!has_connection, egui::Button::new("Connect"))
                                .clicked()
                            {
                                st.net
                                    .commands
                                    .push(NetCommand::Connect(endpoint_name.clone()));
                            }
                            if ui
                                .add_enabled(has_connection, egui::Button::new("Disconnect"))
                                .clicked()
                            {
                                st.net
                                    .commands
                                    .push(NetCommand::Disconnect(endpoint_name.clone()));
                            }
                            if ui
                                .add_enabled(has_connection, egui::Button::new("Reconnect"))
                                .clicked()
                            {
                                st.net
                                    .commands
                                    .push(NetCommand::Reconnect(endpoint_name.clone()));
                            }
                            if ui.button("Remove").clicked() {
                                remove_index = Some(idx);
                            }
                            if ui.button("Command…").clicked() {
                                st.ui.agent_command.target = Some(endpoint_name.clone());
                                st.ui.agent_command.open = true;
                            }
                        });
                        ui.end_row();
                    }
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Default mode");
                egui::ComboBox::from_id_source("agent_default_mode")
                    .selected_text(st.cfg.agent_default_mode.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut st.cfg.agent_default_mode,
                            AgentMode::User,
                            "user",
                        );
                        ui.selectable_value(
                            &mut st.cfg.agent_default_mode,
                            AgentMode::Privileged,
                            "privileged",
                        );
                    });
            });

            if ui.button("Add Agent…").clicked() {
                let default_endpoint = AgentEndpoint::default();
                st.ui.agent_editor.name_input.clear();
                st.ui.agent_editor.uds_input = match default_endpoint.kind {
                    AgentEndpointKind::UdsPath(path) => path,
                };
                st.ui.agent_editor.auto_connect = default_endpoint.auto_connect;
                st.ui.agent_editor.mode_override = None;
                st.ui.agent_editor.notice = None;
                st.ui.show_agent_editor = true;
            }
        });

    if let Some(idx) = remove_index {
        let endpoint = st.net.endpoints.remove(idx);
        if st.net.connections.contains_key(&endpoint.name) {
            st.net
                .commands
                .push(NetCommand::Disconnect(endpoint.name.clone()));
        }
        st.net.streams.remove(&endpoint.name);
    }
    st.ui.show_agent_manager = open;
}

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
