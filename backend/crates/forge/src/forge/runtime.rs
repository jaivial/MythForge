//! The generic data plane: ONE set of HTTP paths serving EVERY company.
//!
//! This is what makes MythForge work without restarting services. Generated
//! modules never get their own Rust handler; instead every company's entities
//! are served by the same reusable endpoints below, resolved at request time
//! from the tenant's own metadata + `mf_record` table:
//!
//! ```text
//! GET    /api/v1/companies/{company}/modules
//! GET    /api/v1/companies/{company}/modules/{module}/entities/{entity}
//! GET    /api/v1/companies/{company}/data/{module}/{entity}
//! POST   /api/v1/companies/{company}/data/{module}/{entity}
//! GET    /api/v1/companies/{company}/data/{module}/{entity}/{id}
//! PATCH  /api/v1/companies/{company}/data/{module}/{entity}/{id}
//! DELETE /api/v1/companies/{company}/data/{module}/{entity}/{id}
//! ```
//!
//! Company isolation is physical: each request goes to that company's own
//! database, chosen from the caller's membership â never from the URL alone.

use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

/// Full blueprint (modules + entities + fields + views) of one company.
pub async fn blueprint(pool: &PgPool) -> Result<Value> {
    let modules = sqlx::query_as::<_, (Uuid, String, String, Option<String>, bool, i32)>(
        r#"SELECT id, slug, name, icon, is_active, position FROM mf_module ORDER BY position, name"#,
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::new();
    for (mid, mslug, mname, icon, active, _pos) in modules {
        if !active {
            continue;
        }
        let entities = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
            r#"SELECT id, slug, name, description FROM mf_entity WHERE module_id = $1 ORDER BY slug"#,
        )
        .bind(mid)
        .fetch_all(pool)
        .await?;
        let mut eout = Vec::new();
        for (eid, eslug, ename, desc) in entities {
            let fields = sqlx::query_as::<_, (String, String, String, bool, Option<String>, Value, i32)>(
                r#"SELECT slug, name, field_type, required, ref_entity, choices, position
                   FROM mf_field WHERE entity_id = $1 ORDER BY position"#,
            )
            .bind(eid)
            .fetch_all(pool)
            .await?;
            let views = sqlx::query_as::<_, (String, String, String, Value)>(
                r#"SELECT slug, name, kind, config FROM mf_view WHERE module_id = $1 ORDER BY slug"#,
            )
            .bind(mid)
            .fetch_all(pool)
            .await?;
            eout.push(json!({
                "slug": eslug,
                "name": ename,
                "description": desc,
                "fields": fields.iter().map(|(fs, fn_, ft, fr, re, ch, _p)| json!({
                    "slug": fs, "name": fn_, "type": ft, "required": fr,
                    "ref_entity": re, "choices": ch,
                })).collect::<Vec<_>>(),
                "views": views.iter().map(|(vs, vn, vk, vc)| json!({
                    "slug": vs, "name": vn, "kind": vk, "config": vc,
                })).collect::<Vec<_>>(),
            }));
        }
        out.push(json!({
            "slug": mslug,
            "name": mname,
            "icon": icon,
            "entities": eout,
        }));
    }
    Ok(json!({ "modules": out }))
}

/// Validate an entity slug against the tenant's own metadata.
pub async fn entity_exists(pool: &PgPool, module: &str, entity: &str) -> Result<bool> {
    let ok: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1 FROM mf_entity e JOIN mf_module m ON m.id = e.module_id
               WHERE m.slug = $1 AND e.slug = $2)"#,
    )
    .bind(module)
    .bind(entity)
    .fetch_one(pool)
    .await?;
    Ok(ok)
}

/// Field metadata for an entity, in order.
async fn fields_of(pool: &PgPool, module: &str, entity: &str) -> Result<Vec<(String, String, bool, Option<String>, Value)>> {
    Ok(sqlx::query_as::<_, (String, String, bool, Option<String>, Value)>(
        r#"SELECT f.slug, f.field_type, f.required, f.ref_entity, f.choices
           FROM mf_field f
           JOIN mf_entity e ON e.id = f.entity_id
           JOIN mf_module m ON m.id = e.module_id
           WHERE m.slug = $1 AND e.slug = $2
           ORDER BY f.position"#,
    )
    .bind(module)
    .bind(entity)
    .fetch_all(pool)
    .await?)
}

