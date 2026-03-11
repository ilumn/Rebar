use crate::{
    app::{Message, Rebar},
    system::{AvailableNetworkSnapshot, NetworkKind},
    ui,
    widgets::{PanelSpec, WidgetKind, icons},
};
use iced::{
    Color, Element, Fill,
    alignment::Horizontal,
    widget::{column, container, row, text},
};
use iced_plot::{Color as PlotColor, LineStyle, PlotWidget, PlotWidgetBuilder, Series};
use lucide_icons::Icon;

const WIDTH: f32 = 560.0;
const HEADER_HEIGHT: f32 = 64.0;
const HERO_HEIGHT: f32 = 138.0;
const PANEL_GAPS: f32 = 48.0;
const MAX_INTERFACES: usize = 12;
const MAX_WIFI_NETWORKS: usize = 12;
const LIST_HEADER_HEIGHT: f32 = 58.0;
const WIFI_ROW_HEIGHT: f32 = 44.0;
const ADAPTER_ROW_HEIGHT: f32 = 44.0;
const WIFI_LIST_MIN_HEIGHT: f32 = 72.0;
const WIFI_LIST_MAX_HEIGHT: f32 = WIFI_ROW_HEIGHT * 4.0;
const ADAPTER_LIST_MIN_HEIGHT: f32 = 72.0;
const ADAPTER_LIST_MAX_HEIGHT: f32 = ADAPTER_ROW_HEIGHT * 4.0;
const HISTORY_MIN_HEIGHT: f32 = 96.0;
const HISTORY_MAX_HEIGHT: f32 = 150.0;

#[derive(Debug, Clone, Copy)]
struct Layout {
    wifi_list_height: f32,
    adapter_list_height: f32,
    history_height: f32,
    total_height: f32,
}

pub(crate) fn panel_spec(state: &Rebar) -> PanelSpec {
    let layout = layout(state);

    PanelSpec {
        min_size: iced::Size::new(440.0, 300.0),
        preferred_size: iced::Size::new(WIDTH, layout.total_height.min(state.max_flyout_height())),
    }
}

pub(crate) fn build_plot(state: &Rebar) -> PlotWidget {
    let max_x = state
        .widget_history
        .network_down_mbps
        .len()
        .max(state.widget_history.network_up_mbps.len())
        .max(2) as f64
        - 1.0;
    let max_y = state
        .widget_history
        .network_down_mbps
        .max()
        .max(state.widget_history.network_up_mbps.max())
        .max(1.0) as f64
        * 1.15;

    let down = Series::line_only(
        state.widget_history.network_down_mbps.line_points(),
        LineStyle::Solid,
    )
    .with_color(plot_color(state.palette.accent));
    let up = Series::line_only(
        state.widget_history.network_up_mbps.line_points(),
        LineStyle::Dashed { length: 8.0 },
    )
    .with_color(plot_color(state.palette.text));

    PlotWidgetBuilder::new()
        .with_x_lim(0.0, max_x.max(1.0))
        .with_y_lim(0.0, max_y.max(1.0))
        .with_x_tick_labels(false)
        .with_y_tick_labels(false)
        .with_tooltips(true)
        .with_cursor_overlay(false)
        .with_crosshairs(true)
        .without_grid()
        .add_series(down)
        .add_series(up)
        .build()
        .unwrap_or_default()
}

pub(crate) fn chip(state: &Rebar) -> Element<'_, Message> {
    let primary = state.system.network.interfaces.iter().find(|interface| interface.is_primary);
    let icon = primary.map(|iface| icon_for_kind(iface.kind)).unwrap_or(Icon::Globe);
    let detail = primary
        .and_then(|iface| iface.detail_name.clone())
        .unwrap_or_else(|| String::from("Network"));

    let content = row![
        icons::themed(icon, 15, state.palette),
        text(format!("↓ {}", compact_rate(state.system.network.received_bps)))
            .size(12)
            .color(state.palette.text_color()),
        text(format!("↑ {}", compact_rate(state.system.network.transmitted_bps)))
            .size(12)
            .color(state.palette.muted_text_color()),
        text(ui::truncate(&detail, 12))
            .size(12)
            .color(state.palette.muted_text_color()),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    ui::chip_button(
        content.into(),
        state.palette,
        state.active_widget == Some(WidgetKind::Network) && state.flyout_target_open,
        Message::WidgetSelected(WidgetKind::Network),
    )
}

