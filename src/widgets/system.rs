use crate::{
    app::{Message, Rebar},
    ui,
    widgets::{PanelSpec, WidgetKind, icons},
};
use iced::{
    Color, Element, Fill,
    widget::{Space, column, container, row, text},
};
use iced_plot::{
    Color as PlotColor, LineStyle, PlotWidget, PlotWidgetBuilder, Series, Tick, TickWeight,
};
use lucide_icons::Icon;

const WIDTH: f32 = 620.0;
const HEADER_HEIGHT: f32 = 64.0;
const SUMMARY_HEIGHT: f32 = 180.0;
const DETAILS_HEIGHT: f32 = 170.0;
const PANEL_GAPS: f32 = 48.0;
const LIST_HEADER_HEIGHT: f32 = 58.0;
const ADAPTER_ROW_HEIGHT: f32 = 44.0;
const ADAPTER_LIST_MIN_HEIGHT: f32 = 72.0;
const ADAPTER_LIST_MAX_HEIGHT: f32 = 176.0;
const HISTORY_MIN_HEIGHT: f32 = 96.0;
const HISTORY_MAX_HEIGHT: f32 = 150.0;

#[derive(Debug, Clone, Copy)]
struct Layout {
    adapter_list_height: f32,
    history_height: f32,
    total_height: f32,
}

pub(crate) fn panel_spec(state: &Rebar) -> PanelSpec {
    let layout = layout(state);

    PanelSpec {
        min_size: iced::Size::new(520.0, 320.0),
        preferred_size: iced::Size::new(WIDTH, layout.total_height.min(state.max_flyout_height())),
    }
}

pub(crate) fn build_plot(state: &Rebar) -> PlotWidget {
    let max_x = state
        .widget_history
        .cpu_usage
        .len()
        .max(state.widget_history.memory_usage.len())
        .max(state.widget_history.gpu_memory_usage.len())
        .max(2) as f64
        - 1.0;

    let cpu = Series::line_only(
        state.widget_history.cpu_usage.line_points(),
        LineStyle::Solid,
    )
    .with_color(plot_color(cpu_line_rgb(state)))
    .with_label("CPU");
    let memory = Series::line_only(
        state.widget_history.memory_usage.line_points(),
        LineStyle::Solid,
    )
    .with_color(plot_color(memory_line_rgb(state)))
    .with_label("Memory");
    let gpu = Series::line_only(
        state.widget_history.gpu_memory_usage.line_points(),
        LineStyle::Dashed { length: 8.0 },
    )
    .with_color(plot_color(gpu_line_rgb(state)))
    .with_label("GPU memory");

    PlotWidgetBuilder::new()
        .with_x_label("Minutes")
        .with_y_label("Usage %")
        .with_x_lim(0.0, max_x.max(1.0))
        .with_y_lim(0.0, 100.0)
        .with_x_tick_labels(true)
        .with_y_tick_labels(true)
        .with_tick_label_size(10.0)
        .with_axis_label_size(11.0)
        .with_x_tick_producer(|min, max| {
            let start = ((min.max(0.0) / 60.0).floor() as i32).max(0) * 60;
            let end = max.ceil() as i32;
            let mut ticks = Vec::new();
            let mut value = start;

            while value <= end {
                ticks.push(Tick::new(value as f64, 60.0, TickWeight::Major));
                value += 60;
            }

            if ticks.is_empty() || ticks.last().map(|tick| tick.value).unwrap_or(0.0) < max {
                ticks.push(Tick::new(max.floor(), 60.0, TickWeight::Major));
            }

            ticks
        })
        .with_y_tick_producer(|_, _| {
            [0.0, 25.0, 50.0, 75.0, 100.0]
                .into_iter()
                .map(|value| Tick::new(value, 25.0, TickWeight::Major))
                .collect()
        })
        .with_x_tick_formatter(|tick| format!("{}m", (tick.value / 60.0).round() as i32))
        .with_y_tick_formatter(|tick| format!("{:.0}%", tick.value))
        .with_tooltips(true)
        .with_tooltip_provider(|context| {
            format!(
                "{}\n{}m {:.0}s\n{:.1}%",
                if context.series_label.is_empty() {
                    "Metric"
                } else {
                    &context.series_label
                },
                (context.x / 60.0).floor() as i32,
                (context.x % 60.0).round() as i32,
                context.y
            )
        })
        .with_crosshairs(false)
        .with_cursor_overlay(false)
        .add_series(cpu)
        .add_series(memory)
        .add_series(gpu)
        .build()
        .unwrap_or_default()
}

