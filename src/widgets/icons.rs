use crate::palette::WallpaperPalette;
use iced::{Color, widget::Text};
use lucide_icons::Icon;

pub(crate) fn icon(icon: Icon, size: u16, color: Color) -> Text<'static> {
    Text::from(icon).size(size as f32).color(color)
}

pub(crate) fn themed(icon: Icon, size: u16, palette: WallpaperPalette) -> Text<'static> {
    self::icon(icon, size, palette.text_color())
}
