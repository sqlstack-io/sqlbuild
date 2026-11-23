use crate::manifest::models::{
    ManifestConfigInput, ManifestHookInput, ManifestInput, ManifestModelInput,
    ManifestModelSchemaInput, ManifestPathInput, ManifestRelationInput, ManifestSqlTestPlanInput,
};
use crate::python_values::main::text_key::text_key;
use crate::python_values::models::{PythonKey, PythonValue};

pub(crate) fn config() -> ManifestConfigInput {
    ManifestConfigInput {
        enabled: PythonValue::Bool(true),
        alias: PythonValue::Null,
        schema: PythonValue::Null,
        database: PythonValue::Null,
        tags: None,
        materialized: "table".to_owned(),
        incremental_strategy: PythonValue::Null,
        merge_exclude_columns: PythonValue::Null,
        full_refresh: PythonValue::Null,
        unique_key: PythonValue::Null,
        on_schema_change: PythonValue::String("ignore".to_owned()),
        pre_hooks: None,
        post_hooks: None,
    }
}

pub(crate) fn sql_hook() -> ManifestHookInput {
    ManifestHookInput::Sql {
        statement: "select 1".to_owned(),
        name: Some("audit_orders".to_owned()),
        relative_path: None,
        definition_sql: None,
        kwargs: PythonValue::Null,
        description: Some(String::new()),
    }
}

pub(crate) fn meta(entries: Vec<(PythonKey, PythonValue)>) -> Option<PythonValue> {
    Some(PythonValue::Object(entries))
}

pub(crate) fn model(meta: Option<PythonValue>, config: ManifestConfigInput) -> ManifestModelInput {
    ManifestModelInput {
        name: "orders".to_owned(),
        path: ManifestPathInput {
            text: "sql/models/orders.sql".to_owned(),
            parent_parts: vec!["models".to_owned()],
            stem: "orders".to_owned(),
        },
        destination: ManifestRelationInput::default(),
        query_sql: "SELECT 1 AS order_id".to_owned(),
        config,
        schema: Some(ManifestModelSchemaInput {
            description: None,
            meta,
            columns: Vec::new(),
            dynamic_columns: Vec::new(),
        }),
        deps: Vec::new(),
    }
}

pub(crate) fn project(models: Vec<ManifestModelInput>) -> ManifestInput {
    ManifestInput {
        project_name: "shop".to_owned(),
        adapter_type: "duckdb".to_owned(),
        run_id: "run".to_owned(),
        generated_at: "2026-10-12T00:00:00+00:00".to_owned(),
        models,
        ..ManifestInput::default()
    }
}

pub(crate) fn authored(key: &str, value: PythonValue) -> (PythonKey, PythonValue) {
    (text_key(key), value)
}

pub(crate) fn sql_test(parameter: &str) -> ManifestSqlTestPlanInput {
    ManifestSqlTestPlanInput {
        key: ("sql_test".to_owned(), "orders_case".to_owned()),
        name: "orders_case".to_owned(),
        chain: Vec::new(),
        scope_deps: Vec::new(),
        source_path: Some("tests/orders.sql".to_owned()),
        block_index: PythonValue::Null,
        parent_name: PythonValue::Null,
        case_name: Some("first".to_owned()),
        case_index: PythonValue::Null,
        case_fingerprint: None,
        parameter_schema: vec![(
            "amount".to_owned(),
            "decimal".to_owned(),
            PythonValue::Bool(false),
        )],
        parameters: vec![(parameter.to_owned(), PythonValue::String("1.50".to_owned()))],
    }
}