pub(crate) fn chip(state: &Rebar) -> Element<'_, Message> {
    let content = row![
        icons::themed(Icon::Activity, 15, state.palette),
        text(format!("CPU {:.0}%", state.system.cpu.usage_percent))
            .size(12)
            .color(state.palette.text_color()),
        text(format!(
            "RAM {:.0}%",
            memory_usage_percent(
                state.system.memory.used_bytes,
                state.system.memory.total_bytes
            )
        ))
        .size(12)
        .color(state.palette.muted_text_color()),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    ui::chip_button(
        content.into(),
        state.palette,
        state.active_widget == Some(WidgetKind::System) && state.flyout_target_open,
        Message::WidgetSelected(WidgetKind::System),
    )
}

pub(crate) fn panel(state: &Rebar) -> Element<'_, Message> {
    let layout = layout(state);
    let primary_gpu = state.system.gpus.first();
    let gpu_summary = primary_gpu
        .map(|gpu| format_gpu_budget(gpu.local_usage_bytes, gpu.local_budget_bytes))
        .unwrap_or_else(|| String::from("Unavailable"));

    let history_plot = state
        .system_plot
        .view()
        .map(|message| Message::PlotMessage(WidgetKind::System, message));

    let summary = ui::section_card(
        column![
            ui::eyebrow("Live snapshot", state.palette),
            row![
                ui::summary_tile(
                    "CPU",
                    format!("{:.0}%", state.system.cpu.usage_percent),
                    format!("{} logical cores", state.system.cpu.logical_cores),
                    state.palette,
                ),
                ui::summary_tile(
                    "Memory",
                    format!(
                        "{:.0}%",
                        memory_usage_percent(
                            state.system.memory.used_bytes,
                            state.system.memory.total_bytes
                        )
                    ),
                    format!(
                        "{} / {}",
                        ui::format_bytes(state.system.memory.used_bytes),
                        ui::format_bytes(state.system.memory.total_bytes)
                    ),
                    state.palette,
                ),
                ui::summary_tile(
                    "GPU memory",
                    gpu_summary,
                    primary_gpu
                        .map(|gpu| ui::truncate(&gpu.name, 18))
                        .unwrap_or_else(|| String::from("No adapter")),
                    state.palette,
                ),
            ]
            .spacing(10),
        ]
        .spacing(12)
        .into(),
        state.palette,
    );

    let details = ui::section_card(
        column![
            ui::eyebrow("Current state", state.palette),
            Space::new().height(8),
            row![
                column![
                    stat_row(
                        Icon::Cpu,
                        "Available memory",
                        ui::format_bytes(state.system.memory.available_bytes),
                        state,
                    ),
                    stat_row(
                        Icon::MemoryStick,
                        "Visible adapters",
                        format!("{}", state.system.gpus.len().max(1)),
                        state,
                    ),
                ]
                .spacing(10)
                .width(Fill),
                column![
                    stat_row(
                        Icon::Gauge,
                        "CPU load",
                        format!("{:.1}%", state.system.cpu.usage_percent),
                        state,
                    ),
                    stat_row(
                        Icon::MonitorSpeaker,
                        "Primary GPU",
                        primary_gpu
                            .map(|gpu| ui::truncate(&gpu.name, 22))
                            .unwrap_or_else(|| String::from("Unavailable")),
                        state,
                    ),
                ]
                .spacing(10)
                .width(Fill),
            ]
            .spacing(10),
        ]
        .into(),
        state.palette,
    );

    let mut adapters = column![].spacing(10);
    if state.system.gpus.is_empty() {
        adapters = adapters.push(ui::inset_card(
            text("No hardware adapter telemetry")
                .size(13)
                .color(state.palette.muted_text_color())
                .into(),
            state.palette,
        ));
    } else {
        for gpu in &state.system.gpus {
            let mut detail = format!(
                "{} • dedicated {}",
                format_gpu_budget(gpu.local_usage_bytes, gpu.local_budget_bytes),
                ui::format_bytes(gpu.dedicated_memory_bytes)
            );

            if gpu.shared_memory_bytes > 0 {
                detail.push_str(" • shared ");
                detail.push_str(&ui::format_bytes(gpu.shared_memory_bytes));
            }

            adapters = adapters.push(ui::selection_row(
                ui::truncate(&gpu.name, 34),
                detail,
                Some(if gpu.is_software {
                    String::from("Software")
                } else {
                    String::from("Hardware")
                }),
                state.palette,
                std::ptr::eq(gpu, primary_gpu.unwrap_or(gpu)),
                None,
            ));
        }
    }

    let history = ui::section_card(
        column![
            ui::eyebrow("Recent history", state.palette),
            text("CPU, memory, and GPU memory over the last five minutes.")
                .size(12)
                .color(state.palette.muted_text_color()),
            row![
                history_label("CPU", color_from_rgb(cpu_line_rgb(state))),
                history_label("Memory", color_from_rgb(memory_line_rgb(state))),
                history_label("GPU memory", color_from_rgb(gpu_line_rgb(state))),
            ]
            .spacing(14)
            .wrap(),
            ui::inset_card(
                container(history_plot)
                    .width(Fill)
                    .height(layout.history_height)
                    .into(),
                state.palette,
            ),
        ]
        .spacing(10)
        .into(),
        state.palette,
    );

    column![
        ui::panel_header(
            "System",
            Some("Live usage across the desktop session"),
            state.palette,
            Message::WidgetSelected(WidgetKind::System),
        ),
        summary,
        details,
        ui::section_card(
            column![
                row![
                    ui::eyebrow("Adapters", state.palette),
                    if state.system.gpus.len() > 3 {
                        ui::scroll_hint("adapter list", state.palette)
                    } else {
                        text("")
                            .size(11)
                            .color(state.palette.muted_text_color())
                            .into()
                    },
                ]
                .width(Fill)
                .spacing(8),
                ui::list_scroll(adapters.into(), state.palette, layout.adapter_list_height,),
            ]
            .spacing(10)
            .into(),
            state.palette,
        ),
        history,
    ]
    .spacing(12)
    .into()
}