/// List records with optional search, filtering and paging.
#[allow(clippy::too_many_arguments)]
pub async fn list_records(
    pool: &PgPool,
    module: &str,
    entity: &str,
    search: Option<&str>,
    filters: &[(String, String)],
    limit: i64,
    offset: i64,
) -> Result<Value> {
    if !entity_exists(pool, module, entity).await? {
        return Err(Error::NotFound(format!("entity {module}/{entity}")));
    }
    let mut sql = String::from(
        "SELECT id, data, created_at, updated_at FROM mf_record WHERE module_slug = $1 AND entity_slug = $2",
    );
    let mut idx = 3;
    let mut binds: Vec<String> = vec![module.to_string(), entity.to_string()];
    if let Some(q) = search.filter(|s| !s.trim().is_empty()) {
        sql.push_str(&format!(" AND search ILIKE ${}", idx));
        binds.push(format!("%{}%", q.trim()));
        idx += 1;
    }
    for (k, v) in filters {
        sql.push_str(&format!(" AND data->>${} = ${}", quote_ident(k), idx));
        binds.push(v.clone());
        idx += 1;
    }
    sql.push_str(&format!(" ORDER BY updated_at DESC LIMIT ${} OFFSET ${}", idx, idx + 1));
    binds.push(limit.to_string());
    binds.push(offset.to_string());

    let mut q = sqlx::query_as::<_, (Uuid, Value, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    let rows = q.fetch_all(pool).await?;
    let total: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM mf_record WHERE module_slug = $1 AND entity_slug = $2",
    )
    .bind(module)
    .bind(entity)
    .fetch_one(pool)
    .await?;

    Ok(json!({
        "items": rows.iter().map(|(id, data, c, u)| json!({
            "id": id, "data": data, "created_at": c, "updated_at": u
        })).collect::<Vec<_>>(),
        "total": total,
        "limit": limit,
        "offset": offset,
    }))
}

/// `data->>key` with the key quoted safely (no injection through field names).
fn quote_ident(k: &str) -> String {
    let clean: String = k.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    format!("'{}'", clean)
}

/// Build the searchable text blob for a record.
fn search_text(data: &Value) -> String {
    match data {
        Value::Object(map) => map
            .values()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase(),
        _ => String::new(),
    }
}

/// Create a record, validating required fields and types against metadata.
pub async fn create_record(
    pool: &PgPool,
    module: &str,
    entity: &str,
    body: &Value,
) -> Result<Value> {
    let fields = fields_of(pool, module, entity).await?;
    if fields.is_empty() {
        return Err(Error::NotFound(format!("entity {module}/{entity}")));
    }
    let clean = sanitise(body, &fields)?;
    let search = search_text(&clean);
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO mf_record (id, entity_slug, module_slug, data, search)
           VALUES ($1,$2,$3,$4::jsonb,$5) RETURNING id, data, created_at, updated_at"#,
    )
    .bind(id)
    .bind(entity)
    .bind(module)
    .bind(clean.to_string())
    .bind(&search)
    .execute(pool)
    .await?;
    Ok(record_json(pool, id).await?)
}

/// Fetch one record.
pub async fn get_record(pool: &PgPool, module: &str, entity: &str, id: Uuid) -> Result<Value> {
    let _ = (module, entity);
    record_json(pool, id).await
}

