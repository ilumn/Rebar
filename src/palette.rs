use crate::config::PaletteMode;
use iced::Color;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy)]
pub(crate) struct WallpaperPalette {
    pub(crate) panel: [u8; 3],
    pub(crate) panel_alt: [u8; 3],
    pub(crate) accent: [u8; 3],
    pub(crate) accent_soft: [u8; 3],
    pub(crate) text: [u8; 3],
    pub(crate) muted_text: [u8; 3],
    pub(crate) border: [u8; 3],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PaletteVariants {
    pub(crate) balanced: WallpaperPalette,
    pub(crate) vibrant: WallpaperPalette,
    pub(crate) contrast: WallpaperPalette,
    pub(crate) center: WallpaperPalette,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WallpaperSignature {
    pub(crate) path: String,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) len: Option<u64>,
}

impl Default for WallpaperPalette {
    fn default() -> Self {
        Self {
            panel: [19, 24, 34],
            panel_alt: [26, 33, 47],
            accent: [108, 123, 255],
            accent_soft: [86, 99, 200],
            text: [244, 247, 255],
            muted_text: [196, 204, 220],
            border: [120, 132, 162],
        }
    }
}

impl Default for PaletteVariants {
    fn default() -> Self {
        let palette = WallpaperPalette::default();

        Self {
            balanced: palette,
            vibrant: palette,
            contrast: palette,
            center: palette,
        }
    }
}

impl PaletteVariants {
    pub(crate) fn select(self, mode: PaletteMode) -> WallpaperPalette {
        match mode {
            PaletteMode::Balanced => self.balanced,
            PaletteMode::Vibrant => self.vibrant,
            PaletteMode::Contrast => self.contrast,
            PaletteMode::Center => self.center,
        }
    }
}

impl WallpaperPalette {
    pub(crate) fn text_color(self) -> Color {
        Color::from_rgb8(self.text[0], self.text[1], self.text[2])
    }

    pub(crate) fn muted_text_color(self) -> Color {
        Color::from_rgb8(self.muted_text[0], self.muted_text[1], self.muted_text[2])
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn current_wallpaper_signature() -> Result<WallpaperSignature, String> {
    use std::fs;

    let path = wallpaper_path()?;
    Ok(wallpaper_signature(&path, fs::metadata(&path).ok()))
}

#[cfg(target_os = "windows")]
pub(crate) fn sample_palettes() -> Result<(WallpaperSignature, PaletteVariants), String> {
    use image::{ImageReader, imageops::FilterType};
    use std::fs;

    const SAMPLE_SIZE: u32 = 96;
    const HUE_BUCKETS: usize = 18;

    let path = wallpaper_path()?;
    let signature = wallpaper_signature(&path, fs::metadata(&path).ok());
    let image = ImageReader::open(&path)
        .map_err(|error| format!("Failed to open wallpaper {}: {error}", path.display()))?
        .decode()
        .map_err(|error| format!("Failed to decode wallpaper {}: {error}", path.display()))?;

    let pixels = image
        .resize_to_fill(SAMPLE_SIZE, SAMPLE_SIZE, FilterType::Triangle)
        .to_rgb8();

    let mut avg_sum = [0.0; 3];
    let mut avg_weight = 0.0;
    let mut dark_sum = [0.0; 3];
    let mut dark_weight = 0.0;
    let mut balanced_buckets = [Bucket::default(); HUE_BUCKETS];
    let mut vibrant_buckets = [Bucket::default(); HUE_BUCKETS];
    let mut center_buckets = [Bucket::default(); HUE_BUCKETS];

    for (x, y, pixel) in pixels.enumerate_pixels() {
        let rgb = [
            pixel[0] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[2] as f32 / 255.0,
        ];
        let (hue, saturation, value) = rgb_to_hsv(rgb);
        let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];

        let weight = 0.4 + saturation * 1.2 + (1.0 - (luma - 0.45).abs()).max(0.0);
        accumulate(&mut avg_sum, &mut avg_weight, rgb, weight);

        let dark_tilt = 0.5 + (1.0 - luma) * 1.1;
        accumulate(&mut dark_sum, &mut dark_weight, rgb, dark_tilt);

        if saturation > 0.10 {
            let bucket_index = ((hue * HUE_BUCKETS as f32).floor() as usize) % HUE_BUCKETS;
            let accent_weight = saturation.powf(1.4) * (0.5 + (1.0 - (value - 0.55).abs()));
            let vibrant_weight = saturation.powf(2.2) * (0.35 + value).powf(1.2);
            let center_weight = accent_weight * center_bias(x, y, SAMPLE_SIZE);

            accumulate(
                &mut balanced_buckets[bucket_index].sum,
                &mut balanced_buckets[bucket_index].weight,
                rgb,
                accent_weight,
            );
            accumulate(
                &mut vibrant_buckets[bucket_index].sum,
                &mut vibrant_buckets[bucket_index].weight,
                rgb,
                vibrant_weight,
            );
            accumulate(
                &mut center_buckets[bucket_index].sum,
                &mut center_buckets[bucket_index].weight,
                rgb,
                center_weight,
            );
        }
    }

    if avg_weight <= f32::EPSILON {
        return Ok((signature, PaletteVariants::default()));
    }

    let average = normalize(avg_sum, avg_weight);
    let dark_average = normalize(dark_sum, dark_weight.max(1.0));
    let balanced_accent = dominant_accent(&balanced_buckets, average);
    let vibrant_accent = dominant_accent(&vibrant_buckets, balanced_accent);
    let contrast_accent = contrast_accent(&balanced_buckets, average, balanced_accent);
    let center_accent = dominant_accent(&center_buckets, balanced_accent);

    let palettes = PaletteVariants {
        balanced: derive_palette(average, dark_average, balanced_accent, 0.20, 1.24, 0.56),
        vibrant: derive_palette(average, dark_average, vibrant_accent, 0.18, 1.46, 0.64),
        contrast: derive_palette(average, dark_average, contrast_accent, 0.24, 1.34, 0.60),
        center: derive_palette(average, dark_average, center_accent, 0.22, 1.30, 0.58),
    };

    Ok((signature, palettes))
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn current_wallpaper_signature() -> Result<WallpaperSignature, String> {
    Ok(WallpaperSignature {
        path: String::new(),
        modified: None,
        len: None,
    })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn sample_palettes() -> Result<(WallpaperSignature, PaletteVariants), String> {
    Ok((current_wallpaper_signature()?, PaletteVariants::default()))
}

#[cfg(target_os = "windows")]
fn wallpaper_path() -> Result<std::path::PathBuf, String> {
    use windows::Win32::UI::WindowsAndMessaging::{SPI_GETDESKWALLPAPER, SystemParametersInfoW};

    let mut buffer = [0u16; 260];
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buffer.len() as u32,
            Some(buffer.as_mut_ptr() as *mut _),
            Default::default(),
        )
    };

    if ok.is_err() {
        return Err(String::from("Failed to read desktop wallpaper path."));
    }

    let end = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());

