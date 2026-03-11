mod types;

pub(crate) use types::{
    AudioCommand, AvailableNetworkSnapshot, DeviceCommand, MediaCommand, MediaPlaybackState,
    MediaSnapshot, NetworkInterfaceSnapshot, NetworkKind, SystemCommand, SystemSnapshot,
};

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub(crate) fn refresh_snapshot() -> Result<SystemSnapshot, String> {
    windows::refresh_snapshot()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn refresh_snapshot() -> Result<SystemSnapshot, String> {
    Ok(SystemSnapshot::default())
}

#[cfg(target_os = "windows")]
pub(crate) fn apply_command(command: SystemCommand) -> Result<(), String> {
    windows::apply_command(command)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn apply_command(_command: SystemCommand) -> Result<(), String> {
    Ok(())
}
