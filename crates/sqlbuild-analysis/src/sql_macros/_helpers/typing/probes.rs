//! Probe queries that read the caller's column types at each SQL macro call, and their verdicts.

use sqlquack_semantics::lineage::models::{Nullability, SchemaColumn};
use sqlquack_semantics::lineage::types::LineageSchema;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::{AnalysisInput, AnalysisRoute, NativeAnalysis};
use crate::analysis_route::types::NativeDialect;
use crate::sql_macros::_helpers::parsing::header::unquoted;
use crate::sql_macros::_helpers::parsing::lexing::{Token, TokenKind, tokens};
use crate::sql_macros::_helpers::rendering::arguments::{fits, type_family};
use crate::sql_macros::_helpers::rendering::placement::{QueryParts, top_level_parts};
use crate::sql_macros::constants::{
    ALIAS_PRECEDING_KEYWORDS, FROM_KEYWORD, INFERRED_TEXT_TYPE, PROBE_COLUMN_PREFIX,
    PROBE_TAIL_ENDS, RELATION_FUNCTION_PREFIX, STAR_PROJECTION, UNKNOWN_TYPE_HINT, VARCHAR_TYPE,
};
use crate::sql_macros::models::{
    ArgumentCheck, ArgumentExpectation, CallScope, SlotMapping, SqlMacroCheck, SqlMacroFinding,
};
use crate::sql_macros::types::{ProbeColumns, RelationShapes};

/// Where one argument's evidence is: a probe's whole column list, or one probe column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Evidence {
    Columns(usize),
    Column(usize, String),
}

/// The typed arguments of one scope, read by one probe, as `(check, argument)` positions.
struct TypedProbe<'c> {
    scope: &'c CallScope,
    columns: Vec<String>,
    arguments: Vec<(usize, usize)>,
}

/// The probes of one model and, per check and argument, where its evidence is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProbePlan {
    pub(crate) probes: Vec<String>,
    pub(crate) evidence: Vec<Vec<Option<Evidence>>>,
}

/// The CTE definitions before index `end` as a `WITH` prefix, empty when there are none.
fn with_prefix(sql: &str, parts: &QueryParts, end: usize) -> String {
    let definitions: Vec<&str> = parts.ctes[..end]
        .iter()
        .map(|cte| &sql[cte.definition.0..cte.definition.1])
        .collect();
    if definitions.is_empty() {
        String::new()
    } else {
        format!("WITH {} ", definitions.join(", "))
    }
}

/// The `FROM ... WHERE ...` text of the first select of `body`, with no later clauses.
fn from_tail(body: &str) -> Option<String> {
    let Ok(scanned) = tokens(body) else {
        return None;
    };
    let first: &Token = scanned.first()?;
    if !first.is_word(body, "SELECT") {
        return None;
    }
    let mut depth: usize = 0;
    let mut from: Option<usize> = None;
    for token in &scanned {
        if token.kind == TokenKind::Punct {
            match token.text(body) {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth = depth.saturating_sub(1),
                ";" if depth == 0 => return from.map(|start| body[start..token.start].to_owned()),
                _ => {}
            }
            continue;
        }
        if depth != 0 || token.kind != TokenKind::Word {
            continue;
        }
        let upper: String = token.text(body).to_ascii_uppercase();
        match from {
            None if upper == FROM_KEYWORD => from = Some(token.start),
            Some(start) if PROBE_TAIL_ENDS.contains(&upper.as_str()) => {
                return Some(body[start..token.start].to_owned());
            }
            _ => {}
        }
    }
    Some(from.map_or_else(String::new, |start| body[start..].to_owned()))
}

