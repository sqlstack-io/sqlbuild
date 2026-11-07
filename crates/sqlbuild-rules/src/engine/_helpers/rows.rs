//! Build rules request models from compiled-model rows, with their type proofs.

use std::cell::RefCell;
use std::collections::HashMap;

use sqlbuild_analysis::type_system::errors::TypeNormalizationError;
use sqlbuild_analysis::type_system::main::normalize_type::normalize_type;
use sqlbuild_analysis::type_system::models::NormalizedType;

use crate::models::{
    Column, DynamicColumnFamily, Model, ModelRow, ModelRowsContext, SchemaRow, SqlTestMode,
};
use crate::types::TypeEquality;

/// The project counts and type comparer one model is built with.
struct ModelInputs<'a, 'c> {
    context: ModelRowsContext<'a>,
    comparer: &'c Comparer<'a>,
    compiled_audit_count: u32,
    targeting_test_count: u32,
}

pub(crate) fn build_models(
    rows: Vec<ModelRow>,
    context: ModelRowsContext<'_>,
    mut fallback: impl FnMut(&str, &str) -> Result<bool, String>,
) -> Result<Vec<Model>, String> {
    let mut audit_counts: HashMap<&str, u32> = HashMap::new();
    for target in context.attached_audit_targets {
        *audit_counts.entry(target.as_str()).or_default() += 1;
    }
    let mut test_counts: HashMap<&str, u32> = HashMap::new();
    for test in context.sql_tests {
        if matches!(test.mode, SqlTestMode::Model) {
            for name in &test.target_model_names {
                *test_counts.entry(name.as_str()).or_default() += 1;
            }
        }
    }
    let comparer: Comparer<'_> = Comparer {
        dialect: context.dialect,
        normalized: RefCell::new(HashMap::new()),
        fallback: RefCell::new(&mut fallback),
    };
    rows.into_iter()
        .map(|row| {
            let inputs: ModelInputs<'_, '_> = ModelInputs {
                context,
                comparer: &comparer,
                compiled_audit_count: audit_counts.get(row.name.as_str()).copied().unwrap_or(0),
                targeting_test_count: test_counts.get(row.name.as_str()).copied().unwrap_or(0),
            };
            build_model(row, &inputs)
        })
        .collect()
}

fn build_model(row: ModelRow, inputs: &ModelInputs<'_, '_>) -> Result<Model, String> {
    let ModelInputs {
        context,
        compiled_audit_count,
        targeting_test_count,
        ..
    } = *inputs;
    let sql_analysis_disabled: bool = !context.sql_analysis_enabled
        || row.sql_analysis_skipped
        || row.config.get("sql_analysis") == Some(&serde_json::Value::Bool(false));
    let bare_dynamic_pivot: bool = row
        .dynamic_proof
        .as_ref()
        .is_some_and(|proof| proof.bare_dynamic_pivot);
    let SchemaFacts {
        columns,
        dynamic_columns,
        dynamic_columns_proven,
        audit_count: schema_audit_count,
    } = match &row.schema {
        Some(schema) => schema_facts(&row, schema, inputs)?,
        None => SchemaFacts::default(),
    };
    Ok(Model {
        sql_analysis_disabled,
        name: row.name,
        relative_path: row.relative_path,
        query_sql: row.query_sql,
        authored_sql: row.authored_sql,
        query_line_offset: row.query_line_offset,
        query_column_offset: row.query_column_offset,
        authored_query: row.authored_query,
        config: row.config,
        authored_config_keys: row.authored_config_keys,
        logical_schema: row.logical_schema,
        references: row.references,
        columns,
        dynamic_columns,
        dynamic_columns_proven,
        bare_dynamic_pivot,
        enum_columns: row.enum_columns,
        enum_declarations: row.enum_declarations,
        constant_declarations: row.constant_declarations,
        typed_calls: row.typed_calls,
        declared_audit_count: schema_audit_count.max(compiled_audit_count),
        targeting_test_count,
        empty_input_only_test_count: 0,
        payload_digest: None,
        stripped: false,
    })
}

