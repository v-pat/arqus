use super::*;

impl Connect {
    pub fn new(heading: Option<String>, ctx: ConnectCtx) -> Connect {
        let picker = provider_picker(&ctx);
        Connect {
            heading,
            step: Step::PickProvider { picker },
            ctx,
            notice: None,
            pending_key: None,
        }
    }

    pub fn handle_key(&mut self, key: Key) -> OverlayOutcome {
        self.notice = None;
        let step = std::mem::replace(
            &mut self.step,
            Step::PickProvider {
                picker: Picker::new("", Vec::new()),
            },
        );
        let (step, outcome) = match step {
            Step::PickProvider { picker } => self.pick_provider_key(key, picker),
            Step::ProviderMenu { provider, picker } => {
                self.provider_menu_key(key, provider, picker)
            }
            Step::EnterBaseUrl { provider, input } => self.base_url_key(key, provider, input),
            Step::EnterKey {
                provider,
                base_url,
                input,
                status,
            } => self.key_key(key, provider, base_url, input, status),
            Step::ConfirmSave {
                provider,
                base_url,
                key: saved,
                error,
            } => self.confirm_key(key, provider, base_url, saved, error),
        };
        self.step = step;
        outcome
    }

    pub fn paste(&mut self, text: &str) {
        match &mut self.step {
            Step::EnterBaseUrl { input, .. } => input.push_str(text.trim()),
            Step::EnterKey { input, .. } => input.push_str(&clean_pasted_key(text)),
            _ => {}
        }
    }

    pub fn on_key_checked(&mut self, result: Result<KeyInfo, KeyError>) -> CheckOutcome {
        let (provider, base_url) = match &self.step {
            Step::EnterKey {
                provider, base_url, ..
            } => (provider.clone(), base_url.clone()),
            _ => return CheckOutcome::Pending,
        };
        match result {
            Ok(info) => {
                let key = self.pending_key.take().unwrap_or_default();
                if let Step::EnterKey { status, .. } = &mut self.step {
                    *status = KeyStatus::Ok(info);
                }
                CheckOutcome::Ok {
                    provider,
                    key,
                    base_url,
                }
            }
            Err(error) => {
                if error.network {
                    let key = self.pending_key.take().unwrap_or_default();
                    let host = host_of(&base_url);
                    self.step = Step::ConfirmSave {
                        provider,
                        base_url,
                        key,
                        error: format!("Could not reach {host}: {}", error.message),
                    };
                    CheckOutcome::NetworkError
                } else {
                    let name = provider_name(&provider);
                    let code = error.status.unwrap_or(0);
                    let message = if matches!(error.status, Some(401 | 403)) {
                        format!("Key rejected by {name} ({code}). Check it and paste again.")
                    } else {
                        format!("{name} answered {code}: {}", error.message)
                    };
                    if let Step::EnterKey { status, .. } = &mut self.step {
                        *status = KeyStatus::Error(message);
                    }
                    self.pending_key = None;
                    CheckOutcome::Rejected
                }
            }
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        draw::draw(self, frame, area, theme);
    }

    fn confirm_key(
        &mut self,
        key: Key,
        provider: String,
        base_url: String,
        saved: String,
        error: String,
    ) -> (Step, OverlayOutcome) {
        match key.code {
            KeyCodeX::Char('y') => {
                self.pending_key = Some(saved.clone());
                (
                    Step::ConfirmSave {
                        provider: provider.clone(),
                        base_url: base_url.clone(),
                        key: saved.clone(),
                        error,
                    },
                    OverlayOutcome::Connected {
                        provider,
                        key: saved,
                        base_url,
                    },
                )
            }
            KeyCodeX::Char('n') | KeyCodeX::Esc => (
                Step::EnterKey {
                    provider,
                    base_url,
                    input: saved,
                    status: KeyStatus::Idle,
                },
                OverlayOutcome::None,
            ),
            _ => (
                Step::ConfirmSave {
                    provider,
                    base_url,
                    key: saved,
                    error,
                },
                OverlayOutcome::None,
            ),
        }
    }
}
