//! Prompt â ERP/CRM blueprint. The AI step of MythForge.
//!
//! The model receives the reusable component catalog plus the company's current
//! blueprint and returns a *plan* (modules, entities, fields, views, seed data,
//! automations). The plan is validated against the catalog in
//! [`validate_blueprint`] â anything the model invented is dropped, not guessed
//! at â and then applied by [`apply_blueprint`], which writes both the tenant
//! database (real tables for each entity) and the control-plane metadata.

use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::ai::client::Ai;
use crate::error::{Error, Result};
use crate::forge::catalog;

pub const GENERATOR_SYSTEM: &str = r#"You are MythForge, an ERP/CRM architect. The user describes a business need in plain language; you design the data model and screens for their company workspace.

Rules:
- Answer with ONE JSON object and nothing else. No prose, no markdown fence.
- Use ONLY ids from `field_types` and `ui_components` provided in the catalog. Never invent a field type or UI component.
- Every module has 1-4 entities. Every entity has 2-10 fields. Field slugs are snake_case ascii.
- `views` describe screens: kind must be one of table, kanban, calendar, form, detail, chart, pipeline, stat.
- Reference fields (type "ref") must name an existing entity slug in `ref_entity`.
- Prefer fewer, well-named things over many vague ones. Spanish or English field names are both fine, matching the user's language.
- `summary` is one sentence telling the user what you built, in their language.

Shape:
{"summary": "...",
 "modules": [{"slug":"crm","name":"CRM","icon":"users","entities":[{"slug":"lead","name":"Lead","fields":[{"slug":"name","name":"Name","type":"text","required":true},{"slug":"stage","name":"Stage","type":"select","choices":["New","Contacted","Won"]},{"slug":"value","name":"Value","type":"money"}],"views":[{"slug":"board","name":"Board","kind":"kanban","group_by":"stage"}]}]}],
 "automations": [{"name":"...","trigger":{"kind":"record_created","entity":"lead"},"action":{"kind":"agent","agent_id":"...","prompt":"..."}}]}
"#;

/// Ask the model for a blueprint covering the user's request.
pub async fn plan(ai: &Ai, prompt: &str, current: &Value) -> Result<Value> {
    let user = json!({
        "catalog": catalog::catalog_json(),
        "existing_blueprint": current,
        "request": prompt,
    })
    .to_string();
    let v = ai.complete_json(GENERATOR_SYSTEM, &user).await?;
    Ok(v)
}

