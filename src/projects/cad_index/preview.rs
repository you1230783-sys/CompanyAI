//! 以快照生成局部幾何示意，不擷取桌面、不縮放 CAD，也不假裝已繪出未知圖元。
//! 固定尺寸 BMP 交給既有 Windows WIC 轉成 JPG，避免新增繪圖或編碼依賴。
use super::{bounds, intersects, point, valid_region, vertices, Snapshot};
use crate::AppResult;
use serde_json::{json, Value};

const W: usize = 960;
const H: usize = 640;
struct Canvas {
    pixels: Vec<u8>,
    region: [f64; 4],
    scale: f64,
    left: f64,
    top: f64,
    labels: Vec<[i32; 4]>,
}
impl Canvas {
    fn new(region: [f64; 4]) -> Self {
        let scale = ((W - 64) as f64 / (region[2] - region[0]))
            .min((H - 64) as f64 / (region[3] - region[1]));
        Self {
            pixels: vec![255; W * H * 3],
            region,
            scale,
            left: (W as f64 - (region[2] - region[0]) * scale) / 2.0,
            top: (H as f64 - (region[3] - region[1]) * scale) / 2.0,
            labels: Vec::new(),
        }
    }
    fn project(&self, p: [f64; 3]) -> [f64; 2] {
        [
            self.left + (p[0] - self.region[0]) * self.scale,
            self.top + (self.region[3] - p[1]) * self.scale,
        ]
    }
    fn pixel(&mut self, x: i32, y: i32, color: [u8; 3]) {
        if x >= 0 && y >= 0 && x < W as i32 && y < H as i32 {
            let i = (y as usize * W + x as usize) * 3;
            self.pixels[i..i + 3].copy_from_slice(&color);
        }
    }
    fn line(&mut self, a: [f64; 3], b: [f64; 3], color: [u8; 3]) {
        let a = self.project(a);
        let b = self.project(b);
        if !a.iter().chain(b.iter()).all(|v| v.is_finite()) {
            return;
        }
        // Liang–Barsky 裁切後才取樣；極遠的 CAD 座標不會產生無界迴圈。
        let d = [b[0] - a[0], b[1] - a[1]];
        let mut lo = 0.0_f64;
        let mut hi = 1.0_f64;
        for (p, q) in [
            (-d[0], a[0]),
            (d[0], W as f64 - 1.0 - a[0]),
            (-d[1], a[1]),
            (d[1], H as f64 - 1.0 - a[1]),
        ] {
            if p.abs() < 1e-12 {
                if q < 0.0 {
                    return;
                }
            } else {
                let t = q / p;
                if p < 0.0 {
                    lo = lo.max(t);
                } else {
                    hi = hi.min(t);
                }
                if lo > hi {
                    return;
                }
            }
        }
        let start = [a[0] + lo * d[0], a[1] + lo * d[1]];
        let end = [a[0] + hi * d[0], a[1] + hi * d[1]];
        let steps = ((end[0] - start[0])
            .abs()
            .max((end[1] - start[1]).abs())
            .ceil() as usize)
            .clamp(1, W + H);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            self.pixel(
                (start[0] + (end[0] - start[0]) * t).round() as i32,
                (start[1] + (end[1] - start[1]) * t).round() as i32,
                color,
            );
        }
    }
    fn number(&mut self, id: usize, p: [f64; 3]) {
        const DIGITS: [&str; 10] = [
            "111101101101111",
            "010110010010111",
            "111001111100111",
            "111001111001111",
            "101101111001001",
            "111100111001111",
            "111100111101111",
            "111001001001001",
            "111101111101111",
            "111101111001111",
        ];
        let at = self.project(p);
        let text = id.to_string();
        let x = (at[0] as i32 + 5).clamp(2, W as i32 - text.len() as i32 * 12 - 4);
        let preferred_y = (at[1] as i32 - 19).clamp(2, H as i32 - 20);
        let width = text.len() as i32 * 12;
        // 重疊圓／相鄰孔的號碼仍可辨認；移開的號碼用細線連回其幾何中心。
        let y = std::iter::once(preferred_y)
            .chain((0..30).map(|row| 2 + row * 20))
            .find(|y| {
                let b = [x - 2, *y - 2, x + width, *y + 17];
                self.labels
                    .iter()
                    .all(|a| a[2] < b[0] || a[0] > b[2] || a[3] < b[1] || a[1] > b[3])
            })
            .unwrap_or(preferred_y);
        self.labels.push([x - 2, y - 2, x + width, y + 17]);
        self.line(
            p,
            [
                self.region[0] + (x as f64 - self.left) / self.scale,
                self.region[3] - (y as f64 - self.top) / self.scale,
                0.0,
            ],
            [80, 80, 180],
        );
        for dy in -2..17 {
            for dx in -2..text.len() as i32 * 12 {
                self.pixel(x + dx, y + dy, [240, 255, 255]);
            }
        }
        for (index, ch) in text.bytes().enumerate() {
            for (i, bit) in DIGITS[(ch - b'0') as usize].bytes().enumerate() {
                if bit == b'1' {
                    for dy in 0..3 {
                        for dx in 0..3 {
                            self.pixel(
                                x + index as i32 * 12 + (i % 3) as i32 * 3 + dx,
                                y + (i / 3) as i32 * 3 + dy,
                                [0, 0, 180],
                            );
                        }
                    }
                }
            }
        }
    }
    fn bmp(self) -> Vec<u8> {
        let mut bytes = vec![0; 54];
        bytes[0..2].copy_from_slice(b"BM");
        bytes[2..6].copy_from_slice(&((54 + self.pixels.len()) as u32).to_le_bytes());
        bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
        bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
        bytes[18..22].copy_from_slice(&(W as i32).to_le_bytes());
        bytes[22..26].copy_from_slice(&(-(H as i32)).to_le_bytes());
        bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
        bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
        bytes.extend(self.pixels);
        bytes
    }
}

