//! 同一快照的讀取進度與常駐精確幾何。這裡只記「已交付」，不自稱需求驗證通過。
use super::*;

const MAX_CONTEXT_FACTS: usize = 64;
const MAX_CONTEXT_BYTES: usize = 8_000;

fn basic_geometry(kind: &Value) -> bool {
    matches!(kind.as_str(), Some("line" | "circle" | "arc" | "polyline"))
}

impl Snapshot {
    pub fn delivered_count(&self) -> usize {
        self.delivered_candidates.len()
    }

    /// 只送種類計數，不把十萬個Handle加入每輪請求。所有候選的已讀集合仍留本機。
    pub fn delivery_status(&self) -> Value {
        let mut kinds = BTreeMap::<String, (usize, usize)>::new();
        let mut basic_total = 0;
        let mut basic_read = 0;
        for (index, row) in self.rows.iter().enumerate() {
            let counts = kinds
                .entry(row["kind"].as_str().unwrap_or("unknown").to_owned())
                .or_default();
            counts.0 += 1;
            let delivered = self.delivered_candidates.contains(&index);
            if delivered {
                counts.1 += 1;
            }
            if basic_geometry(&row["kind"]) {
                basic_total += 1;
                basic_read += usize::from(delivered);
            }
        }
        let all_basic = basic_total > 0 && basic_read == basic_total;
        json!({"revision":self.data["revision"],"total_entities":self.rows.len(),
            "details_delivered":self.delivered_candidates.len(),"basic_geometry_total":basic_total,
            "basic_geometry_details_delivered":basic_read,"all_basic_geometry_details_delivered":all_basic,
            "kinds":kinds.into_iter().map(|(kind,(total,read))|json!({"kind":kind,"total":total,"details_delivered":read,"not_delivered":total-read})).collect::<Vec<_>>(),
            "consecutive_queries_without_new_details":self.repeated_detail_queries,
            "meaning":"已交付不等於已核對使用者規格，文字／尺寸也不能由基本幾何數量推定通過。",
            "next_action":if all_basic {
                "全部基本幾何詳細資料已交付；沿用本版本的精確資料與已保存結論。若需求已核對，直接finish；不要為重寫逐步報告再查圓、線或弧。真正缺少的文字／尺寸或外觀才另查，未確認項目如實列出。"
            } else if self.repeated_detail_queries > 0 {
                "本次沒有新增已讀圖元；請沿用同版本已讀結果。只有需要未提供的屬性、來源變更或出現矛盾才再查相同目標；原文移出歷史不是必須重查的理由。"
            } else { "只查與需求相關且尚未交付的目標，不強制讀完整張圖。核對完成即finish。" }})
    }

    /// 簡單線／圓／弧／多段線保留完整座標，不被尺寸或DXF內部欄位擠出。
    /// 大多段線超出預算時整筆略過並明示，不截斷頂點造成虛假的完整幾何。
    pub fn delivered_context(&self) -> Value {
        let mut facts = Vec::new();
        let mut bytes = 2;
        let mut omitted = 0;
        for index in &self.delivered_candidates {
            if facts.len() == MAX_CONTEXT_FACTS {
                // 事實上限已滿後只計數，不再為其餘大型幾何建立JSON投影。
                omitted = self.delivered_candidates.len() - facts.len();
                break;
            }
            let row = &self.rows[*index];
            let mut properties = serde_json::Map::new();
            for key in [
                "StartPoint",
                "EndPoint",
                "Center",
                "Radius",
                "StartAngle",
                "EndAngle",
                "Normal",
                "Layer",
                "Color",
                "Linetype",
                "LinetypeScale",
                "Thickness",
                "TextString",
                "Height",
                "Measurement",
                "TextOverride",
                "InsertionPoint",
                "Rotation",
            ] {
                if let Some(value) = row["properties"].get(key) {
                    properties.insert(key.to_owned(), value.clone());
                }
            }
            let fact = json!({"candidate":index+1,"handle":row["handle"],"kind":row["kind"],
                "editable":row["editable"],"allowed_edits":row["allowed_edits"],"properties":properties,
                "geometry":row["geometry"],"dimension_geometry":row["dimension_geometry"]});
            let length = fact.to_string().len() + 1;
            if bytes + length > MAX_CONTEXT_BYTES {
                omitted += 1;
                continue;
            }
            bytes += length;
            facts.push(fact);
        }
        json!({"facts":facts,"omitted_read_entities":omitted,
            "properties_scope":"只投影列出的精確屬性；geometry頂點不截斷。其他屬性按需要查cad_query。完整資料與已讀集合仍在本機；未列出不表示未讀過。",
            "coverage":self.delivery_status()})
    }

