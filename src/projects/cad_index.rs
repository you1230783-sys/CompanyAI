//! CAD 快照的本機查詢。幾何由程式計算，語意名稱（例如固定孔）留給使用者確認。
//! 全部圖元不送入模型；候選編號只在同一 snapshot_id 內有效。
mod delivery;
pub mod preview;
use crate::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub kind: Option<String>,
    pub layer: Option<String>,
    pub color: Option<u64>,
    pub text: Option<String>,
    pub diameter: Option<f64>,
    pub tolerance: Option<f64>,
    /// 世界座標 XY 的 [左、下、右、上]；以可確認的幾何外接框相交判斷。
    pub region: Option<[f64; 4]>,
    pub handles: Option<Vec<String>>,
    pub candidates: Option<Vec<usize>>,
    /// 已識別矩形的編號；同時回傳圓心及圓外緣到四邊的距離。
    pub rectangle: Option<usize>,
}

#[derive(Clone)]
pub struct Snapshot {
    pub id: String,
    pub data: Value,
    pub rows: Vec<Value>,
    rectangles: Vec<Value>,
    contours: Vec<Value>,
    /// 只保留少量已交付的精確資料，避免整理工具歷史後又查相同圖元。
    recent_details: Vec<Value>,
    /// 全快照的已交付集合留在本機，不因常駐小視窗淘汰資料而遺忘。
    delivered_candidates: BTreeSet<usize>,
    repeated_detail_queries: usize,
}

pub fn point(value: &Value) -> Option<[f64; 3]> {
    let p = value.as_array()?;
    if p.len() != 3 {
        return None;
    }
    let p = [p[0].as_f64()?, p[1].as_f64()?, p[2].as_f64()?];
    p.iter()
        .all(|v| v.is_finite() && v.abs() <= 1e12)
        .then_some(p)
}

pub fn planar(row: &Value) -> bool {
    point(&row["properties"]["Normal"])
        .is_some_and(|p| p[0].abs() < 1e-9 && p[1].abs() < 1e-9 && (p[2] - 1.0).abs() < 1e-9)
}

pub fn vertices(row: &Value) -> Option<Vec<[f64; 3]>> {
    row["geometry"]["vertices"]
        .as_array()?
        .iter()
        .map(point)
        .collect()
}

/// 未讀出的曲線／圖塊範圍不猜測，避免把錨點誤當成整個物件的邊界。
pub fn bounds(row: &Value) -> Option<[f64; 4]> {
    let p = &row["properties"];
    match row["kind"].as_str()? {
        "circle" if planar(row) => {
            let c = point(&p["Center"])?;
            let r = p["Radius"]
                .as_f64()
                .filter(|v| v.is_finite() && *v > 0.0 && *v <= 1e12)?;
            Some([c[0] - r, c[1] - r, c[0] + r, c[1] + r])
        }
        "line" => extent(&[point(&p["StartPoint"])?, point(&p["EndPoint"])?]),
        "polyline" => extent(&vertices(row)?),
        _ => None,
    }
}

fn extent(points: &[[f64; 3]]) -> Option<[f64; 4]> {
    let first = points.first()?;
    let mut b = [first[0], first[1], first[0], first[1]];
    for p in points {
        b = [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ];
    }
    Some(b)
}

pub fn valid_region(b: &[f64; 4]) -> bool {
    b.iter().all(|v| v.is_finite() && v.abs() <= 1e12) && b[0] < b[2] && b[1] < b[3]
}

pub fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

/// 僅辨識四個直角、等高且平行世界 XY 軸的封閉輪廓；不把外接框當矩形。
fn rectangle(points: &[[f64; 3]]) -> Option<[f64; 4]> {
    if points.len() != 4 {
        return None;
    }
    let b = extent(points)?;
    if !valid_region(&b) {
        return None;
    }
    let mut corners = std::collections::BTreeSet::new();
    for (i, p) in points.iter().enumerate() {
        if (p[2] - points[0][2]).abs() > 1e-6 {
            return None;
        }
        let x = if (p[0] - b[0]).abs() < 1e-6 {
            0
        } else if (p[0] - b[2]).abs() < 1e-6 {
            1
        } else {
            return None;
        };
        let y = if (p[1] - b[1]).abs() < 1e-6 {
            0
        } else if (p[1] - b[3]).abs() < 1e-6 {
            1
        } else {
            return None;
        };
        corners.insert((x, y));
        let q = points[(i + 1) % 4];
        if ((p[0] - q[0]).abs() < 1e-6) == ((p[1] - q[1]).abs() < 1e-6) {
            return None;
        }
    }
    (corners.len() == 4).then_some(b)
}

