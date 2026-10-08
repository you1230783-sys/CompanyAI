//! 圖表偏好與一般補充指示共用加密收件匣；讀資料的 worker 不等待可選問題。
use super::*;
use crate::projects::charts::quality::{Choice, Policy, Prepared};

impl Broker {
    /// 只有可選偏好入口可以延後此圖並繼續；舊 chooser 的取消仍保留可續接狀態。
    pub(super) fn deferred_chart(&self) -> Value {
        if self.preferences.is_none() {
            return json!({"waiting_for_user":true});
        }
        json!({"deferred":true,"chart_created":false,"notice":"無效 X 座標尚未取得排除決策，先處理其他工作；收到回答後可重試本圖。交付時需說明尚未建立的圖表，不可宣稱已完成。"})
    }

    pub fn set_preferences(&mut self, inbox: Option<super::super::steering::Inbox>) {
        self.preferences = inbox;
    }
    pub(super) fn review_optional_chart(
        &self,
        prepared: Prepared,
    ) -> AppResult<Option<super::super::charts::Chart>> {
        let inbox = self.preferences.as_ref().ok_or("偏好收件匣未就緒。")?;
        let invalid_x = prepared.review.groups.iter().any(|g| g.x_axis);
        if invalid_x {
            let question = format!("圖表「{}」含無效 X 座標（{}）。排除會移除所有系列的對應整列；未回答先完成其他工作並暫不建立此圖。", prepared.review.title, prepared.review.source);
            let id = inbox.ask(
                &crate::jobs::new_id()?,
                &question,
                &["暫不建立此圖".into(), "排除無效 X 的整列".into()],
                "暫不建立此圖",
            )?;
            let approved = inbox
                .questions()?
                .iter()
                .any(|q| q.id == id && q.answer.as_deref() == Some("排除無效 X 的整列"));
            if !approved {
                return Ok(None);
            }
        }
        let y_issues =
            prepared.review.blanks > 0 || prepared.review.groups.iter().any(|g| !g.x_axis);
        let question_id = if y_issues {
            let question = format!("圖表 #{}「{}」的 Y 空白與異常如何顯示？先保留缺值並繼續，任務完成後仍可在編輯器切換。", self.charts.len(), prepared.review.title);
            Some(inbox.ask(
                &crate::jobs::new_id()?,
                &question,
                &["保留缺值".into(), "略過此點".into(), "設為 0".into()],
                "保留缺值",
            )?)
        } else {
            None
        };
        let answer = question_id.as_ref().and_then(|id| {
            inbox
                .questions()
                .ok()?
                .into_iter()
                .find(|q| &q.id == id)?
                .answer
        });
        let choice = match answer.as_deref() {
            Some("略過此點") => Choice::Skip,
            Some("設為 0") => Choice::Zero,
            _ => Choice::Gap,
        };
        let choices = prepared
            .review
            .groups
            .iter()
            .map(|g| {
                if g.x_axis {
                    Choice::Skip
                } else {
                    choice.clone()
                }
            })
            .collect::<Vec<_>>();
        let policy = Policy {
            blank: Some(choice.clone()),
            invalid: Some(choice),
        };
        let mut chart = prepared.apply_policy(&choices, y_issues.then_some(&policy))?;
        if let Some(source) = &mut chart.quality {
            source.question_id = question_id;
        }
        Ok(Some(chart))
    }
    /// 明確選項由原生直接套用；答案在模型等待期間到達時也不遺漏。
    /// PNG 依完整圖表版本去重，因此更新後再次匯出會建立新檔，不覆寫先前成果。
    pub fn refresh_preferences(&mut self) -> AppResult<bool> {
        let Some(inbox) = &self.preferences else {
            return Ok(false);
        };
        let questions = inbox.questions()?;
        let mut changed = false;
        for chart in &mut self.charts {
            let Some(id) = chart.quality.as_ref().and_then(|s| s.question_id.as_ref()) else {
                continue;
            };
            let Some(answer) = questions
                .iter()
                .find(|q| &q.id == id)
                .and_then(|q| q.answer.as_deref())
            else {
                continue;
            };
            if chart
                .quality
                .as_ref()
                .and_then(|s| s.applied_answer.as_deref())
                == Some(answer)
            {
                continue;
            }
            let choice = match answer {
                "保留缺值" => Choice::Gap,
                "略過此點" => Choice::Skip,
                "設為 0" => Choice::Zero,
                _ => continue, // 自由文字交回模型解讀，不能自行猜使用者的意思。
            };
            let mut updated = Policy {
                blank: Some(choice.clone()),
                invalid: Some(choice),
            }
            .view(chart)?;
            if let Some(source) = &mut updated.quality {
                source.applied_answer = Some(answer.into());
            }
            if serde_json::to_value(&updated).map_err(|e| e.to_string())?
                != serde_json::to_value(&*chart).map_err(|e| e.to_string())?
            {
                updated.validate()?;
                *chart = updated;
                changed = true;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chart_question_does_not_block_and_answers_change_only_missing_cells() {
        let root = std::env::temp_dir().join(format!(
            "lmai-chart-preference-{}",
            crate::jobs::new_id().unwrap()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let project = Project {
            id: "p".into(),
            name: "test".into(),
            root: root.clone(),
            imports: Default::default(),
        };
        let inbox = crate::projects::steering::Inbox::open(&root, "run").unwrap();
        let mut broker = Broker::new(project, "run".into()).unwrap();
        broker.set_preferences(Some(inbox.clone()));
        let page=serde_json::from_value(json!({"sheet":1,"sheet_name":"s","columns":["A","B"],"header_row":1,
            "headers":[{"value":"x","text":"x","kind":"text"},{"value":"y","text":"y","kind":"text"}],"start_row":2,
            "rows":[{"row":2,"cells":[{"value":1,"text":"1","kind":"number"},{"value":0,"text":"0","kind":"number"}]},
                {"row":3,"cells":[{"value":2,"text":"2","kind":"number"},{"value":"NG","text":"NG","kind":"text"}]}],
            "next_row":null,"used_range":{"first_row":1,"last_row":3,"first_column":"A","last_column":"B"},"date_1904":false})).unwrap();
        let chart=serde_json::from_value(json!({"kind":"line","title":"t","x_label":"x","y_label":"y","x":[],"series":[],"source":"test"})).unwrap();
        let prepared = crate::projects::charts::quality::prepare(&page, chart).unwrap();
        let result = broker.review_optional_chart(prepared).unwrap().unwrap();
        assert_eq!(result.series[0].values, vec![Some(0.0), None]);
        broker.add_chart(result).unwrap();
        let question = inbox.questions().unwrap().remove(0);
        assert_eq!(question.state, "pending");
        assert!(inbox.boundary(false).unwrap().is_empty());
        inbox.answer(&question.id, "設為 0").unwrap();
        assert!(broker.refresh_preferences().unwrap());
        assert!(!broker.refresh_preferences().unwrap());
        assert_eq!(
            broker.charts[0].series[0].values,
            vec![Some(0.0), Some(0.0)]
        );
        let view = Policy {
            blank: None,
            invalid: Some(Choice::Gap),
        }
        .view(&broker.charts[0])
        .unwrap();
        assert_eq!(view.series[0].values, vec![Some(0.0), None]);
    }
}
