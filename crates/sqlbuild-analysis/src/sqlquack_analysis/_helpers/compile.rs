//! One compiled model's lineage from SQLQuack, in SQLBuild's compile lineage terms.

use std::collections::{BTreeSet, HashMap, HashSet};

use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::NameFinding;
use sqlquack_semantics::lineage::models::{
    LineageContext, Nullability, OutputLineage, SchemaColumn,
};
use sqlquack_semantics::{query_lineage_and_findings, query_name_findings};
use sqlquack_syntax::Dialect;

use crate::constants::UNKNOWN_SQL_TYPE;
use crate::sqlquack_analysis::_helpers::outputs::{
    confidence, lineage_parse_options, transform_kind,
};
use crate::sqlquack_analysis::_helpers::schema::{TypedRelations, relation_key, schema_columns};
use crate::sqlquack_analysis::constants::{
    NON_NULL, NULLABLE, QUOTE, TEXT_TYPE, UNKNOWN_NULLABILITY, VARCHAR_TYPE,
};
use crate::sqlquack_analysis::models::{
    CompileFinding, CompileFunctions, CompileLineageColumn, CompiledLineage, CompiledModelSql,
};

/// The model's lineage columns, `None` when SQLQuack cannot read its SQL.
pub(crate) fn model_lineage(
    model: &CompiledModelSql<'_>,
    functions: CompileFunctions<'_>,
    (dialect, profile): (Dialect, &'static dyn DialectProfile),
) -> Option<CompiledLineage> {
    let quoted: HashSet<&str> = quoted_identifiers(model.sql);
    let schema = typed_relations(model, functions, (&quoted, profile), true);
    let references: HashMap<String, (&str, &str)> = model
        .references
        .iter()
        .map(|(name, kind, resource)| {
            (
                relation_key(name, profile),
                (kind.as_str(), resource.as_str()),
            )
        })
        .collect();
    let context = LineageContext {
        profile,
        schema: &schema,
        sql: None,
    };
    match query_lineage_and_findings(model.sql, dialect, &lineage_parse_options(), context) {
        Ok((lineage, findings)) => Some(CompiledLineage {
            findings: catalog_findings(model, functions, (&quoted, dialect, profile), findings)
                .into_iter()
                .map(|finding| CompileFinding {
                    code: finding.code.to_owned(),
                    message: finding.message,
                    location: finding
                        .span
                        .map(|span| (span.start, span.end, span.line, span.column)),
                    written: finding.written,
                })
                .collect(),
            columns: lineage
                .outputs
                .iter()
                .filter(|output| !output.name.is_empty())
                .map(|output| lineage_column(output, &references, profile))
                .collect(),
            projects_star: lineage.projects_star,
            open_star: lineage.open_star,
        }),
        Err(_unreadable) => None,
    }
}

/// The model's relations as SQLQuack reads them; `quoting` reads a lower-case catalog name the
/// SQL quotes as that quoted name, as lineage does.
fn typed_relations<'a>(
    model: &CompiledModelSql<'_>,
    functions: CompileFunctions<'a>,
    (quoted, profile): (&HashSet<&str>, &'static dyn DialectProfile),
    quoting: bool,
) -> TypedRelations<'a> {
    let relations =
        |shapes: &[(String, Vec<(String, String)>)]| -> HashMap<String, Vec<SchemaColumn>> {
            shapes
                .iter()
                .filter(|(_, columns)| !columns.is_empty())
                .map(|(relation, columns)| {
                    let nullability: &[(String, String)] = relation_nullability(model, relation);
                    (
                        relation_key(relation, profile),
                        typed_columns(columns, nullability, (quoted, profile), quoting),
                    )
                })
                .collect()
        };
    let closed: HashMap<String, Vec<SchemaColumn>> = relations(model.schema);
    let open: HashMap<String, Vec<SchemaColumn>> = relations(model.types)
        .into_iter()
        .filter(|(relation, _)| !closed.contains_key(relation))
        .collect();
    TypedRelations {
        closed,
        open,
        functions,
    }
}

/// The findings validation reports: a lower-case catalog name stays unquoted, as polyglot reads
/// it, so findings are made again over that schema when lineage quoted one.
fn catalog_findings(
    model: &CompiledModelSql<'_>,
    functions: CompileFunctions<'_>,
    (quoted, dialect, profile): (&HashSet<&str>, Dialect, &'static dyn DialectProfile),
    findings: Vec<NameFinding>,
) -> Vec<NameFinding> {
    let quotes_catalog_name: bool = model
        .schema
        .iter()
        .chain(model.types)
        .flat_map(|(_, columns)| columns.iter())
        .any(|(name, _)| {
            quoted.contains(name.as_str()) && profile.resolved_identifier(name, false) != *name
        });
    if !quotes_catalog_name {
        return findings;
    }
    let catalog_schema = typed_relations(model, functions, (quoted, profile), false);
    let context = LineageContext {
        profile,
        schema: &catalog_schema,
        sql: None,
    };
    query_name_findings(model.sql, dialect, &lineage_parse_options(), context).unwrap_or(findings)
}

/// The catalog's `(column, nullability)` pairs for `relation`.
fn relation_nullability<'a>(
    model: &CompiledModelSql<'a>,
    relation: &str,
) -> &'a [(String, String)] {
    model
        .nullability
        .iter()
        .find(|(name, _)| name == relation)
        .map_or(&[], |(_, pairs)| pairs.as_slice())
}

