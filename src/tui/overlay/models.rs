//! The model picker: recent models first, then every model the provider lists.

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::models::{model_detail, ModelInfo};
use crate::state::{ModelRef, State};

use super::super::keys::{Key, KeyCodeX};
use super::super::picker::{to_pkey, Item, Picker, PickerEvent};
use super::super::theme::Theme;
use super::super::widgets::picker_view;
use super::OverlayOutcome;

#[derive(Debug, Clone, PartialEq)]
pub enum ModelChoice {
    Model(String),
    Typed(String),
}

#[derive(Debug, Clone)]
pub struct Models {
    pub provider: String,
    pub picker: Picker<ModelChoice>,
    pub all: Vec<ModelInfo>,
    pub loading: bool,
    pub error: Option<String>,
    pub tools_only: bool,
    pub show_toggle: bool,
    pub recent: Vec<ModelRef>,
    pub current: Option<String>,
}

impl Models {
    pub fn loading(provider: &str, state: &State, current: &str) -> Models {
        let mut picker = Picker::new(format!("Select model · {provider}"), Vec::new());
        picker.status = Some("Loading models...".to_string());
        Models {
            provider: provider.to_string(),
            picker,
            all: Vec::new(),
            loading: true,
            error: None,
            tools_only: false,
            show_toggle: false,
            recent: state.recent_for(provider),
            current: Some(current.to_string()).filter(|model| !model.is_empty()),
        }
    }

    pub fn on_loaded(&mut self, result: Result<Vec<ModelInfo>, String>) {
        self.loading = false;
        match result {
            Ok(list) => {
                self.all = list;
                self.show_toggle = self.all.iter().any(|model| model.tools.is_some());
                self.tools_only = self.show_toggle;
                self.error = None;
                self.picker.status = if self.all.is_empty() {
                    Some("No models returned. Type a model id.".to_string())
                } else {
                    None
                };
            }
            Err(message) => {
                self.error = Some(message.clone());
                self.picker.status = Some(message);
            }
        }
        self.rebuild();
    }

    pub fn handle_key(&mut self, key: Key) -> OverlayOutcome {
        if key.code == KeyCodeX::Tab {
            if self.show_toggle {
                self.tools_only = !self.tools_only;
                self.rebuild();
            }
            return OverlayOutcome::None;
        }
        let Some(pkey) = to_pkey(key) else {
            return OverlayOutcome::None;
        };
        let query_changed = matches!(
            pkey,
            super::super::picker::PKey::Char(_)
                | super::super::picker::PKey::Backspace
                | super::super::picker::PKey::ClearQuery
                | super::super::picker::PKey::Esc
        );
        match self.picker.handle_key(pkey) {
            PickerEvent::Selected(choice) => {
                let id = match choice {
                    ModelChoice::Model(id) | ModelChoice::Typed(id) => id,
                };
                OverlayOutcome::CloseWith(vec![super::super::msg::Effect::SetModel {
                    provider: self.provider.clone(),
                    id,
                }])
            }
            PickerEvent::Cancel => OverlayOutcome::Close,
            PickerEvent::None => {
                if query_changed {
                    self.rebuild();
                }
                OverlayOutcome::None
            }
        }
    }

    pub fn paste(&mut self, text: &str) {
        for ch in text.chars().filter(|ch| !ch.is_whitespace()) {
            self.picker.query.push(ch);
        }
        self.rebuild();
    }

    pub fn rebuild(&mut self) {
        let query = self.picker.query.clone();
        let mut items: Vec<Item<ModelChoice>> = Vec::new();
        for recent in &self.recent {
            let detail = self
                .all
                .iter()
                .find(|model| model.id == recent.id)
                .map(model_detail)
                .unwrap_or_default();
            items.push(
                Item::new(recent.id.clone(), ModelChoice::Model(recent.id.clone()))
                    .detail(detail)
                    .group("Recent")
                    .current(self.current.as_deref() == Some(recent.id.as_str())),
            );
        }
        let mut all: Vec<&ModelInfo> = self
            .all
            .iter()
            .filter(|model| !self.tools_only || model.tools == Some(true))
            .collect();
        all.sort_by(|a, b| a.id.cmp(&b.id));
        for model in all {
            items.push(
                Item::new(model.id.clone(), ModelChoice::Model(model.id.clone()))
                    .detail(model_detail(model))
                    .group("All models")
                    .current(self.current.as_deref() == Some(model.id.as_str())),
            );
        }
        let exact = self.all.iter().any(|model| model.id == query);
        let typed = !query.is_empty() && !exact;
        if typed {
            items.push(
                Item::new(
                    format!("Use \"{query}\" as model id"),
                    ModelChoice::Typed(query.clone()),
                )
                .group("Custom"),
            );
        }
        self.picker.set_items(items);
        if typed {
            self.picker.select_last();
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let hint = if self.show_toggle {
            "↑↓ select · enter use · tab tools/all · esc close"
        } else {
            "↑↓ select · enter use · esc close"
        };
        picker_view::draw(frame, area, &mut self.picker, hint, theme);
    }
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
