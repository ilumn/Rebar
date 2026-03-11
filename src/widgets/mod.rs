pub(crate) mod audio;
pub(crate) mod device;
pub(crate) mod history;
pub(crate) mod icons;
pub(crate) mod kinds;
pub(crate) mod media;
pub(crate) mod network;
pub(crate) mod system;

pub(crate) use history::WidgetHistory;
pub(crate) use kinds::WidgetKind;

use crate::app::{Message, Rebar};
use iced::{Element, Size};

#[derive(Debug, Clone, Copy)]
pub(crate) struct PanelSpec {
    pub(crate) min_size: Size,
    pub(crate) preferred_size: Size,
}

pub(crate) fn panel_spec(state: &Rebar, kind: WidgetKind) -> PanelSpec {
    match kind {
        WidgetKind::System => system::panel_spec(state),
        WidgetKind::Network => network::panel_spec(state),
        WidgetKind::Audio => audio::panel_spec(state),
        WidgetKind::Media => media::panel_spec(state),
        WidgetKind::Device => PanelSpec {
            min_size: Size::new(360.0, 220.0),
            preferred_size: device::preferred_size(state),
        },
    }
}

pub(crate) fn chip_order(state: &Rebar) -> Vec<Element<'_, Message>> {
    let mut chips = vec![system::chip(state), network::chip(state)];

    if let Some(media) = media::chip(state) {
        chips.push(media);
    }

    chips.push(audio::chip(state));
    chips
}

pub(crate) fn active_panel(state: &Rebar) -> Element<'_, Message> {
    match state.active_widget.unwrap_or(WidgetKind::System) {
        WidgetKind::System => system::panel(state),
        WidgetKind::Network => network::panel(state),
        WidgetKind::Audio => audio::panel(state),
        WidgetKind::Media => media::panel(state),
        WidgetKind::Device => device::panel(state),
    }
}