/// 候選至多20個；其餘附近圖元灰色顯示，遺漏範圍明確列出。
pub fn render(
    snapshot: &Snapshot,
    ids: &[usize],
    region: Option<[f64; 4]>,
) -> AppResult<(Vec<u8>, Value)> {
    if ids.is_empty()
        || ids.len() > 20
        || ids.iter().any(|i| *i == 0 || *i > snapshot.rows.len())
        || ids.iter().collect::<std::collections::BTreeSet<_>>().len() != ids.len()
    {
        return Err("局部預覽需要1–20個不重複且有效的候選編號。".into());
    }
    let mut selected_bounds = Vec::new();
    for id in ids {
        selected_bounds.push(
            bounds(&snapshot.rows[id - 1])
                .ok_or("所選圖元沒有可繪製的精確幾何；請使用文字資料定位。")?,
        );
    }
    let area = if let Some(area) = region {
        if !valid_region(&area)
            || selected_bounds
                .iter()
                .any(|b| b[0] < area[0] || b[1] < area[1] || b[2] > area[2] || b[3] > area[3])
        {
            return Err("預覽範圍需完整涵蓋所選候選，避免裁掉目標造成誤認。".into());
        }
        area
    } else {
        let mut b = selected_bounds[0];
        for v in &selected_bounds {
            b = [
                b[0].min(v[0]),
                b[1].min(v[1]),
                b[2].max(v[2]),
                b[3].max(v[3]),
            ];
        }
        let pad = ((b[2] - b[0]).max(b[3] - b[1]) * 0.3).max(1.0);
        [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad]
    };
    if !valid_region(&area) {
        return Err("預覽範圍超過支援界線。".into());
    }
    let mut canvas = Canvas::new(area);
    let mut drawn = 0;
    let mut omitted = 0;
    let mut background_segments = 0;
    // 先背景再候選；即使背景數量過多，也不省略使用者正在辨認的目標。
    for selected in [false, true] {
        for (i, row) in snapshot.rows.iter().enumerate() {
            if ids.contains(&(i + 1)) != selected {
                continue;
            }
            let Some(b) = bounds(row) else {
                if !selected {
                    omitted += 1;
                }
                continue;
            };
            if !intersects(b, area) {
                continue;
            }
            if !selected && drawn >= 2000 {
                omitted += 1;
                continue;
            }
            let segments = match row["kind"].as_str() {
                Some("circle") => 180,
                Some("polyline") => row["geometry"]["vertices"].as_array().map_or(0, Vec::len),
                _ => 1,
            };
            if !selected {
                if background_segments + segments > 20_000 {
                    omitted += 1;
                    continue;
                }
                background_segments += segments;
            }
            let color = if selected {
                [0, 0, 200]
            } else {
                [140, 140, 140]
            };
            let p = &row["properties"];
            match row["kind"].as_str() {
                Some("line") => canvas.line(
                    point(&p["StartPoint"]).ok_or("缺少線端點")?,
                    point(&p["EndPoint"]).ok_or("缺少線端點")?,
                    color,
                ),
                Some("circle") => {
                    let c = point(&p["Center"]).ok_or("缺少圓心")?;
                    let r = p["Radius"].as_f64().ok_or("缺少半徑")?;
                    for step in 0..180 {
                        let a = step as f64 * std::f64::consts::TAU / 180.0;
                        let b = (step + 1) as f64 * std::f64::consts::TAU / 180.0;
                        canvas.line(
                            [c[0] + r * a.cos(), c[1] + r * a.sin(), c[2]],
                            [c[0] + r * b.cos(), c[1] + r * b.sin(), c[2]],
                            color,
                        );
                    }
                }
                Some("polyline") => {
                    let v = vertices(row).ok_or("缺少折線頂點")?;
                    if v.len() > 2048 {
                        if selected {
                            return Err("候選折線過大，請以文字定位。".into());
                        }
                        omitted += 1;
                        continue;
                    }
                    for segment in v.windows(2) {
                        canvas.line(segment[0], segment[1], color);
                    }
                    if row["geometry"]["closed"] == true && v.len() > 1 {
                        canvas.line(v[v.len() - 1], v[0], color);
                    }
                }
                _ => {
                    omitted += 1;
                    continue;
                }
            }
            drawn += 1;
        }
    }
    for (id, b) in ids.iter().zip(selected_bounds) {
        canvas.number(*id, [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0, 0.0]);
    }
    Ok((
        canvas.bmp(),
        json!({"region":area,"width":W,"height":H,"drawn":drawn,"omitted_or_unsupported":omitted,
        "notice":"WCS XY幾何示意，X右/Y上；紅色與數字表示候選，灰色為周邊。非CAD截圖，不呈現原始顏色、線型、字型、尺寸、剖面或圖塊內容；取樣圓周僅供預覽，不能量測。"}),
    ))
}
