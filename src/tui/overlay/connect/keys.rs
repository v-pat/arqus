use super::*;
use crate::tui::msg::Effect;

impl Connect {
    pub(super) fn pick_provider_key(
        &mut self,
        key: Key,
        mut picker: Picker<String>,
    ) -> (Step, OverlayOutcome) {
        let Some(pkey) = to_pkey(key) else {
            return (Step::PickProvider { picker }, OverlayOutcome::None);
        };
        match picker.handle_key(pkey) {
            PickerEvent::Selected(id) => (self.choose_provider(&id), OverlayOutcome::None),
            PickerEvent::Cancel => (Step::PickProvider { picker }, OverlayOutcome::Close),
            PickerEvent::None => (Step::PickProvider { picker }, OverlayOutcome::None),
        }
    }

    fn choose_provider(&self, id: &str) -> Step {
        let Some(info) = catalog::find(id) else {
            return Step::PickProvider {
                picker: provider_picker(&self.ctx),
            };
        };
        let connected =
            catalog::resolve_key(info, &|name| self.ctx.env.key(name), &self.ctx.auth).is_some();
        if connected {
            Step::ProviderMenu {
                provider: id.to_string(),
                picker: menu_picker(),
            }
        } else if !info.needs_key || info.base_url.is_empty() {
            Step::EnterBaseUrl {
                provider: id.to_string(),
                input: base_url_for(info, &self.ctx),
            }
        } else {
            Step::EnterKey {
                provider: id.to_string(),
                base_url: base_url_for(info, &self.ctx),
                input: String::new(),
                status: KeyStatus::Idle,
            }
        }
    }

    pub(super) fn provider_menu_key(
        &mut self,
        key: Key,
        provider: String,
        mut picker: Picker<MenuChoice>,
    ) -> (Step, OverlayOutcome) {
        let Some(pkey) = to_pkey(key) else {
            return (
                Step::ProviderMenu { provider, picker },
                OverlayOutcome::None,
            );
        };
        match picker.handle_key(pkey) {
            PickerEvent::Selected(MenuChoice::ChooseModel) => (
                Step::ProviderMenu {
                    provider: provider.clone(),
                    picker,
                },
                OverlayOutcome::OpenModels { provider },
            ),
            PickerEvent::Selected(MenuChoice::ReplaceKey) => {
                let info = catalog::find(&provider);
                let base_url = info
                    .map(|info| base_url_for(info, &self.ctx))
                    .unwrap_or_default();
                (
                    Step::EnterKey {
                        provider,
                        base_url,
                        input: String::new(),
                        status: KeyStatus::Idle,
                    },
                    OverlayOutcome::None,
                )
            }
            PickerEvent::Selected(MenuChoice::Disconnect) => {
                if let Some(info) = catalog::find(&provider) {
                    if let Some((_, KeySource::Env(name))) =
                        catalog::resolve_key(info, &|name| self.ctx.env.key(name), &self.ctx.auth)
                    {
                        self.notice =
                            Some(format!("key comes from {name}; unset it to disconnect"));
                        return (
                            Step::ProviderMenu { provider, picker },
                            OverlayOutcome::None,
                        );
                    }
                }
                (
                    Step::ProviderMenu {
                        provider: provider.clone(),
                        picker,
                    },
                    OverlayOutcome::CloseWith(vec![Effect::Disconnect { provider }]),
                )
            }
            PickerEvent::Cancel => (
                Step::ProviderMenu { provider, picker },
                OverlayOutcome::Close,
            ),
            PickerEvent::None => (
                Step::ProviderMenu { provider, picker },
                OverlayOutcome::None,
            ),
        }
    }

    pub(super) fn base_url_key(
        &mut self,
        key: Key,
        provider: String,
        mut input: String,
    ) -> (Step, OverlayOutcome) {
        match key.code {
            KeyCodeX::Esc => (
                Step::PickProvider {
                    picker: provider_picker(&self.ctx),
                },
                OverlayOutcome::None,
            ),
            KeyCodeX::Enter => {
                let url = input.trim().to_string();
                if url.is_empty() {
                    self.notice = Some("base URL required".to_string());
                    return (Step::EnterBaseUrl { provider, input }, OverlayOutcome::None);
                }
                (
                    Step::EnterKey {
                        provider,
                        base_url: url,
                        input: String::new(),
                        status: KeyStatus::Idle,
                    },
                    OverlayOutcome::None,
                )
            }
            KeyCodeX::Backspace => {
                input.pop();
                (Step::EnterBaseUrl { provider, input }, OverlayOutcome::None)
            }
            KeyCodeX::Char('u') if key.ctrl => {
                input.clear();
                (Step::EnterBaseUrl { provider, input }, OverlayOutcome::None)
            }
            KeyCodeX::Char(c) if !key.ctrl && !key.alt => {
                input.push(c);
                (Step::EnterBaseUrl { provider, input }, OverlayOutcome::None)
            }
            _ => (Step::EnterBaseUrl { provider, input }, OverlayOutcome::None),
        }
    }

    pub(super) fn key_key(
        &mut self,
        key: Key,
        provider: String,
        base_url: String,
        mut input: String,
        mut status: KeyStatus,
    ) -> (Step, OverlayOutcome) {
        match key.code {
            KeyCodeX::Esc => (
                Step::PickProvider {
                    picker: provider_picker(&self.ctx),
                },
                OverlayOutcome::None,
            ),
            KeyCodeX::Enter => {
                let cleaned = clean_pasted_key(&input);
                let needs_key = catalog::find(&provider)
                    .map(|info| info.needs_key)
                    .unwrap_or(true);
                if cleaned.is_empty() && needs_key {
                    status = KeyStatus::Error("paste a key first".to_string());
                    return (
                        Step::EnterKey {
                            provider,
                            base_url,
                            input,
                            status,
                        },
                        OverlayOutcome::None,
                    );
                }
                self.pending_key = Some(cleaned.clone());
                status = KeyStatus::Checking;
                (
                    Step::EnterKey {
                        provider: provider.clone(),
                        base_url: base_url.clone(),
                        input,
                        status,
                    },
                    OverlayOutcome::Effects(vec![Effect::ValidateKey {
                        provider,
                        key: cleaned,
                        base_url,
                    }]),
                )
            }
            KeyCodeX::Char('o') if key.ctrl => {
                let url = catalog::find(&provider)
                    .map(|info| info.key_url.to_string())
                    .unwrap_or_default();
                let outcome = if url.is_empty() {
                    OverlayOutcome::None
                } else {
                    OverlayOutcome::Effects(vec![Effect::OpenUrl(url)])
                };
                (
                    Step::EnterKey {
                        provider,
                        base_url,
                        input,
                        status,
                    },
                    outcome,
                )
            }
            KeyCodeX::Char('u') if key.ctrl => {
                input.clear();
                (
                    Step::EnterKey {
                        provider,
                        base_url,
                        input,
                        status,
                    },
                    OverlayOutcome::None,
                )
            }
            KeyCodeX::Backspace => {
                input.pop();
                (
                    Step::EnterKey {
                        provider,
                        base_url,
                        input,
                        status,
                    },
                    OverlayOutcome::None,
                )
            }
            KeyCodeX::Char(c) if !key.ctrl && !key.alt => {
                input.push(c);
                (
                    Step::EnterKey {
                        provider,
                        base_url,
                        input,
                        status,
                    },
                    OverlayOutcome::None,
                )
            }
            _ => (
                Step::EnterKey {
                    provider,
                    base_url,
                    input,
                    status,
                },
                OverlayOutcome::None,
            ),
        }
    }
}
