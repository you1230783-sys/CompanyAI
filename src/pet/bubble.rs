//! 氣泡輪廓獨立於Win32文字繪製，以次像素取樣保留透明邊緣。
//! 回傳的alpha遮罩在GDI畫字後套用，避免文字把透明角落填成方框。
use super::Settings;

pub(super) fn color(hex: &str) -> [u8; 3] {
    let value = u32::from_str_radix(&hex[1..], 16).unwrap_or(0);
    [
        (value & 255) as u8,
        ((value >> 8) & 255) as u8,
        (value >> 16) as u8,
    ]
}

fn rounded(x: f32, y: f32, rect: [f32; 4], radius: f32) -> bool {
    let [left, top, right, bottom] = rect;
    let dx = x - x.clamp(left + radius, right - radius);
    let dy = y - y.clamp(top + radius, bottom - radius);
    x >= left && x <= right && y >= top && y <= bottom && dx * dx + dy * dy <= radius * radius
}

fn shape(x: f32, y: f32, style: &str) -> bool {
    let radius = match style {
        "comic" => 10.0,
        "thought" => 34.0,
        _ => 24.0,
    };
    let body = rounded(x, y, [6.0, 4.0, 314.0, 102.0], radius);
    let tail = if style == "thought" {
        [(263.0, 108.0, 7.0), (279.0, 118.0, 4.0)]
            .iter()
            .any(|(cx, cy, r)| (x - cx).powi(2) + (y - cy).powi(2) <= r * r)
    } else {
        (96.0..=121.0).contains(&y) && x >= 257.0 + (y - 96.0) * 0.72 && x <= 284.0
    };
    body || tail
}

/// 在320×128邏輯畫布上取樣；僅泡泡使用此透明度，角色仍保留原PNG透明度。
pub(super) fn paint(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    settings: &Settings,
) -> Vec<u8> {
    let mut mask = vec![0; width * height];
    let background = color(&settings.bubble_color);
    let ratio = width as f32 / 320.0;
    for y in 0..height {
        for x in 0..width {
            let mut coverage = 0;
            for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                coverage += u16::from(shape(
                    (x as f32 + sx) / ratio,
                    (y as f32 + sy) / ratio,
                    &settings.bubble_style,
                ));
            }
            let index = y * width + x;
            mask[index] =
                (u32::from(coverage) * 255 * u32::from(settings.bubble_opacity) / 400) as u8;
            let px = &mut pixels[index * 4..index * 4 + 4];
            let edge = settings.bubble_style == "comic"
                && coverage > 0
                && [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)]
                    .iter()
                    .any(|(dx, dy)| {
                        !shape(
                            x as f32 / ratio + dx,
                            y as f32 / ratio + dy,
                            &settings.bubble_style,
                        )
                    });
            for c in 0..3 {
                px[c] = if edge {
                    background[c].saturating_sub(65)
                } else {
                    background[c]
                };
            }
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_bubble_has_transparent_corners_tail_and_independent_opacity() {
        for style in ["round", "comic", "thought"] {
            let mut settings = Settings {
                bubble_style: style.into(),
                ..Default::default()
            };
            let mut pixels = vec![0; 320 * 128 * 4];
            let full = paint(&mut pixels, 320, 128, &settings);
            assert_eq!(full[0], 0);
            assert_eq!(full[50 * 320 + 160], 255);
            assert!(full[108 * 320 + if style == "thought" { 263 } else { 275 }] > 0);
            settings.bubble_opacity = 0;
            assert!(paint(&mut pixels, 320, 128, &settings)
                .iter()
                .all(|v| *v == 0));
        }
    }
}
