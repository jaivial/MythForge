//! The reusable component catalog.
//!
//! Everything the generator may emit is described here first. This is the
//! MythForge equivalent of Odoo's module registry / MythCortex's operation
//! registry: the model never invents a component, it picks from this list, so
//! generated ERP/CRMs are assembled from known-good, reusable parts.

use serde_json::{json, Value};

/// A reusable UI block the frontend knows how to render.
pub struct UiComponent {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: &'static str,
    pub description: &'static str,
}

/// A reusable field type with its storage + input semantics.
pub struct FieldType {
    pub id: &'static str,
    pub label: &'static str,
    pub pg_type: &'static str,
    pub description: &'static str,
}

/// A business template: a starting set of modules for a vertical.
pub struct Template {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub modules: &'static [&'static str],
}

pub const UI_COMPONENTS: &[UiComponent] = &[
    UiComponent { id: "table", label: "Data table", kind: "view", description: "Paginated, sortable, searchable list of records with row actions." },
    UiComponent { id: "kanban", label: "Kanban board", kind: "view", description: "Columns from a choice field; drag-and-drop moves records between stages." },
    UiComponent { id: "calendar", label: "Calendar", kind: "view", description: "Records with a date field plotted on a month/week view." },
    UiComponent { id: "form", label: "Form", kind: "view", description: "Create/edit a record with typed inputs and validation." },
    UiComponent { id: "detail", label: "Detail page", kind: "view", description: "Single record with related records from reference fields." },
    UiComponent { id: "chart", label: "Chart", kind: "view", description: "Aggregate a numeric field grouped by a choice or date field." },
    UiComponent { id: "pipeline", label: "Pipeline summary", kind: "view", description: "Per-stage totals with conversion for a sales pipeline." },
    UiComponent { id: "stat", label: "Stat cards", kind: "view", description: "Row of KPI cards (count, sum, average) over an entity." },
    UiComponent { id: "input.text", label: "Text input", kind: "field", description: "Single-line text input." },
    UiComponent { id: "input.number", label: "Number input", kind: "field", description: "Numeric input stored as numeric." },
    UiComponent { id: "input.money", label: "Money input", kind: "field", description: "Currency amount." },
    UiComponent { id: "input.date", label: "Date picker", kind: "field", description: "Calendar date." },
    UiComponent { id: "input.datetime", label: "Date & time picker", kind: "field", description: "Timestamp with time." },
    UiComponent { id: "input.bool", label: "Switch", kind: "field", description: "Boolean toggle." },
    UiComponent { id: "input.email", label: "Email input", kind: "field", description: "Validated email address." },
    UiComponent { id: "input.phone", label: "Phone input", kind: "field", description: "Phone number." },
    UiComponent { id: "input.select", label: "Select", kind: "field", description: "Single choice from a fixed list." },
    UiComponent { id: "input.multiselect", label: "Multi select", kind: "field", description: "Multiple choices from a fixed list." },
    UiComponent { id: "input.textarea", label: "Textarea", kind: "field", description: "Long text." },
    UiComponent { id: "input.ref", label: "Reference", kind: "field", description: "Link to records of another entity." },
    UiComponent { id: "input.attachment", label: "Attachment", kind: "field", description: "File reference by URL/name." },
    UiComponent { id: "badge", label: "Badge", kind: "atom", description: "Status word pill." },
    UiComponent { id: "separator", label: "Separator", kind: "atom", description: "Horizontal rule between sections." },
];

pub const FIELD_TYPES: &[FieldType] = &[
    FieldType { id: "text", label: "Text", pg_type: "text", description: "Single-line string." },
    FieldType { id: "textarea", label: "Long text", pg_type: "text", description: "Multi-line string." },
    FieldType { id: "number", label: "Number", pg_type: "numeric", description: "Any number." },
    FieldType { id: "money", label: "Money", pg_type: "numeric", description: "Currency amount." },
    FieldType { id: "date", label: "Date", pg_type: "date", description: "Calendar date." },
    FieldType { id: "datetime", label: "Date & time", pg_type: "timestamptz", description: "Timestamp." },
    FieldType { id: "bool", label: "Boolean", pg_type: "boolean", description: "True/false." },
    FieldType { id: "email", label: "Email", pg_type: "text", description: "Email address." },
    FieldType { id: "phone", label: "Phone", pg_type: "text", description: "Phone number." },
    FieldType { id: "select", label: "Choice", pg_type: "text", description: "One of a fixed list of choices." },
    FieldType { id: "multiselect", label: "Multi choice", pg_type: "jsonb", description: "Several of a fixed list." },
    FieldType { id: "ref", label: "Reference", pg_type: "uuid", description: "Link to another entity's record." },
    FieldType { id: "uuid", label: "Identifier", pg_type: "uuid", description: "Unique identifier." },
    FieldType { id: "attachment", label: "Attachment", pg_type: "jsonb", description: "File reference (name/url)." },
];

pub const TEMPLATES: &[Template] = &[
    Template { id: "blank", label: "Blank", description: "No modules; build everything from prompts.", modules: &[] },
    Template { id: "crm", label: "CRM", description: "Leads, deals, contacts, activities and a sales pipeline.", modules: &["crm"] },
    Template { id: "erp", label: "ERP", description: "Products, inventory, orders, suppliers, invoices.", modules: &["inventory", "sales", "purchasing", "accounting"] },
    Template { id: "services", label: "Services", description: "Clients, projects, tickets, time entries, invoicing.", modules: &["clients", "projects", "tickets", "time"] },
    Template { id: "retail", label: "Retail", description: "Catalog, stock, POS sales, suppliers.", modules: &["catalog", "stock", "pos", "suppliers"] },
];

/// JSON description of the whole catalog, handed to the model as its palette.
pub fn catalog_json() -> Value {
    json!({
        "field_types": FIELD_TYPES.iter().map(|f| json!({
            "id": f.id, "label": f.label, "description": f.description
        })).collect::<Vec<_>>(),
        "ui_components": UI_COMPONENTS.iter().map(|c| json!({
            "id": c.id, "label": c.label, "kind": c.kind, "description": c.description
        })).collect::<Vec<_>>(),
        "templates": TEMPLATES.iter().map(|t| json!({
            "id": t.id, "label": t.label, "description": t.description, "modules": t.modules
        })).collect::<Vec<_>>(),
    })
}

/// True when `id` is a known UI component.
pub fn is_ui_component(id: &str) -> bool {
    UI_COMPONENTS.iter().any(|c| c.id == id)
}

/// True when `id` is a known field type.
pub fn is_field_type(id: &str) -> bool {
    FIELD_TYPES.iter().any(|f| f.id == id)
}

/// True when `id` is a known template.
pub fn is_template(id: &str) -> bool {
    TEMPLATES.iter().any(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique() {
        let mut ids: Vec<&str> = UI_COMPONENTS.iter().map(|c| c.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate ui component id");
        let mut fids: Vec<&str> = FIELD_TYPES.iter().map(|f| f.id).collect();
        let fn_ = fids.len();
        fids.sort();
        fids.dedup();
        assert_eq!(fids.len(), fn_, "duplicate field type id");
    }

    #[test]
    fn lookups_work() {
        assert!(is_ui_component("table"));
        assert!(is_field_type("money"));
        assert!(is_template("crm"));
        assert!(!is_ui_component("nope"));
    }
}
