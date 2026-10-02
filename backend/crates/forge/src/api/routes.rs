//! Every HTTP handler. One reusable set of paths serves all companies.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::ai::agent::{self, OpContext};
use crate::api::extractors::{company_pool, AdminUser, CurrentUser};
use crate::api::state::SharedState;
use crate::db::tenant;
use crate::error::{Error, Result};
use crate::forge::{generate, runtime};

// ---------------------------------------------------------------------------
// health / meta
// ---------------------------------------------------------------------------

pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "mythforge", "version": env!("CARGO_PKG_VERSION") }))
}

pub async fn catalog(State(_s): State<SharedState>) -> Result<Json<Value>> {
    Ok(Json(crate::forge::catalog::catalog_json()))
}

// ---------------------------------------------------------------------------
// auth
// ---------------------------------------------------------------------------

pub async fn signup(
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let email = body["email"].as_str().unwrap_or("").trim().to_lowercase();
    let password = body["password"].as_str().unwrap_or("");
    let name = body["name"].as_str().unwrap_or("").trim();
    let company_name = body["company_name"].as_str().unwrap_or("").trim();
    let template = body["template"].as_str().unwrap_or("blank");
    if email.is_empty() || !email.contains('@') {
        return Err(Error::BadRequest("a valid email is required".into()));
    }
    if password.len() < 8 {
        return Err(Error::BadRequest("password must be at least 8 characters".into()));
    }
    if name.is_empty() {
        return Err(Error::BadRequest("name is required".into()));
    }
    if company_name.is_empty() {
        return Err(Error::BadRequest("company_name is required".into()));
    }
    if !crate::forge::catalog::is_template(template) {
        return Err(Error::BadRequest(format!("unknown template {template}")));
    }

    let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM app_user WHERE email = $1")
        .bind(&email)
        .fetch_optional(&state.pool)
        .await?;
    if existing.is_some() {
        return Err(Error::Conflict("an account with this email already exists".into()));
    }

    let hash = crate::auth::hash_password(password)?;
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO app_user (id, email, name, password_hash) VALUES ($1,$2,$3,$4)",
    )
    .bind(user_id)
    .bind(&email)
    .bind(name)
    .bind(&hash)
    .execute(&state.pool)
    .await?;

    let company_id = Uuid::new_v4();
    let company_slug = unique_company_slug(&state, company_name).await?;
    sqlx::query(
        "INSERT INTO company (id, name, slug, template, created_by) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(company_id)
    .bind(company_name)
    .bind(&company_slug)
    .bind(template)
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    sqlx::query("INSERT INTO membership (id, user_id, company_id, role) VALUES ($1,$2,$3,'owner')")
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(company_id)
        .execute(&state.pool)
        .await?;

    // Provision the per-company database (and base schema) now.
    let info = tenant::provision(&state.admin_pool, company_id, company_name).await?;
    sqlx::query("UPDATE company SET database_name = $1 WHERE id = $2")
        .bind(&info.database_name)
        .bind(company_id)
        .execute(&state.pool)
        .await?;

    // Seed the chosen template's modules.
    if template != "blank" {
        let pool = state.tenant_pool(&info.database_name).await?;
        let tpl = template_blueprint(template);
        generate::apply_blueprint(&pool, &tpl).await?;
    }

    let token = state.jwt.issue(user_id, company_id, "owner")?;
    Ok(Json(json!({
        "token": token,
        "user": { "id": user_id, "email": email, "name": name },
        "company": { "id": company_id, "name": company_name, "slug": company_slug, "template": template, "database": info.database_name },
    })))
}