/// The `WITH` prefix and `FROM` tail a probe at `scope` selects through.
fn scope_frame(sql: &str, parts: &QueryParts, scope: &CallScope) -> Option<(String, String)> {
    match scope {
        CallScope::WithList => None,
        CallScope::Main => Some((
            with_prefix(sql, parts, parts.ctes.len()),
            from_tail(&sql[parts.main..])?,
        )),
        CallScope::Cte(name) => {
            let index: usize = parts
                .ctes
                .iter()
                .position(|cte| cte.name.eq_ignore_ascii_case(name))?;
            let body: (usize, usize) = parts.ctes[index].body;
            Some((
                with_prefix(sql, parts, index),
                from_tail(sql[body.0..body.1].trim())?,
            ))
        }
    }
}

/// The probe listing relation `name`'s columns at `scope`: a `FROM` alias first, then a CTE.
fn relation_probe(sql: &str, parts: &QueryParts, scope: &CallScope, name: &str) -> Option<String> {
    let bare: String = unquoted(name);
    let frame: Option<(String, String)> = scope_frame(sql, parts, scope);
    if let Some((prefix, tail)) = &frame
        && defines_alias(tail, &bare)
    {
        return Some(format!("{prefix}SELECT {name}.* {tail}"));
    }
    if let Some(index) = parts
        .ctes
        .iter()
        .position(|cte| cte.name.eq_ignore_ascii_case(&bare))
    {
        return Some(format!(
            "{}SELECT * FROM {name}",
            with_prefix(sql, parts, index + 1)
        ));
    }
    let (prefix, tail) = frame?;
    (!tail.is_empty()).then(|| format!("{prefix}SELECT {name}.* {tail}"))
}

/// The probe listing the columns of `source`, a relation a subquery in a macro's SQL reads.
fn capture_probe(sql: &str, parts: &QueryParts, scope: &CallScope, source: &str) -> Option<String> {
    if source.starts_with(RELATION_FUNCTION_PREFIX) {
        return Some(format!(
            "{}SELECT * FROM {source}",
            with_prefix(sql, parts, parts.ctes.len())
        ));
    }
    relation_probe(sql, parts, scope, source)
}

/// Whether the `FROM` text `tail` defines `name` as an alias, as `t AS name` or `t name`.
fn defines_alias(tail: &str, name: &str) -> bool {
    let Ok(scanned) = tokens(tail) else {
        return true;
    };
    for (index, token) in scanned.iter().enumerate() {
        let named: bool = matches!(token.kind, TokenKind::Word | TokenKind::Quoted)
            && unquoted(token.text(tail)).eq_ignore_ascii_case(name);
        let qualifies: bool = scanned
            .get(index + 1)
            .is_some_and(|next| next.is(tail, "."));
        let previous: Option<&Token> = index.checked_sub(1).and_then(|before| scanned.get(before));
        if named && !qualifies && previous.is_some_and(|before| ends_relation(tail, before)) {
            return true;
        }
    }
    false
}

/// Whether `token` can end the relation an alias follows: `AS`, `)` or a relation name.
fn ends_relation(tail: &str, token: &Token) -> bool {
    if token.is_word(tail, "AS") || token.is(tail, ")") {
        return true;
    }
    if !matches!(token.kind, TokenKind::Word | TokenKind::Quoted) {
        return false;
    }
    !ALIAS_PRECEDING_KEYWORDS
        .iter()
        .any(|keyword| token.is_word(tail, keyword))
}

