//! Provider and key facts the Connect dialog needs.

use super::app::App;
use super::footer::ProviderState;
use super::overlay::connect::ConnectCtx;
use crate::catalog;

impl App {
    /// The provider the footer and the dialogs treat as current.
    pub fn current_provider(&self) -> Option<String> {
        match &self.provider_state {
            ProviderState::Connected { provider, .. } => Some(provider.clone()),
            ProviderState::NeedsProvider => None,
        }
    }

    /// True when a key is available for this provider, or none is needed.
    pub fn has_key(&self, provider: &str) -> bool {
        match catalog::find(provider) {
            Some(info) => {
                catalog::resolve_key(info, &|name| self.env.key(name), &self.auth).is_some()
                    || !info.needs_key
            }
            None => self
                .env
                .key(catalog::UNIVERSAL_ENV)
                .or_else(|| self.auth.get_key(provider).map(str::to_string))
                .is_some(),
        }
    }

    /// Snapshot for the Connect dialog.
    pub fn connect_ctx(&self) -> ConnectCtx {
        ConnectCtx {
            current_provider: self.current_provider(),
            current_model: Some(self.model.clone()).filter(|model| !model.is_empty()),
            auth: self.auth.clone(),
            env: self.env.clone(),
            base_urls: self.state.base_urls.clone(),
        }
    }
}
