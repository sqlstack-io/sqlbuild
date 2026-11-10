//! The session's binding catalog copy and Python's published relation shapes.

use std::collections::{HashMap, HashSet};

use sqlquack_semantics::lineage::models::SchemaColumn;

use crate::assembly::analysis_session::_helpers::compact_batch::{MemoKey, SharedResult};
use crate::assembly::analysis_session::_helpers::mappings::{
    ShapeTable, catalog_columns, catalog_relations, same_mapping, with_pair,
};
use crate::assembly::analysis_session::constants::{QUOTED_IDENTIFIER_DELIMITER, UNKNOWN_TYPE};
use crate::assembly::analysis_session::types::{Pairs, Shapes};
use crate::semantic_validation::main::normalize::normalize_analysis_sql;
use crate::semantic_validation::models::{Columns, NormalizationInput, ProjectCatalog};
use crate::semantic_validation::types::{BindingRequest, Relations};

/// Python's `BindingCatalog` state that model analysis reads and changes.
#[derive(Debug)]
pub(crate) struct SessionCatalog {
    pub(crate) native: ProjectCatalog,
    schemas: HashMap<String, Pairs>,
    additions: Shapes,
    analysis_names: Vec<String>,
    recorded_analysis: HashSet<String>,
    /// The `(types, nullability)` this session gave each relation's analysis.
    analysis_shapes: HashMap<String, (Pairs, Pairs)>,
    pub(crate) shared_members: usize,
    pub(crate) reanalysed_members: usize,
    /// Python's `shared_analyses`: exact shared results by their complete native input.
    pub(crate) shared_results: HashMap<MemoKey, SharedResult>,
}

impl SessionCatalog {
    /// Copy `native`, whose Python wrapper holds `schemas`.
    pub(crate) fn new(native: &ProjectCatalog, schemas: &Shapes) -> Self {
        Self {
            native: native.with_relations(Relations::new()),
            schemas: schemas.iter().cloned().collect(),
            additions: Vec::new(),
            analysis_names: Vec::new(),
            recorded_analysis: HashSet::new(),
            analysis_shapes: HashMap::new(),
            shared_members: 0,
            reanalysed_members: 0,
            shared_results: HashMap::new(),
        }
    }

    /// Python's `BindingCatalog.prepare`: add unknown closed relations, then override the rest.
    pub(crate) fn prepare(&mut self, requests: &[(&str, &Shapes)]) -> Vec<BindingRequest> {
        let mut additions: Shapes = Vec::new();
        for (_, schema) in requests {
            for (name, columns) in schema.iter() {
                if !columns.is_empty() && !self.schemas.contains_key(name) {
                    self.schemas.insert(name.clone(), columns.clone());
                    additions.push((name.clone(), columns.clone()));
                }
            }
        }
        if !additions.is_empty() {
            self.native.update_relations(catalog_relations(&additions));
            self.additions.extend(additions);
        }
        requests
            .iter()
            .map(|(sql, schema)| self.binding_request(sql, schema))
            .collect()
    }

    fn binding_request(&self, sql: &str, schema: &Shapes) -> BindingRequest {
        let mut overrides: Relations = Relations::new();
        let mut references: Vec<(String, bool)> = Vec::with_capacity(schema.len());
        for (name, columns) in schema {
            references.push((name.clone(), !columns.is_empty()));
            let known: bool = self
                .schemas
                .get(name)
                .is_some_and(|known| same_mapping(known, columns));
            if !columns.is_empty() && !known {
                overrides.insert(name.clone(), catalog_columns(columns));
            }
        }
        references.sort();
        (sql.to_owned(), references, overrides)
    }

    /// Python's `BindingCatalog.prepare_analysis` for the relations `names` a batch reads.
    pub(crate) fn prepare_analysis(
        &mut self,
        names: &[String],
        types: &ShapeTable,
        nullability: &ShapeTable,
    ) {
        let mut updates: HashMap<String, (Columns, Columns)> = HashMap::new();
        for name in names {
            let Some(columns) = nullability.get(name) else {
                continue;
            };
            let column_types: Pairs = types.get(name).cloned().unwrap_or_default();
            updates.insert(
                name.clone(),
                (catalog_columns(&column_types), catalog_columns(columns)),
            );
            self.analysis_shapes
                .insert(name.clone(), (column_types, columns.clone()));
            if self.recorded_analysis.insert(name.clone()) {
                self.analysis_names.push(name.clone());
            }
        }
        if !updates.is_empty() {
            self.native.update_analysis(updates);
        }
    }

