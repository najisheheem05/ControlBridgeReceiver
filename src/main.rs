/*
 * Copyright (C) 2026 Ishan
 *
 * This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 3 only.
 *
 * This program is distributed without any warranty. See the GNU General Public License for more details.
 */

mod data;
mod input;
mod session;
mod utils;
mod viewmodel;

use slint::ComponentHandle;
use viewmodel::ReceiverViewModel;

slint::slint! {
    import { VerticalBox, HorizontalBox } from "std-widgets.slint";

    export struct PlayerData {
        connected: bool,
        player-label: string,
        btn-state: string,
        l-stick: string,
        r-stick: string,
        triggers: string,
    }

    component DataChip inherits Rectangle {
        in property <string> label;
        in property <string> value;

        background: #1f1f1f;
        border-radius: 6px;
        border-width: 1px;
        border-color: #333333;

        HorizontalBox {
            padding-left: 10px;
            padding-right: 10px;
            padding-top: 6px;
            padding-bottom: 6px;
            spacing: 6px;

            Text {
                text: root.label;
                font-size: 11px;
                color: #9ca3af;
                vertical-alignment: center;
            }
            Text {
                text: root.value;
                font-size: 12px;
                color: #e5e7eb;
                font-weight: 600;
                vertical-alignment: center;
            }
        }
    }

    component PlayerCard inherits Rectangle {
        in property <PlayerData> data;

        background: #2a2a2a;
        border-radius: 10px;
        border-width: 1px;
        border-color: #3d3d3d;

        VerticalBox {
            padding: 14px;
            spacing: 10px;

            HorizontalBox {
                alignment: space-between;

                Text {
                    text: data.player-label;
                    font-size: 15px;
                    color: #ffffff;
                    font-weight: 600;
                    vertical-alignment: center;
                }

                Rectangle {
                    background: #22c55e14;
                    border-radius: 12px;
                    border-width: 1px;
                    border-color: #22c55e;

                    HorizontalBox {
                        padding-left: 12px;
                        padding-right: 12px;
                        padding-top: 4px;
                        padding-bottom: 4px;
                        Text {
                            text: "Connected";
                            color: #22c55e;
                            font-size: 11px;
                            font-weight: 700;
                            vertical-alignment: center;
                        }
                    }
                }
            }

            HorizontalBox {
                spacing: 8px;
                padding: 0px;
                DataChip { label: "Btns"; value: data.btn-state; }
                DataChip { label: "Trig"; value: data.triggers; }
                DataChip { label: "LS"; value: data.l-stick; }
                DataChip { label: "RS"; value: data.r-stick; }
            }
        }
    }

    export component ReceiverScreen inherits Window {
        title: "ControlBridgeReceiver";
        icon: @image-url("icons/icon.ico");
        preferred-width: 500px;
        preferred-height: 480px;
        background: #1a1a1a;

        in property <PlayerData> player-0;
        in property <PlayerData> player-1;
        in property <PlayerData> player-2;
        in property <PlayerData> player-3;

        in property <string> header-status: "Searching...";
        in property <color> header-status-color: #3b82f6;

        property <int> connected-count:
            (player-0.connected ? 1 : 0) +
            (player-1.connected ? 1 : 0) +
            (player-2.connected ? 1 : 0) +
            (player-3.connected ? 1 : 0);

        VerticalLayout {
            alignment: start;
            padding: 24px;
            spacing: 14px;

            // ── Header bar ──
            Rectangle {
                background: #2a2a2a;
                border-radius: 12px;
                border-width: 1px;
                border-color: #3d3d3d;
                height: 56px;

                HorizontalBox {
                    padding: 16px;
                    alignment: space-between;

                    Text {
                        text: "ControlBridge";
                        font-size: 18px;
                        color: #ffffff;
                        font-weight: 600;
                        vertical-alignment: center;
                    }

                    Rectangle {
                        background: root.header-status-color.transparentize(85%);
                        border-radius: 16px;
                        border-width: 1px;
                        border-color: root.header-status-color;

                        HorizontalBox {
                            padding-left: 16px;
                            padding-right: 16px;
                            Text {
                                text: root.header-status;
                                color: root.header-status-color;
                                font-size: 13px;
                                font-weight: 700;
                                vertical-alignment: center;
                            }
                        }
                    }
                }
            }

            // ── Empty-state placeholder ──
            if connected-count == 0 : Rectangle {
                height: 120px;
                background: transparent;

                VerticalBox {
                    alignment: center;
                    Text {
                        text: "Waiting for controllers...";
                        font-size: 15px;
                        color: #6b7280;
                        horizontal-alignment: center;
                    }
                    Text {
                        text: "Open ControlBridge on your phone and connect";
                        font-size: 12px;
                        color: #4b5563;
                        horizontal-alignment: center;
                    }
                }
            }

            // ── Player cards (only rendered when connected) ──
            if player-0.connected : PlayerCard { data: player-0; }
            if player-1.connected : PlayerCard { data: player-1; }
            if player-2.connected : PlayerCard { data: player-2; }
            if player-3.connected : PlayerCard { data: player-3; }
        }
    }
}

fn main() -> Result<(), slint::PlatformError> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Trace)
        .init();

    let ui = ReceiverScreen::new()?;
    let ui_handle = ui.as_weak();

    let view_model = ReceiverViewModel::new({
        let ui_handle = ui_handle.clone();

        move |slot: usize, state, is_connected| {
            let ui_handle = ui_handle.clone();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle.upgrade() {
                    let player = if is_connected {
                        PlayerData {
                            connected: true,
                            player_label: format!("Player {}", slot + 1).into(),
                            btn_state: format!("{}", state.buttons).into(),
                            l_stick: format!("{} / {}", state.lx, state.ly).into(),
                            r_stick: format!("{} / {}", state.rx, state.ry).into(),
                            triggers: format!("{} / {}", state.lt, state.rt).into(),
                        }
                    } else {
                        PlayerData::default()
                    };

                    // Set the correct player property by slot index
                    match slot {
                        0 => ui.set_player_0(player),
                        1 => ui.set_player_1(player),
                        2 => ui.set_player_2(player),
                        3 => ui.set_player_3(player),
                        _ => {}
                    }

                    // Update header status
                    let count = [
                        ui.get_player_0().connected,
                        ui.get_player_1().connected,
                        ui.get_player_2().connected,
                        ui.get_player_3().connected,
                    ]
                    .iter()
                    .filter(|&&c| c)
                    .count();

                    if count > 0 {
                        ui.set_header_status(format!("{}/4 Connected", count).into());
                        ui.set_header_status_color(slint::Color::from_rgb_u8(34, 197, 94));
                    } else {
                        ui.set_header_status("Searching...".into());
                        ui.set_header_status_color(slint::Color::from_rgb_u8(59, 130, 246));
                    }
                }
            });
        }
    });

    ui.run()?;
    view_model.shutdown();
    Ok(())
}