/// Deterministic starter blueprints for templates, so a new company gets a
/// usable workspace in one click even before any prompt.
fn template_blueprint(template: &str) -> Value {
    match template {
        "crm" => json!({
            "summary": "CRM starter",
            "modules": [{
                "slug": "crm", "name": "CRM", "icon": "users",
                "entities": [
                    { "slug": "lead", "name": "Lead", "fields": [
                        { "slug": "name", "name": "Name", "type": "text", "required": true },
                        { "slug": "email", "name": "Email", "type": "email" },
                        { "slug": "stage", "name": "Stage", "type": "select", "choices": ["New","Contacted","Qualified","Won","Lost"] },
                        { "slug": "value", "name": "Value", "type": "money" }
                    ], "views": [
                        { "slug": "board", "name": "Board", "kind": "kanban", "group_by": "stage" },
                        { "slug": "list", "name": "Leads", "kind": "table" }
                    ]},
                    { "slug": "contact", "name": "Contact", "fields": [
                        { "slug": "name", "name": "Name", "type": "text", "required": true },
                        { "slug": "email", "name": "Email", "type": "email" },
                        { "slug": "phone", "name": "Phone", "type": "phone" },
                        { "slug": "company", "name": "Company", "type": "text" }
                    ], "views": [ { "slug": "list", "name": "Contacts", "kind": "table" } ]}
                ]
            }]
        }),
        "erp" => json!({
            "summary": "ERP starter",
            "modules": [
                { "slug": "inventory", "name": "Inventory", "icon": "package", "entities": [
                    { "slug": "product", "name": "Product", "fields": [
                        { "slug": "sku", "name": "SKU", "type": "text", "required": true },
                        { "slug": "name", "name": "Name", "type": "text", "required": true },
                        { "slug": "stock", "name": "Stock", "type": "number" },
                        { "slug": "cost", "name": "Cost", "type": "money" }
                    ], "views": [ { "slug": "list", "name": "Products", "kind": "table" } ]}
                ]},
                { "slug": "sales", "name": "Sales", "icon": "cart", "entities": [
                    { "slug": "order", "name": "Order", "fields": [
                        { "slug": "number", "name": "Number", "type": "text", "required": true },
                        { "slug": "customer", "name": "Customer", "type": "text" },
                        { "slug": "total", "name": "Total", "type": "money" },
                        { "slug": "status", "name": "Status", "type": "select", "choices": ["Draft","Confirmed","Shipped","Invoiced"] }
                    ], "views": [
                        { "slug": "list", "name": "Orders", "kind": "table" },
                        { "slug": "board", "name": "Board", "kind": "kanban", "group_by": "status" }
                    ]}
                ]}
            ]
        }),
        "services" => json!({
            "summary": "Services starter",
            "modules": [{ "slug": "tickets", "name": "Tickets", "icon": "life-buoy", "entities": [
                { "slug": "ticket", "name": "Ticket", "fields": [
                    { "slug": "subject", "name": "Subject", "type": "text", "required": true },
                    { "slug": "client", "name": "Client", "type": "text" },
                    { "slug": "priority", "name": "Priority", "type": "select", "choices": ["Low","Normal","High","Urgent"] },
                    { "slug": "status", "name": "Status", "type": "select", "choices": ["Open","In progress","Waiting","Closed"] }
                ], "views": [ { "slug": "board", "name": "Board", "kind": "kanban", "group_by": "status" } ]}
            ]}]
        }),
        "retail" => json!({
            "summary": "Retail starter",
            "modules": [{ "slug": "catalog", "name": "Catalog", "icon": "tag", "entities": [
                { "slug": "item", "name": "Item", "fields": [
                    { "slug": "name", "name": "Name", "type": "text", "required": true },
                    { "slug": "price", "name": "Price", "type": "money" },
                    { "slug": "category", "name": "Category", "type": "select", "choices": ["General","Food","Drinks","Other"] }
                ], "views": [ { "slug": "list", "name": "Items", "kind": "table" } ]}
            ]}]
        }),
        _ => json!({ "summary": "Blank", "modules": [] }),
    }
}

async fn unique_company_slug(state: &SharedState, name: &str) -> Result<String> {
    let base = tenant::slugify(name);
    let mut candidate = base.clone();
    for i in 2..50 {
        let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM company WHERE slug = $1)")
            .bind(&candidate)
            .fetch_one(&state.pool)
            .await?;
        if !taken {
            return Ok(candidate);
        }
        candidate = format!("{base}-{i}");
    }
    Ok(format!("{base}-{}", Uuid::new_v4().simple()))
}