    if end == 0 {
        return Err(String::from("Desktop wallpaper path was empty."));
    }

    Ok(std::path::PathBuf::from(String::from_utf16_lossy(
        &buffer[..end],
    )))
}

#[cfg(target_os = "windows")]
fn wallpaper_signature(
    path: &std::path::Path,
    metadata: Option<std::fs::Metadata>,
) -> WallpaperSignature {
    WallpaperSignature {
        path: path.display().to_string(),
        modified: metadata.as_ref().and_then(|metadata| metadata.modified().ok()),
        len: metadata.as_ref().map(|metadata| metadata.len()),
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Default)]
struct Bucket {
    sum: [f32; 3],
    weight: f32,
}

#[cfg(target_os = "windows")]
fn dominant_accent<const HUE_BUCKETS: usize>(
    buckets: &[Bucket; HUE_BUCKETS],
    fallback: [f32; 3],
) -> [f32; 3] {
    buckets
        .iter()
        .max_by(|left, right| left.weight.total_cmp(&right.weight))
        .filter(|bucket| bucket.weight > 0.01)
        .map(|bucket| normalize(bucket.sum, bucket.weight))
        .unwrap_or_else(|| fallback)
}

#[cfg(target_os = "windows")]
fn contrast_accent<const HUE_BUCKETS: usize>(
    buckets: &[Bucket; HUE_BUCKETS],
    average: [f32; 3],
    fallback: [f32; 3],
) -> [f32; 3] {
    buckets
        .iter()
        .filter(|bucket| bucket.weight > 0.01)
        .max_by(|left, right| {
            let left_rgb = normalize(left.sum, left.weight);
            let right_rgb = normalize(right.sum, right.weight);
            let left_score = left.weight * color_distance(left_rgb, average).powf(1.35);
            let right_score = right.weight * color_distance(right_rgb, average).powf(1.35);
            left_score.total_cmp(&right_score)
        })
        .map(|bucket| normalize(bucket.sum, bucket.weight))
        .unwrap_or(fallback)
}

#[cfg(target_os = "windows")]
fn rgb_to_hsv(rgb: [f32; 3]) -> (f32, f32, f32) {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    let delta = max - min;

    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - rgb[0]).abs() <= f32::EPSILON {
        ((rgb[1] - rgb[2]) / delta).rem_euclid(6.0) / 6.0
    } else if (max - rgb[1]).abs() <= f32::EPSILON {
        (((rgb[2] - rgb[0]) / delta) + 2.0) / 6.0
    } else {
        (((rgb[0] - rgb[1]) / delta) + 4.0) / 6.0
    };

    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue, saturation, max)
}

