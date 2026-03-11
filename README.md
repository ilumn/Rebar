<p align="center">
<img width="60%" alt="banner" src="https://github.com/user-attachments/assets/bf1a4f64-24c7-452e-8029-e154d0bdd7b4" />
</p>



\
Rebar is a Windows top taskbar replacement built with Rust and `iced`.

It reserves a block at the top of the monitor, themes itself from the current wallpaper automatically, and provides expanding widgets for:
Left side:
- Device Actions (shutdown, restart, log off, etc)

Right side:
- System Usage
- Network Status
- Audio Devices and Volume
- Media Playback

## Gallery
<img width="2559" height="1439" alt="example (4)" src="https://github.com/user-attachments/assets/cb9e11a0-2779-437e-8e20-c5fc44d53fd8" />


## Build

```bash
cargo build --release
```

The release binary is written to:

```bash
target\release\rebar.exe
```

## Startup

Startup registration is controlled by `rebar.toml`.

When `launch_on_startup = true`, Rebar writes a `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Rebar` entry that points at the current executable and config path.

## Config

Example:

```toml
palette = "balanced"
hide_windows_taskbar = true
auto_hide_panels_on_focus_loss = true
flyout_animation_ms = 100
launch_on_startup = false
startup_mode = "background"
```

Supported `palette` values:

- `balanced`
- `vibrant`
- `contrast`
- `center`

Supported `startup_mode` values:

- `foreground`
- `background`

## Vendored `vendor/iced_plot

This project patches `iced_plot` in `Cargo.toml` because Rebar depends on changes to the original plot widget:
Changes include:
- transparent plot background
- disabled built-in legend
- disabled pan/zoom behavior for the embedded charts
