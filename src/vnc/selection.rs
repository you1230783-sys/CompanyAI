//! 批次操作以同一版清單的索引為依據，先驗證全部選取，再一次儲存。
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Deserialize, Serialize)]
pub struct MachineKey {
    pub group: String,
    pub index: usize,
}

#[derive(Deserialize)]
pub struct Selection {
    pub machines: Vec<MachineKey>,
    pub groups: Vec<String>,
}

/// 被選取的連續區塊整體移動一步，保留區塊內的相對順序。
fn move_selected<T>(items: &mut [T], selected: &mut [bool], up: bool) {
    if up {
        for i in 1..items.len() {
            if selected[i] && !selected[i - 1] {
                items.swap(i, i - 1);
                selected.swap(i, i - 1);
            }
        }
    } else {
        for i in (0..items.len().saturating_sub(1)).rev() {
            if selected[i] && !selected[i + 1] {
                items.swap(i, i + 1);
                selected.swap(i, i + 1);
            }
        }
    }
}

impl Manager {
    /// 機台上／下移只改變類內順序；刪除可同時涵蓋多個分類。
    pub fn batch_machines(&mut self, selection: &Selection, action: &str) -> AppResult<()> {
        let mut keys = BTreeSet::new();
        for key in &selection.machines {
            self.machine(&key.group, key.index)?;
            keys.insert((key.group.clone(), key.index));
        }
        for group in &selection.groups {
            let list = self.machines.get(group).ok_or("選取的分類已不存在。")?;
            keys.extend((0..list.len()).map(|i| (group.clone(), i)));
        }
        if keys.is_empty() && selection.groups.is_empty() {
            return Err("請先勾選機台。".into());
        }
        if !matches!(action, "up" | "down" | "delete") {
            return Err("批次操作無效。".into());
        }
        let mut machines = self.machines.clone();
        for (group, list) in &mut machines {
            let mut chosen: Vec<_> = (0..list.len())
                .map(|i| keys.contains(&(group.clone(), i)))
                .collect();
            if action == "delete" {
                let mut i = 0;
                list.retain(|_| {
                    let keep = !chosen[i];
                    i += 1;
                    keep
                });
            } else {
                move_selected(list, &mut chosen, action == "up");
            }
        }
        if action == "delete" {
            machines.retain(|group, list| {
                !list.is_empty()
                    || (!selection.groups.contains(group) && !keys.iter().any(|(g, _)| g == group))
            });
        }
        self.save_machines(machines)
    }

    pub fn move_groups(&mut self, groups: &[String], up: bool) -> AppResult<()> {
        self.ensure_unchanged()?;
        if groups.is_empty() || groups.iter().any(|g| !self.machines.contains_key(g)) {
            return Err("請勾選完整分類。".into());
        }
        let mut order = self.ordered_groups();
        let mut chosen: Vec<_> = order.iter().map(|g| groups.contains(g)).collect();
        move_selected(&mut order, &mut chosen, up);
        let mut config = self.config.clone();
        config.group_order = order;
        self.save_config(config)
    }
}