/// 只描述閉合直線輪廓的外接框，不擅自命名零件或推算面積重心。
fn contour_info(points: &[[f64; 3]], handles: Vec<Value>) -> Option<Value> {
    if !(3..=64).contains(&points.len())
        || points.iter().any(|p| (p[2] - points[0][2]).abs() > 1e-6)
    {
        return None;
    }
    let b = extent(points)?;
    if !valid_region(&b) {
        return None;
    }
    Some(
        json!({"handles":handles,"vertex_count":points.len(),"bounds":b,
        "bounds_center":[(b[0]+b[2])/2.0,(b[1]+b[3])/2.0,points[0][2]],
        "width":b[2]-b[0],"height":b[3]-b[1],"z":points[0][2]}),
    )
}

impl Snapshot {
    /// 小量精確資料不靠 AI 摘寫，與快照同生共滅。超限移除整筆，
    /// 不截斷座標或字串，也不因移出常駐上下文而撤銷已讀 Handle。
    pub fn remember_details(&mut self, result: &Value) {
        let previous = self.delivered_candidates.len();
        for candidate in result["candidates"].as_array().into_iter().flatten() {
            let row = &candidate["entity"];
            if !row.is_object() {
                continue;
            }
            if let Some(index) = candidate["candidate"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .and_then(|n| n.checked_sub(1))
            {
                if self
                    .rows
                    .get(index)
                    .is_some_and(|original| original["handle"] == row["handle"])
                {
                    self.delivered_candidates.insert(index);
                }
            }
            let fact = json!({"candidate":candidate["candidate"],"handle":row["handle"],
                "kind":row["kind"],"editable":row["editable"],"allowed_edits":row["allowed_edits"],
                "properties":row["properties"],"geometry":row["geometry"]});
            self.recent_details
                .retain(|previous| previous["handle"] != fact["handle"]);
            if fact.to_string().len() > 8000 {
                continue;
            }
            self.recent_details.push(fact);
            while self.recent_details.len() > 10
                || serde_json::to_string(&self.recent_details).map_or(usize::MAX, |s| s.len())
                    > 8000
            {
                self.recent_details.remove(0);
            }
        }
        self.repeated_detail_queries = if self.delivered_candidates.len() == previous {
            self.repeated_detail_queries.saturating_add(1)
        } else {
            0
        };
    }

    pub fn recent_details(&self) -> &[Value] {
        &self.recent_details
    }

    pub fn new(mut data: Value) -> AppResult<Self> {
        let rows: Vec<Value> =
            serde_json::from_value(data["entities"].take()).map_err(|e| e.to_string())?;
        if rows.len() > 200_000 || data["entity_count"].as_u64() != Some(rows.len() as u64) {
            return Err("CAD 快照不完整，不能建立索引。".into());
        }
        let mut handles = std::collections::BTreeSet::new();
        for row in &rows {
            let h = row["handle"].as_str().ok_or("CAD 快照缺少 Handle。")?;
            if h.is_empty() || !handles.insert(h.to_owned()) {
                return Err("CAD 快照 Handle 重複或空白。".into());
            }
        }
        let mut rectangles = Vec::new();
        let mut contours = Vec::new();
        for row in &rows {
            if row["geometry"]["closed"] == true {
                if let Some(points) = vertices(row) {
                    if let Some(contour) = contour_info(&points, vec![row["handle"].clone()]) {
                        contours.push(contour);
                    }
                    if let Some(b) = rectangle(&points) {
                        rectangles.push(json!({"id":rectangles.len()+1,"handles":[row["handle"]],"bounds":b,"z":points[0][2],"width":b[2]-b[0],"height":b[3]-b[1]}));
                    }
                }
            }
        }
        // 獨立線亦可形成四邊或八邊等封閉輪廓。只走頂點度數為二的閉環，
        // 最多64段，不從雜亂交線猜外框，也不把外接框當成真正矩形。
        let key = |p: [f64; 3]| format!("{:.6},{:.6},{:.6}", p[0], p[1], p[2]);
        let mut links: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut lines = BTreeMap::new();
        for (i, row) in rows.iter().enumerate() {
            if row["kind"] != "line" {
                continue;
            }
            if let (Some(a), Some(b)) = (
                point(&row["properties"]["StartPoint"]),
                point(&row["properties"]["EndPoint"]),
            ) {
                links.entry(key(a)).or_default().push(i);
                links.entry(key(b)).or_default().push(i);
                lines.insert(i, (a, b));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for (&start, &(a, _)) in &lines {
            if seen.contains(&start) {
                continue;
            }
            let mut at = a;
            let mut edge = start;
            let mut ids = Vec::new();
            let mut points = Vec::new();
            for _ in 0..64 {
                if ids.contains(&edge) {
                    break;
                }
                ids.push(edge);
                points.push(at);
                let (p, q) = lines[&edge];
                at = if key(p) == key(at) { q } else { p };
                let Some(next) = links.get(&key(at)).filter(|v| v.len() == 2) else {
                    break;
                };
                edge = if next[0] == edge { next[1] } else { next[0] };
            }
            if ids.len() >= 3 && edge == start && key(at) == key(a) {
                seen.extend(ids.iter().copied());
                let handles = ids.iter().map(|i| rows[*i]["handle"].clone()).collect();
                if let Some(contour) = contour_info(&points, handles) {
                    contours.push(contour);
                }
                if let Some(b) = rectangle(&points) {
                    rectangles.push(json!({"id":rectangles.len()+1,"handles":ids.iter().map(|i|&rows[*i]["handle"]).collect::<Vec<_>>(),"bounds":b,"z":a[2],"width":b[2]-b[0],"height":b[3]-b[1]}));
                }
            }
        }
        Ok(Self {
            id: crate::jobs::new_id()?,
            data,
            rows,
            rectangles,
            contours,
            recent_details: Vec::new(),
            delivered_candidates: BTreeSet::new(),
            repeated_detail_queries: 0,
        })
    }

    pub fn overview(&self) -> Value {
        let mut kinds = BTreeMap::<String, usize>::new();
        let mut layers = BTreeMap::<String, usize>::new();
        let mut known = Vec::new();
        let mut unknown = 0;
        for row in &self.rows {
            *kinds
                .entry(row["kind"].as_str().unwrap_or("unknown").into())
                .or_default() += 1;
            *layers
                .entry(
                    row["properties"]["Layer"]
                        .as_str()
                        .unwrap_or("unknown")
                        .into(),
                )
                .or_default() += 1;
            if let Some(b) = bounds(row) {
                known.push([b[0], b[1], 0.0]);
                known.push([b[2], b[3], 0.0]);
            } else {
                unknown += 1;
            }
        }
        let mut descriptions: Vec<String> = self.contours.iter().take(8).enumerate().map(|(i,c)|
            format!("封閉直線輪廓 {}：{}邊，寬 {}、高 {}，WCS XY外接框 {}，外接框中心 {}（不是面積重心）。",
                i+1,c["vertex_count"],c["width"],c["height"],c["bounds"],c["bounds_center"])).collect();
        descriptions.extend(
            self.rectangles
                .iter()
                .take(8)
                .map(|r| {
                    format!(
                        "矩形 {}：寬 {}、高 {}，WCS XY 邊界 {}，所在高度 {}。",
                        r["id"], r["width"], r["height"], r["bounds"], r["z"]
                    )
                })
                .collect::<Vec<_>>(),
        );
        for (i, row) in self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r["kind"] == "circle")
            .take(8)
        {
            let (Some(c), Some(r)) = (
                point(&row["properties"]["Center"]),
                row["properties"]["Radius"].as_f64(),
            ) else {
                continue;
            };
            let mut description = format!(
                "候選 {} 為圓：圓心 ({}, {}, {})，直徑 {}。",
                i + 1,
                c[0],
                c[1],
                c[2],
                2.0 * r
            );
            // 僅在樣本內有唯一明確包含關係時描述距邊；不推論零件用途。
            let frames: Vec<_> = self
                .rectangles
                .iter()
                .take(20)
                .filter_map(|frame| {
                    let value = self.candidate(i, row, Some(frame), false);
                    (value["rectangle_relation"]["fully_inside"] == true)
                        .then_some(value["rectangle_relation"].clone())
                })
                .collect();
            if frames.len() == 1 {
                description.push_str(&format!(
                    " 位於矩形 {} 內；圓心到左／下／右／上邊距離 {}；圓外緣間隙 {}。",
                    frames[0]["rectangle"],
                    frames[0]["center_to_edge"],
                    frames[0]["circle_edge_clearance"]
                ));
            }
            descriptions.push(description);
        }
        json!({"snapshot_id":self.id,"revision":self.data["revision"],"document":self.data["document"],"source":self.data["source"],
            "selected":self.data["selected"],"entity_count":self.rows.len(),"units_code":self.data["units_code"],"edit_capabilities":self.data["edit_capabilities"],"coordinates":"WCS XY; X右/Y上；不是目前螢幕方向；距離為圖面單位，INSUNITS=4才是毫米",
            "kinds":kinds,"layers":layers.iter().take(50).collect::<BTreeMap<_,_>>(),"omitted_layers":layers.len().saturating_sub(50),
            "known_geometry_bounds":extent(&known),"entities_without_geometry_bounds":unknown,
            "rectangles":self.rectangles.iter().take(20).collect::<Vec<_>>(),"rectangle_count":self.rectangles.len(),
            "closed_contours":self.contours.iter().take(12).collect::<Vec<_>>(),"closed_contour_count":self.contours.len(),
            "geometry_summary":descriptions,"summary_scope":"只描述前8個封閉輪廓／矩形／圓，不代表整張圖。輪廓僅連通性與平面直線閉合，未驗證自交；外接框中心不是面積重心。距離是圖面單位，包含關係僅核對前20個矩形，不判斷用途。",
            "sample":self.rows.iter().enumerate().take(8).map(|(i,r)|self.candidate(i,r,None,false)).collect::<Vec<_>>(),
            "notice":"完整資料留在本機；以cad_query按條件或候選編號取得目標，不逐頁重讀。此為唯讀快照，不保證目前CAD未被手動修改；修改前仍重新核對全部可讀屬性。未讀取屬性、剖面、圖塊內容及外觀不可宣稱已驗證。"})
    }

    fn rectangle_info(&self, id: Option<usize>) -> AppResult<Option<&Value>> {
        id.map(|i| {
            self.rectangles
                .get(i.wrapping_sub(1))
                .ok_or("矩形編號不存在。".into())
        })
        .transpose()
    }

    fn candidate(&self, i: usize, row: &Value, frame: Option<&Value>, detail: bool) -> Value {
        let p = &row["properties"];
        let mut value = json!({"candidate":i+1,"handle":row["handle"],"kind":row["kind"],"layer":p["Layer"],"color":p["Color"],"bounds":bounds(row),
            "center":p["Center"],"diameter":if row["kind"]=="circle" {p["Radius"].as_f64().map(|r|r*2.0)} else {None},
            "text":p["TextString"].as_str().map(|s|s.chars().take(120).collect::<String>()),"editable":row["editable"]});
        if let Some(frame) = frame {
            if let (Some(c), Some(r), Some(b)) = (
                point(&p["Center"]),
                p["Radius"].as_f64(),
                frame["bounds"].as_array(),
            ) {
                if row["kind"] == "circle"
                    && planar(row)
                    && (c[2] - frame["z"].as_f64().unwrap_or(f64::NAN)).abs() < 1e-6
                {
                    let d = [
                        c[0] - b[0].as_f64().unwrap_or(0.0),
                        c[1] - b[1].as_f64().unwrap_or(0.0),
                        b[2].as_f64().unwrap_or(0.0) - c[0],
                        b[3].as_f64().unwrap_or(0.0) - c[1],
                    ];
                    value["rectangle_relation"] = json!({"rectangle":frame["id"],"edge_order":["left","bottom","right","top"],"center_to_edge":d,"circle_edge_clearance":d.map(|v|v-r),"fully_inside":d.iter().all(|v|*v>=r)});
                }
            }
        }
        if detail {
            value["entity"] = row.clone();
            // 原生DXF版本資料供本機修改前後逐項比對，不是模型的編輯參數。
            // 完整原件仍在self.rows；不在每次查詢重送這份大型內部資料。
            if let Some(entity) = value["entity"].as_object_mut() {
                entity.remove("native_revision");
            }
        }
        value
    }

    pub fn query(
        &self,
        filter: &Filter,
        offset: usize,
        limit: usize,
        detail: bool,
    ) -> AppResult<Value> {
        let tolerance = filter.tolerance.unwrap_or(1e-6);
        if !(1..=if detail { 10 } else { 50 }).contains(&limit)
            || filter.kind.as_ref().is_some_and(|kind| {
                !matches!(
                    kind.as_str(),
                    "line"
                        | "circle"
                        | "polyline"
                        | "arc"
                        | "dimension"
                        | "leader"
                        | "text"
                        | "mtext"
                        | "blockreference"
                        | "unknown"
                )
            })
            || !tolerance.is_finite()
            || !(0.0..=1e6).contains(&tolerance)
            || filter
                .diameter
                .is_some_and(|v| !v.is_finite() || v <= 0.0 || v > 1e12)
            || filter.region.is_some_and(|b| !valid_region(&b))
            || filter.color.is_some_and(|c| c > 256)
            || [&filter.kind, &filter.layer, &filter.text]
                .iter()
                .any(|s| s.as_ref().is_some_and(|v| v.len() > 2000))
            || filter
                .handles
                .as_ref()
                .is_some_and(|v| v.len() > 50 || v.iter().any(|s| s.len() > 64))
            || filter
                .candidates
                .as_ref()
                .is_some_and(|v| v.len() > 50 || v.iter().any(|i| *i == 0 || *i > self.rows.len()))
        {
            return Err("查詢範圍或數量不合法；候選每頁1–50，詳細資料每頁1–10。".into());
        }
        let frame = self.rectangle_info(filter.rectangle)?;
        let mut matches = Vec::new();
        let mut unavailable = 0;
        for (i, row) in self.rows.iter().enumerate() {
            let p = &row["properties"];
            // 屬性缺值不是不符合條件；單獨回報，避免把無法判斷說成已排除。
            if (filter.kind.as_ref().is_some_and(|k| k != "unknown") && row["kind"] == "unknown")
                || (filter.layer.is_some() && !p["Layer"].is_string())
                || (filter.color.is_some() && !p["Color"].is_number())
                || (filter.text.is_some()
                    && matches!(row["kind"].as_str(), Some("text" | "mtext"))
                    && !p["TextString"].is_string())
            {
                unavailable += 1;
                continue;
            }
            if filter
                .kind
                .as_ref()
                .is_some_and(|v| row["kind"].as_str() != Some(v.as_str()))
                || filter
                    .layer
                    .as_ref()
                    .is_some_and(|v| p["Layer"].as_str() != Some(v.as_str()))
                || filter.color.is_some_and(|v| p["Color"].as_u64() != Some(v))
                || filter.text.as_ref().is_some_and(|v| {
                    !p["TextString"]
                        .as_str()
                        .is_some_and(|s| s.to_lowercase().contains(&v.to_lowercase()))
                })
                || filter.handles.as_ref().is_some_and(|v| {
                    !v.iter().any(|h| {
                        row["handle"]
                            .as_str()
                            .is_some_and(|r| r.eq_ignore_ascii_case(h))
                    })
                })
                || filter
                    .candidates
                    .as_ref()
                    .is_some_and(|v| !v.contains(&(i + 1)))
            {
                continue;
            }
            if let Some(d) = filter.diameter {
                if row["kind"] != "circle" {
                    continue;
                }
                let Some(r) = p["Radius"].as_f64() else {
                    unavailable += 1;
                    continue;
                };
                if (2.0 * r - d).abs() > tolerance {
                    continue;
                }
            }
            if let Some(region) = filter.region {
                let Some(b) = bounds(row) else {
                    unavailable += 1;
                    continue;
                };
                if !intersects(b, region) {
                    continue;
                }
            }
            matches.push((i, row));
        }
        if offset > matches.len() {
            return Err("查詢 offset 超過符合數量。".into());
        }
        // 大型折線的詳細座標亦受總位元組限制；明示下一頁，絕不截掉單一圖元內容。
        let mut page = Vec::new();
        let mut page_bytes = 0;
        for (i, row) in matches.iter().skip(offset).take(limit) {
            let candidate = self.candidate(*i, row, frame, detail);
            let bytes = serde_json::to_vec(&candidate)
                .map_err(|e| e.to_string())?
                .len();
            if page_bytes + bytes > 100_000 {
                if page.is_empty() {
                    return Err(
                        "單一圖元詳細資料超過100KB；保留快照，不截斷資料或授權修改。".into(),
                    );
                }
                break;
            }
            page_bytes += bytes;
            page.push(candidate);
        }
        let end = offset + page.len();
        Ok(
            json!({"snapshot_id":self.id,"revision":self.data["revision"],"source":self.data["source"],"matched_count":matches.len(),"unavailable_for_filter":unavailable,
            "offset":offset,"next_offset":(end<matches.len()).then_some(end),"candidates":page,
            "notice":"候選編號綁定本快照；候選列表不是修改授權。修改前以details=true取回精確資料；有歧義請讓使用者選擇。區域為WCS XY外接框相交，不是精確碰撞檢查。"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(rows: Vec<Value>) -> Snapshot {
        Snapshot::new(
            json!({"entity_count":rows.len(),"entities":rows,"revision":"r","units_code":4}),
        )
        .unwrap()
    }

    #[test]
    fn isolated_eight_line_contour_exposes_bounds_without_becoming_rectangle() {
        let points = [
            [20., 0., 0.],
            [200., 0., 0.],
            [220., 20., 0.],
            [220., 120., 0.],
            [200., 140., 0.],
            [20., 140., 0.],
            [0., 120., 0.],
            [0., 20., 0.],
        ];
        let rows = (0..8)
            .map(|i| {
                json!({"handle":format!("{:X}",i+1),"kind":"line",
            "properties":{"StartPoint":points[i],"EndPoint":points[(i+1)%8],"Layer":"0"}})
            })
            .collect();
        let view = snapshot(rows).overview();
        assert_eq!(view["closed_contour_count"], 1);
        assert_eq!(view["closed_contours"][0]["vertex_count"], 8);
        assert_eq!(
            view["closed_contours"][0]["bounds_center"],
            json!([110., 70., 0.])
        );
        assert_eq!(view["rectangle_count"], 0);
    }

    #[test]
    fn details_remain_exact_bounded_and_do_not_survive_new_snapshot() {
        let mut state = snapshot(vec![json!({"handle":"A","kind":"circle","editable":true,
            "properties":{"Center":[110.,70.,0.],"Radius":30.,"Normal":[0.,0.,1.]}})]);
        let result = state.query(&Filter::default(), 0, 10, true).unwrap();
        state.remember_details(&result);
        state.remember_details(&result);
        assert_eq!(state.recent_details().len(), 1);
        assert_eq!(state.recent_details()[0]["properties"]["Radius"], 30.);
        let replacement = snapshot(state.rows.clone());
        assert!(replacement.recent_details().is_empty());
        for i in 0..30 {
            state.remember_details(&json!({"candidates":[{"candidate":i+1,"entity":{
                "handle":format!("H{i}"),"kind":"text","properties":{"TextString":"字".repeat(500)}}}]}));
        }
        assert!(state.recent_details().len() <= 10);
        assert!(serde_json::to_string(state.recent_details()).unwrap().len() <= 8000);
    }
}
