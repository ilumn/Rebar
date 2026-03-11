use crate::{
    app::{BAR_RADIUS, FLYOUT_RADIUS, Message, Rebar},
    palette::WallpaperPalette,
    widgets::{self, WidgetKind},
};
use iced::{
    Background, Color, Element, Fill, Length, Theme, alignment, border,
    widget::{Space, button, column, container, progress_bar, row, scrollable, slider, text},
};
use std::ops::RangeInclusive;

pub(crate) fn view_bar(state: &Rebar) -> Element<'_, Message> {
    let chips = widgets::chip_order(state)
        .into_iter()
        .fold(row![], |row, chip| row.push(chip));

    container(
        row![
            Space::new().width(12),
            button(
                column![
                    // text("TASKBAR")
                    //     .size(10)
                    //     .color(Color::from_rgba8(
                    //         state.palette.muted_text[0],
                    //         state.palette.muted_text[1],
                    //         state.palette.muted_text[2],
                    //         0.85,
                    //     )),
                    text(truncate(
                        if state.system.active_window_title.trim().is_empty() {
                            "Desktop"
                        } else {
                            &state.system.active_window_title
                        },
                        256,
                    ))
                    .size(13)
                    .color(state.palette.text_color())
                    .width(Fill),
                ]
                .spacing(0)
                .width(Fill),
            )
            .width(Fill)
            .padding(0)
            .style(ghost_button_style())
            .on_press(Message::WidgetSelected(WidgetKind::Device)),
            chips
                .spacing(8)
                .align_y(alignment::Vertical::Center)
                .width(Length::Shrink),
            Space::new().width(12),
        ]
        .spacing(10)
        .align_y(alignment::Vertical::Center)
        .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .padding([2, 0])
    .style(glass_style(state.palette, BAR_RADIUS as f32, false))
    .into()
}

pub(crate) fn view_flyout(state: &Rebar) -> Element<'_, Message> {
    container(
        container(widgets::active_panel(state))
            .width(Fill)
            .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .padding(18)
    .style(glass_style(state.palette, FLYOUT_RADIUS as f32, true))
    .into()
}

pub(crate) fn panel_header(
    title: &'static str,
    _subtitle: Option<&'static str>,
    palette: WallpaperPalette,
    close_message: Message,
) -> Element<'static, Message> {
    row![
        container(
            text(title).size(22).color(palette.text_color())
        )
        .width(Fill),
        container(panel_button("Close", palette, close_message)).align_right(Length::Shrink),
    ]
    .align_y(alignment::Vertical::Center)
    .spacing(12)
    .width(Fill)
    .into()
}

pub(crate) fn summary_tile(
    label: &'static str,
    value: impl Into<String>,
    detail: impl Into<String>,
    palette: WallpaperPalette,
) -> Element<'static, Message> {
    inset_card(
        column![
            eyebrow(label, palette),
            text(value.into()).size(24).color(palette.text_color()),
            text(detail.into())
                .size(12)
                .color(palette.muted_text_color()),
        ]
        .spacing(4)
        .into(),
        palette,
    )
}

pub(crate) fn selection_row(
    title: String,
    subtitle: String,
    trailing: Option<String>,
    palette: WallpaperPalette,
    active: bool,
    message: Option<Message>,
) -> Element<'static, Message> {
    let content = row![
        column![
            text(title).size(13).color(palette.text_color()),
            text(subtitle).size(12).color(palette.muted_text_color()),
        ]
        .spacing(2)
        .width(Fill),
        if let Some(trailing) = trailing {
            text(trailing).size(12).color(if active {
                Color::from_rgb8(palette.accent[0], palette.accent[1], palette.accent[2])
            } else {
                palette.muted_text_color()
            })
        } else {
            text("").size(12).color(palette.muted_text_color())
        },
    ]
    .spacing(12)
    .align_y(alignment::Vertical::Center);

    let button = button(content)
        .padding([10, 12])
        .style(selection_row_style(palette, active));

    match message {
        Some(message) => button.on_press(message).into(),
        None => button.into(),
    }
}