/// The probes reading every check's evidence from the analysed query `sql`.
pub(crate) fn probe_plan(sql: &str, checks: &[SqlMacroCheck]) -> ProbePlan {
    let mut plan: ProbePlan = ProbePlan::default();
    let parts: Option<QueryParts> = match tokens(sql) {
        Ok(scanned) => top_level_parts(sql, &scanned),
        Err(_) => None,
    };
    let Some(parts) = parts else {
        plan.evidence = checks
            .iter()
            .map(|check| vec![None; check.arguments.len()])
            .collect();
        return plan;
    };
    let mut scopes: Vec<TypedProbe<'_>> = Vec::new();
    for (position, check) in checks.iter().enumerate() {
        let mut evidence: Vec<Option<Evidence>> = vec![None; check.arguments.len()];
        for (index, argument) in check.arguments.iter().enumerate() {
            if let ArgumentExpectation::Capture {
                source: Some(source),
                ..
            } = &argument.expected
                && let Some(probe) = capture_probe(sql, &parts, &check.scope, source)
            {
                plan.probes.push(probe);
                evidence[index] = Some(Evidence::Columns(plan.probes.len() - 1));
            }
        }
        if !check.plain_select {
            plan.evidence.push(evidence);
            continue;
        }
        for (index, argument) in check.arguments.iter().enumerate() {
            match &argument.expected {
                ArgumentExpectation::Relation { .. } => {
                    if let Some(probe) = relation_probe(sql, &parts, &check.scope, &argument.text) {
                        plan.probes.push(probe);
                        evidence[index] = Some(Evidence::Columns(plan.probes.len() - 1));
                    }
                }
                ArgumentExpectation::Capture { .. } => {}
                ArgumentExpectation::Typed { .. } | ArgumentExpectation::Family { .. } => {
                    let group: usize = match typed_probe_index(&scopes, &check.scope) {
                        Some(group) => group,
                        None => {
                            scopes.push(TypedProbe {
                                scope: &check.scope,
                                columns: Vec::new(),
                                arguments: Vec::new(),
                            });
                            scopes.len() - 1
                        }
                    };
                    let probe: &mut TypedProbe<'_> = &mut scopes[group];
                    probe.columns.push(format!(
                        "({}) AS {PROBE_COLUMN_PREFIX}{}",
                        argument.text,
                        probe.columns.len()
                    ));
                    probe.arguments.push((position, index));
                }
            }
        }
        plan.evidence.push(evidence);
    }
    for probe in scopes {
        let Some((prefix, tail)) = scope_frame(sql, &parts, probe.scope) else {
            continue;
        };
        plan.probes.push(format!(
            "{prefix}SELECT {} {tail}",
            probe.columns.join(", ")
        ));
        let index: usize = plan.probes.len() - 1;
        for (column, (check, argument)) in probe.arguments.into_iter().enumerate() {
            plan.evidence[check][argument] = Some(Evidence::Column(
                index,
                format!("{PROBE_COLUMN_PREFIX}{column}"),
            ));
        }
    }
    plan
}

/// The index of `scope`'s typed probe in `scopes`, if one was started.
fn typed_probe_index(scopes: &[TypedProbe<'_>], scope: &CallScope) -> Option<usize> {
    scopes.iter().position(|probe| probe.scope == scope)
}

/// The column type of `name` in `columns`, matching names without case or quotes.
fn column_type<'c>(
    columns: &'c [(String, Option<String>)],
    name: &str,
) -> Option<&'c Option<String>> {
    let bare: String = unquoted(name);
    columns
        .iter()
        .find(|(column, _)| unquoted(column).eq_ignore_ascii_case(&bare))
        .map(|(_, data_type)| data_type)
}

/// Whether a value of type `actual` fits where `expected` is declared, unknown types fitting.
fn compatible(actual: Option<&str>, expected: &str, dialect: &str) -> bool {
    let Some(actual) = actual else {
        return true;
    };
    fits(type_family(actual, dialect), type_family(expected, dialect))
}

