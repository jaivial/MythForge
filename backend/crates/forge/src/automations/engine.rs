//! Automation execution: schedule triggers + record triggers -> agent runs.

use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::ai::agent::{self, OpContext};
use crate::api::state::SharedState;
use crate::error::{Error, Result};

/// Automations whose schedule says now, claimed with SKIP LOCKED.
pub async fn due_automations(state: &SharedState) -> Result<Vec<Uuid>> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT id FROM automation
           WHERE is_active
             AND trigger->>'kind' = 'schedule'
             AND (last_run_at IS NULL
                  OR last_run_at < now() - make_interval(secs => LEAST(COALESCE((trigger->>'interval_seconds')::bigint, 3600), 86400)))
           FOR UPDATE SKIP LOCKED"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// Run one automation now (admin-triggered), returning the run detail.
pub async fn run_by_id(state: &SharedState, company_id: &Uuid, id: Uuid) -> Result<Value> {
    let row: Option<(Uuid, Option<Uuid>, Value, Value)> = sqlx::query_as(
        "SELECT company_id, agent_id, trigger, action FROM automation WHERE id = $1 AND company_id = $2",
    )
    .bind(id)
    .bind(company_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((owner, agent_id, trigger, action)) = row else {
        return Err(Error::NotFound(format!("automation {id}")));
    };
    execute(state, owner, agent_id, id, &trigger, &action).await?;
    Ok(json!({ "ran": true, "id": id }))
}

/// Run one automation by id within its own company (scheduler path).
pub async fn run_by_id_no_company(state: &SharedState, id: Uuid) -> Result<()> {
    let row: Option<(Uuid, Option<Uuid>, Option<Uuid>, Value, Value)> = sqlx::query_as(
        "SELECT company_id, agent_id, id, trigger, action FROM automation WHERE id = $1 AND is_active",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((company_id, agent_id, _id, trigger, action)) = row else {
        return Ok(());
    };
    execute(state, company_id, agent_id, id, &trigger, &action).await
}

/// Record-trigger path: called after every create/update on the data plane.
pub async fn fire_record_trigger(
    state: &SharedState,
    company_id: &Uuid,
    _pool: &Arc<sqlx::PgPool>,
    module: &str,
    entity: &str,
    kind: &str,
    record: &Value,
) {
    let rows: Vec<(Uuid, Option<Uuid>, Value, Value)> = sqlx::query_as(
        r#"SELECT id, agent_id, trigger, action FROM automation
           WHERE company_id = $1 AND is_active AND trigger->>'kind' = $2
             AND (trigger->>'entity' IS NULL OR trigger->>'entity' = $3)"#,
    )
    .bind(company_id)
    .bind(kind)
    .bind(entity)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    for (id, agent_id, trigger, action) in rows {
        let ctx_prompt = action["prompt"].as_str().unwrap_or("").to_string();
        let enriched = format!(
            "{ctx_prompt}\n\nContext: automation trigger {kind} on {module}/{entity}. Record: {}",
            serde_json::to_string(&record).unwrap_or_default()
        );
        let mut act = action.clone();
        act["prompt"] = json!(enriched);
        let _ = execute(state, *company_id, agent_id, id, &trigger, &act).await;
        let _ = module;
    }
}

async fn execute(
    state: &SharedState,
    company_id: Uuid,
    agent_id: Option<Uuid>,
    automation_id: Uuid,
    trigger: &Value,
    action: &Value,
) -> Result<()> {
    let started = chrono::Utc::now();
    let db_row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT database_name FROM company WHERE id = $1")
            .bind(company_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some((Some(database_name),)) = db_row else {
        return Err(Error::NotFound(format!("company {company_id} database")));
    };
    let pool = state.tenant_pool(&database_name).await?;

    let (system_prompt, bound) = match agent_id {
        Some(aid) => {
            let row: Option<(String, Value)> = sqlx::query_as(
                "SELECT system_prompt, tools FROM agent WHERE id = $1 AND is_active",
            )
            .bind(aid)
            .fetch_optional(&state.pool)
            .await?;
            match row {
                Some((sp, tools)) => (
                    sp,
                    tools.as_array().cloned().unwrap_or_default()
                        .into_iter().filter_map(|t| t.as_str().map(String::from)).collect::<Vec<_>>(),
                ),
                None => (String::new(), Vec::new()),
            }
        }
        None => (String::new(), Vec::new()),
    };

    let registry = if bound.is_empty() {
        state.registry.clone()
    } else {
        crate::agents::tools::sub_registry(&state.registry, &bound)
    };

    let ctx = OpContext {
        company_id,
        company_slug: String::new(),
        database_name: database_name.clone(),
        pool: &pool,
        user_id: Uuid::nil(),
    };
    let prompt = action["prompt"].as_str().unwrap_or("Run your automation task.");
    let outcome = agent::run_turn(&state.ai, &registry, &ctx, &system_prompt, &[], prompt).await;

    let (status, detail) = match outcome {
        Ok(r) => ("ok", json!({ "text": r.text, "tool_calls": r.tool_calls, "rounds": r.rounds })),
        Err(e) => ("error", json!({ "error": e.to_string() })),
    };
    sqlx::query(
        "INSERT INTO mf_automation_run (id, automation_id, status, started_at, finished_at, detail) VALUES ($1,$2,$3,$4,now(),$5::jsonb)",
    )
    .bind(Uuid::new_v4())
    .bind(automation_id)
    .bind(status)
    .bind(started)
    .bind(detail.to_string())
    .execute(&*pool)
    .await?;
    sqlx::query("UPDATE automation SET last_run_at = now(), run_count = run_count + 1 WHERE id = $1")
        .bind(automation_id)
        .execute(&state.pool)
        .await?;
    let _ = trigger;
    Ok(())
}
