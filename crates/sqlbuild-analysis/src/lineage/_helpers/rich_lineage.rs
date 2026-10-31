//! Python's `_build_polyglot_model_column_lineage`: rich lineage from polyglot query analysis.

use std::collections::{BTreeSet, HashMap, HashSet};

use polyglot_sql::{
    AnalyzeQueryOptions, ColumnReferenceFact, ComplexityGuardOptions, DialectType, ProjectionFact,
    ProjectionNullability, QueryAnalysis, ReferenceConfidence, SchemaColumn, SchemaTable,
    TransformKind, ValidationSchema, analyze_query,
};

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;

use crate::lineage::_helpers::references::{
    normalized_sql, physical_resource_name, physical_resources,
};
use crate::lineage::_helpers::stars::star_lineage;
use crate::lineage::constants::{MAX_FUNCTION_CALL_DEPTH, STAR_COLUMN_NAME, UNKNOWN_COLUMN_TYPE};
use crate::lineage::models::PhysicalResource;
use crate::lineage::models::{
    LineageColumn, LineageConfidence, LineageNullability, LineageResourceType,
    LineageSchemaResource, LineageSource, LineageTransformKind, RichLineageColumn,
    RichLineageOutcome, RichSchemaResource,
};

/// What every model of one request shares: the dialect, the guard and the known tables.
pub(crate) struct RichContext {
    dialect: DialectType,
    guard: ComplexityGuardOptions,
    /// `_polyglot_schema_tables`: each physical name's columns, sorted by name.
    tables: HashMap<String, SchemaTable>,
    /// `_build_schema_mapping`'s column names, in its insertion order, for star expansion.
    names: HashMap<String, Vec<String>>,
}

impl RichContext {
    /// Only tables some model references are built: the wheel is only ever sent those.
    pub(crate) fn new(
        dialect: DialectType,
        schema: &[RichSchemaResource],
        referenced: &HashSet<String>,
        names: HashMap<String, Vec<String>>,
    ) -> Self {
        Self {
            dialect,
            guard: ComplexityGuardOptions {
                max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
                ..ComplexityGuardOptions::default()
            },
            tables: schema_tables(schema, referenced),
            names,
        }
    }
}

/// Analyse every model in parallel; the first panic in request order is the request's error.
pub(crate) fn outcomes_in_order(
    models: &[String],
    analyse: impl Fn(&str) -> RichLineageOutcome + Sync,
) -> Result<Vec<RichLineageOutcome>, String> {
    let outcomes: Vec<Result<RichLineageOutcome, String>> = models
        .par_iter()
        .map(|query_sql| catch_compiler_panic(|| Ok(analyse(query_sql))))
        .collect();
    outcomes.into_iter().collect()
}

/// The column names star expansion reads: assigned then defaulted, first position kept.
pub(crate) fn schema_names(resource: &RichSchemaResource) -> LineageSchemaResource {
    LineageSchemaResource {
        resource_type: resource.resource_type,
        name: resource.name.clone(),
        columns: resource
            .assigned
            .iter()
            .chain(&resource.defaulted)
            .map(|(name, _)| name.clone())
            .collect(),
    }
}

/// The physical name's typed columns; a later resource with columns replaces an earlier one.
pub(crate) fn schema_tables(
    schema: &[RichSchemaResource],
    referenced: &HashSet<String>,
) -> HashMap<String, SchemaTable> {
    let mut tables: HashMap<String, SchemaTable> = HashMap::with_capacity(referenced.len());
    for resource in schema {
        let physical_name = physical_resource_name(resource.resource_type, &resource.name);
        if !referenced.contains(&physical_name) {
            continue;
        }
        let mut types: HashMap<&str, &str> = HashMap::new();
        for (name, column_type) in &resource.assigned {
            let _ = types.insert(name, python_type(column_type.as_deref()));
        }
        for (name, column_type) in &resource.defaulted {
            let _ = types
                .entry(name)
                .or_insert_with(|| python_type(column_type.as_deref()));
        }
        if types.is_empty() {
            continue;
        }
        let mut columns: Vec<(&str, &str)> = types.into_iter().collect();
        columns.sort_unstable();
        let table = SchemaTable {
            name: physical_name.clone(),
            schema: None,
            columns: columns
                .into_iter()
                .map(|(name, column_type)| SchemaColumn {
                    name: name.to_owned(),
                    data_type: column_type.to_owned(),
                    nullable: None,
                    primary_key: false,
                    unique: false,
                    references: None,
                })
                .collect(),
            aliases: Vec::new(),
            primary_key: Vec::new(),
            unique_keys: Vec::new(),
            foreign_keys: Vec::new(),
        };
        let _ = tables.insert(physical_name, table);
    }
    tables
}

/// Python's `type or "UNKNOWN"`.
fn python_type(column_type: Option<&str>) -> &str {
    column_type
        .filter(|column_type| !column_type.is_empty())
        .unwrap_or(UNKNOWN_COLUMN_TYPE)
}