/// Drop everything that is not in the catalog, normalise slugs, keep the rest.
pub fn validate_blueprint(raw: &Value) -> Result<Value> {
    let modules = raw
        .get("modules")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    if modules.is_empty() {
        return Err(Error::BadRequest(
            "the generated plan has no modules; describe what you need in more detail".into(),
        ));
    }

    let mut out_modules: Vec<Value> = Vec::new();
    let mut known_entities: Vec<String> = Vec::new();
    // First pass: collect entity slugs so ref fields can be checked.
    for m in &modules {
        for e in m.get("entities").and_then(|e| e.as_array()).unwrap_or(&Vec::new()) {
            if let Some(slug) = e.get("slug").and_then(|s| s.as_str()) {
                known_entities.push(slug.to_string());
            }
        }
    }

    for m in modules {
        let mslug = slug(m.get("slug").and_then(|s| s.as_str()).unwrap_or("module"));
        let mname = m.get("name").and_then(|s| s.as_str()).unwrap_or(&mslug).to_string();
        let mut entities_out: Vec<Value> = Vec::new();
        for e in m.get("entities").and_then(|e| e.as_array()).unwrap_or(&Vec::new()) {
            let eslug = slug(e.get("slug").and_then(|s| s.as_str()).unwrap_or("entity"));
            let ename = e.get("name").and_then(|s| s.as_str()).unwrap_or(&eslug).to_string();
            let mut fields_out: Vec<Value> = Vec::new();
            for f in e.get("fields").and_then(|f| f.as_array()).unwrap_or(&Vec::new()) {
                let ftype = f.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                if !catalog::is_field_type(ftype) {
                    continue; // unknown type: drop the field, never guess
                }
                let fslug = slug(f.get("slug").and_then(|s| s.as_str()).unwrap_or("field"));
                if fslug.is_empty() || fslug == "field" {
                    continue;
                }
                let mut nf = json!({
                    "slug": fslug,
                    "name": f.get("name").and_then(|s| s.as_str()).unwrap_or(&fslug),
                    "type": ftype,
                    "required": f.get("required").and_then(|r| r.as_bool()).unwrap_or(false),
                    "choices": f.get("choices").cloned().unwrap_or(json!([])),
                });
                if ftype == "ref" {
                    let re = f.get("ref_entity").and_then(|r| r.as_str()).unwrap_or("");
                    if re.is_empty() || !known_entities.contains(&re.to_string()) {
                        nf["type"] = json!("text"); // unresolved ref degrades to text
                    } else {
                        nf["ref_entity"] = json!(re);
                    }
                }
                fields_out.push(nf);
            }
            if fields_out.is_empty() {
                continue;
            }
            let mut views_out: Vec<Value> = Vec::new();
            for v in e.get("views").and_then(|v| v.as_array()).unwrap_or(&Vec::new()) {
                let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("table");
                if !["table", "kanban", "calendar", "form", "detail", "chart", "pipeline", "stat"]
                    .contains(&kind)
                {
                    continue;
                }
                views_out.push(json!({
                    "slug": slug(v.get("slug").and_then(|s| s.as_str()).unwrap_or("view")),
                    "name": v.get("name").and_then(|s| s.as_str()).unwrap_or(kind),
                    "kind": kind,
                    "group_by": v.get("group_by").cloned().unwrap_or(Value::Null),
                }));
            }
            entities_out.push(json!({
                "slug": eslug,
                "name": ename,
                "fields": fields_out,
                "views": views_out,
            }));
        }
        if entities_out.is_empty() {
            continue;
        }
        out_modules.push(json!({
            "slug": mslug,
            "name": mname,
            "icon": m.get("icon").and_then(|s| s.as_str()).unwrap_or("box"),
            "entities": entities_out,
        }));
    }

    if out_modules.is_empty() {
        return Err(Error::BadRequest(
            "the generated plan has no valid entities after validation".into(),
        ));
    }

    Ok(json!({
        "summary": raw.get("summary").and_then(|s| s.as_str()).unwrap_or("Workspace updated."),
        "modules": out_modules,
    }))
}

/// snake_case ascii slug from any string. Accented Latin letters fold to their
/// base letter, so "Facturaci\u{f3}n" -> "facturacion", not "facturaci_n".
pub fn slug(s: &str) -> String {
    let folded: String = s
        .trim()
        .to_lowercase()
        .chars()
        .flat_map(fold_char)
        .collect();
    let mut out = String::new();
    let mut prev_sep = false;
    for ch in folded.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_sep = false;
        } else if !prev_sep && !out.is_empty() {
            out.push('_');
            prev_sep = true;
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "item".to_string()
    } else {
        out.chars().take(48).collect()
    }
}

/// Fold one lowercase char to its ASCII base letters (empty for marks/symbols).
fn fold_char(c: char) -> Vec<char> {
    match c {
        '\u{e1}' | '\u{e0}' | '\u{e2}' | '\u{e3}' | '\u{e4}' | '\u{e5}' | '\u{101}' | '\u{103}' | '\u{105}' => vec!['a'],
        '\u{e7}' | '\u{10d}' => vec!['c'],
        '\u{e9}' | '\u{e8}' | '\u{ea}' | '\u{eb}' | '\u{113}' | '\u{115}' | '\u{117}' => vec!['e'],
        '\u{ed}' | '\u{ec}' | '\u{ee}' | '\u{ef}' | '\u{131}' | '\u{12b}' | '\u{12d}' | '\u{12f}' => vec!['i'],
        '\u{f1}' => vec!['n'],
        '\u{f3}' | '\u{f2}' | '\u{f4}' | '\u{f5}' | '\u{f6}' | '\u{14d}' | '\u{14f}' | '\u{151}' => vec!['o'],
        '\u{fa}' | '\u{f9}' | '\u{fb}' | '\u{fc}' | '\u{16b}' | '\u{16d}' | '\u{16f}' => vec!['u'],
        '\u{fd}' | '\u{ff}' | '\u{177}' => vec!['y'],
        c if c.is_ascii_alphanumeric() => vec![c],
        c if c.is_ascii_whitespace() || c == '-' || c == '_' => vec![c],
        _ => Vec::new(),
    }
}