pub async fn login(
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let email = body["email"].as_str().unwrap_or("").trim().to_lowercase();
    let password = body["password"].as_str().unwrap_or("");
    let row: Option<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, name, password_hash FROM app_user WHERE email = $1")
            .bind(&email)
            .fetch_optional(&state.pool)
            .await?;
    let Some((user_id, name, hash)) = row else {
        return Err(Error::Unauthorized("invalid email or password".into()));
    };
    if !crate::auth::verify_password(&hash, password)? {
        return Err(Error::Unauthorized("invalid email or password".into()));
    }
    let membership: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT company_id, role FROM membership WHERE user_id = $1 ORDER BY created_at LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    let (company_id, role) = membership.unwrap_or((Uuid::nil(), "member".to_string()));
    let token = state.jwt.issue(user_id, company_id, &role)?;
    Ok(Json(json!({
        "token": token,
        "user": { "id": user_id, "email": email, "name": name },
        "company_id": company_id,
    })))
}

pub async fn me(State(state): State<SharedState>, user: CurrentUser) -> Result<Json<Value>> {
    let user_row: (String, String, Option<String>) = sqlx::query_as(
        "SELECT email, name, avatar_url FROM app_user WHERE id = $1",
    )
    .bind(user.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| Error::NotFound("user".into()))?;
    let companies = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
        "SELECT c.id, c.name, c.slug, c.database_name FROM company c
         JOIN membership m ON m.company_id = c.id WHERE m.user_id = $1 ORDER BY c.created_at",
    )
    .bind(user.user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({
        "user": { "id": user.user_id, "email": user_row.0, "name": user_row.1, "avatar_url": user_row.2 },
        "role": user.role,
        "companies": companies.iter().map(|(id, n, s, d)| json!({
            "id": id, "name": n, "slug": s, "database": d
        })).collect::<Vec<_>>(),
    })))
}

// ---------------------------------------------------------------------------
// workspace / blueprint
// ---------------------------------------------------------------------------

pub async fn blueprint(
    State(state): State<SharedState>,
    user: CurrentUser,
) -> Result<Json<Value>> {
    let (_cid, pool) = company_pool(&state, &user, None).await?;
    Ok(Json(runtime::blueprint(&pool).await?))
}

/// The core prompt endpoint: user text -> plan -> validate -> apply.
pub async fn build(
    State(state): State<SharedState>,
    user: CurrentUser,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let prompt = body["prompt"].as_str().unwrap_or("").trim();
    if prompt.is_empty() {
        return Err(Error::BadRequest("prompt is required".into()));
    }
    let (company_id, pool) = company_pool(&state, &user, None).await?;
    let current = runtime::blueprint(&pool).await?;
    let raw = generate::plan(&state.ai, prompt, &current).await?;
    let plan = generate::validate_blueprint(&raw)?;
    generate::apply_blueprint(&pool, &plan).await?;
    audit(&state, company_id, Some(user.user_id), "forge.build", &json!({ "prompt": prompt, "summary": plan["summary"] })).await;
    let after = runtime::blueprint(&pool).await?;
    Ok(Json(json!({
        "summary": plan["summary"],
        "applied": plan["modules"],
        "blueprint": after,
    })))
}

// ---------------------------------------------------------------------------
// generic data plane
// ---------------------------------------------------------------------------

pub async fn list_records(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path((module, entity)): Path<(String, String)>,
    Query(q): Query<Vec<(String, String)>>,
) -> Result<Json<Value>> {
    let (_cid, pool) = company_pool(&state, &user, None).await?;
    let search = q.iter().find(|(k, _)| k == "q").map(|(_, v)| v.clone());
    let (limit, offset) = pagination(&q);
    let filters: Vec<(String, String)> = q
        .iter()
        .filter(|(k, _)| k.starts_with("f_"))
        .map(|(k, v)| (k.trim_start_matches("f_").to_string(), v.clone()))
        .collect();
    Ok(Json(
        runtime::list_records(&pool, &module, &entity, search.as_deref(), &filters, limit, offset).await?,
    ))
}