pub(crate) fn panel(state: &Rebar) -> Element<'_, Message> {
    let layout = layout(state);
    let primary = state
        .system
        .network
        .interfaces
        .iter()
        .find(|interface| interface.is_primary)
        .or_else(|| state.system.network.interfaces.first());
    let history_plot = state
        .network_plot
        .view()
        .map(|message| Message::PlotMessage(WidgetKind::Network, message));

    let hero = ui::section_card(
        column![
            ui::eyebrow("Primary connection", state.palette),
            if let Some(primary) = primary {
                ui::inset_card(
                    row![
                        icons::themed(icon_for_kind(primary.kind), 18, state.palette),
                        column![
                            text(if primary.connected { "Connected" } else { "Idle link" })
                                .size(14)
                                .color(Color::from_rgb8(
                                    state.palette.accent[0],
                                    state.palette.accent[1],
                                    state.palette.accent[2],
                                )),
                            text(primary_label(primary))
                                .size(15)
                                .color(state.palette.text_color()),
                            text(primary_subtitle(primary))
                                .size(12)
                                .color(state.palette.muted_text_color()),
                        ]
                        .spacing(2)
                        .width(Fill),
                        column![
                            text(format!("↓ {}", compact_rate(state.system.network.received_bps)))
                                .size(12)
                                .color(state.palette.text_color()),
                            text(format!("↑ {}", compact_rate(state.system.network.transmitted_bps)))
                                .size(12)
                                .color(state.palette.muted_text_color()),
                        ]
                        .spacing(2)
                        .align_x(Horizontal::Right),
                    ]
                    .spacing(12)
                    .align_y(iced::alignment::Vertical::Center)
                    .into(),
                    state.palette,
                )
            } else {
                ui::inset_card(
                    text("No active adapter telemetry reported.")
                        .size(13)
                        .color(state.palette.muted_text_color())
                        .into(),
                    state.palette,
                )
            },
        ]
        .spacing(12)
        .into(),
        state.palette,
    );

    let mut wifi_rows = column![].spacing(10);
    if let Some(primary) = primary.filter(|interface| interface.kind == NetworkKind::Wifi) {
        if primary.available_networks.is_empty() {
            wifi_rows = wifi_rows.push(
                ui::inset_card(
                    text("No Wi-Fi scan results reported right now.")
                        .size(13)
                        .color(state.palette.muted_text_color())
                        .into(),
                    state.palette,
                ),
            );
        } else {
            for network in primary.available_networks.iter().take(MAX_WIFI_NETWORKS) {
                wifi_rows = wifi_rows.push(wifi_network_row(network, state));
            }
        }
    } else {
        wifi_rows = wifi_rows.push(
            ui::inset_card(
                text("Primary connection is not a Wi-Fi interface.")
                    .size(13)
                    .color(state.palette.muted_text_color())
                    .into(),
                state.palette,
            ),
        );
    }

    let mut adapter_rows = column![].spacing(10);
    if state.system.network.interfaces.is_empty() {
        adapter_rows = adapter_rows.push(
            ui::inset_card(
                text("No adapters reported")
                    .size(13)
                    .color(state.palette.muted_text_color())
                    .into(),
                state.palette,
            ),
        );
    } else {
        for interface in state.system.network.interfaces.iter().take(MAX_INTERFACES) {
            adapter_rows = adapter_rows.push(ui::selection_row(
                ui::truncate(&interface.name, 32),
                format!(
                    "{} • ↓ {} • ↑ {}",
                    primary_subtitle(interface),
                    compact_rate(interface.received_bps),
                    compact_rate(interface.transmitted_bps),
                ),
                Some(if interface.is_primary {
                    String::from("Primary")
                } else {
                    kind_label(interface.kind).to_string()
                }),
                state.palette,
                interface.is_primary,
                None,
            ));
        }
    }

    let wifi_count = primary
        .map(|interface| interface.available_networks.len())
        .unwrap_or(0);

    let wifi_section = ui::section_card(
        column![
            row![
                ui::eyebrow("Available Wi-Fi", state.palette),
                if wifi_count > 4 {
                    ui::scroll_hint("SSID list", state.palette)
                } else {
                    text("").size(11).color(state.palette.muted_text_color()).into()
                },
            ]
            .width(Fill)
            .spacing(8),
            ui::list_scroll(wifi_rows.into(), state.palette, layout.wifi_list_height),
        ]
        .spacing(10)
        .into(),
        state.palette,
    );

    let adapter_section = ui::section_card(
        column![
            row![
                ui::eyebrow("Adapters", state.palette),
                if state.system.network.interfaces.len() > 4 {
                    ui::scroll_hint("adapter list", state.palette)
                } else {
                    text("").size(11).color(state.palette.muted_text_color()).into()
                },
            ]
            .width(Fill)
            .spacing(8),
            ui::list_scroll(adapter_rows.into(), state.palette, layout.adapter_list_height),
        ]
        .spacing(10)
        .into(),
        state.palette,
    );

    let history = ui::section_card(
        column![
            ui::eyebrow("Recent activity", state.palette),
            text("Throughput trend over the last minute.")
                .size(12)
                .color(state.palette.muted_text_color()),
            container(history_plot).width(Fill).height(layout.history_height),
        ]
        .spacing(10)
        .into(),
        state.palette,
    );

    column![
        ui::panel_header(
            "Network",
            Some("Primary link, Wi-Fi detail, and activity"),
            state.palette,
            Message::WidgetSelected(WidgetKind::Network),
        ),
        hero,
        wifi_section,
        adapter_section,
        history,
    ]
    .spacing(12)
    .into()
}