/// The findings of one check, given each probe's columns where its analysis knew them.
pub(crate) fn check_findings(
    (check, evidence): (&SqlMacroCheck, &[Option<Evidence>]),
    results: &[ProbeColumns],
    dialect: &str,
) -> Vec<SqlMacroFinding> {
    let Some((call, call_end)) = &check.call else {
        return Vec::new();
    };
    let finding =
        |argument: &ArgumentCheck, message: String, help: Option<String>| SqlMacroFinding {
            message,
            help,
            call: call.clone(),
            call_end: *call_end,
            declared_at: argument.declared_at.clone(),
        };
    let columns_of = |index: usize| -> Option<&Vec<(String, Option<String>)>> {
        match evidence.get(index)?.as_ref()? {
            Evidence::Columns(probe) => results.get(*probe)?.as_ref(),
            Evidence::Column(..) => None,
        }
    };
    let name: &str = &check.macro_name;
    let mut findings: Vec<SqlMacroFinding> = Vec::new();
    for (index, argument) in check.arguments.iter().enumerate() {
        let parameter: &str = &argument.parameter;
        match &argument.expected {
            ArgumentExpectation::Capture {
                slot,
                columns,
                source,
            } => {
                let (message, help) = capture_finding(
                    (parameter, slot),
                    columns,
                    source.as_deref(),
                    columns_of(index),
                );
                if let Some(message) = message {
                    findings.push(finding(argument, message, Some(help)));
                }
            }
            ArgumentExpectation::Relation { columns, slots } => {
                let Some(found) = columns_of(index) else {
                    continue;
                };
                if let Some(slot_finding) = slot_finding(argument, (columns, slots), found, dialect)
                {
                    findings.push(finding(argument, slot_finding.0, Some(slot_finding.1)));
                    continue;
                }
                let listed: Vec<String> = columns
                    .iter()
                    .map(|(column, data_type)| format!("{column} {data_type}"))
                    .collect();
                for (column, data_type) in columns {
                    if slots
                        .iter()
                        .any(|mapping| mapping.slot.eq_ignore_ascii_case(column))
                    {
                        continue;
                    }
                    let reason: Option<String> = match column_type(found, column) {
                        None => Some(format!("{} has no column {column}", argument.text)),
                        Some(actual) if !compatible(actual.as_deref(), data_type, dialect) => {
                            Some(format!(
                                "{}.{column} is {}",
                                argument.text,
                                shown_type(actual.as_deref().unwrap_or_default())
                            ))
                        }
                        Some(_) => None,
                    };
                    if let Some(reason) = reason {
                        findings.push(finding(
                            argument,
                            format!("argument {parameter} of macro {name}() needs column {column} {data_type}; {reason}"),
                            Some(format!("pass a relation with these columns: {}", listed.join(", "))),
                        ));
                        break;
                    }
                }
            }
            ArgumentExpectation::Typed { .. } | ArgumentExpectation::Family { .. } => {
                let Some(Some(Evidence::Column(probe, column))) = evidence.get(index) else {
                    continue;
                };
                let Some(Some(found)) = results.get(*probe) else {
                    continue;
                };
                let Some(Some(actual)) = column_type(found, column) else {
                    continue;
                };
                let data_type: &str = match &argument.expected {
                    ArgumentExpectation::Family { family } => {
                        if !family.admits(type_family(actual, dialect)) {
                            findings.push(finding(
                                argument,
                                format!(
                                    "argument {parameter} of macro {name}() expects {}; {} is {}",
                                    family.noun(),
                                    argument.text,
                                    shown_type(actual)
                                ),
                                Some(format!(
                                    "pass {} or declare {parameter} with a wider type",
                                    family.noun()
                                )),
                            ));
                        }
                        continue;
                    }
                    ArgumentExpectation::Typed { data_type } => data_type,
                    _ => continue,
                };
                if !compatible(Some(actual), data_type, dialect) {
                    findings.push(finding(
                        argument,
                        format!(
                            "argument {parameter} of macro {name}() expects {data_type}; {} is {}",
                            argument.text,
                            shown_type(actual)
                        ),
                        None,
                    ));
                }
            }
        }
    }
    findings
}