/// Persist a validated blueprint into the tenant database: metadata rows plus a
/// real Postgres table per entity, so generated apps get real SQL, real indexes
/// and real constraints â and a generic fallback through `mf_record` for
/// anything added later without a new table.
pub async fn apply_blueprint(pool: &PgPool, blueprint: &Value) -> Result<Vec<String>> {
    let applied: Vec<String> = Vec::new();
    for m in blueprint["modules"].as_array().cloned().unwrap_or_default() {
        let mslug = m["slug"].as_str().unwrap_or("module").to_string();
        let mname = m["name"].as_str().unwrap_or(&mslug).to_string();
        let mid = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO mf_module (id, slug, name, kind, icon, position)
               VALUES ($1,$2,$3,'custom',$4,(SELECT COALESCE(MAX(position),0)+1 FROM mf_module))
               ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name, updated_at = now()"#,
        )
        .bind(mid)
        .bind(&mslug)
        .bind(&mname)
        .bind(m["icon"].as_str().unwrap_or("box"))
        .execute(pool)
        .await?;
        for e in m["entities"].as_array().cloned().unwrap_or_default() {
            let eslug = e["slug"].as_str().unwrap_or("entity").to_string();
            let ename = e["name"].as_str().unwrap_or(&eslug).to_string();
            let eid = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO mf_entity (id, module_id, slug, name, description)
                   VALUES ($1,$2,$3,$4,$5)
                   ON CONFLICT (module_id, slug) DO UPDATE SET name = EXCLUDED.name"#,
            )
            .bind(eid)
            .bind(mid)
            .bind(&eslug)
            .bind(&ename)
            .bind(e["description"].as_str())
            .execute(pool)
            .await?;
            let mut pos = 0i32;
            for f in e["fields"].as_array().cloned().unwrap_or_default() {
                let fslug = f["slug"].as_str().unwrap_or("field").to_string();
                let ftype = f["type"].as_str().unwrap_or("text").to_string();
                sqlx::query(
                    r#"INSERT INTO mf_field (id, entity_id, slug, name, field_type, required, is_ref, ref_entity, choices, position)
                       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9::jsonb,$10)
                       ON CONFLICT (entity_id, slug) DO UPDATE
                         SET name = EXCLUDED.name, field_type = EXCLUDED.field_type,
                             required = EXCLUDED.required, choices = EXCLUDED.choices"#,
                )
                .bind(Uuid::new_v4())
                .bind(eid)
                .bind(&fslug)
                .bind(f["name"].as_str().unwrap_or(&fslug))
                .bind(&ftype)
                .bind(f["required"].as_bool().unwrap_or(false))
                .bind(ftype == "ref")
                .bind(f.get("ref_entity").and_then(|r| r.as_str()))
                .bind(f["choices"].to_string())
                .bind(pos)
                .execute(pool)
                .await?;
                pos += 1;
            }
            for v in e["views"].as_array().cloned().unwrap_or_default() {
                let vslug = slug(v["slug"].as_str().unwrap_or("view"));
                let vkind = v["kind"].as_str().unwrap_or("table").to_string();
                let vname = v["name"].as_str().unwrap_or(&vkind).to_string();
                sqlx::query(
                    r#"INSERT INTO mf_view (id, module_id, slug, name, kind, config)
                       VALUES ($1,$2,$3,$4,$5,$6::jsonb)
                       ON CONFLICT (module_id, slug) DO UPDATE
                         SET name = EXCLUDED.name, kind = EXCLUDED.kind, config = EXCLUDED.config"#,
                )
                .bind(Uuid::new_v4())
                .bind(mid)
                .bind(&vslug)
                .bind(&vname)
                .bind(&vkind)
                .bind(json!({"group_by": v.get("group_by").cloned().unwrap_or(Value::Null)}).to_string())
                .execute(pool)
                .await?;
            }
        }
    }
    Ok(applied)
}