pub async fn create_record(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path((module, entity)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>)> {
    let (company_id, pool) = company_pool(&state, &user, None).await?;
    let rec = runtime::create_record(&pool, &module, &entity, &body).await?;
    fire_record_triggers(&state, &company_id, &pool, &module, &entity, "record_created", &rec).await;
    audit(&state, company_id, Some(user.user_id), "record.create", &json!({"module": module, "entity": entity, "id": rec["id"]})).await;
    Ok((StatusCode::CREATED, Json(rec)))
}

pub async fn get_record(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path((module, entity, id)): Path<(String, String, Uuid)>,
) -> Result<Json<Value>> {
    let (_cid, pool) = company_pool(&state, &user, None).await?;
    Ok(Json(runtime::get_record(&pool, &module, &entity, id).await?))
}

pub async fn update_record(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path((module, entity, id)): Path<(String, String, Uuid)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let (company_id, pool) = company_pool(&state, &user, None).await?;
    let rec = runtime::update_record(&pool, &module, &entity, id, &body).await?;
    fire_record_triggers(&state, &company_id, &pool, &module, &entity, "record_updated", &rec).await;
    audit(&state, company_id, Some(user.user_id), "record.update", &json!({"module": module, "entity": entity, "id": id})).await;
    Ok(Json(rec))
}

pub async fn delete_record(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path((module, entity, id)): Path<(String, String, Uuid)>,
) -> Result<Json<Value>> {
    let (company_id, pool) = company_pool(&state, &user, None).await?;
    let out = runtime::delete_record(&pool, &module, &entity, id).await?;
    audit(&state, company_id, Some(user.user_id), "record.delete", &json!({"module": module, "entity": entity, "id": id})).await;
    Ok(Json(out))
}

fn pagination(q: &[(String, String)]) -> (i64, i64) {
    let limit = q
        .iter()
        .find(|(k, _)| k == "limit")
        .and_then(|(_, v)| v.parse::<i64>().ok())
        .unwrap_or(25)
        .clamp(1, 200);
    let offset = q
        .iter()
        .find(|(k, _)| k == "offset")
        .and_then(|(_, v)| v.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    (limit, offset)
}

// ---------------------------------------------------------------------------
// mascots / agents / automations
// ---------------------------------------------------------------------------

pub async fn list_mascots(
    State(state): State<SharedState>,
    user: CurrentUser,
) -> Result<Json<Value>> {
    let (_cid, _pool) = company_pool(&state, &user, None).await?;
    let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>, String, String, String, String)>(
        "SELECT id, slug, name, tagline, persona, greeting, glyph, accent FROM mascot WHERE company_id = $1 ORDER BY name",
    )
    .bind(user.company_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "items": rows.iter().map(|(id, slug, name, tagline, persona, greeting, glyph, accent)| json!({
        "id": id, "slug": slug, "name": name, "tagline": tagline, "persona": persona,
        "greeting": greeting, "glyph": glyph, "accent": accent
    })).collect::<Vec<_>>() })))
}

pub async fn create_mascot(
    State(state): State<SharedState>,
    AdminUser(user): AdminUser,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>)> {
    let name = body["name"].as_str().unwrap_or("").trim();
    if name.is_empty() {
        return Err(Error::BadRequest("mascot name is required".into()));
    }
    let slug = generate::slug(name);
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO mascot (id, company_id, slug, name, tagline, persona, greeting, glyph, accent)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
           ON CONFLICT (company_id, slug) DO UPDATE SET name = EXCLUDED.name, persona = EXCLUDED.persona"#,
    )
    .bind(id)
    .bind(user.company_id)
    .bind(&slug)
    .bind(name)
    .bind(body["tagline"].as_str())
    .bind(body["persona"].as_str().unwrap_or(""))
    .bind(body["greeting"].as_str().unwrap_or(""))
    .bind(body["glyph"].as_str().unwrap_or("\u{25c6}"))
    .bind(body["accent"].as_str().unwrap_or("silver"))
    .execute(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "slug": slug, "name": name }))))
}

