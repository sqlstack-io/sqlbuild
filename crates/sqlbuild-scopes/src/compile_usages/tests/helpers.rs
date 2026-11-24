use std::collections::HashMap;

use crate::compile_usages::models::{NamedRecord, SchemaDeclaration};
use crate::scope_index::models::{DeclaredScope, ScopeKind};

fn record(handle: usize, path: &str, scope: ScopeKind, owning_path: Option<&str>) -> NamedRecord {
    NamedRecord {
        handle,
        path: path.to_owned(),
        declared: DeclaredScope {
            scope,
            owner: None,
            owning_path: owning_path.map(str::to_owned),
        },
        ownership_root: "sql/models".to_owned(),
    }
}

pub(super) fn sales_schemas() -> Vec<SchemaDeclaration> {
    vec![
        SchemaDeclaration {
            name: "orders_schema".to_owned(),
            extends: Some("base_schema".to_owned()),
            columns: vec![
                ("status".to_owned(), Some("order_status".to_owned())),
                ("amount".to_owned(), Some("DECIMAL".to_owned())),
                ("note".to_owned(), None),
            ],
        },
        SchemaDeclaration {
            name: "unindexed".to_owned(),
            extends: None,
            columns: Vec::new(),
        },
    ]
}

pub(super) fn visible_schema_records() -> HashMap<String, NamedRecord> {
    HashMap::from([
        (
            "orders_schema".to_owned(),
            record(
                0,
                "sql/models/sales/schemas/orders.yml",
                ScopeKind::Inherited,
                Some("sql/models/sales"),
            ),
        ),
        (
            "base_schema".to_owned(),
            record(1, "sql/schemas/base.yml", ScopeKind::Global, None),
        ),
    ])
}

pub(super) fn hidden_schema_records() -> HashMap<String, NamedRecord> {
    HashMap::from([
        (
            "orders_schema".to_owned(),
            record(
                0,
                "sql/models/sales/schemas/orders.yml",
                ScopeKind::Inherited,
                Some("sql/models/sales"),
            ),
        ),
        (
            "base_schema".to_owned(),
            record(
                1,
                "sql/models/finance/schemas/base.yml",
                ScopeKind::Inherited,
                Some("sql/models/finance"),
            ),
        ),
    ])
}

pub(super) fn visible_enum_records() -> HashMap<String, NamedRecord> {
    HashMap::from([(
        "order_status".to_owned(),
        record(2, "sql/enums/status.yml", ScopeKind::Global, None),
    )])
}

pub(super) fn hidden_enum_records() -> HashMap<String, NamedRecord> {
    HashMap::from([(
        "order_status".to_owned(),
        record(
            2,
            "sql/models/finance/enums/status.yml",
            ScopeKind::Local,
            Some("sql/models/finance"),
        ),
    )])
}
