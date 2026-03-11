use crate::{
    app::{Message, Rebar},
    system::{DeviceCommand, SystemCommand},
    ui,
    widgets::WidgetKind,
};
use iced::{
    Element, Fill, Size,
    widget::{column, row, text},
};

pub(crate) fn preferred_size(_state: &Rebar) -> Size {
    Size::new(332.0, 356.0)
}

pub(crate) fn panel(state: &Rebar) -> Element<'_, Message> {
    column![
        row![
            text("Device")
                .size(20)
                .color(state.palette.text_color())
                .width(Fill),
            ui::panel_button(
                "Close",
                state.palette,
                Message::WidgetSelected(WidgetKind::Device),
            ),
        ]
        .align_y(iced::alignment::Vertical::Center)
        .width(Fill),
        actions_body(state),
    ]
    .spacing(12)
    .into()
}

fn actions_body(state: &Rebar) -> Element<'_, Message> {
    column![
        ui::selection_row(
            String::from("Settings"),
            String::from("Open the Windows Settings app"),
            Some(String::from("Open")),
            state.palette,
            false,
            Some(Message::SystemCommand(SystemCommand::Device(
                DeviceCommand::OpenSettings,
            ))),
        ),
        ui::selection_row(
            String::from("Lock"),
            String::from("Lock this workstation immediately"),
            Some(String::from("Now")),
            state.palette,
            false,
            Some(Message::SystemCommand(SystemCommand::Device(
                DeviceCommand::Lock,
            ))),
        ),
        ui::selection_row(
            String::from("Log Out"),
            String::from("End the current desktop session"),
            Some(String::from("Exit")),
            state.palette,
            false,
            Some(Message::SystemCommand(SystemCommand::Device(
                DeviceCommand::LogOut,
            ))),
        ),
        ui::selection_row(
            String::from("Sleep"),
            String::from("Put this machine to sleep"),
            Some(String::from("Sleep")),
            state.palette,
            false,
            Some(Message::SystemCommand(SystemCommand::Device(
                DeviceCommand::Sleep,
            ))),
        ),
        ui::selection_row(
            String::from("Shutdown"),
            String::from("Power off the machine"),
            Some(String::from("Off")),
            state.palette,
            false,
            Some(Message::SystemCommand(SystemCommand::Device(
                DeviceCommand::Shutdown,
            ))),
        ),
    ]
    .spacing(10)
    .into()
}