pub async fn list_agents(
    State(state): State<SharedState>,
    user: CurrentUser,
) -> Result<Json<Value>> {
    let (_cid, _pool) = company_pool(&state, &user, None).await?;
    let rows = sqlx::query_as::<_, (Uuid, Option<Uuid>, String, String, String, Value, bool)>(
        "SELECT id, mascot_id, slug, name, description, tools, is_active FROM agent WHERE company_id = $1 ORDER BY name",
    )
    .bind(user.company_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "items": rows.iter().map(|(id, mascot, slug, name, desc, tools, active)| json!({
        "id": id, "mascot_id": mascot, "slug": slug, "name": name, "description": desc,
        "tools": tools, "is_active": active
    })).collect::<Vec<_>>() })))
}

/// Compose an agent from a prompt: the model drafts system prompt + tool
/// bindings from the registry (MythCortex's custom-agent contract), the user
/// reviews, then creates.
pub async fn compose_agent(
    State(state): State<SharedState>,
    AdminUser(user): AdminUser,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let prompt = body["prompt"].as_str().unwrap_or("").trim();
    if prompt.is_empty() {
        return Err(Error::BadRequest("prompt is required".into()));
    }
    let (_cid, pool) = company_pool(&state, &user, None).await?;
    let ops = state.registry.search("", 50);
    let system = format!(
        "You are MythForge's agent composer. Given a request and the operation registry, draft ONE agent as JSON: {{\"name\":\"...\",\"description\":\"...\",\"system_prompt\":\"...\",\"tools\":[operation ids]}}. Use only registry operation ids. Answer JSON only."
    );
    let user_msg = json!({
        "request": prompt,
        "operation_registry": ops,
        "company_blueprint": runtime::blueprint(&pool).await?,
    })
    .to_string();
    let drafted = state.ai.complete_json(&system, &user_msg).await?;
    // Only registry ids survive.
    let mut tools: Vec<String> = Vec::new();
    for t in drafted["tools"].as_array().cloned().unwrap_or_default() {
        if let Some(id) = t.as_str() {
            if state.registry.get(id).is_some() {
                tools.push(id.to_string());
            }
        }
    }
    Ok(Json(json!({ "draft": drafted, "tools": tools, "registry_size": state.registry.ops().len() })))
}

pub async fn create_agent(
    State(state): State<SharedState>,
    AdminUser(user): AdminUser,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>)> {
    let name = body["name"].as_str().unwrap_or("").trim();
    if name.is_empty() {
        return Err(Error::BadRequest("agent name is required".into()));
    }
    let slug = generate::slug(name);
    let id = Uuid::new_v4();
    let mut tools: Vec<String> = Vec::new();
    for t in body["tools"].as_array().cloned().unwrap_or_default() {
        if let Some(id) = t.as_str() {
            if state.registry.get(id).is_some() {
                tools.push(id.to_string());
            }
        }
    }
    sqlx::query(
        r#"INSERT INTO agent (id, company_id, mascot_id, slug, name, description, system_prompt, model, tools, created_from)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9::jsonb,$10::jsonb)
           ON CONFLICT (company_id, slug) DO UPDATE SET system_prompt = EXCLUDED.system_prompt,
             tools = EXCLUDED.tools, updated_at = now()"#,
    )
    .bind(id)
    .bind(user.company_id)
    .bind(body["mascot_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()))
    .bind(&slug)
    .bind(name)
    .bind(body["description"].as_str().unwrap_or(""))
    .bind(body["system_prompt"].as_str().unwrap_or(""))
    .bind(body["model"].as_str())
    .bind(serde_json::to_string(&tools).unwrap_or_else(|_| "[]".into()))
    .bind(body["created_from"].to_string())
    .execute(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "slug": slug, "name": name, "tools": tools }))))
}

