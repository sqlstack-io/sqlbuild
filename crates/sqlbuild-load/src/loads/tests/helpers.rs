use sqlload::column_types::models::Warehouse;

use crate::loads::main::check_load::check_load;
use crate::loads::models::{FilledSource, LoadCheck, LoadDeclaration};
use crate::loads::tests::test_types::ListedSources;

/// The project every case checks against: a Postgres source connection and a DuckDB warehouse connection without `kind`.
pub(super) fn project_check() -> LoadCheck {
    LoadCheck {
        connections: toml::from_str(
            "[shop_db]\nkind = \"postgres\"\nhost = \"db\"\nuser = \"reader\"\ndatabase = \"shop\"\n\
             password = \"${ENV:SHOP_PASSWORD}\"\n",
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
