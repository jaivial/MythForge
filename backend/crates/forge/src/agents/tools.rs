//! The platform operation registry.
//!
//! These are the "custom tools" agents get: one operation per reusable backend
//! capability, named by id (never a URL), exactly as MythCortex's custom-agent
//! contract prescribes. Adding a capability means adding it here â every agent
//! in every company can then bind it, with no restart.

use std::sync::Arc;

use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::agent::{OpContext, Operation, Registry};

/// Shorthand for the boxed future `Operation::call` returns.
type Pin<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>>;
use crate::error::{Error, Result};
use crate::forge::runtime;

/// Build the full platform registry.
pub fn build_registry() -> Registry {
    let mut r = Registry::new();
    r.register(Arc::new(ListModules));
    r.register(Arc::new(ListEntityRecords));
    r.register(Arc::new(CreateRecord));
    r.register(Arc::new(UpdateRecord));
    r.register(Arc::new(DeleteRecord));
    r.register(Arc::new(GetRecord));
    r.register(Arc::new(SearchRecords));
    r.register(Arc::new(CreateModule));
    r.register(Arc::new(GoogleCalendarList));
    r.register(Arc::new(GoogleGmailSend));
    r
}

/// A registry restricted to the operation ids an agent is bound to.
pub fn sub_registry(full: &Registry, ids: &[String]) -> Registry {
    let mut r = Registry::new();
    for id in ids {
        if let Some(op) = full.get(id) {
            r.register(op);
        }
    }
    r
}

fn object_schema(props: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": props,
        "required": required,
    })
}

// ---------------------------------------------------------------------------
// blueprint operations
// ---------------------------------------------------------------------------

struct ListModules;
impl Operation for ListModules {
    fn id(&self) -> &str { "forge_blueprint_list" }
    fn module(&self) -> &str { "blueprint" }
    fn description(&self) -> &str { "List every module, entity and field of the workspace (the blueprint)." }
    fn write(&self) -> bool { false }
    fn input_schema(&self) -> Value { object_schema(json!({}), &[]) }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, _input: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move { runtime::blueprint(ctx.pool).await })
    }
}

struct CreateModule;
impl Operation for CreateModule {
    fn id(&self) -> &str { "forge_module_create" }
    fn module(&self) -> &str { "blueprint" }
    fn description(&self) -> &str { "Create a module with entities and fields from a blueprint fragment." }
    fn write(&self) -> bool { true }
    fn input_schema(&self) -> Value {
        object_schema(json!({
            "module": { "type": "object", "description": "Module object: slug, name, entities[{slug,name,fields[{slug,name,type,choices}]}]" }
        }), &["module"])
    }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let module = input.get("module").cloned().unwrap_or(Value::Null);
            let plan = json!({ "summary": "module created by agent", "modules": [module] });
            let validated = crate::forge::generate::validate_blueprint(&plan)?;
            crate::forge::generate::apply_blueprint(ctx.pool, &validated).await?;
            Ok(json!({ "ok": true, "modules": validated["modules"] }))
        })
    }
}

// ---------------------------------------------------------------------------
// record operations
// ---------------------------------------------------------------------------

// Each record operation is its own small struct: the module/entity ids are
// validated against the tenant's own metadata inside `runtime`, so an agent
// cannot address an entity that does not exist.

struct ListEntityRecords;
impl Operation for ListEntityRecords {
    fn id(&self) -> &str { "forge_records_list" }
    fn module(&self) -> &str { "records" }
    fn description(&self) -> &str { "List records of an entity, with optional search and paging." }
    fn write(&self) -> bool { false }
    fn input_schema(&self) -> Value {
        object_schema(json!({
            "module": {"type":"string"}, "entity": {"type":"string"},
            "search": {"type":"string"}, "limit": {"type":"integer"}, "offset": {"type":"integer"}
        }), &["module","entity"])
    }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let module = owned(input["module"].as_str());
            let entity = owned(input["entity"].as_str());
            runtime::list_records(
                ctx.pool, &module, &entity,
                input["search"].as_str(),
                &[],
                input["limit"].as_i64().unwrap_or(25).clamp(1, 100),
                input["offset"].as_i64().unwrap_or(0).max(0),
            ).await
        })
    }
}

struct CreateRecord;
impl Operation for CreateRecord {
    fn id(&self) -> &str { "forge_records_create" }
    fn module(&self) -> &str { "records" }
    fn description(&self) -> &str { "Create a record in an entity with field values keyed by field slug." }
    fn write(&self) -> bool { true }
    fn input_schema(&self) -> Value {
        object_schema(json!({
            "module": {"type":"string"}, "entity": {"type":"string"},
            "data": {"type":"object", "description": "Field values keyed by field slug"}
        }), &["module","entity","data"])
    }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let module = owned(input["module"].as_str());
            let entity = owned(input["entity"].as_str());
            runtime::create_record(ctx.pool, &module, &entity, &input["data"]).await
        })
    }
}