/// A relation's typed schema columns; a lower-case name the SQL reads quoted is that quoted name.
fn typed_columns(
    columns: &[(String, String)],
    nullability: &[(String, String)],
    (quoted, profile): (&HashSet<&str>, &dyn DialectProfile),
    quoting: bool,
) -> Vec<SchemaColumn> {
    let mut typed: Vec<SchemaColumn> =
        schema_columns(columns.iter().map(|(name, _)| name.as_str()));
    for column in &mut typed {
        if quoting
            && !column.quoted
            && quoted.contains(column.name.as_str())
            && profile.resolved_identifier(&column.name, false) != column.name
        {
            column.quoted = true;
        }
        let written = |name: &String| name.trim_matches(QUOTE) == column.name;
        column.data_type = columns
            .iter()
            .find(|(name, _)| written(name))
            .map(|(_, data_type)| data_type.clone())
            .filter(|data_type| !data_type.is_empty() && data_type != UNKNOWN_SQL_TYPE);
        column.nullability = match nullability.iter().find(|(name, _)| written(name)) {
            Some((_, value)) if value == NON_NULL => Nullability::NonNull,
            Some((_, value)) if value == NULLABLE => Nullability::Nullable,
            _ => Nullability::Unknown,
        };
    }
    typed
}

fn lineage_column(
    output: &OutputLineage,
    references: &HashMap<String, (&str, &str)>,
    profile: &dyn DialectProfile,
) -> CompileLineageColumn {
    let mut sources: BTreeSet<(String, String, String)> = BTreeSet::new();
    for source in &output.sources {
        let Some((kind, resource)) = references.get(&source.relation) else {
            continue;
        };
        sources.insert((
            (*kind).to_owned(),
            (*resource).to_owned(),
            profile.resolved_identifier(&source.column, source.quoted),
        ));
    }
    CompileLineageColumn {
        output_column: output.name.clone(),
        transform: transform_kind(output.kind),
        confidence: confidence(output.resolved, !sources.is_empty()),
        sources: sources.into_iter().collect(),
        data_type: output.data_type.as_deref().map(project_type),
        nullability: match output.nullability {
            Nullability::NonNull => NON_NULL,
            Nullability::Nullable => NULLABLE,
            Nullability::Unknown => UNKNOWN_NULLABILITY,
        },
    }
}

/// An output type in SQLBuild's spelling: unsized text is `TEXT`, sized text `VARCHAR(n)`.
fn project_type(data_type: &str) -> String {
    let upper: String = data_type.to_uppercase();
    if upper == VARCHAR_TYPE {
        return TEXT_TYPE.to_owned();
    }
    match upper
        .strip_prefix("TEXT(")
        .and_then(|suffix| suffix.strip_suffix(')'))
    {
        Some(length) => format!("{VARCHAR_TYPE}({length})"),
        None => data_type.to_owned(),
    }
}

/// The double-quoted identifiers of `sql`, outside string literals and comments.
fn quoted_identifiers(sql: &str) -> HashSet<&str> {
    let mut found: HashSet<&str> = HashSet::new();
    let bytes: &[u8] = sql.as_bytes();
    let mut index: usize = 0;
    while index < bytes.len() {
        match (bytes[index], bytes.get(index + 1)) {
            (b'\'', _) => index = string_end(bytes, index + 1),
            (b'-', Some(b'-')) => {
                index = bytes[index..]
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |end| index + end);
            }
            (b'/', Some(b'*')) => {
                index = sql[index + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |end| index + 2 + end + 2);
            }
            (b'"', _) => {
                let start: usize = index + 1;
                match bytes[start..].iter().position(|byte| *byte == b'"') {
                    Some(length) => {
                        found.insert(&sql[start..start + length]);
                        index = start + length + 1;
                    }
                    None => index = bytes.len(),
                }
            }
            _ => index += 1,
        }
    }
    found
}

/// The position after the single-quoted literal whose body starts at `index`.
fn string_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'\'' if bytes.get(index + 1) == Some(&b'\'') => index += 2,
            b'\'' => return index + 1,
            _ => index += 1,
        }
    }
    index
}
