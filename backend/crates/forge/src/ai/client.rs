//! Thin wrapper over the vendored `misanthropy` Anthropic client SDK.

use misanthropy::{Anthropic, Content, Message, MessagesRequest, Role};
use serde_json::Value;

use crate::config::Config;
use crate::error::{Error, Result};

#[derive(Clone)]
pub struct Ai {
    client: std::sync::Arc<Anthropic>,
    pub model: String,
    pub max_tokens: u32,
}

impl Ai {
    pub fn from_config(cfg: &Config) -> Self {
        let client = std::sync::Arc::new(
            Anthropic::new(&cfg.anthropic_api_key).with_base_url(&cfg.anthropic_base_url),
        );
        Self {
            client,
            model: cfg.anthropic_model.clone(),
            max_tokens: cfg.anthropic_max_tokens,
        }
    }

    /// One-shot completion. `system` may be empty.
    pub async fn complete(&self, system: &str, user: &str) -> Result<String> {
        let mut req = MessagesRequest::default()
            .with_model(&self.model)
            .with_max_tokens(self.max_tokens.min(1024));
        if !system.is_empty() {
            req = req.with_system(vec![Content::text(system)]);
        }
        let mut msg = Message::new(Role::User);
        msg.content.push(Content::text(user));
        req.messages.push(msg);
        let resp = self.client.messages(&req).await?;
        Ok(resp.format_content())
    }

    /// JSON-only completion: the model is asked for a single JSON document and
    /// the first balanced JSON object/array is extracted from the reply.
    pub async fn complete_json(&self, system: &str, user: &str) -> Result<Value> {
        let raw = self.complete(system, user).await?;
        parse_first_json(&raw).ok_or_else(|| {
            Error::Ai(format!(
                "model returned no JSON object (first 200 chars: {})",
                truncate(&raw, 200)
            ))
        })
    }

    pub fn anthropic(&self) -> &Anthropic {
        &self.client
    }

}

/// Extract the first balanced JSON object or array from arbitrary model text.
pub fn parse_first_json(text: &str) -> Option<Value> {
    let text = text.trim();
    if let Some(v) = try_parse(text) {
        return Some(v);
    }
    // Strip a markdown fence if present.
    if let Some(rest) = text.strip_prefix("```") {
        let rest = rest.trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == '\n');
        if let Some(v) = try_parse(rest.trim()) {
            return Some(v);
        }
    }
    let bytes = text.as_bytes();
    let mut start: Option<usize> = None;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' | b'[' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        if let Some(v) = try_parse(&text[s..=i]) {
                            return Some(v);
                        }
                    }
                    start = None;
                }
            }
            _ => {}
        }
    }
    None
}

fn try_parse(s: &str) -> Option<Value> {
    serde_json::from_str::<Value>(s).ok()
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_prose() {
        let v = parse_first_json("Here you go:\n{\"a\":1}\nDone.").unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn extracts_fenced_json() {
        let v = parse_first_json("```json\n{\"b\":[1,2]}\n```").unwrap();
        assert_eq!(v["b"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn handles_nested_braces_in_strings() {
        let v = parse_first_json(r#"prefix {"k":"a } b"} suffix"#).unwrap();
        assert_eq!(v["k"], "a } b");
    }

    #[test]
    fn no_json_is_none() {
        assert!(parse_first_json("no json at all").is_none());
    }
}
