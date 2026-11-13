//! Parse the answer of GET {base_url}/models into rows for the model picker.
//! Handles the OpenAI shape, OpenRouter's extras (context_length, pricing, supported_parameters),
//! Anthropic's `display_name`, and Ollama / LM Studio lists.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context: Option<u64>,
    pub prompt_per_mtok: Option<f64>, // dollars per million input tokens
    pub completion_per_mtok: Option<f64>, // dollars per million output tokens
    pub tools: Option<bool>,          // None = the provider does not say
}

fn price_per_mtok(v: &Value) -> Option<f64> {
    let per_token: f64 = match v {
        Value::String(s) => s.parse().ok()?,
        Value::Number(n) => n.as_f64()?,
        _ => return None,
    };
    if per_token < 0.0 {
        return None; // OpenRouter uses -1 for "variable pricing"
    }
    Some(per_token * 1_000_000.0)
}

pub fn parse_models(json: &Value) -> Vec<ModelInfo> {
    let list = match json.get("data").and_then(|d| d.as_array()) {
        Some(l) => l,
        None => return vec![],
    };
    let mut out = Vec::new();
    for m in list {
        let id = match m.get("id").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let name = m
            .get("name")
            .or_else(|| m.get("display_name"))
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();
        let context = m
            .get("context_length")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                m.get("top_provider")
                    .and_then(|t| t.get("context_length"))
                    .and_then(|v| v.as_u64())
            });
        let pricing = m.get("pricing");
        let prompt_per_mtok = pricing
            .and_then(|p| p.get("prompt"))
            .and_then(price_per_mtok);
        let completion_per_mtok = pricing
            .and_then(|p| p.get("completion"))
            .and_then(price_per_mtok);
        let tools = m
            .get("supported_parameters")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().any(|x| x.as_str() == Some("tools")));
        out.push(ModelInfo {
            id,
            name,
            context,
            prompt_per_mtok,
            completion_per_mtok,
            tools,
        });
    }
    out
}

/// 8192 -> "8K", 128000 -> "128K", 1048576 -> "1M"
pub fn format_ctx(n: u64) -> String {
    if n >= 1_000_000 {
        let m = n as f64 / 1_000_000.0;
        if (m - m.round()).abs() < 0.06 {
            format!("{}M", m.round() as u64)
        } else {
            format!("{m:.1}M")
        }
    } else if n >= 1000 {
        format!("{}K", (n as f64 / 1000.0).round() as u64)
    } else {
        n.to_string()
    }
}

/// 0.14 -> "$0.14/M", 0 -> "free", 15.0 -> "$15/M"
pub fn format_price(per_mtok: f64) -> String {
    if per_mtok == 0.0 {
        "free".to_string()
    } else if per_mtok < 1.0 {
        format!("${per_mtok:.2}/M")
    } else if (per_mtok - per_mtok.round()).abs() < 0.005 {
        format!("${}/M", per_mtok.round() as u64)
    } else {
        format!("${per_mtok:.2}/M")
    }
}

/// Text for the right-hand column of the picker: "128K ctx · $0.14/M in · tools".
pub fn model_detail(m: &ModelInfo) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(c) = m.context {
        parts.push(format!("{} ctx", format_ctx(c)));
    }
    if let Some(p) = m.prompt_per_mtok {
        parts.push(format!("{} in", format_price(p)));
    }
    if m.tools == Some(true) {
        parts.push("tools".to_string());
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn openrouter_shape() {
        let v = json!({"data":[
            {"id":"deepseek/deepseek-v4-flash","name":"DeepSeek: V4 Flash","context_length":1048576,
             "pricing":{"prompt":"0.00000014","completion":"0.00000028"},
             "top_provider":{"context_length":1048576,"max_completion_tokens":65536},
             "supported_parameters":["max_tokens","tools","tool_choice"]},
            {"id":"foo/free-model:free","name":"Foo (free)","context_length":8192,
             "pricing":{"prompt":"0","completion":"0"},"supported_parameters":["max_tokens"]},
            {"id":"router/auto","pricing":{"prompt":"-1","completion":"-1"}}
        ]});
        let m = parse_models(&v);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].name, "DeepSeek: V4 Flash");
        assert_eq!(m[0].tools, Some(true));
        assert!((m[0].prompt_per_mtok.unwrap() - 0.14).abs() < 1e-9);
        assert_eq!(m[1].tools, Some(false));
        assert_eq!(m[1].prompt_per_mtok, Some(0.0));
        assert_eq!(m[2].prompt_per_mtok, None); // -1 = variable
        assert_eq!(m[2].tools, None);
        assert_eq!(m[2].name, "router/auto");
        assert_eq!(model_detail(&m[0]), "1M ctx · $0.14/M in · tools");
        assert_eq!(model_detail(&m[1]), "8K ctx · free in");
    }

    #[test]
    fn anthropic_and_ollama_shapes() {
        let a = json!({"data":[{"type":"model","id":"claude-sonnet-x","display_name":"Claude Sonnet X","created_at":"2025-01-01T00:00:00Z"}],"has_more":false});
        let m = parse_models(&a);
        assert_eq!(m[0].name, "Claude Sonnet X");
        assert_eq!(m[0].context, None);
        let o = json!({"object":"list","data":[{"id":"qwen3:8b","object":"model","owned_by":"library"}]});
        let m = parse_models(&o);
        assert_eq!(m[0].id, "qwen3:8b");
        assert_eq!(model_detail(&m[0]), "");
    }

    #[test]
    fn garbage_in_empty_out() {
        assert!(parse_models(&json!({})).is_empty());
        assert!(parse_models(&json!({"data":"nope"})).is_empty());
        assert!(parse_models(&json!({"data":[{"name":"no id"},{"id":""},42]})).is_empty());
        assert!(parse_models(&json!({"error":{"message":"bad key"}})).is_empty());
    }

    #[test]
    fn formatting() {
        assert_eq!(format_ctx(8192), "8K");
        assert_eq!(format_ctx(128000), "128K");
        assert_eq!(format_ctx(200000), "200K");
        assert_eq!(format_ctx(1_048_576), "1M");
        assert_eq!(format_ctx(2_000_000), "2M");
        assert_eq!(format_ctx(512), "512");
        assert_eq!(format_price(0.0), "free");
        assert_eq!(format_price(0.14), "$0.14/M");
        assert_eq!(format_price(15.0), "$15/M");
        assert_eq!(format_price(2.5), "$2.50/M");
    }
}