pub(crate) fn rich_model_lineage(query_sql: &str, context: &RichContext) -> RichLineageOutcome {
    let dialect = context.dialect;
    let physical = physical_resources(query_sql);
    let referenced: BTreeSet<&str> = physical
        .iter()
        .map(|resource| resource.physical_name.as_str())
        .collect();
    let options = AnalyzeQueryOptions {
        complexity_guard: Some(context.guard),
        dialect,
        schema: Some(ValidationSchema {
            tables: referenced
                .into_iter()
                .filter_map(|name| context.tables.get(name).cloned())
                .collect(),
            strict: None,
        }),
    };
    let analysis: QueryAnalysis = match analyze_query(&normalized_sql(query_sql), options) {
        Ok(analysis) => analysis,
        Err(error) => return RichLineageOutcome::Skipped(error.to_string()),
    };
    let resource_by_physical_name: HashMap<&str, &PhysicalResource> = physical
        .iter()
        .map(|resource| (resource.physical_name.as_str(), resource))
        .collect();
    let star_expanded: HashSet<&str> = analysis
        .star_projections
        .iter()
        .flat_map(|star| &star.expanded_columns)
        .filter(|column| !column.is_empty())
        .map(String::as_str)
        .collect();
    let mut columns: Vec<RichLineageColumn> = Vec::new();
    for projection in &analysis.projections {
        if projection.is_star {
            continue;
        }
        let Some(output_column) = projection.name.as_deref().filter(|name| !name.is_empty()) else {
            continue;
        };
        if star_expanded.contains(output_column) {
            continue;
        }
        let (upstream_columns, confidence) =
            projection_upstreams(&projection.upstream, &resource_by_physical_name);
        let transform_kind =
            transform_kind(projection.transform_kind, !upstream_columns.is_empty());
        let confidence =
            if !upstream_columns.is_empty() || transform_kind == LineageTransformKind::Constant {
                confidence
            } else {
                LineageConfidence::Unknown
            };
        columns.push(RichLineageColumn {
            column: LineageColumn {
                output_column: output_column.to_owned(),
                transform_kind,
                confidence,
                upstream_columns,
            },
            nullability: nullability(projection),
        });
    }
    let has_star = !star_expanded.is_empty();
    if has_star {
        let star_columns = star_lineage(
            &context.names,
            &physical,
            columns
                .iter()
                .map(|column| column.column.output_column.as_str()),
        );
        columns.extend(star_columns.into_iter().map(|column| RichLineageColumn {
            column,
            nullability: LineageNullability::Unknown,
        }));
    }
    RichLineageOutcome::Built { columns, has_star }
}

/// `_projection_upstreams`, including its order: a later unaliased guess overwrites UNKNOWN.
fn projection_upstreams(
    upstream: &[ColumnReferenceFact],
    resource_by_physical_name: &HashMap<&str, &PhysicalResource>,
) -> (Vec<LineageSource>, LineageConfidence) {
    let mut sources: Vec<LineageSource> = Vec::new();
    let mut seen: HashSet<(LineageResourceType, &str, &str)> = HashSet::new();
    let mut confidence = LineageConfidence::High;
    for reference in upstream {
        let column = reference.column.as_str();
        if column.is_empty() || column == STAR_COLUMN_NAME {
            continue;
        }
        let source_name = reference
            .source_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .or(reference.table.as_deref())
            .filter(|name| !name.is_empty());
        let Some(source_name) = source_name else {
            confidence = LineageConfidence::Unknown;
            continue;
        };
        let Some(resource) = resource_by_physical_name.get(source_name) else {
            continue;
        };
        if reference.confidence != ReferenceConfidence::Resolved && reference.source_alias.is_none()
        {
            confidence = LineageConfidence::Medium;
        }
        if !seen.insert((
            resource.resource_type,
            resource.resource_name.as_str(),
            column,
        )) {
            continue;
        }
        sources.push(LineageSource {
            resource_type: resource.resource_type,
            resource_name: resource.resource_name.clone(),
            column_name: column.to_owned(),
        });
    }
    sources.sort_by(|left, right| {
        (
            left.resource_type.as_str(),
            &left.resource_name,
            &left.column_name,
        )
            .cmp(&(
                right.resource_type.as_str(),
                &right.resource_name,
                &right.column_name,
            ))
    });
    (sources, confidence)
}

/// `_projection_transform_kind`: polyglot's `star` reads as an expression or a constant.
fn transform_kind(kind: TransformKind, has_upstream: bool) -> LineageTransformKind {
    match kind {
        TransformKind::Cast => LineageTransformKind::Cast,
        TransformKind::Aggregation => LineageTransformKind::Aggregation,
        TransformKind::Constant => LineageTransformKind::Constant,
        _ if !has_upstream => LineageTransformKind::Constant,
        TransformKind::Direct => LineageTransformKind::Direct,
        TransformKind::Expression | TransformKind::Star => LineageTransformKind::Expression,
    }
}

fn nullability(projection: &ProjectionFact) -> LineageNullability {
    match projection.nullability {
        ProjectionNullability::NonNull => LineageNullability::NonNull,
        ProjectionNullability::Nullable => LineageNullability::Nullable,
        ProjectionNullability::Unknown => LineageNullability::Unknown,
    }
}