pub(crate) fn list_scroll<'a>(
    content: Element<'a, Message>,
    palette: WallpaperPalette,
    height: f32,
) -> Element<'a, Message> {
    scrollable(content)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(8)
                .scroller_width(8)
                .margin(2)
                .spacing(10),
        ))
        .height(height)
        .style(move |_theme, status| scrollable::Style {
            container: container::Style::default(),
            vertical_rail: scrollable_rail_style(palette, status, true),
            horizontal_rail: scrollable_rail_style(palette, status, false),
            gap: None,
            auto_scroll: scrollable::default(&_theme.clone(), status).auto_scroll,
        })
        .into()
}

pub(crate) fn scroll_hint(
    label: &'static str,
    palette: WallpaperPalette,
) -> Element<'static, Message> {
    row![
        text("Scroll").size(11).color(palette.muted_text_color()),
        text(label).size(11).color(Color::from_rgb8(
            palette.accent_soft[0],
            palette.accent_soft[1],
            palette.accent_soft[2],
        )),
    ]
    .spacing(6)
    .into()
}

pub(crate) fn section_card<'a>(
    content: Element<'a, Message>,
    palette: WallpaperPalette,
) -> Element<'a, Message> {
    container(content)
        .padding(14)
        .style(section_style(palette))
        .into()
}

pub(crate) fn inset_card<'a>(
    content: Element<'a, Message>,
    palette: WallpaperPalette,
) -> Element<'a, Message> {
    container(content)
        .padding([12, 14])
        .style(inset_style(palette))
        .into()
}

pub(crate) fn action_chip(
    label: &'static str,
    palette: WallpaperPalette,
    active: bool,
    message: Option<Message>,
) -> Element<'static, Message> {
    let button = button(text(label).size(12).color(if active {
        palette.text_color()
    } else {
        palette.muted_text_color()
    }))
    .padding([7, 12])
    .style(action_chip_style(palette, active));

    match message {
        Some(message) => button.on_press(message).into(),
        None => button.into(),
    }
}

pub(crate) fn panel_button(
    label: &'static str,
    palette: WallpaperPalette,
    message: Message,
) -> Element<'static, Message> {
    panel_button_optional(label, palette, Some(message))
}

pub(crate) fn panel_button_optional(
    label: &'static str,
    palette: WallpaperPalette,
    message: Option<Message>,
) -> Element<'static, Message> {
    let button = button(text(label).size(12).color(palette.text_color()))
        .padding([5, 12])
        .style(button_style(palette));

    match message {
        Some(message) => button.on_press(message).into(),
        None => button.into(),
    }
}

pub(crate) fn chip_button<'a>(
    content: Element<'a, Message>,
    palette: WallpaperPalette,
    active: bool,
    message: Message,
) -> Element<'a, Message> {
    button(content)
        .padding([5, 12])
        .style(chip_button_style(palette, active))
        .on_press(message)
        .into()
}

pub(crate) fn progress_meter(value: f32, palette: WallpaperPalette) -> Element<'static, Message> {
    progress_bar(0.0..=1.0, value.clamp(0.0, 1.0))
        .girth(6)
        .style(move |_theme| progress_bar::Style {
            background: Background::Color(Color::from_rgba8(255, 255, 255, 0.10)),
            bar: Background::Color(Color::from_rgb8(
                palette.accent[0],
                palette.accent[1],
                palette.accent[2],
            )),
            border: border::rounded(999.0),
        })
        .into()
}

pub(crate) fn value_slider<'a, F>(
    range: RangeInclusive<f32>,
    value: f32,
    on_change: F,
    palette: WallpaperPalette,
) -> Element<'a, Message>
where
    F: 'a + Fn(f32) -> Message,
{
    slider(range, value, on_change)
        .height(16)
        .style(move |_theme, status| {
            let accent = Color::from_rgb8(palette.accent[0], palette.accent[1], palette.accent[2]);
            let accent_hover = Color::from_rgb8(
                palette.accent_soft[0],
                palette.accent_soft[1],
                palette.accent_soft[2],
            );
            let handle = match status {
                slider::Status::Active => accent,
                slider::Status::Hovered => accent_hover,
                slider::Status::Dragged => palette.text_color(),
            };

            slider::Style {
                rail: slider::Rail {
                    backgrounds: (accent.into(), Color::from_rgba8(255, 255, 255, 0.12).into()),
                    width: 4.0,
                    border: border::rounded(999.0),
                },
                handle: slider::Handle {
                    shape: slider::HandleShape::Circle { radius: 6.0 },
                    background: handle.into(),
                    border_width: 0.0,
                    border_color: Color::TRANSPARENT,
                },
            }
        })
        .into()
}

