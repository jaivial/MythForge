//! The MythForge agent loop: an Anthropic Messages conversation with tool use.
//!
//! Ported from MythAgent's `backend/agent.py` loop (and MythCortex's custom-agent
//! runner): send messages + tools, execute every `tool_use` block against the
//! platform's operation registry, feed `tool_result`s back, repeat until the
//! model stops asking for tools. Bounded by `MAX_ROUNDS` so a runaway model
//! cannot loop forever.

use misanthropy::{Content, Message, MessagesRequest, MessagesResponse, Role, Tool, ToolChoice};
use serde_json::{json, Value};

use crate::ai::client::Ai;
use crate::error::Result;

/// Hard cap on tool rounds per turn.
pub const MAX_ROUNDS: usize = 8;

/// A callable operation the platform exposes to agents, mirroring MythCortex's
/// `cortex` tool shape: the agent names an operation id, never a URL, so it
/// cannot reach anything the registry does not expose.
pub trait Operation {
    fn id(&self) -> &str;
    fn module(&self) -> &str;
    fn description(&self) -> &str;
    fn write(&self) -> bool;
    /// JSON schema for the operation input.
    fn input_schema(&self) -> Value;
    /// Execute against the given tenant pool.
    fn call<'a>(
        &self,
        ctx: &'a OpContext<'a>,
        input: Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>>;
}

/// Everything an operation needs to run for one company.
pub struct OpContext<'a> {
    pub company_id: uuid::Uuid,
    pub company_slug: String,
    pub database_name: String,
    pub pool: &'a sqlx::PgPool,
    pub user_id: uuid::Uuid,
}

impl Clone for OpContext<'_> {
    fn clone(&self) -> Self {
        Self {
            company_id: self.company_id,
            company_slug: self.company_slug.clone(),
            database_name: self.database_name.clone(),
            pool: self.pool,
            user_id: self.user_id,
        }
    }
}

/// Registry of operations available in this process.
#[derive(Default, Clone)]
pub struct Registry {
    ops: Vec<std::sync::Arc<dyn Operation + Send + Sync>>,
}

impl Registry {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn register(&mut self, op: std::sync::Arc<dyn Operation + Send + Sync>) {
        self.ops.push(op);
    }

    pub fn list(&self) -> Vec<Value> {
        self.ops
            .iter()
            .map(|o| {
                json!({
                    "id": o.id(),
                    "module": o.module(),
                    "description": o.description(),
                    "write": o.write(),
                    "parameters": o.input_schema(),
                })
            })
            .collect()
    }

    pub fn describe(&self, id: &str) -> Option<Value> {
        self.ops.iter().find(|o| o.id() == id).map(|_| self.search(id, 1).into_iter().next().unwrap_or(json!({})))
    }

    pub fn get(&self, id: &str) -> Option<std::sync::Arc<dyn Operation + Send + Sync>> {
        self.ops.iter().find(|o| o.id() == id).cloned()
    }

    /// Every registered operation (used to build the tool list).
    pub fn ops(&self) -> &[std::sync::Arc<dyn Operation + Send + Sync>] {
        &self.ops
    }

    /// Module-aware search, as in MythCortex v1.3: operations of a module the
    /// request names rank first, then lexical order.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Value> {
        let q = fold(query);
        let mut scored: Vec<(i64, Value)> = self
            .ops
            .iter()
            .map(|o| {
                let id = o.id();
                let hay = format!("{} {} {}", id, o.module(), o.description());
                let score = lexical_score(&fold(&hay), &q);
                (score, json!({ "id": id, "module": o.module(), "description": o.description(), "write": o.write(), "parameters": o.input_schema() }))
            })
            .filter(|(s, _)| q.is_empty() || *s > 0)
            .collect();
        // An empty query lists everything (registration order), a query ranks by score.
        if q.is_empty() {
            return scored.into_iter().take(limit).map(|(_, v)| v).collect();
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1["id"].to_string().cmp(&b.1["id"].to_string())));
        scored.into_iter().take(limit).map(|(_, v)| v).collect()
    }
}

