use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Container, button, column, container, row, text};
use iced::{Color, Element, Length, color};

use crate::LineStyle;
use crate::{message::PlotUiMessage, plot_widget::PlotWidget};

#[derive(Debug, Clone)]
/// An entry in the plot legend.
pub(crate) struct LegendEntry {
    pub(crate) label: String,
    pub(crate) color: Color,
    pub(crate) _marker: u32,
    pub(crate) _line_style: Option<LineStyle>,
    pub(crate) hidden: bool,
}

pub(crate) fn legend(widget: &PlotWidget, collapsed: bool) -> Element<'_, PlotUiMessage> {
    let _ = (widget, collapsed);
    container("")
        .width(Length::Fixed(0.0))
        .height(Length::Fixed(0.0))
        .into()
}

fn label_button(label: &str) -> Element<'_, PlotUiMessage> {
    button(text(label).size(12.0))
        .on_press(PlotUiMessage::ToggleLegend)
        .into()
}

fn legend_container<'a>(
    content: impl Into<Element<'a, PlotUiMessage>>,
) -> Container<'a, PlotUiMessage> {
    container(content)
        .padding(4.0)
        .align_x(Horizontal::Left)
        .align_y(Vertical::Top)
}