    /// 只改送出投影：已在本輪常駐事實中的同版逐字資料，不再於歷史重送。
    /// 每個欄位均先比對，未常駐的屬性照留；原始工具結果與工作日誌不改寫。
    pub(crate) fn project_delivered_reads(
        messages: &mut [crate::projects::agent::Message],
        context: &Value,
    ) {
        let Some(snapshot_id) = context["snapshot_id"].as_str() else {
            return;
        };
        let facts: BTreeMap<_, _> = context["recent_details"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|fact| fact["handle"].as_str().map(|handle| (handle, fact)))
            .collect();
        for message in messages.iter_mut().filter(|m| m.role == "tool") {
            let Ok(mut envelope) = serde_json::from_str::<Value>(&message.content) else {
                continue;
            };
            if envelope["ok"] != true
                || envelope["result"]["snapshot_id"] != snapshot_id
                || envelope["result"]["revision"] != context["revision"]
            {
                continue;
            }
            let Some(candidates) = envelope["result"]["candidates"].as_array_mut() else {
                continue;
            };
            for candidate in candidates {
                let Some(fact) = candidate["handle"].as_str().and_then(|h| facts.get(h)) else {
                    continue;
                };
                let Some(entity) = candidate["entity"].as_object_mut() else {
                    continue;
                };
                let mut referenced = false;
                if let (Some(properties), Some(saved)) = (
                    entity.get_mut("properties").and_then(Value::as_object_mut),
                    fact["properties"].as_object(),
                ) {
                    for (key, value) in saved {
                        if properties.get(key) == Some(value) {
                            properties.remove(key);
                            referenced = true;
                        }
                    }
                }
                if referenced {
                    if entity["properties"]
                        .as_object()
                        .is_some_and(|p| p.is_empty())
                    {
                        entity.remove("properties");
                    }
                    entity.insert("properties_ref".into(), json!(format!("本機CAD工作狀態.recent_details中handle={}的properties；其餘未參照屬性仍列於此結果。", fact["handle"].as_str().unwrap_or(""))));
                }
                if !fact["geometry"].is_null() && entity.get("geometry") == Some(&fact["geometry"])
                {
                    entity.remove("geometry");
                    entity.insert(
                        "geometry_ref".into(),
                        json!(format!(
                            "本機CAD工作狀態.recent_details中handle={}的完整geometry。",
                            fact["handle"].as_str().unwrap_or("")
                        )),
                    );
                }
            }
            message.content = envelope.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawing() -> Snapshot {
        let rows: Vec<_> = (0..15).map(|n| json!({"handle":format!("{:X}",n+1),"kind":"circle",
            "editable":true,"allowed_edits":["offset"],"native_revision":[[5,"internal DXF"]],
            "properties":{"Center":[n,0,0],"Radius":10,"Normal":[0,0,1],"Layer":"0","Extra":"keep"}})).collect();
        Snapshot::new(json!({"entity_count":rows.len(),"entities":rows,"revision":"r"})).unwrap()
    }

    #[test]
    fn details_survive_the_old_ten_item_window_and_detect_no_new_reads() {
        let mut snapshot = drawing();
        for offset in [0, 10] {
            let page = snapshot
                .query(&Filter::default(), offset, 10, true)
                .unwrap();
            assert!(page["candidates"][0]["entity"]
                .get("native_revision")
                .is_none());
            snapshot.remember_details(&page);
        }
        assert_eq!(snapshot.delivered_count(), 15);
        assert!(snapshot.recent_details().len() <= 10);
        let context = snapshot.delivered_context();
        assert_eq!(context["facts"].as_array().unwrap().len(), 15);
        assert!(context["facts"].to_string().len() <= MAX_CONTEXT_BYTES);
        assert_eq!(
            context["coverage"]["all_basic_geometry_details_delivered"],
            true
        );
        let repeat = snapshot.query(&Filter::default(), 0, 10, true).unwrap();
        snapshot.remember_details(&repeat);
        assert_eq!(
            snapshot.delivery_status()["consecutive_queries_without_new_details"],
            1
        );
        assert!(snapshot.rows[0].get("native_revision").is_some());
        assert_eq!(drawing().delivered_count(), 0);
    }

    #[test]
    fn outgoing_projection_only_deduplicates_equal_fields_of_the_same_revision() {
        let mut snapshot = drawing();
        let page = snapshot.query(&Filter::default(), 0, 10, true).unwrap();
        snapshot.remember_details(&page);
        let context = json!({"snapshot_id":snapshot.id,"revision":"r","recent_details":snapshot.delivered_context()["facts"]});
        let original = json!({"ok":true,"result":page});
        let mut messages = vec![crate::projects::agent::Message {
            role: "tool".into(),
            content: original.to_string(),
            tool_calls: vec![],
            tool_call_id: Some("call".into()),
        }];
        Snapshot::project_delivered_reads(&mut messages, &context);
        let projected: Value = serde_json::from_str(&messages[0].content).unwrap();
        let entity = &projected["result"]["candidates"][0]["entity"];
        assert!(entity["properties_ref"].is_string());
        assert_eq!(entity["properties"]["Extra"], "keep");
        assert_eq!(
            original["result"]["candidates"][0]["entity"]["properties"]["Radius"],
            10
        );
        messages[0].content = original.to_string();
        let mut other = context;
        other["revision"] = json!("different");
        Snapshot::project_delivered_reads(&mut messages, &other);
        assert_eq!(messages[0].content, original.to_string());
    }

    #[test]
    fn oversized_geometry_is_omitted_whole_and_not_called_unread() {
        let mut snapshot = Snapshot::new(json!({"entity_count":1,"revision":"r","entities":[{
            "handle":"A","kind":"polyline","geometry":{"vertices":vec![[1,2,0];4000]}}]}))
        .unwrap();
        let page = snapshot.query(&Filter::default(), 0, 10, true).unwrap();
        snapshot.remember_details(&page);
        let context = snapshot.delivered_context();
        assert_eq!(context["facts"], json!([]));
        assert_eq!(context["omitted_read_entities"], 1);
        assert_eq!(context["coverage"]["details_delivered"], 1);
    }
}
