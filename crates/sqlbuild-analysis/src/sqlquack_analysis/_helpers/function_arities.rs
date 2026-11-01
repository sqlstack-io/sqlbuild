//! The argument counts DuckDB's built-in functions accept, from the catalog polyglot validates.

use std::collections::HashMap;
use std::sync::LazyLock;

use polyglot_sql::{
    DialectType, FunctionCatalog, FunctionNameCase, FunctionSignature, HashMapFunctionCatalog,
};
use polyglot_sql_function_catalogs::CatalogSink;

use crate::sqlquack_analysis::constants::GROUPING_FUNCTIONS;

/// The embedded catalogs, registered as polyglot registers them, with grouping syntax added.
static CATALOG: LazyLock<HashMapFunctionCatalog> = LazyLock::new(|| {
    let mut sink: Sink = Sink {
        catalog: HashMapFunctionCatalog::default(),
        dialects: HashMap::new(),
    };
    polyglot_sql_function_catalogs::register_enabled_catalogs(&mut sink);
    for name in GROUPING_FUNCTIONS {
        sink.catalog.register(
            DialectType::DuckDB,
            name,
            vec![FunctionSignature::variadic(1)],
        );
    }
    sink.catalog
});

struct Sink {
    catalog: HashMapFunctionCatalog,
    dialects: HashMap<&'static str, Option<DialectType>>,
}

impl Sink {
    fn dialect(&mut self, dialect: &'static str) -> Option<DialectType> {
        *self
            .dialects
            .entry(dialect)
            .or_insert_with(|| dialect.parse::<DialectType>().ok())
    }
}

fn name_case(case: polyglot_sql_function_catalogs::FunctionNameCase) -> FunctionNameCase {
    match case {
        polyglot_sql_function_catalogs::FunctionNameCase::Insensitive => {
            FunctionNameCase::Insensitive
        }
        polyglot_sql_function_catalogs::FunctionNameCase::Sensitive => FunctionNameCase::Sensitive,
    }
}

impl CatalogSink for Sink {
    fn set_dialect_name_case(
        &mut self,
        dialect: &'static str,
        case: polyglot_sql_function_catalogs::FunctionNameCase,
    ) {
        if let Some(dialect) = self.dialect(dialect) {
            self.catalog.set_dialect_name_case(dialect, name_case(case));
        }
    }

    fn set_function_name_case(
        &mut self,
        dialect: &'static str,
        function_name: &str,
        case: polyglot_sql_function_catalogs::FunctionNameCase,
    ) {
        if let Some(dialect) = self.dialect(dialect) {
            self.catalog
                .set_function_name_case(dialect, function_name, name_case(case));
        }
    }

    fn register(
        &mut self,
        dialect: &'static str,
        function_name: &str,
        signatures: Vec<polyglot_sql_function_catalogs::FunctionSignature>,
    ) {
        if let Some(dialect) = self.dialect(dialect) {
            let signatures: Vec<FunctionSignature> = signatures
                .into_iter()
                .map(|signature| FunctionSignature {
                    min_arity: signature.min_arity,
                    max_arity: signature.max_arity,
                })
                .collect();
            self.catalog.register(dialect, function_name, signatures);
        }
    }
}

/// The `(least, most)` argument counts DuckDB accepts for `name`, its canonical name `canonical`.
pub(crate) fn duckdb_arities(name: &str, canonical: &str) -> Option<Vec<(usize, Option<usize>)>> {
    let written: &str = name.rsplit('.').next().unwrap_or(name).trim();
    let signatures: &[FunctionSignature] =
        CATALOG.lookup(DialectType::DuckDB, written, canonical)?;
    Some(
        signatures
            .iter()
            .map(|signature| (signature.min_arity, signature.max_arity))
            .collect(),
    )
}