#[cfg(target_os = "windows")]
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let sector = (hue * 6.0).floor();
    let fraction = hue * 6.0 - sector;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - fraction * saturation);
    let t = value * (1.0 - (1.0 - fraction) * saturation);

    match sector as i32 % 6 {
        0 => [value, t, p],
        1 => [q, value, p],
        2 => [p, value, t],
        3 => [p, q, value],
        4 => [t, p, value],
        _ => [value, p, q],
    }
}

#[cfg(target_os = "windows")]
fn accumulate(sum: &mut [f32; 3], total_weight: &mut f32, rgb: [f32; 3], weight: f32) {
    sum[0] += rgb[0] * weight;
    sum[1] += rgb[1] * weight;
    sum[2] += rgb[2] * weight;
    *total_weight += weight;
}

#[cfg(target_os = "windows")]
fn normalize(sum: [f32; 3], weight: f32) -> [f32; 3] {
    [
        (sum[0] / weight).clamp(0.0, 1.0),
        (sum[1] / weight).clamp(0.0, 1.0),
        (sum[2] / weight).clamp(0.0, 1.0),
    ]
}

#[cfg(target_os = "windows")]
fn mix(a: [f32; 3], b: [f32; 3], amount: f32) -> [f32; 3] {
    let amount = amount.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * amount,
        a[1] + (b[1] - a[1]) * amount,
        a[2] + (b[2] - a[2]) * amount,
    ]
}

#[cfg(target_os = "windows")]
fn darken(rgb: [f32; 3], amount: f32) -> [f32; 3] {
    [
        (rgb[0] * amount).clamp(0.0, 1.0),
        (rgb[1] * amount).clamp(0.0, 1.0),
        (rgb[2] * amount).clamp(0.0, 1.0),
    ]
}

#[cfg(target_os = "windows")]
fn saturate_and_balance(rgb: [f32; 3], saturation_scale: f32, target_value: f32) -> [f32; 3] {
    let (h, s, v) = rgb_to_hsv(rgb);
    hsv_to_rgb(
        h,
        (s * saturation_scale).clamp(0.18, 0.92),
        target_value.max(v * 0.92).clamp(0.34, 0.82),
    )
}

#[cfg(target_os = "windows")]
fn choose_text(panel: [f32; 3]) -> [f32; 3] {
    let luma = 0.2126 * panel[0] + 0.7152 * panel[1] + 0.0722 * panel[2];
    if luma > 0.54 {
        [0.08, 0.10, 0.14]
    } else {
        [0.96, 0.97, 1.0]
    }
}

#[cfg(target_os = "windows")]
fn to_rgb8(rgb: [f32; 3]) -> [u8; 3] {
    [
        (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

#[cfg(target_os = "windows")]
fn derive_palette(
    average: [f32; 3],
    dark_average: [f32; 3],
    accent: [f32; 3],
    panel_mix: f32,
    accent_scale: f32,
    target_value: f32,
) -> WallpaperPalette {
    let panel = darken(mix(dark_average, average, panel_mix), 0.84);
    let panel_alt = darken(mix(panel, accent, 0.16), 0.68);
    let accent = saturate_and_balance(accent, accent_scale, target_value);
    let accent_soft = mix(accent, panel_alt, 0.34);
    let border = mix(accent, [0.92, 0.94, 1.0], 0.38);
    let text = choose_text(panel);
    let muted_text = mix(text, border, 0.36);

    WallpaperPalette {
        panel: to_rgb8(panel),
        panel_alt: to_rgb8(panel_alt),
        accent: to_rgb8(accent),
        accent_soft: to_rgb8(accent_soft),
        text: to_rgb8(text),
        muted_text: to_rgb8(muted_text),
        border: to_rgb8(border),
    }
}

#[cfg(target_os = "windows")]
fn center_bias(x: u32, y: u32, size: u32) -> f32 {
    let center = (size as f32 - 1.0) * 0.5;
    let dx = (x as f32 - center).abs() / center.max(1.0);
    let dy = (y as f32 - center).abs() / center.max(1.0);
    let distance = (dx * dx + dy * dy).sqrt().clamp(0.0, 1.4);
    1.25 - (distance / 1.4) * 0.85
}

#[cfg(target_os = "windows")]
fn color_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    (dr * dr + dg * dg + db * db).sqrt()
}
