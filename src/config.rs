use std::{fs, path::Path};

#[derive(Debug, Clone, Copy)]
pub(crate) enum PaletteMode {
    Balanced,
    Vibrant,
    Contrast,
    Center,
}

impl PaletteMode {
    pub(crate) fn as_key(self) -> &'static str {
        match self {
            PaletteMode::Balanced => "balanced",
            PaletteMode::Vibrant => "vibrant",
            PaletteMode::Contrast => "contrast",
            PaletteMode::Center => "center",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupMode {
    Foreground,
    Background,
}

impl StartupMode {
    pub(crate) fn as_startup_arg(self) -> Option<&'static str> {
        match self {
            StartupMode::Foreground => None,
            StartupMode::Background => Some("--background"),
        }
    }

    pub(crate) fn as_key(self) -> &'static str {
        match self {
            StartupMode::Foreground => "foreground",
            StartupMode::Background => "background",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AppConfig {
    pub(crate) palette_mode: PaletteMode,
    pub(crate) hide_windows_taskbar: bool,
    pub(crate) auto_hide_panels_on_focus_loss: bool,
    pub(crate) flyout_animation_ms: u64,
    pub(crate) launch_on_startup: bool,
    pub(crate) startup_mode: StartupMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            palette_mode: PaletteMode::Balanced,
            hide_windows_taskbar: true,
            auto_hide_panels_on_focus_loss: true,
            flyout_animation_ms: 200,
            launch_on_startup: false,
            startup_mode: StartupMode::Background,
        }
    }
}

impl AppConfig {
    pub(crate) fn ensure_default_at_path(path: &Path) -> Result<(), String> {
        if path.exists() {
            return Ok(());
        }

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "Failed to create the config directory {}: {error}",
                        parent.display()
                    )
                })?;
            }
        }

        fs::write(path, Self::default().to_toml()).map_err(|error| {
            format!(
                "Failed to write the default config to {}: {error}",
                path.display()
            )
        })
    }

    pub(crate) fn to_toml(self) -> String {
        format!(
            "palette = \"{}\"\nhide_windows_taskbar = {}\nauto_hide_panels_on_focus_loss = {}\nflyout_animation_ms = {}\nlaunch_on_startup = {}\nstartup_mode = \"{}\"\n",
            self.palette_mode.as_key(),
            self.hide_windows_taskbar,
            self.auto_hide_panels_on_focus_loss,
            self.flyout_animation_ms,
            self.launch_on_startup,
            self.startup_mode.as_key(),
        )
    }

    pub(crate) fn load_from_path(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;

        let mut config = Self::default();

        for line in contents.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            if key.trim() == "palette" {
                config.palette_mode = parse_palette_mode(value.trim())?;
            } else if key.trim() == "hide_windows_taskbar" {
                config.hide_windows_taskbar = parse_bool(value.trim())?;
            } else if key.trim() == "auto_hide_panels_on_focus_loss" {
                config.auto_hide_panels_on_focus_loss = parse_bool(value.trim())?;
            } else if key.trim() == "flyout_animation_ms" {
                config.flyout_animation_ms = parse_u64(value.trim(), "flyout_animation_ms")?;
            } else if key.trim() == "launch_on_startup" {
                config.launch_on_startup = parse_bool(value.trim())?;
            } else if key.trim() == "startup_mode" {
                config.startup_mode = parse_startup_mode(value.trim())?;
            }
        }

        Ok(config)
    }
}

fn parse_palette_mode(value: &str) -> Result<PaletteMode, String> {
    let value = value.trim().trim_matches('"').trim_matches('\'');

    match value {
        "balanced" => Ok(PaletteMode::Balanced),
        "vibrant" => Ok(PaletteMode::Vibrant),
        "contrast" => Ok(PaletteMode::Contrast),
        "center" | "center_bias" => Ok(PaletteMode::Center),
        other => Err(format!(
            "Unknown palette mode '{other}' in rebar.toml. Expected balanced, vibrant, contrast, or center."
        )),
    }
}

fn parse_bool(value: &str) -> Result<bool, String> {
    let value = value.trim().trim_matches('"').trim_matches('\'');

    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => Err(format!(
            "Unknown boolean value '{other}' in rebar.toml. Expected true or false."
        )),
    }
}

fn parse_startup_mode(value: &str) -> Result<StartupMode, String> {
    let value = value.trim().trim_matches('"').trim_matches('\'');

    match value {
        "foreground" | "normal" => Ok(StartupMode::Foreground),
        "background" => Ok(StartupMode::Background),
        other => Err(format!(
            "Unknown startup mode '{other}' in rebar.toml. Expected foreground or background."
        )),
    }
}

fn parse_u64(value: &str, key: &str) -> Result<u64, String> {
    let value = value.trim().trim_matches('"').trim_matches('\'');

    value.parse::<u64>().map_err(|error| {
        format!("Invalid integer value '{value}' for {key} in rebar.toml: {error}")
    })
}
