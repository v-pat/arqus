//! The Connect dialog: pick a provider, paste a key, validate it.
//!
//! The state machine is plain data so every step can be tested without a
//! terminal or a network.

use std::collections::BTreeMap;

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::auth::{clean_pasted_key, AuthStore};
use crate::catalog::{self, ConnStatus, KeySource, ProviderInfo, PROVIDERS};
use crate::settings::EnvOverrides;

use super::super::keys::{Key, KeyCodeX};
use super::super::picker::{to_pkey, Item, Picker, PickerEvent};
use super::super::theme::Theme;
use super::OverlayOutcome;

mod draw;
mod input;
mod keys;

#[derive(Debug, Clone, PartialEq)]
pub struct KeyInfo {
    pub limit_remaining: Option<f64>,
    pub limit: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyError {
    pub status: Option<u16>,
    pub message: String,
    pub network: bool,
}

#[derive(Debug, Clone)]
pub struct ConnectCtx {
    pub current_provider: Option<String>,
    pub current_model: Option<String>,
    pub auth: AuthStore,
    pub env: EnvOverrides,
    pub base_urls: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MenuChoice {
    ChooseModel,
    ReplaceKey,
    Disconnect,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyStatus {
    Idle,
    Checking,
    Ok(KeyInfo),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum Step {
    PickProvider {
        picker: Picker<String>,
    },
    ProviderMenu {
        provider: String,
        picker: Picker<MenuChoice>,
    },
    EnterBaseUrl {
        provider: String,
        input: String,
    },
    EnterKey {
        provider: String,
        base_url: String,
        input: String,
        status: KeyStatus,
    },
    ConfirmSave {
        provider: String,
        base_url: String,
        key: String,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct Connect {
    pub heading: Option<String>,
    pub step: Step,
    pub ctx: ConnectCtx,
    pub notice: Option<String>,
    pending_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckOutcome {
    Pending,
    Rejected,
    NetworkError,
    Ok {
        provider: String,
        key: String,
        base_url: String,
    },
}

fn provider_picker(ctx: &ConnectCtx) -> Picker<String> {
    let mut items = Vec::new();
    for info in PROVIDERS {
        let group = if info.recommended {
            "Recommended"
        } else {
            "Other"
        };
        // The suffix keeps "open" ranked below OpenRouter, the recommended
        // choice, while still naming the provider clearly.
        let label = if info.id == "openai" {
            "OpenAI (GPT)"
        } else {
            info.name
        };
        items.push(
            Item::new(label, info.id.to_string())
                .detail(status_text(info, ctx))
                .group(group)
                .current(ctx.current_provider.as_deref() == Some(info.id)),
        );
    }
    Picker::new("Connect a provider", items)
}

fn menu_picker() -> Picker<MenuChoice> {
    Picker::new(
        "Provider",
        vec![
            Item::new("Choose model", MenuChoice::ChooseModel).detail("pick from the list"),
            Item::new("Replace key", MenuChoice::ReplaceKey).detail("paste a new key"),
            Item::new("Disconnect", MenuChoice::Disconnect).detail("remove the saved key"),
        ],
    )
}

fn status_text(info: &ProviderInfo, ctx: &ConnectCtx) -> String {
    match catalog::status(info, &|name| ctx.env.key(name), &ctx.auth) {
        ConnStatus::Connected(KeySource::Env(name)) => format!("env · {name}"),
        ConnStatus::Connected(KeySource::Stored) => "✓ connected".to_string(),
        ConnStatus::NoKeyNeeded => "local".to_string(),
        ConnStatus::NotConnected => info.blurb.to_string(),
    }
}

fn base_url_for(info: &ProviderInfo, ctx: &ConnectCtx) -> String {
    ctx.env
        .base_url
        .clone()
        .or_else(|| ctx.base_urls.get(info.id).cloned())
        .unwrap_or_else(|| info.base_url.to_string())
}

fn provider_name(id: &str) -> String {
    catalog::find(id)
        .map(|info| info.name.to_string())
        .unwrap_or_else(|| id.to_string())
}

fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    rest.split('/').next().unwrap_or(url).to_string()
}

#[cfg(test)]
#[path = "connect_tests.rs"]
mod tests;