    /// The closed shape Python's catalog `schemas` holds for `name`.
    pub(crate) fn known_schema(&self, name: &str) -> Option<&Pairs> {
        self.schemas.get(name)
    }

    /// Every closed shape the catalog holds, sorted by name.
    pub(crate) fn schema_rows(&self) -> Shapes {
        let mut rows: Shapes = self
            .schemas
            .iter()
            .map(|(name, shape)| (name.clone(), shape.clone()))
            .collect();
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        rows
    }

    /// `(shared members, shared members re-analysed alone)` so far.
    pub(crate) fn sharing(&self) -> (usize, usize) {
        (self.shared_members, self.reanalysed_members)
    }

    /// The analysis shape this session set for `name`; inherited shapes are not known here.
    pub(crate) fn session_analysis_shape(&self, name: &str) -> Option<&(Pairs, Pairs)> {
        self.analysis_shapes.get(name)
    }

    /// The shape a binding of `columns` for `name` resolves: Python's override or catalog entry.
    pub(crate) fn effective_binding_shape(&self, name: &str, columns: &Pairs) -> Pairs {
        if columns.is_empty() {
            return Vec::new();
        }
        match self.schemas.get(name) {
            Some(known) if same_mapping(known, columns) => known.clone(),
            _ => columns.clone(),
        }
    }

    pub(crate) fn register_override(&mut self, overrides: Relations) -> usize {
        self.native.register_override(overrides)
    }

    /// The `schemas` additions and analysis-shape names Python's catalog records.
    pub(crate) fn into_changes(self) -> (Shapes, Vec<String>) {
        (self.additions, self.analysis_names)
    }
}

/// How a session publishes relation shapes.
pub(crate) struct ShapeOptions<'a> {
    pub(crate) dialect: &'a str,
    pub(crate) case_sensitive: bool,
}

/// One relation's inferred columns and the SQL and inputs they came from.
pub(crate) struct ShapeSource<'a> {
    pub(crate) sql: &'a str,
    pub(crate) columns: Pairs,
    pub(crate) inputs: &'a Shapes,
    pub(crate) snapshot_columns: Option<&'a (String, String)>,
}

impl SessionCatalog {
    /// A model relation's shape, including snapshot validity columns.
    pub(crate) fn published_model_shape(
        &self,
        options: &ShapeOptions<'_>,
        source: ShapeSource<'_>,
    ) -> Result<Pairs, String> {
        let snapshot_columns: Option<&(String, String)> = source.snapshot_columns;
        let mut shape: Pairs = self.inferred_binding_shape(options, source)?;
        if let Some((valid_from, valid_to)) = snapshot_columns {
            shape = with_pair(shape, valid_from, UNKNOWN_TYPE);
            shape = with_pair(shape, valid_to, UNKNOWN_TYPE);
        }
        Ok(shape)
    }

    /// Column names keep authored quoting only where the dialect binds quoted names exactly.
    pub(crate) fn inferred_binding_shape(
        &self,
        options: &ShapeOptions<'_>,
        source: ShapeSource<'_>,
    ) -> Result<Pairs, String> {
        if !options.case_sensitive {
            return Ok(source.columns);
        }
        let normalized: String = normalize_analysis_sql(NormalizationInput {
            sql: source.sql.to_owned(),
            dialect: options.dialect.to_owned(),
            stubs: HashMap::new(),
            placeholders: HashMap::new(),
        })?;
        if !normalized.contains(QUOTED_IDENTIFIER_DELIMITER) && !has_exact_input(source.inputs) {
            return Ok(source.columns);
        }
        Ok(self
            .native
            .inferred_schema(
                &normalized,
                catalog_columns(&source.columns),
                catalog_relations(source.inputs),
            )?
            .into_iter()
            .map(|(name, data_type)| (name, data_type.unwrap_or_default()))
            .collect())
    }
}

fn has_exact_input(inputs: &Shapes) -> bool {
    inputs
        .iter()
        .flat_map(|(_, shape)| shape.iter())
        .any(|(name, _)| name.contains(QUOTED_IDENTIFIER_DELIMITER))
}

/// A relation's columns by exact name, else by the first name equal to it without case.
pub(crate) fn relation_columns<'a>(
    shapes: &'a HashMap<String, Vec<SchemaColumn>>,
    relation: &str,
) -> Option<&'a [SchemaColumn]> {
    shapes
        .get(relation)
        .or_else(|| {
            shapes
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(relation))
                .map(|(_, columns)| columns)
        })
        .map(Vec::as_slice)
}
