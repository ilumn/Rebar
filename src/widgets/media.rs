use crate::{
    app::{Message, Rebar},
    system::{MediaCommand, MediaPlaybackState, SystemCommand},
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
const PANEL_GAP: f32 = 12.0;
const NOW_PLAYING_BASE_HEIGHT: f32 = 168.0;
const NOW_PLAYING_WITH_TRANSPORT_HEIGHT: f32 = 206.0;
const SESSION_HEIGHT: f32 = 92.0;
const IDLE_HEIGHT: f32 = 168.0;

pub(crate) fn panel_spec(state: &Rebar) -> PanelSpec {
    let preferred_height = if let Some(media) = state.system.media.as_ref() {
        let transport_height = if media.can_toggle_play_pause || media.can_go_previous || media.can_go_next {
            NOW_PLAYING_WITH_TRANSPORT_HEIGHT
        } else {
            NOW_PLAYING_BASE_HEIGHT
        };
        let session_height = if media.source_app_id.is_empty() {
            0.0
        } else {
            PANEL_GAP + SESSION_HEIGHT
        };

        HEADER_HEIGHT + PANEL_GAP + transport_height + session_height
    } else {
        HEADER_HEIGHT + PANEL_GAP + IDLE_HEIGHT
    };

    PanelSpec {
        min_size: iced::Size::new(360.0, 180.0),
        preferred_size: iced::Size::new(WIDTH, preferred_height.min(state.max_flyout_height())),
    }
}

pub(crate) fn chip(state: &Rebar) -> Option<Element<'_, Message>> {
    let media = state.system.media.as_ref();

    if media.is_none() && state.active_widget != Some(WidgetKind::Media) {
        return None;
    }

    let media = media.cloned().unwrap_or_default();
    let title = if media.title.is_empty() {
        String::from("No media")
    } else {
        ui::truncate(&media.title, 20)
    };

    Some(ui::chip_button(
        row![
            icons::themed(icon_for_playback(media.playback), 15, state.palette),
            text(title).size(12).color(state.palette.text_color()),
            text(media.playback.as_label())
                .size(12)
                .color(state.palette.muted_text_color()),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .into(),
        state.palette,
        state.active_widget == Some(WidgetKind::Media) && state.flyout_target_open,
        Message::WidgetSelected(WidgetKind::Media),
    ))
}

pub(crate) fn panel(state: &Rebar) -> Element<'_, Message> {
    let media = state.system.media.as_ref();
    let live_timing = state.current_media_timing();
    let progress = live_timing
        .map(|(position, duration)| {
            if duration <= f64::EPSILON {
                0.0
            } else {
                (position / duration).clamp(0.0, 1.0) as f32
            }
        })
        .unwrap_or(0.0);

    let body = if let Some(media) = media {
        let play_pause_label = if matches!(media.playback, MediaPlaybackState::Playing) {
            "Pause"
        } else {
            "Play"
        };
        let subtitle = if media.artist.is_empty() {
            ui::truncate(&media.source_app_id, 36)
        } else if media.album.is_empty() {
            format!(
                "{} • {}",
                ui::truncate(&media.artist, 22),
                ui::truncate(&media.source_app_id, 16)
            )
        } else {
            format!(
                "{} • {}",
                ui::truncate(&media.artist, 18),
                ui::truncate(&media.album, 16)
            )
        };

        {
            let mut content = column![
                ui::section_card(
                    column![
                        ui::eyebrow("Now playing", state.palette),
                        row![
                            icons::themed(icon_for_playback(media.playback), 18, state.palette),
                            column![
                                text(ui::truncate(&media.title, 34))
                                    .size(20)
                                    .color(state.palette.text_color()),
                                text(subtitle)
                                    .size(13)
                                    .color(state.palette.muted_text_color()),
                            ]
                            .spacing(3)
                            .width(Fill),
                        ]
                        .spacing(12),
                        ui::progress_meter(progress, state.palette),
                        row![
                            text(
                                live_timing
                                    .map(|(position, _)| format!("{position:.0}s"))
                                    .unwrap_or_else(|| String::from("--")),
                            )
                            .size(12)
                            .color(state.palette.muted_text_color()),
                            text(
                                live_timing
                                    .map(|(_, duration)| format!("{duration:.0}s"))
                                    .unwrap_or_else(|| String::from("--")),
                            )
                            .size(12)
                            .color(state.palette.muted_text_color()),
                        ]
                        .width(Fill)
                        .spacing(8),
                        transport_row(media, play_pause_label, state),
                    ]
                    .spacing(12)
                    .into(),
                    state.palette,
                ),
            ]
            .spacing(12);

            if !media.source_app_id.is_empty() {
                content = content.push(ui::section_card(
                    column![
                        ui::eyebrow("Session", state.palette),
                        ui::selection_row(
                            ui::truncate(&media.source_app_id, 34),
                            format!("State: {}", media.playback.as_label()),
                            None,
                            state.palette,
                            false,
                            None,
                        ),
                    ]
                    .spacing(10)
                    .into(),
                    state.palette,
                ));
            }

            content
        }
    } else {
        column![ui::section_card(
            column![
                ui::eyebrow("No active session", state.palette),
                row![
                    icons::themed(Icon::Music4, 18, state.palette),
                    column![
                        text("Nothing is currently exposing media transport controls.")
                            .size(15)
                            .color(state.palette.text_color()),
                        text("The widget will populate automatically when a supported player starts.")
                            .size(12)
                            .color(state.palette.muted_text_color()),
                    ]
                    .spacing(2),
                ]
                .spacing(12),
            ]
            .into(),
            state.palette,
        )]
        .spacing(12)
    };

    column![
        ui::panel_header(
            "Media",
            Some("Now playing with timeline and controls"),
            state.palette,
            Message::WidgetSelected(WidgetKind::Media),
        ),
        body,
    ]
    .spacing(12)
    .into()
}

fn icon_for_playback(playback: MediaPlaybackState) -> Icon {
    match playback {
        MediaPlaybackState::Playing => Icon::Play,
        MediaPlaybackState::Paused => Icon::Pause,
        MediaPlaybackState::Stopped => Icon::MonitorStop,
        _ => Icon::Music4,
    }
}

fn transport_row<'a>(
    media: &crate::system::MediaSnapshot,
    play_pause_label: &'static str,
    state: &'a Rebar,
) -> Element<'a, Message> {
    let mut row = row![].spacing(8);

    if media.can_go_previous {
        row = row.push(ui::action_chip(
            "Prev",
            state.palette,
            false,
            Some(Message::SystemCommand(SystemCommand::Media(
                MediaCommand::Previous,
            ))),
        ));
    }

    if media.can_toggle_play_pause {
        row = row.push(ui::action_chip(
            play_pause_label,
            state.palette,
            matches!(media.playback, MediaPlaybackState::Playing),
            Some(Message::SystemCommand(SystemCommand::Media(
                MediaCommand::PlayPause,
            ))),
        ));
    }

    if media.can_go_next {
        row = row.push(ui::action_chip(
            "Next",
            state.palette,
            false,
            Some(Message::SystemCommand(SystemCommand::Media(MediaCommand::Next))),
        ));
    }

    row.into()
}