fn stat_row<'a>(
    icon: Icon,
    label: &'static str,
    value: String,
    state: &'a Rebar,
) -> Element<'a, Message> {
    ui::inset_card(
        row![
            icons::themed(icon, 15, state.palette),
            column![
                text(label).size(12).color(state.palette.muted_text_color()),
                text(value).size(14).color(state.palette.text_color()),
            ]
            .spacing(2),
        ]
        .spacing(10)
        .align_y(iced::alignment::Vertical::Center)
        .width(Fill)
        .into(),
        state.palette,
    )
}

fn memory_usage_percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        used as f32 / total as f32 * 100.0
    }
}

fn format_gpu_budget(used: u64, budget: u64) -> String {
    if budget > 0 {
        format!("{} / {}", ui::format_bytes(used), ui::format_bytes(budget))
    } else {
        ui::format_bytes(used)
    }
}

fn plot_color(rgb: [u8; 3]) -> PlotColor {
    PlotColor::from_rgb8(rgb[0], rgb[1], rgb[2])
}

fn color_from_rgb(rgb: [u8; 3]) -> Color {
    Color::from_rgb8(rgb[0], rgb[1], rgb[2])
}

fn cpu_line_rgb(state: &Rebar) -> [u8; 3] {
    state.palette.accent
}

fn memory_line_rgb(state: &Rebar) -> [u8; 3] {
    state.palette.text
}

fn gpu_line_rgb(state: &Rebar) -> [u8; 3] {
    state.palette.accent_soft
}

fn history_label(label: &'static str, color: Color) -> Element<'static, Message> {
    text(label).size(12).color(color).into()
}

fn adapter_list_height(count: usize) -> f32 {
    (count.max(1) as f32 * ADAPTER_ROW_HEIGHT)
        .clamp(ADAPTER_LIST_MIN_HEIGHT, ADAPTER_LIST_MAX_HEIGHT)
}

fn layout(state: &Rebar) -> Layout {
    let mut adapter_list_height = adapter_list_height(state.system.gpus.len());
    let mut history_height = HISTORY_MAX_HEIGHT;

    let fixed_height =
        HEADER_HEIGHT + SUMMARY_HEIGHT + DETAILS_HEIGHT + PANEL_GAPS + LIST_HEADER_HEIGHT;
    let available_flexible = (state.max_flyout_height() - fixed_height)
        .max(ADAPTER_LIST_MIN_HEIGHT + HISTORY_MIN_HEIGHT);
    let natural_flexible = adapter_list_height + history_height;

    if natural_flexible > available_flexible {
        let mut overflow = natural_flexible - available_flexible;

        let history_reduction = (history_height - HISTORY_MIN_HEIGHT).min(overflow);
        history_height -= history_reduction;
        overflow -= history_reduction;

        let adapter_reduction = (adapter_list_height - ADAPTER_LIST_MIN_HEIGHT).min(overflow);
        adapter_list_height -= adapter_reduction;
    }

    Layout {
        adapter_list_height,
        history_height,
        total_height: fixed_height + adapter_list_height + history_height,
    }
}