/// Run an agent turn with its bound tools.
pub async fn run_agent(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path(agent_slug): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let message = body["message"].as_str().unwrap_or("").trim();
    if message.is_empty() {
        return Err(Error::BadRequest("message is required".into()));
    }
    let (company_id, pool) = company_pool(&state, &user, None).await?;
    let row: Option<(String, Option<String>, Value)> = sqlx::query_as(
        "SELECT system_prompt, model, tools FROM agent WHERE company_id = $1 AND slug = $2 AND is_active",
    )
    .bind(user.company_id)
    .bind(&agent_slug)
    .fetch_optional(&state.pool)
    .await?;
    let Some((system_prompt, _model, tools)) = row else {
        return Err(Error::NotFound(format!("agent {agent_slug}")));
    };
    let bound: Vec<String> = tools.as_array().cloned().unwrap_or_default().iter().filter_map(|t| t.as_str().map(String::from)).collect();
    let registry = crate::agents::tools::sub_registry(&state.registry, &bound);
    let ctx = OpContext {
        company_id,
        company_slug: String::new(),
        database_name: String::new(),
        pool: &pool,
        user_id: user.user_id,
    };
    let history: Vec<Value> = body["history"].as_array().cloned().unwrap_or_default();
    let result = agent::run_turn(&state.ai, &registry, &ctx, &system_prompt, &history, message).await?;
    audit(&state, company_id, Some(user.user_id), "agent.run", &json!({"agent": agent_slug, "rounds": result.rounds})).await;
    Ok(Json(serde_json::to_value(&result)?))
}

// ---------------------------------------------------------------------------
// automations
// ---------------------------------------------------------------------------

pub async fn list_automations(
    State(state): State<SharedState>,
    user: CurrentUser,
) -> Result<Json<Value>> {
    company_pool(&state, &user, None).await?;
    let rows = sqlx::query_as::<_, (Uuid, Option<Uuid>, String, String, Value, Value, bool, Option<chrono::DateTime<chrono::Utc>>, i32)>(
        "SELECT id, agent_id, name, description, trigger, action, is_active, last_run_at, run_count FROM automation WHERE company_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.company_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "items": rows.iter().map(|(id, agent, name, desc, trig, act, active, last, count)| json!({
        "id": id, "agent_id": agent, "name": name, "description": desc, "trigger": trig,
        "action": act, "is_active": active, "last_run_at": last, "run_count": count
    })).collect::<Vec<_>>() })))
}

