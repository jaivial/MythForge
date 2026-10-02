//! The workspace assistant chat: history persistence + system prompt.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::api::state::SharedState;
use crate::error::Result;

pub const SYSTEM_PROMPT: &str = r#"You are the MythForge assistant inside a company's ERP/CRM workspace.

You can build and change the workspace with the operations available to you: list modules and entities, create or change records, and inspect the blueprint. When the user asks for something the workspace does not have yet, explain that they can use the Build prompt to add it, and suggest exactly what to ask for.

Be concise and concrete. Use the user's language (Spanish/English). Never invent record ids; look them up with the operations first."#;

/// Recent chat history for a company, oldest first.
pub async fn history(state: &SharedState, company_id: Uuid, limit: usize) -> Result<Vec<Value>> {
    let rows = sqlx::query_as::<_, (String, String)>(
        r#"SELECT role, content FROM chat_message m JOIN chat c ON c.id = m.chat_id
           WHERE c.company_id = $1 ORDER BY m.created_at DESC LIMIT $2"#,
    )
    .bind(company_id)
    .bind(limit as i64)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .rev()
        .map(|(role, content)| json!({ "role": role, "content": content }))
        .collect())
}

/// Append a message to the company's chat thread (creating it on first use).
pub async fn append(
    state: &SharedState,
    company_id: Uuid,
    user_id: Uuid,
    role: &str,
    content: &str,
    tool_calls: &[Value],
) -> Result<()> {
    let chat_id: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM chat WHERE company_id = $1 ORDER BY created_at LIMIT 1")
            .bind(company_id)
            .fetch_optional(&state.pool)
            .await?;
    let chat_id = match chat_id {
        Some((id,)) => id,
        None => {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO chat (id, company_id, user_id) VALUES ($1,$2,$3)")
                .bind(id)
                .bind(company_id)
                .bind(user_id)
                .execute(&state.pool)
                .await?;
            id
        }
    };
    sqlx::query(
        "INSERT INTO chat_message (id, chat_id, role, content, tool_calls) VALUES ($1,$2,$3,$4,$5::jsonb)",
    )
    .bind(Uuid::new_v4())
    .bind(chat_id)
    .bind(role)
    .bind(content)
    .bind(serde_json::to_string(tool_calls).unwrap_or_else(|_| "[]".into()))
    .execute(&state.pool)
    .await?;
    Ok(())
}
