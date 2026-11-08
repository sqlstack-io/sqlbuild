use crate::explicit_references::models::{
    LiteralSql, ModelReferences, NamedRelation, ReferenceRequest, Relation, SourceRelation,
    SqlOwner,
};

pub(crate) fn relation(name: &str, schema: Option<&str>, database: Option<&str>) -> Relation {
    Relation {
        name: name.to_owned(),
        schema: schema.map(str::to_owned),
        database: database.map(str::to_owned),
    }
}

fn model(name: &str, migrate_from: Option<&str>) -> ModelReferences {
    ModelReferences {
        name: name.to_owned(),
        path: format!("models/{name}.sql"),
        destination: relation(name, Some("main"), Some("memory")),
        python_hooks: Vec::new(),
        sql_hooks: Vec::new(),
        migrate_from: migrate_from.map(str::to_owned),
    }
}

pub(crate) fn request() -> ReferenceRequest {
    ReferenceRequest {
        enforce_explicit: true,
        dialect: None,
        explicit_help: String::new(),
        placeholder: "__sqlbuild_python_value_".to_owned(),
        resources: Vec::new(),
        models: vec![
            model("customers", Some(" `legacy_customers` ")),
            model("orders", Some("\"archive\".old_orders")),
            model("regions", Some("customers")),
        ],
        sources: vec![
            SourceRelation {
                name: "raw_orders".to_owned(),
                table: Some(String::new()),
                schema: Some("landing".to_owned()),
                database: None,
                expression: false,
            },
            SourceRelation {
                name: "street_addresses".to_owned(),
                table: Some("straße".to_owned()),
                schema: Some("landing".to_owned()),
                database: None,
                expression: false,
            },
            SourceRelation {
                name: "computed".to_owned(),
                table: None,
                schema: None,
                database: None,
                expression: true,
            },
        ],
        seeds: vec![NamedRelation {
            name: "country_codes".to_owned(),
            relation: relation("country_codes", Some("seeds"), None),
        }],
        audits: Vec::new(),
        hooks: Vec::new(),
        owners: Vec::new(),
    }
}

/// `request()` for ClickHouse with one node running `statements` as literal SQL, in line order.
pub(crate) fn clickhouse_request(statements: &[&str]) -> ReferenceRequest {
    let literals: Vec<LiteralSql> = statements
        .iter()
        .enumerate()
        .map(|(index, sql)| LiteralSql {
            method: "execute_sql".to_owned(),
            sql: (*sql).to_owned(),
            line: index + 1,
        })
        .collect();
    ReferenceRequest {
        dialect: Some("clickhouse".to_owned()),
        owners: vec![SqlOwner {
            label: "task 'export_orders'".to_owned(),
            kind: "node".to_owned(),
            path_order: 0,
            own_refs: Vec::new(),
            upstream_loaders: Vec::new(),
            literals,
        }],
        ..request()
    }
}