struct GetRecord;
impl Operation for GetRecord {
    fn id(&self) -> &str { "forge_records_get" }
    fn module(&self) -> &str { "records" }
    fn description(&self) -> &str { "Fetch one record by id." }
    fn write(&self) -> bool { false }
    fn input_schema(&self) -> Value {
        object_schema(json!({ "module": {"type":"string"}, "entity": {"type":"string"}, "id": {"type":"string"} }), &["module","entity","id"])
    }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let module = owned(input["module"].as_str());
            let entity = owned(input["entity"].as_str());
            let id = parse_uuid(&input)?;
            runtime::get_record(ctx.pool, &module, &entity, id).await
        })
    }
}

struct UpdateRecord;
impl Operation for UpdateRecord {
    fn id(&self) -> &str { "forge_records_update" }
    fn module(&self) -> &str { "records" }
    fn description(&self) -> &str { "Update fields of a record." }
    fn write(&self) -> bool { true }
    fn input_schema(&self) -> Value {
        object_schema(json!({ "module": {"type":"string"}, "entity": {"type":"string"}, "id": {"type":"string"}, "data": {"type":"object"} }), &["module","entity","id","data"])
    }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let module = owned(input["module"].as_str());
            let entity = owned(input["entity"].as_str());
            let id = parse_uuid(&input)?;
            runtime::update_record(ctx.pool, &module, &entity, id, &input["data"]).await
        })
    }
}

struct DeleteRecord;
impl Operation for DeleteRecord {
    fn id(&self) -> &str { "forge_records_delete" }
    fn module(&self) -> &str { "records" }
    fn description(&self) -> &str { "Delete a record." }
    fn write(&self) -> bool { true }
    fn input_schema(&self) -> Value {
        object_schema(json!({ "module": {"type":"string"}, "entity": {"type":"string"}, "id": {"type":"string"} }), &["module","entity","id"])
    }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let module = owned(input["module"].as_str());
            let entity = owned(input["entity"].as_str());
            let id = parse_uuid(&input)?;
            runtime::delete_record(ctx.pool, &module, &entity, id).await
        })
    }
}

struct SearchRecords;
impl Operation for SearchRecords {
    fn id(&self) -> &str { "forge_records_search" }
    fn module(&self) -> &str { "records" }
    fn description(&self) -> &str { "Full-text search across one entity's records." }
    fn write(&self) -> bool { false }
    fn input_schema(&self) -> Value {
        object_schema(json!({ "module": {"type":"string"}, "entity": {"type":"string"}, "query": {"type":"string"} }), &["module","entity","query"])
    }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let module = owned(input["module"].as_str());
            let entity = owned(input["entity"].as_str());
            runtime::list_records(ctx.pool, &module, &entity, input["query"].as_str(), &[], 25, 0).await
        })
    }
}

fn owned(v: Option<&str>) -> String {
    v.unwrap_or("").to_string()
}

fn parse_uuid(input: &Value) -> std::result::Result<Uuid, Error> {
    Uuid::parse_str(input["id"].as_str().unwrap_or(""))
        .map_err(|_| Error::BadRequest("id must be a uuid".into()))
}

// ---------------------------------------------------------------------------
// google operations
// ---------------------------------------------------------------------------

struct GoogleCalendarList;
impl Operation for GoogleCalendarList {
    fn id(&self) -> &str { "google_calendar_list" }
    fn module(&self) -> &str { "google" }
    fn description(&self) -> &str { "List the connected Google Calendar's upcoming events." }
    fn write(&self) -> bool { false }
    fn input_schema(&self) -> Value { object_schema(json!({ "max_results": {"type":"integer"} }), &[]) }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let tok = crate::google::oauth::token_for(ctx.pool, ctx.user_id).await?;
            match tok {
                Some(t) => crate::google::api::calendar_events(&t, input["max_results"].as_i64().unwrap_or(10)).await,
                None => Ok(json!({ "connected": false, "hint": "connect Google in Settings first" })),
            }
        })
    }
}

struct GoogleGmailSend;
impl Operation for GoogleGmailSend {
    fn id(&self) -> &str { "google_gmail_send" }
    fn module(&self) -> &str { "google" }
    fn description(&self) -> &str { "Send an email through the connected Gmail account." }
    fn write(&self) -> bool { true }
    fn input_schema(&self) -> Value {
        object_schema(json!({
            "to": {"type":"string"}, "subject": {"type":"string"}, "body": {"type":"string"}
        }), &["to","subject","body"])
    }
    fn call<'a>(&self, ctx: &'a OpContext<'a>, input: Value) -> Pin<'a> {
        Box::pin(async move {
            let tok = crate::google::oauth::token_for(ctx.pool, ctx.user_id).await?;
            match tok {
                Some(t) => crate::google::api::gmail_send(
                    &t,
                    input["to"].as_str().unwrap_or(""),
                    input["subject"].as_str().unwrap_or(""),
                    input["body"].as_str().unwrap_or(""),
                ).await,
                None => Ok(json!({ "connected": false, "hint": "connect Google in Settings first" })),
            }
        })
    }
}