pub(crate) fn eyebrow(label: &'static str, palette: WallpaperPalette) -> Element<'static, Message> {
    text(label)
        .size(11)
        .color(Color::from_rgba8(
            palette.muted_text[0],
            palette.muted_text[1],
            palette.muted_text[2],
            0.95,
        ))
        .into()
}

pub(crate) fn glass_style(
    palette: WallpaperPalette,
    radius: f32,
    alt_panel: bool,
) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        text_color: Some(palette.text_color()),
        background: Some(Background::Color(if alt_panel {
            Color::from_rgba8(
                palette.panel_alt[0],
                palette.panel_alt[1],
                palette.panel_alt[2],
                0.94,
            )
        } else {
            Color::from_rgba8(palette.panel[0], palette.panel[1], palette.panel[2], 0.96)
        })),
        border: border::rounded(radius).width(1.0).color(Color::from_rgba8(
            palette.border[0],
            palette.border[1],
            palette.border[2],
            0.18,
        )),
        ..container::Style::default()
    }
}

fn section_style(palette: WallpaperPalette) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(Color::from_rgba8(
            palette.panel_alt[0],
            palette.panel_alt[1],
            palette.panel_alt[2],
            0.42,
        ))),
        border: border::rounded(16.0).width(1.0).color(Color::from_rgba8(
            palette.border[0],
            palette.border[1],
            palette.border[2],
            0.14,
        )),
        ..container::Style::default()
    }
}

fn inset_style(palette: WallpaperPalette) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(Color::from_rgba8(255, 255, 255, 0.045))),
        border: border::rounded(12.0).width(1.0).color(Color::from_rgba8(
            palette.border[0],
            palette.border[1],
            palette.border[2],
            0.10,
        )),
        ..container::Style::default()
    }
}

fn button_style(palette: WallpaperPalette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let background = match status {
            button::Status::Active => Color::from_rgba8(255, 255, 255, 0.06),
            button::Status::Hovered => Color::from_rgba8(
                palette.accent_soft[0],
                palette.accent_soft[1],
                palette.accent_soft[2],
                0.42,
            ),
            button::Status::Pressed => Color::from_rgba8(
                palette.accent[0],
                palette.accent[1],
                palette.accent[2],
                0.28,
            ),
            button::Status::Disabled => Color::from_rgba8(255, 255, 255, 0.03),
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: palette.text_color(),
            border: border::rounded(10.0)
                .width(1.0)
                .color(Color::from_rgba8(255, 255, 255, 0.05)),
            ..button::Style::default()
        }
    }
}

fn action_chip_style(
    palette: WallpaperPalette,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let background = match status {
            button::Status::Hovered => Color::from_rgba8(
                palette.accent[0],
                palette.accent[1],
                palette.accent[2],
                if active { 0.42 } else { 0.24 },
            ),
            button::Status::Pressed => Color::from_rgba8(
                palette.accent[0],
                palette.accent[1],
                palette.accent[2],
                0.48,
            ),
            _ if active => Color::from_rgba8(
                palette.accent[0],
                palette.accent[1],
                palette.accent[2],
                0.32,
            ),
            _ => Color::from_rgba8(255, 255, 255, 0.045),
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: if active {
                palette.text_color()
            } else {
                palette.muted_text_color()
            },
            border: border::rounded(10.0)
                .width(if active { 1.0 } else { 0.0 })
                .color(Color::from_rgba8(255, 255, 255, 0.08)),
            ..button::Style::default()
        }
    }
}

fn chip_button_style(
    palette: WallpaperPalette,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let background = match status {
            button::Status::Hovered => Color::from_rgba8(
                palette.accent_soft[0],
                palette.accent_soft[1],
                palette.accent_soft[2],
                if active { 0.44 } else { 0.28 },
            ),
            button::Status::Pressed => Color::from_rgba8(
                palette.accent[0],
                palette.accent[1],
                palette.accent[2],
                0.28,
            ),
            _ if active => Color::from_rgba8(
                palette.accent_soft[0],
                palette.accent_soft[1],
                palette.accent_soft[2],
                0.34,
            ),
            _ => Color::from_rgba8(255, 255, 255, 0.05),
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: palette.text_color(),
            border: border::rounded(12.0).width(1.0).color(Color::from_rgba8(
                255,
                255,
                255,
                if active { 0.10 } else { 0.05 },
            )),
            ..button::Style::default()
        }
    }
}