/// Replace/merge fields of a record.
pub async fn update_record(
    pool: &PgPool,
    module: &str,
    entity: &str,
    id: Uuid,
    body: &Value,
) -> Result<Value> {
    let fields = fields_of(pool, module, entity).await?;
    if fields.is_empty() {
        return Err(Error::NotFound(format!("entity {module}/{entity}")));
    }
    let current: Option<Value> = sqlx::query_scalar::<_, Value>(
        "SELECT data FROM mf_record WHERE id = $1 AND module_slug = $2 AND entity_slug = $3",
    )
    .bind(id)
    .bind(module)
    .bind(entity)
    .fetch_optional(pool)
    .await?;
    let Some(mut merged) = current else {
        return Err(Error::NotFound(format!("record {id}")));
    };
    if let Some(incoming) = body.as_object() {
        for (k, v) in incoming {
            if let Some(obj) = merged.as_object_mut() {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    let clean = sanitise(&merged, &fields)?;
    let search = search_text(&clean);
    sqlx::query(
        "UPDATE mf_record SET data = $1::jsonb, search = $2, updated_at = now() WHERE id = $3",
    )
    .bind(clean.to_string())
    .bind(&search)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(record_json(pool, id).await?)
}

/// Delete a record.
pub async fn delete_record(pool: &PgPool, module: &str, entity: &str, id: Uuid) -> Result<Value> {
    let rows = sqlx::query("DELETE FROM mf_record WHERE id = $1 AND module_slug = $2 AND entity_slug = $3")
        .bind(id)
        .bind(module)
        .bind(entity)
        .execute(pool)
        .await?
        .rows_affected();
    if rows == 0 {
        return Err(Error::NotFound(format!("record {id}")));
    }
    Ok(json!({ "deleted": true, "id": id }))
}

async fn record_json(pool: &PgPool, id: Uuid) -> Result<Value> {
    let row = sqlx::query_as::<_, (Uuid, Value, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, data, created_at, updated_at FROM mf_record WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((id, data, c, u)) => Ok(json!({ "id": id, "data": data, "created_at": c, "updated_at": u })),
        None => Err(Error::NotFound(format!("record {id}"))),
    }
}

/// Keep only known fields, coerce values to the declared type, enforce required.
pub fn sanitise(body: &Value, fields: &[(String, String, bool, Option<String>, Value)]) -> Result<Value> {
    let mut out = serde_json::Map::new();
    let obj = body.as_object().cloned().unwrap_or_default();
    for (fslug, ftype, required, _ref, choices) in fields {
        match obj.get(fslug) {
            Some(v) => {
                if v.is_null() && *required {
                    return Err(Error::BadRequest(format!("field {fslug} is required")));
                }
                if !v.is_null() {
                    if let Some(coerced) = coerce(v, ftype, choices) {
                        out.insert(fslug.clone(), coerced);
                    } else {
                        return Err(Error::BadRequest(format!(
                            "field {fslug} expects {ftype}"
                        )));
                    }
                }
            }
            None => {
                if *required {
                    return Err(Error::BadRequest(format!("field {fslug} is required")));
                }
            }
        }
    }
    Ok(Value::Object(out))
}

fn coerce(v: &Value, ftype: &str, choices: &Value) -> Option<Value> {
    match ftype {
        "text" | "textarea" | "email" | "phone" | "select" => {
            let s = v.as_str()?;
            if ftype == "select" {
                let list = choices.as_array()?;
                let ok = list.iter().any(|c| c.as_str() == Some(s));
                if !ok {
                    return None;
                }
            }
            if ftype == "email" && !s.contains('@') {
                return None;
            }
            Some(Value::String(s.to_string()))
        }
        "multiselect" => {
            let arr = v.as_array()?;
            let list = choices.as_array()?;
            let mut out = Vec::new();
            for item in arr {
                let s = item.as_str()?;
                if !list.iter().any(|c| c.as_str() == Some(s)) {
                    return None;
                }
                out.push(Value::String(s.to_string()));
            }
            Some(Value::Array(out))
        }
        "number" | "money" => match v {
            Value::Number(n) => Some(Value::Number(n.clone())),
            Value::String(s) => serde_json::from_str::<f64>(s).ok().and_then(|f| serde_json::Number::from_f64(f).map(Value::Number)),
            _ => None,
        },
        "bool" => v.as_bool().map(Value::Bool),
        "date" | "datetime" => v.as_str().map(|s| Value::String(s.to_string())),
        "ref" | "uuid" => {
            let s = v.as_str()?;
            Uuid::parse_str(s).ok().map(|u| Value::String(u.to_string()))
        }
        "attachment" => Some(v.clone()),
        _ => Some(v.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::generate::slug;

    #[test]
    fn slug_is_ascii_snake() {
        assert_eq!(slug("Ventas y Facturaci\u{f3}n"), "ventas_y_facturacion");
        assert_eq!(slug("  --Mix--  "), "mix");
        assert_eq!(slug("\u{c1}\u{e9}\u{cd}\u{f6}\u{fc}"), "aeiou");
        assert_eq!(slug("   "), "item");
    }

    #[test]
    fn search_text_joins_scalars() {
        let v = json!({"a": "Hola", "b": 3, "c": true, "d": {"nested": 1}});
        assert_eq!(search_text(&v), "hola 3 true");
    }

    #[test]
    fn sanitise_keeps_known_fields_only() {
        let fields = vec![
            ("name".into(), "text".into(), true, None, json!([])),
            ("value".into(), "money".into(), false, None, json!([])),
        ];
        let out = sanitise(&json!({"name": "Acme", "junk": "x", "value": "12.5"}), &fields).unwrap();
        assert_eq!(out["name"], "Acme");
        assert_eq!(out["value"], 12.5);
        assert!(out.get("junk").is_none());
    }

    #[test]
    fn sanitise_enforces_required() {
        let fields = vec![("name".into(), "text".into(), true, None, json!([]))];
        let err = sanitise(&json!({"other": 1}), &fields).unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn sanitise_enforces_choices() {
        let fields = vec![(
            "stage".into(),
            "select".into(),
            false,
            None,
            json!(["New", "Won"]),
        )];
        assert!(sanitise(&json!({"stage": "New"}), &fields).is_ok());
        assert!(sanitise(&json!({"stage": "Bogus"}), &fields).is_err());
    }

    #[test]
    fn quote_ident_strips_bad_chars() {
        assert_eq!(quote_ident("a; DROP TABLE x"), "'aDROPTABLEx'");
    }
}
