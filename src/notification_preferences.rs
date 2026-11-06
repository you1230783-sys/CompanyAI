//! 彈出通知的方式與分類。通知中心和提醒紀錄有獨立的保存規則。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// 只用於移轉舊設定：不同來源原本有不同的通知方式。
    Legacy,
    #[default]
    Windows,
    Pet,
    Off,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    AiResult,
    ProjectResult,
    Attention,
    Reminder,
    Schedule,
    OvernightBatch,
    AiInbox,
    SiteInbox,
    TrayHint,
    ChatMessage,
    ChatInvite,
}

impl Category {
    fn used_legacy_switch(self) -> bool {
        !matches!(self, Self::Reminder | Self::TrayHint)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub mode: Mode,
    pub categories: BTreeMap<Category, bool>,
    pub chat_preview: bool,
    legacy_popups: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            mode: Mode::Windows,
            categories: BTreeMap::new(),
            chat_preview: false,
            legacy_popups: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Delivery {
    Silent,
    Windows,
    Pet,
    Both,
}

impl Preferences {
    pub(crate) fn migrate(legacy_popups: Option<bool>) -> Self {
        Self {
            mode: Mode::Legacy,
            legacy_popups: legacy_popups.unwrap_or(true),
            ..Self::default()
        }
    }

    pub(crate) fn category_enabled(&self, category: Category) -> bool {
        self.categories
            .get(&category)
            .copied()
            .unwrap_or(self.legacy_popups || !category.used_legacy_switch())
    }

    pub(crate) fn delivery(
        &self,
        category: Category,
        legacy: Delivery,
        pet_available: bool,
    ) -> Delivery {
        if !self.category_enabled(category) {
            return Delivery::Silent;
        }
        match self.mode {
            Mode::Legacy => legacy,
            Mode::Windows => Delivery::Windows,
            Mode::Pet if pet_available => Delivery::Pet,
            Mode::Pet => Delivery::Windows,
            Mode::Off => Delivery::Silent,
        }
    }

    /// 舊版總開關只控制彈出通知。移轉後仍閃爍工作列，直到使用者更改這類設定。
    pub(crate) fn flash_attention(&self) -> bool {
        matches!(self.mode, Mode::Legacy | Mode::Windows)
            && (self.category_enabled(Category::Attention)
                || (self.mode == Mode::Legacy
                    && !self.categories.contains_key(&Category::Attention)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn legacy_settings_preserve_per_source_channels_and_flash() {
        let config: Config = serde_json::from_str(r#"{"notification_popups":false}"#).unwrap();
        let prefs = config.notification_preferences;
        assert_eq!(prefs.mode, Mode::Legacy);
        assert_eq!(
            prefs.delivery(Category::AiInbox, Delivery::Pet, true),
            Delivery::Silent
        );
        assert_eq!(
            prefs.delivery(Category::Reminder, Delivery::Windows, true),
            Delivery::Windows
        );
        assert_eq!(
            prefs.delivery(Category::Reminder, Delivery::Pet, true),
            Delivery::Pet
        );
        assert!(prefs.flash_attention());
        let restored: Preferences =
            serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
        assert!(!restored.category_enabled(Category::AiInbox));
        assert!(restored.category_enabled(Category::TrayHint));
    }

    #[test]
    fn master_off_and_each_category_control_delivery() {
        let categories = [
            Category::AiResult,
            Category::ProjectResult,
            Category::Attention,
            Category::Reminder,
            Category::Schedule,
            Category::OvernightBatch,
            Category::AiInbox,
            Category::SiteInbox,
            Category::TrayHint,
            Category::ChatMessage,
            Category::ChatInvite,
        ];
        for category in categories {
            let mut prefs = Preferences::default();
            assert_eq!(
                prefs.delivery(category, Delivery::Both, true),
                Delivery::Windows
            );
            prefs.mode = Mode::Pet;
            assert_eq!(
                prefs.delivery(category, Delivery::Both, true),
                Delivery::Pet
            );
            assert_eq!(
                prefs.delivery(category, Delivery::Both, false),
                Delivery::Windows
            );
            prefs.categories.insert(category, false);
            assert_eq!(
                prefs.delivery(category, Delivery::Both, true),
                Delivery::Silent
            );
            prefs.categories.insert(category, true);
            prefs.mode = Mode::Off;
            assert_eq!(
                prefs.delivery(category, Delivery::Both, true),
                Delivery::Silent
            );
            assert!(!prefs.flash_attention());
        }
    }
}