/// A model's declared columns and dynamic families with their proofs and audit count.
#[derive(Default)]
struct SchemaFacts {
    columns: Vec<Column>,
    dynamic_columns: Vec<DynamicColumnFamily>,
    dynamic_columns_proven: bool,
    audit_count: u32,
}

fn schema_facts(
    row: &ModelRow,
    schema: &SchemaRow,
    inputs: &ModelInputs<'_, '_>,
) -> Result<SchemaFacts, String> {
    let ModelInputs {
        context, comparer, ..
    } = *inputs;
    let inferred: HashMap<&str, Option<&str>> = row
        .inferred_columns
        .iter()
        .map(|(name, data_type)| (name.as_str(), data_type.as_deref()))
        .collect();
    let mut columns: Vec<Column> = Vec::with_capacity(schema.columns.len());
    for column in &schema.columns {
        let declared: &str = column.data_type.as_deref().unwrap_or_default();
        let inferred_type: Option<&str> = inferred.get(column.name.as_str()).copied().flatten();
        let type_proven: bool = match inferred_type {
            Some(inferred_type)
                if context.include_type_proof
                    && !declared.is_empty()
                    && !inferred_type.is_empty() =>
            {
                comparer.equal(declared, inferred_type)?
            }
            _ => false,
        };
        columns.push(Column {
            name: column.name.clone(),
            data_type: declared.to_owned(),
            nullable: column.nullable,
            audit_count: column.audit_count,
            type_proven,
        });
    }
    let audit_count: u32 = schema.audit_count
        + schema
            .columns
            .iter()
            .map(|column| column.audit_count)
            .sum::<u32>();
    if schema.dynamic_columns.is_empty() {
        return Ok(SchemaFacts {
            columns,
            dynamic_columns: Vec::new(),
            dynamic_columns_proven: false,
            audit_count,
        });
    }
    let proof_types: HashMap<&str, Option<&str>> = row
        .dynamic_proof
        .iter()
        .flat_map(|proof| &proof.families)
        .map(|(key, inferred_type)| (key.as_str(), inferred_type.as_deref()))
        .collect();
    let mut dynamic_columns: Vec<DynamicColumnFamily> = Vec::new();
    for family in &schema.dynamic_columns {
        let inferred_type: Option<&str> = proof_types.get(family.key.as_str()).copied().flatten();
        let type_proven: bool = match inferred_type {
            Some(inferred_type) if context.include_type_proof && !inferred_type.is_empty() => {
                comparer.equal(&family.data_type, inferred_type)?
            }
            _ => false,
        };
        dynamic_columns.push(DynamicColumnFamily {
            name: family.name.clone(),
            pivot_column: family.pivot_column.clone(),
            value_column: family.value_column.clone(),
            aggregate: family.aggregate.clone(),
            data_type: family.data_type.clone(),
            name_pattern: family.name_pattern.clone(),
            type_proven,
        });
    }
    let proven: bool = row
        .dynamic_proof
        .as_ref()
        .is_some_and(|proof| proof.output_proven);
    Ok(SchemaFacts {
        columns,
        dynamic_columns,
        dynamic_columns_proven: proven,
        audit_count,
    })
}

/// Python's `types_equal`, normalizing each type once; Python raises what normalization rejects.
struct Comparer<'a> {
    dialect: &'a str,
    normalized: RefCell<HashMap<String, Result<NormalizedType, TypeNormalizationError>>>,
    fallback: RefCell<&'a mut TypeEquality<'a>>,
}

impl Comparer<'_> {
    fn equal(&self, left: &str, right: &str) -> Result<bool, String> {
        match (self.normalized(left), self.normalized(right)) {
            (Ok(left), Ok(right)) => Ok(left == right),
            _ => (self.fallback.borrow_mut())(left, right),
        }
    }

    fn normalized(&self, type_sql: &str) -> Result<NormalizedType, TypeNormalizationError> {
        let dialect: &str = self.dialect;
        self.normalized
            .borrow_mut()
            .entry(type_sql.to_owned())
            .or_insert_with(|| normalize_type(type_sql, dialect).map(|found| found.normalized))
            .clone()
    }
}