fn selection_row_style(
    palette: WallpaperPalette,
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let background = match status {
            button::Status::Hovered => Color::from_rgba8(
                palette.accent_soft[0],
                palette.accent_soft[1],
                palette.accent_soft[2],
                if active { 0.26 } else { 0.16 },
            ),
            button::Status::Pressed => Color::from_rgba8(
                palette.accent[0],
                palette.accent[1],
                palette.accent[2],
                0.18,
            ),
            _ if active => Color::from_rgba8(
                palette.accent_soft[0],
                palette.accent_soft[1],
                palette.accent_soft[2],
                0.14,
            ),
            _ => Color::from_rgba8(255, 255, 255, 0.035),
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: palette.text_color(),
            border: border::rounded(12.0).width(1.0).color(Color::from_rgba8(
                255,
                255,
                255,
                if active { 0.08 } else { 0.04 },
            )),
            ..button::Style::default()
        }
    }
}

fn ghost_button_style() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, _status| button::Style {
        background: None,
        border: border::rounded(0.0).width(0.0).color(Color::TRANSPARENT),
        shadow: Default::default(),
        ..button::Style::default()
    }
}

fn scrollable_rail_style(
    palette: WallpaperPalette,
    status: scrollable::Status,
    vertical: bool,
) -> scrollable::Rail {
    let (is_hovered, is_dragged, is_disabled) = match status {
        scrollable::Status::Active {
            is_vertical_scrollbar_disabled,
            is_horizontal_scrollbar_disabled,
        } => (
            false,
            false,
            if vertical {
                is_vertical_scrollbar_disabled
            } else {
                is_horizontal_scrollbar_disabled
            },
        ),
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered,
            is_horizontal_scrollbar_hovered,
            is_vertical_scrollbar_disabled,
            is_horizontal_scrollbar_disabled,
        } => (
            if vertical {
                is_vertical_scrollbar_hovered
            } else {
                is_horizontal_scrollbar_hovered
            },
            false,
            if vertical {
                is_vertical_scrollbar_disabled
            } else {
                is_horizontal_scrollbar_disabled
            },
        ),
        scrollable::Status::Dragged {
            is_vertical_scrollbar_dragged,
            is_horizontal_scrollbar_dragged,
            is_vertical_scrollbar_disabled,
            is_horizontal_scrollbar_disabled,
        } => (
            false,
            if vertical {
                is_vertical_scrollbar_dragged
            } else {
                is_horizontal_scrollbar_dragged
            },
            if vertical {
                is_vertical_scrollbar_disabled
            } else {
                is_horizontal_scrollbar_disabled
            },
        ),
    };

    let rail_alpha = if is_disabled { 0.06 } else { 0.18 };
    let scroller_alpha = if is_dragged {
        0.85
    } else if is_hovered {
        0.72
    } else if is_disabled {
        0.20
    } else {
        0.52
    };

    scrollable::Rail {
        background: Some(Background::Color(Color::from_rgba8(
            palette.panel[0],
            palette.panel[1],
            palette.panel[2],
            rail_alpha,
        ))),
        border: border::rounded(999.0).width(1.0).color(Color::from_rgba8(
            palette.border[0],
            palette.border[1],
            palette.border[2],
            0.10,
        )),
        scroller: scrollable::Scroller {
            background: Background::Color(Color::from_rgba8(
                palette.accent_soft[0],
                palette.accent_soft[1],
                palette.accent_soft[2],
                scroller_alpha,
            )),
            border: border::rounded(999.0).width(1.0).color(Color::from_rgba8(
                palette.accent[0],
                palette.accent[1],
                palette.accent[2],
                if is_dragged { 0.45 } else { 0.20 },
            )),
        },
    }
}

pub(crate) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub(crate) fn truncate(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }

    let mut truncated = value
        .chars()
        .take(max_len.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}