/// Draft an automation (trigger + action + optional agent) from a natural
/// language prompt. Returns the draft for review; the UI creates it.
pub async fn compose_automation(
    State(state): State<SharedState>,
    AdminUser(user): AdminUser,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let prompt = body["prompt"].as_str().unwrap_or("").trim();
    if prompt.is_empty() {
        return Err(Error::BadRequest("prompt is required".into()));
    }
    let (_cid, pool) = company_pool(&state, &user, None).await?;
    let ops = state.registry.search("", 50);
    let agents: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, name FROM agent WHERE company_id = $1 ORDER BY name",
    )
    .bind(user.company_id)
    .fetch_all(&state.pool)
    .await?;
    let system = r#"You are MythForge's automation composer. Given a request, the operation registry and the company blueprint, draft ONE automation as JSON with keys: name, description, trigger, action, agent_id. trigger is {"kind":"schedule","interval_seconds":3600} or {"kind":"record_created","module":"...","entity":"..."} or {"kind":"record_updated","module":"...","entity":"..."}. action is {"prompt":"instruction the agent runs"}. Use module/entity slugs that exist in the blueprint and agent ids from the agent list (or null to use the default assistant). Answer JSON only."#;
    let user_msg = json!({
        "request": prompt,
        "operation_registry": ops,
        "company_blueprint": runtime::blueprint(&pool).await?,
        "agents": agents.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
    })
    .to_string();
    let drafted = state.ai.complete_json(&system, &user_msg).await?;
    // Normalise: only known trigger kinds, only blueprint modules/entities.
    let bp = runtime::blueprint(&pool).await?;
    let mut trigger = drafted["trigger"].clone();
    let kind = trigger["kind"].as_str().unwrap_or("schedule").to_string();
    let valid_kinds = ["schedule", "record_created", "record_updated"];
    if !valid_kinds.contains(&kind.as_str()) {
        trigger["kind"] = json!("schedule");
    }
    if kind != "schedule" {
        let module = trigger["module"].as_str().unwrap_or("").to_string();
        let entity = trigger["entity"].as_str().unwrap_or("").to_string();
        let module_ok = bp["modules"]
            .as_array()
            .map(|ms| ms.iter().any(|m| m["slug"].as_str() == Some(module.as_str())))
            .unwrap_or(false);
        let entity_ok = bp["modules"]
            .as_array()
            .map(|ms| {
                ms.iter().any(|m| {
                    m["entities"]
                        .as_array()
                        .map(|es| es.iter().any(|e| e["slug"].as_str() == Some(entity.as_str())))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if !module_ok || !entity_ok {
            trigger = json!({"kind": "schedule", "interval_seconds": 3600});
        }
    }
    let agent_id = drafted["agent_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .filter(|id| agents.iter().any(|(aid, _)| aid == id));
    Ok(Json(json!({
        "draft": {
            "name": drafted["name"].as_str().unwrap_or("New automation"),
            "description": drafted["description"].as_str().unwrap_or(""),
            "trigger": trigger,
            "action": if drafted["action"].is_object() {
                drafted["action"].clone()
            } else {
                json!({"prompt": drafted["action"].as_str().unwrap_or(prompt)})
            },
            "agent_id": agent_id,
        },
        "agents": agents.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
    })))
}

pub async fn create_automation(
    State(state): State<SharedState>,
    AdminUser(user): AdminUser,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>)> {
    let name = body["name"].as_str().unwrap_or("").trim();
    if name.is_empty() {
        return Err(Error::BadRequest("automation name is required".into()));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO automation (id, company_id, agent_id, name, description, trigger, action)
           VALUES ($1,$2,$3,$4,$5,$6::jsonb,$7::jsonb)"#,
    )
    .bind(id)
    .bind(user.company_id)
    .bind(body["agent_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()))
    .bind(name)
    .bind(body["description"].as_str().unwrap_or(""))
    .bind(body["trigger"].to_string())
    .bind(body["action"].to_string())
    .execute(&state.pool)
    .await?;
    crate::automations::scheduler::notify_changed(&state).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "name": name }))))
}

pub async fn delete_automation(
    State(state): State<SharedState>,
    AdminUser(user): AdminUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let rows = sqlx::query("DELETE FROM automation WHERE id = $1 AND company_id = $2")
        .bind(id)
        .bind(user.company_id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if rows == 0 {
        return Err(Error::NotFound(format!("automation {id}")));
    }
    crate::automations::scheduler::notify_changed(&state).await;
    Ok(Json(json!({ "deleted": true })))
}

pub async fn run_automation_now(
    State(state): State<SharedState>,
    AdminUser(user): AdminUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let out = crate::automations::engine::run_by_id(&state, &user.company_id, id).await?;
    Ok(Json(out))
}

// ---------------------------------------------------------------------------
// chat (assistant)
// ---------------------------------------------------------------------------

pub async fn chat(
    State(state): State<SharedState>,
    user: CurrentUser,
    Json(body): Json<Value>,
) -> Result<Json<Value>> {
    let message = body["message"].as_str().unwrap_or("").trim();
    if message.is_empty() {
        return Err(Error::BadRequest("message is required".into()));
    }
    let (company_id, pool) = company_pool(&state, &user, None).await?;
    let history = crate::agents::chat::history(&state, user.company_id, 12).await?;
    let system = crate::agents::chat::SYSTEM_PROMPT.to_string();
    let ctx = OpContext {
        company_id,
        company_slug: String::new(),
        database_name: String::new(),
        pool: &pool,
        user_id: user.user_id,
    };
    let result = agent::run_turn(&state.ai, &state.registry, &ctx, &system, &history, message).await?;
    crate::agents::chat::append(&state, user.company_id, user.user_id, "user", message, &[]).await?;
    crate::agents::chat::append(&state, user.company_id, user.user_id, "assistant", &result.text, &result.tool_calls).await?;
    audit(&state, company_id, Some(user.user_id), "chat", &json!({"rounds": result.rounds})).await;
    Ok(Json(serde_json::to_value(&result)?))
}

// ---------------------------------------------------------------------------
// google
// ---------------------------------------------------------------------------

pub async fn google_start(State(state): State<SharedState>) -> Result<Json<Value>> {
    if state.cfg.google_client_id.is_empty() {
        return Err(Error::BadRequest("google integration is not configured".into()));
    }
    Ok(Json(json!({ "client_id": state.cfg.google_client_id, "redirect_url": state.cfg.google_redirect_url, "scope": crate::google::oauth::SCOPES.join(" ") })))
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

pub async fn audit(
    state: &SharedState,
    company_id: Uuid,
    user_id: Option<Uuid>,
    action: &str,
    detail: &Value,
) {
    let _ = sqlx::query(
        "INSERT INTO audit_log (id, company_id, user_id, action, detail) VALUES ($1,$2,$3,$4,$5::jsonb)",
    )
    .bind(Uuid::new_v4())
    .bind(company_id)
    .bind(user_id)
    .bind(action)
    .bind(detail.to_string())
    .execute(&state.pool)
    .await;
}

/// Run record-triggered automations (fire-and-forget style, errors logged).
async fn fire_record_triggers(
    state: &SharedState,
    company_id: &Uuid,
    pool: &Arc<sqlx::PgPool>,
    module: &str,
    entity: &str,
    kind: &str,
    record: &Value,
) {
    crate::automations::engine::fire_record_trigger(state, company_id, pool, module, entity, kind, record).await;
}

// ---------------------------------------------------------------------------
// google callbacks (appended: status + callback)
// ---------------------------------------------------------------------------

pub async fn google_callback(
    State(state): State<SharedState>,
    Query(q): Query<Vec<(String, String)>>,
) -> Result<axum::response::Response> {
    use axum::response::Redirect;
    let code = q.iter().find(|(k, _)| k == "code").map(|(_, v)| v.clone());
    let state_param = q.iter().find(|(k, _)| k == "state").map(|(_, v)| v.clone());
    let err = q.iter().find(|(k, _)| k == "error").map(|(_, v)| v.clone());
    if let Some(e) = err {
        return Ok(Redirect::to(&format!("{}/settings?google_error={}", state.cfg.app_url, urlencoding::encode(&e))).into_response());
    }
    let (Some(code), Some(state_param)) = (code, state_param) else {
        return Ok(Redirect::to(&format!("{}/settings?google_error=missing_code", state.cfg.app_url)).into_response());
    };
    // state carries "<jwt>" issued at connect time.
    let claims = match state.jwt.verify(&state_param) {
        Ok(c) => c,
        Err(e) => {
            return Ok(Redirect::to(&format!("{}/settings?google_error={}", state.cfg.app_url, urlencoding::encode(&e.to_string()))).into_response())
        }
    };
    let gcfg = crate::google::oauth::GoogleConfig::from_cfg(&state.cfg)?;
    let http = reqwest::Client::new();
    match crate::google::oauth::exchange_and_store(&gcfg, &http, &state.pool, &code, claims.sub, claims.company_id).await {
        Ok(_) => Ok(Redirect::to(&format!("{}/settings?google=connected", state.cfg.app_url)).into_response()),
        Err(e) => Ok(Redirect::to(&format!("{}/settings?google_error={}", state.cfg.app_url, urlencoding::encode(&e.to_string()))).into_response()),
    }
}

pub async fn google_status(
    State(state): State<SharedState>,
    user: CurrentUser,
) -> Result<Json<Value>> {
    let row: Option<(Option<String>, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT email, expires_at FROM google_token WHERE user_id = $1 AND company_id = $2",
    )
    .bind(user.user_id)
    .bind(user.company_id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(json!({
        "connected": row.is_some(),
        "email": row.as_ref().and_then(|(e, _)| e.clone()),
        "expires_at": row.as_ref().and_then(|(_, x)| *x),
        "configured": !state.cfg.google_client_id.is_empty(),
    })))
}
