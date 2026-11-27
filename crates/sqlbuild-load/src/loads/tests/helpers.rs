use sqlload::adapters::dispatch::main::dialect::sql_adapter::sql_adapter;
use sqlload::column_types::models::{ColumnType, Warehouse};

use crate::loads::_helpers::column_types::created_type;
use crate::loads::main::declared_type_mismatch::declared_type_mismatch;

use crate::loads::main::check_load::check_load;
use crate::loads::models::{FilledSource, LoadCheck, LoadDeclaration};
use crate::loads::tests::test_types::ListedSources;

/// The project every case checks against: a Postgres source connection and a DuckDB warehouse connection without `kind`.
pub(super) fn project_check() -> LoadCheck {
    LoadCheck {
        connections: toml::from_str(
            "[shop_db]\nkind = \"postgres\"\nhost = \"db\"\nuser = \"reader\"\ndatabase = \"shop\"\n\
             password = \"${ENV:SHOP_PASSWORD}\"\n\
             [landing]\nkind = \"filesystem\"\nroot = \"/landing\"\n",
        )
        .expect("connections"),
        warehouse_connections: vec![("warehouse".to_owned(), false)],
        warehouse: Warehouse::DuckDb,
        motherduck: false,
    }
}

/// `settings` checked as the load `shop` with `sources`, each declaring `on_schema_change`: its `tables` as TOML text, or the message and path.
pub(super) fn checked(
    settings: &str,
    (sources, on_schema_change): (ListedSources, Option<&str>),
) -> Result<String, (String, Vec<String>)> {
    let load = LoadDeclaration {
        name: "shop".to_owned(),
        settings: toml::from_str(settings).expect("settings"),
        sources: sources
            .iter()
            .map(|(name, columns)| FilledSource {
                name: (*name).to_owned(),
                columns: columns
                    .iter()
                    .map(|(column, kind)| ((*column).to_owned(), (*kind).to_owned()))
                    .collect(),
                on_schema_change: on_schema_change.map(str::to_owned),
            })
            .collect(),
    };
    check_load(&project_check(), &load)
        .map(|settings| {
            settings
                .get("tables")
                .map(ToString::to_string)
                .unwrap_or_default()
        })
        .map_err(|problem| (problem.message, problem.path))
}

pub(super) const LOGICAL_TYPE_COUNT: usize = 15;

/// Every logical type SQLLoad creates columns for.
pub(super) fn logical_types() -> [ColumnType; LOGICAL_TYPE_COUNT] {
    [
        ColumnType::Bool,
        ColumnType::SmallInt,
        ColumnType::Int,
        ColumnType::Decimal {
            precision: 18,
            scale: 2,
        },
        ColumnType::Float,
        ColumnType::Text,
        ColumnType::Varchar(20),
        ColumnType::Date,
        ColumnType::Time,
        ColumnType::TimestampTz,
        ColumnType::TimestampNtz,
        ColumnType::Json,
        ColumnType::Uuid,
        ColumnType::Binary,
        ColumnType::Array(Box::new(ColumnType::Int)),
    ]
}

/// Whether declaring `column_type`'s advice in `warehouse` creates the type SQLLoad created for it.
pub(super) fn advice_creates_exactly(warehouse: Warehouse, column_type: &ColumnType) -> bool {
    let created: String = sql_adapter(warehouse)
        .render_loader_logical_type(column_type)
        .sql;
    let advice: String = declared_type_mismatch(
        &[("c".to_owned(), column_type.to_string())],
        warehouse,
        |_, _| false,
    )
    .map(|mismatch| mismatch.declare)
    .unwrap_or_default();
    created_type(&advice, warehouse).as_deref() == Some(created.as_str())
}