/// Why a plain group read in a subquery over `source` may bind elsewhere, if it may, and the fix.
fn capture_finding(
    (relation, slot): (&str, &str),
    columns: &[String],
    source: Option<&str>,
    found: Option<&Vec<(String, Option<String>)>>,
) -> (Option<String>, String) {
    let help: String = format!(
        "qualify the read with the alias of the relation it reads, as alias.@{relation}.{slot}, so its columns cannot bind to the subquery's relations"
    );
    let Some(source) = source else {
        return (
            Some(format!(
                "slot {relation}.{slot} is read plainly in a subquery over an unnamed relation, whose columns cannot be checked; write alias.@{relation}.{slot}"
            )),
            help,
        );
    };
    let Some(found) = found else {
        return (
            Some(format!(
                "slot {relation}.{slot} is read plainly in a subquery over {source}, whose columns are not known here; write alias.@{relation}.{slot}"
            )),
            help,
        );
    };
    let captured: Option<&String> = columns
        .iter()
        .find(|column| column_type(found, column).is_some());
    let message: Option<String> = captured.map(|column| {
        format!("slot {relation}.{slot} column {column} would bind to {source} in this subquery; write alias.@{relation}.{slot}")
    });
    (message, help)
}

/// The first mapped slot whose column the passed relation lacks or holds with another type.
fn slot_finding(
    argument: &ArgumentCheck,
    (columns, slots): (&[(String, String)], &[SlotMapping]),
    found: &[(String, Option<String>)],
    dialect: &str,
) -> Option<(String, String)> {
    let relation: &str = &argument.parameter;
    let passed: &str = &argument.text;
    for mapping in slots {
        let slot: &str = &mapping.slot;
        let help: String = format!("map {relation}.{slot} to columns of {passed}");
        if mapping.group {
            if let Some(missing) = mapping
                .columns
                .iter()
                .find(|column| column_type(found, column).is_none())
            {
                return Some((
                    format!(
                        "slot {relation}.{slot} groups column {missing}; {passed} has no column {missing}"
                    ),
                    help,
                ));
            }
            continue;
        }
        let Some((_, data_type)) = columns
            .iter()
            .find(|(column, _)| column.eq_ignore_ascii_case(slot))
        else {
            continue;
        };
        let column: &str = &mapping.columns[0];
        match column_type(found, column) {
            None => {
                return Some((
                    format!(
                        "slot {relation}.{slot} needs a {data_type} column; {passed} has no column {column}"
                    ),
                    help,
                ));
            }
            Some(actual) if !compatible(actual.as_deref(), data_type, dialect) => {
                return Some((
                    format!(
                        "slot {relation}.{slot} needs a {data_type} column; {passed}.{column} is {}",
                        shown_type(actual.as_deref().unwrap_or_default())
                    ),
                    help,
                ));
            }
            Some(_) => {}
        }
    }
    None
}

/// An inferred type as messages show it, in the SQL type names schema declarations use.
fn shown_type(inferred: &str) -> &str {
    if inferred.eq_ignore_ascii_case(INFERRED_TEXT_TYPE) {
        VARCHAR_TYPE
    } else {
        inferred
    }
}

/// Each model's probes' output columns, inferred against that model's bound relations.
pub(crate) fn analysed_probes(
    dialect: &str,
    models: &[(&[String], &RelationShapes)],
) -> Vec<Vec<ProbeColumns>> {
    let mut analysed: Vec<Vec<ProbeColumns>> = Vec::with_capacity(models.len());
    for (probes, schema) in models {
        analysed.push(model_probes(dialect, probes, schema));
    }
    analysed
}

/// One model's probes' output columns, inferred against its bound relations by the native route.
fn model_probes(dialect: &str, probes: &[String], schema: &RelationShapes) -> Vec<ProbeColumns> {
    let AnalysisRoute::Native(parser) = analysis_route(Some(dialect)) else {
        return vec![None; probes.len()];
    };
    let schema: ProbeSchema = ProbeSchema::new(schema);
    let mut columns: Vec<ProbeColumns> = Vec::with_capacity(probes.len());
    for sql in probes {
        columns.push(probe_outputs(parser, sql, &schema).and_then(probe_columns));
    }
    columns
}

