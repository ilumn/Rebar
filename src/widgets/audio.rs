use crate::{
    app::{Message, Rebar},
    system::{AudioCommand, SystemCommand},
    ui,
    widgets::{PanelSpec, WidgetKind, icons},
};
use iced::{
    Element, Fill,
    widget::{column, row, text},
};
use lucide_icons::Icon;

const WIDTH: f32 = 520.0;
const HEADER_HEIGHT: f32 = 64.0;
const HERO_HEIGHT: f32 = 160.0;
const PANEL_GAPS: f32 = 36.0;
const DEVICE_ROW_HEIGHT: f32 = 52.0;
const OUTPUT_LIST_MIN_HEIGHT: f32 = 72.0;
const OUTPUT_LIST_MAX_HEIGHT: f32 = DEVICE_ROW_HEIGHT * 4.0;

pub(crate) fn panel_spec(state: &Rebar) -> PanelSpec {
    let rows = state
        .system
        .audio
        .as_ref()
        .map(|audio| audio.devices.len())
        .unwrap_or(1);
    let output_section_height = output_section_height(rows);

    PanelSpec {
        min_size: iced::Size::new(420.0, 240.0),
        preferred_size: iced::Size::new(
            WIDTH,
            (HEADER_HEIGHT + HERO_HEIGHT + output_section_height + PANEL_GAPS)
                .min(state.max_flyout_height()),
        ),
    }
}

pub(crate) fn chip(state: &Rebar) -> Element<'_, Message> {
    let (icon, label, detail) = match &state.system.audio {
        Some(audio) if audio.muted => (Icon::VolumeX, String::from("Muted"), String::from("0%")),
        Some(audio) => (
            if audio.volume < 0.33 {
                Icon::Volume1
            } else {
                Icon::Volume2
            },
            format!("{:.0}%", audio.volume * 100.0),
            String::from("Output"),
        ),
        None => (
            Icon::VolumeOff,
            String::from("--"),
            String::from("Unavailable"),
        ),
    };

    ui::chip_button(
        row![
            icons::themed(icon, 15, state.palette),
            text(label).size(12).color(state.palette.text_color()),
            text(detail)
                .size(12)
                .color(state.palette.muted_text_color()),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .into(),
        state.palette,
        state.active_widget == Some(WidgetKind::Audio) && state.flyout_target_open,
        Message::WidgetSelected(WidgetKind::Audio),
    )
}

pub(crate) fn panel(state: &Rebar) -> Element<'_, Message> {
    let audio = state.system.audio.as_ref();
    let volume = audio.map(|value| value.volume).unwrap_or(0.0);
    let is_muted = audio.map(|value| value.muted).unwrap_or(false);
    let device_name = audio
        .map(|value| ui::truncate(&value.device_name, 42))
        .unwrap_or_else(|| String::from("No default output device"));

    let hero = ui::section_card(
        column![
            ui::eyebrow("Output", state.palette),
            row![
                icons::themed(
                    if is_muted {
                        Icon::VolumeX
                    } else {
                        Icon::Volume2
                    },
                    18,
                    state.palette,
                ),
                column![
                    text("Default device")
                        .size(12)
                        .color(state.palette.muted_text_color()),
                    text(device_name).size(16).color(state.palette.text_color()),
                ]
                .spacing(2)
                .width(Fill),
                text(format!("{:.0}", volume * 100.0))
                    .size(26)
                    .color(state.palette.text_color()),
            ]
            .spacing(12)
            .align_y(iced::alignment::Vertical::Center),
            ui::progress_meter(volume, state.palette),
            ui::value_slider(
                0.0..=100.0,
                volume * 100.0,
                |next| {
                    Message::SystemCommand(SystemCommand::Audio(AudioCommand::SetVolume(
                        (next / 100.0).clamp(0.0, 1.0),
                    )))
                },
                state.palette,
            ),
            row![
                ui::action_chip(
                    "Mute",
                    state.palette,
                    is_muted,
                    audio.map(|value| {
                        Message::SystemCommand(SystemCommand::Audio(AudioCommand::SetMuted(
                            !value.muted,
                        )))
                    }),
                ),
                ui::action_chip(
                    "-5",
                    state.palette,
                    false,
                    audio.map(|value| {
                        Message::SystemCommand(SystemCommand::Audio(AudioCommand::SetVolume(
                            (value.volume - 0.05).max(0.0),
                        )))
                    }),
                ),
                ui::action_chip(
                    "+5",
                    state.palette,
                    false,
                    audio.map(|value| {
                        Message::SystemCommand(SystemCommand::Audio(AudioCommand::SetVolume(
                            (value.volume + 0.05).min(1.0),
                        )))
                    }),
                ),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into(),
        state.palette,
    );

    let mut outputs = column![].spacing(10);
    if let Some(audio) = audio {
        for device in &audio.devices {
            let subtitle = if device.is_default {
                String::from("Current default output")
            } else {
                String::from("Click to route default audio here")
            };

            outputs = outputs.push(ui::selection_row(
                ui::truncate(&device.name, 44),
                subtitle,
                Some(if device.is_default {
                    String::from("Default")
                } else {
                    String::from("Switch")
                }),
                state.palette,
                device.is_default,
                if device.is_default {
                    None
                } else {
                    Some(Message::SystemCommand(SystemCommand::Audio(
                        AudioCommand::SetDefaultOutputDevice(device.id.clone()),
                    )))
                },
            ));
        }
    } else {
        outputs = outputs.push(ui::inset_card(
            text("No active render devices reported.")
                .size(13)
                .color(state.palette.muted_text_color())
                .into(),
            state.palette,
        ));
    }

    column![
        ui::panel_header(
            "Audio",
            Some("Volume, mute, and output routing"),
            state.palette,
            Message::WidgetSelected(WidgetKind::Audio),
        ),
        hero,
        ui::section_card(
            column![
                row![
                    ui::eyebrow("Outputs", state.palette),
                    if audio.map(|value| value.devices.len()).unwrap_or(0) > 4 {
                        ui::scroll_hint("inside list", state.palette)
                    } else {
                        text("")
                            .size(11)
                            .color(state.palette.muted_text_color())
                            .into()
                    },
                ]
                .width(Fill)
                .spacing(8),
                ui::list_scroll(
                    outputs.into(),
                    state.palette,
                    output_list_height(audio.map(|value| value.devices.len()).unwrap_or(1)),
                ),
            ]
            .spacing(10)
            .into(),
            state.palette,
        ),
    ]
    .spacing(12)
    .into()
}

fn output_list_height(device_count: usize) -> f32 {
    (device_count.max(1) as f32 * DEVICE_ROW_HEIGHT)
        .clamp(OUTPUT_LIST_MIN_HEIGHT, OUTPUT_LIST_MAX_HEIGHT)
}

fn output_section_height(device_count: usize) -> f32 {
    58.0 + output_list_height(device_count)
}