fn fold(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lexical_score(haystack: &str, needle: &str) -> i64 {
    let mut score = 0i64;
    for word in needle.split_whitespace() {
        if haystack.contains(word) {
            score += 1;
        }
    }
    score
}

/// Outcome of one agent turn.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TurnResult {
    pub text: String,
    pub tool_calls: Vec<Value>,
    pub rounds: usize,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Run one agent turn: conversation in, final answer + trace out.
pub async fn run_turn(
    ai: &Ai,
    registry: &Registry,
    ctx: &OpContext<'_>,
    system: &str,
    history: &[Value],
    user_message: &str,
) -> Result<TurnResult> {
    let mut request = MessagesRequest::default()
        .with_model(&ai.model)
        .with_max_tokens(ai.max_tokens)
        .with_tool_choice(ToolChoice::Auto);
    if !system.is_empty() {
        request = request.with_system(vec![Content::text(system)]);
    }
    // Expose every registry operation as an Anthropic tool. The model names an
    // operation id (never a URL), exactly like MythCortex's `cortex` tool shape.
    for op in registry.ops() {
        request.tools.push(tool_from(op.as_ref()));
    }
    // Replay prior turns.
    for m in history {
        if let Some(text) = m.get("content").and_then(|c| c.as_str()) {
            let role = if m.get("role") == Some(&json!("assistant")) {
                Role::Assistant
            } else {
                Role::User
            };
            let mut msg = Message::new(role);
            msg.content.push(Content::text(text));
            request.messages.push(msg);
        }
    }
    let mut msg = Message::new(Role::User);
    msg.content.push(Content::text(user_message));
    request.messages.push(msg);

    let mut tool_calls: Vec<Value> = Vec::new();
    let mut final_text = String::new();
    let mut rounds = 0usize;
    let (mut in_tok, mut out_tok) = (0u32, 0u32);

    loop {
        if rounds >= MAX_ROUNDS {
            final_text.push_str("\n\n(tool round limit reached; stopping)");
            break;
        }
        rounds += 1;
        let resp: MessagesResponse = ai.anthropic().messages(&request).await?;
        if let Some(t) = resp.usage.input_tokens {
            in_tok += t;
        }
        if let Some(t) = resp.usage.output_tokens {
            out_tok += t;
        }
        // Assistant message goes back into the conversation verbatim.
        request.messages.push(Message {
            role: Role::Assistant,
            content: resp.content.clone(),
        });

        let tool_uses: Vec<misanthropy::ToolUse> = resp
            .content
            .iter()
            .filter_map(|c| match c {
                Content::ToolUse(t) => Some(t.clone()),
                _ => None,
            })
            .collect();

        for c in &resp.content {
            if let Content::Text(t) = c {
                final_text.push_str(&t.text);
            }
        }

        if tool_uses.is_empty() {
            break;
        }

        let mut result_msg = Message::new(Role::User);
        for tu in tool_uses {
            let (ok, out) = match registry.get(&tu.name) {
                Some(op) => match op.call(ctx, tu.input.clone()).await {
                    Ok(v) => (true, v),
                    Err(e) => (false, json!({ "error": e.to_string() })),
                },
                None => (
                    false,
                    json!({ "error": format!("unknown_operation {}", tu.name), "available": registry.list().iter().map(|o| o["id"].clone()).collect::<Vec<_>>() }),
                ),
            };
            tool_calls.push(json!({
                "id": tu.id,
                "name": tu.name,
                "input": tu.input,
                "ok": ok,
                "output": out,
            }));
            let payload = serde_json::to_string(&out).unwrap_or_else(|_| "{}".to_string());
            let tr = if ok {
                misanthropy::ToolResult::new(tu.id.clone(), payload)
            } else {
                misanthropy::ToolResult {
                    tool_use_id: tu.id.clone(),
                    content: payload,
                    is_error: true,
                }
            };
            result_msg.content.push(Content::ToolResult(tr));
        }
        request.messages.push(result_msg);
    }

    Ok(TurnResult {
        text: final_text.trim().to_string(),
        tool_calls,
        rounds,
        input_tokens: in_tok,
        output_tokens: out_tok,
    })
}

/// Build an Anthropic `Tool` from a registry operation.
fn tool_from(op: &(dyn Operation + Send + Sync)) -> Tool {
    let schema = op.input_schema();
    let parsed: schemars::Schema = serde_json::from_value(schema)
        .unwrap_or_else(|_| serde_json::from_value(json!({"type":"object","properties":{}})).unwrap());
    Tool::Custom {
        name: op.id().to_string(),
        description: op.description().to_string(),
        input_schema: parsed,
        cache_control: None,
    }
}
