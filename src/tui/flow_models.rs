//! Model-picker actions shared by keys and slash commands.

use super::app::App;
use super::msg::{Effect, Level};
use super::overlay::connect::Connect;
use super::overlay::models::Models;
use super::overlay::Overlay;

/// Open the model picker, or the Connect dialog when no provider is ready.
pub fn open_models(app: &mut App) -> Vec<Effect> {
    match app.current_provider() {
        Some(provider) if app.has_key(&provider) => {
            app.push_overlay(Overlay::Models(Models::loading(
                &provider, &app.state, &app.model,
            )));
            vec![Effect::FetchModels { provider }]
        }
        Some(provider) => {
            app.toast(format!("connect {provider} first"), Level::Warn);
            app.push_overlay(Overlay::Connect(Connect::new(None, app.connect_ctx())));
            Vec::new()
        }
        None => {
            app.toast("no provider connected", Level::Warn);
            app.push_overlay(Overlay::Connect(Connect::new(None, app.connect_ctx())));
            Vec::new()
        }
    }
}

/// F2: use the next recent model, across providers.
pub fn cycle_model(app: &mut App) -> Vec<Effect> {
    if app.state.recent.is_empty() {
        app.toast("no recent models", Level::Info);
        return Vec::new();
    }
    let current = app
        .state
        .model
        .as_ref()
        .map(|model| (model.provider.clone(), model.id.clone()));
    let next = app
        .state
        .recent
        .iter()
        .position(|model| Some((model.provider.clone(), model.id.clone())) == current)
        .map(|index| (index + 1) % app.state.recent.len())
        .unwrap_or(0);
    let model = app.state.recent[next].clone();
    if !app.has_key(&model.provider) {
        app.toast(format!("connect {} first", model.provider), Level::Warn);
        app.push_overlay(Overlay::Connect(Connect::new(None, app.connect_ctx())));
        return Vec::new();
    }
    vec![Effect::SetModel {
        provider: model.provider,
        id: model.id,
    }]
}

pub fn should_save_base_url(provider: &str, base_url: &str, env_base_url: Option<&str>) -> bool {
    let default = crate::catalog::find(provider)
        .map(|info| info.base_url)
        .unwrap_or("");
    !base_url.is_empty() && base_url != default && Some(base_url) != env_base_url
}