fn wifi_network_row<'a>(
    network: &AvailableNetworkSnapshot,
    state: &'a Rebar,
) -> Element<'a, Message> {
    ui::selection_row(
        ui::truncate(&network.ssid, 34),
        format!(
            "{} • {} • {}",
            if network.connected {
                "Connected"
            } else if network.saved {
                "Saved"
            } else {
                "Visible"
            },
            if network.secure { "Secure" } else { "Open" },
            signal_label(network.signal_percent),
        ),
        Some(if network.connected {
            String::from("Live")
        } else {
            format!("{}%", network.signal_percent)
        }),
        state.palette,
        network.connected,
        None,
    )
}

fn compact_rate(bytes_per_second: u64) -> String {
    format!("{}/s", ui::format_bytes(bytes_per_second))
}

fn plot_color(rgb: [u8; 3]) -> PlotColor {
    PlotColor::from_rgb8(rgb[0], rgb[1], rgb[2])
}

fn icon_for_kind(kind: NetworkKind) -> Icon {
    match kind {
        NetworkKind::Wifi => Icon::WifiHigh,
        NetworkKind::Ethernet => Icon::Cable,
        NetworkKind::Other => Icon::Globe,
    }
}

fn kind_label(kind: NetworkKind) -> &'static str {
    match kind {
        NetworkKind::Wifi => "Wi-Fi",
        NetworkKind::Ethernet => "Ethernet",
        NetworkKind::Other => "Other",
    }
}

fn primary_label(interface: &crate::system::NetworkInterfaceSnapshot) -> String {
    interface
        .detail_name
        .clone()
        .unwrap_or_else(|| interface.name.clone())
}

fn primary_subtitle(interface: &crate::system::NetworkInterfaceSnapshot) -> String {
    let mut parts = vec![kind_label(interface.kind).to_string()];
    if let Some(signal) = interface.signal_percent {
        parts.push(signal_label(signal));
    }
    if interface.connected {
        parts.push(String::from("Online"));
    }
    parts.join(" • ")
}

fn signal_label(signal_percent: u32) -> String {
    format!("{signal_percent}% signal")
}

fn list_height(count: usize, row_height: f32, min_height: f32, max_height: f32) -> f32 {
    (count.max(1) as f32 * row_height).clamp(min_height, max_height)
}

fn layout(state: &Rebar) -> Layout {
    let wifi_count = state
        .system
        .network
        .interfaces
        .iter()
        .find(|interface| interface.is_primary)
        .map(|interface| interface.available_networks.len())
        .unwrap_or(0);
    let adapter_count = state.system.network.interfaces.len();

    let mut wifi_list_height =
        list_height(wifi_count, WIFI_ROW_HEIGHT, WIFI_LIST_MIN_HEIGHT, WIFI_LIST_MAX_HEIGHT);
    let mut adapter_list_height = list_height(
        adapter_count,
        ADAPTER_ROW_HEIGHT,
        ADAPTER_LIST_MIN_HEIGHT,
        ADAPTER_LIST_MAX_HEIGHT,
    );
    let mut history_height = HISTORY_MAX_HEIGHT;

    let fixed_height = HEADER_HEIGHT + HERO_HEIGHT + PANEL_GAPS + (LIST_HEADER_HEIGHT * 2.0);
    let available_flexible = (state.max_flyout_height() - fixed_height).max(
        WIFI_LIST_MIN_HEIGHT + ADAPTER_LIST_MIN_HEIGHT + HISTORY_MIN_HEIGHT,
    );
    let natural_flexible = wifi_list_height + adapter_list_height + history_height;

    if natural_flexible > available_flexible {
        let mut overflow = natural_flexible - available_flexible;

        let history_reduction = (history_height - HISTORY_MIN_HEIGHT).min(overflow);
        history_height -= history_reduction;
        overflow -= history_reduction;

        let wifi_reduction = (wifi_list_height - WIFI_LIST_MIN_HEIGHT).min(overflow);
        wifi_list_height -= wifi_reduction;
        overflow -= wifi_reduction;

        let adapter_reduction = (adapter_list_height - ADAPTER_LIST_MIN_HEIGHT).min(overflow);
        adapter_list_height -= adapter_reduction;
    }

    let total_height =
        fixed_height + wifi_list_height + adapter_list_height + history_height;

    Layout {
        wifi_list_height,
        adapter_list_height,
        history_height,
        total_height,
    }
}