/// The relations a probe reads, by name; each also answers to the trailing parts of its name.
#[derive(Debug)]
pub(crate) struct ProbeSchema {
    relations: Vec<(String, Vec<SchemaColumn>)>,
}

impl ProbeSchema {
    /// Each relation with its `(column, type)` list; an unknown type leaves the column untyped.
    pub(crate) fn new(relations: &RelationShapes) -> Self {
        Self {
            relations: relations
                .iter()
                .map(|(name, columns)| (unquoted_parts(name), schema_columns(columns)))
                .collect(),
        }
    }
}

impl LineageSchema for ProbeSchema {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        let relation: String = relation.to_ascii_lowercase();
        let suffix: String = format!(".{relation}");
        self.relations
            .iter()
            .find(|(name, _)| *name == relation || name.ends_with(&suffix))
            .map(|(_, columns)| columns.as_slice())
    }
}

/// A relation name with each dotted part unquoted, lower-cased as the schema compares it.
fn unquoted_parts(name: &str) -> String {
    let parts: Vec<String> = name.split('.').map(unquoted).collect();
    parts.join(".").to_ascii_lowercase()
}

/// Columns of the given spellings, of unknown nullability.
fn schema_columns(columns: &[(String, String)]) -> Vec<SchemaColumn> {
    columns
        .iter()
        .map(|(name, data_type)| SchemaColumn {
            name: name.clone(),
            quoted: false,
            data_type: known_type(data_type).then(|| data_type.clone()),
            nullability: Nullability::Unknown,
        })
        .collect()
}

/// The native analysis of probe `sql` over `schema`; `None` where the parser cannot read it whole.
fn probe_analysis(
    parser: &dyn NativeDialect,
    sql: &str,
    schema: &ProbeSchema,
) -> Option<NativeAnalysis> {
    let input = AnalysisInput {
        sql,
        schema,
        references: &[],
        extension_functions: &[],
        quoted_ignore_case: false,
    };
    let Ok(analysis) = parser.analyse(&input) else {
        return None;
    };
    Some(analysis)
}

/// The output columns of `sql` over `schema` with their types, unless a star stayed unexpanded.
pub(crate) fn probe_outputs(
    parser: &dyn NativeDialect,
    sql: &str,
    schema: &ProbeSchema,
) -> Option<Vec<(String, Option<String>)>> {
    let analysis: NativeAnalysis = probe_analysis(parser, sql, schema)?;
    if analysis.has_star && !analysis.star_resolved {
        return None;
    }
    Some(
        analysis
            .columns
            .into_iter()
            .map(|column| (column.name, column.data_type))
            .collect(),
    )
}

/// The `(code, message)` findings, in SQLBuild codes, of the native analysis of `sql`.
pub(crate) fn probe_type_errors(
    parser: &dyn NativeDialect,
    sql: &str,
    schema: &ProbeSchema,
) -> Vec<(String, String)> {
    probe_analysis(parser, sql, schema).map_or_else(Vec::new, |analysis| {
        analysis
            .diagnostics
            .into_iter()
            .map(|finding| (finding.code, finding.message))
            .collect()
    })
}

/// A probe's typed output columns, unless analysis left a star unexpanded.
fn probe_columns(outputs: Vec<(String, Option<String>)>) -> ProbeColumns {
    if outputs.is_empty() || outputs.iter().any(|(name, _)| name == STAR_PROJECTION) {
        return None;
    }
    let mut columns: Vec<(String, Option<String>)> = Vec::with_capacity(outputs.len());
    for (name, data_type) in outputs {
        columns.push((name, data_type.filter(|hint| known_type(hint))));
    }
    Some(columns)
}

/// Whether inference reported a type rather than its unknown placeholder.
fn known_type(hint: &str) -> bool {
    !hint.eq_ignore_ascii_case(UNKNOWN_TYPE_HINT)
}
