//! Deterministic comparison SQL rendering for planned SQL-native tests.

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::analysis_route::types::NativeDialect;
use std::collections::{HashMap, HashSet};

use crate::compiler::_helpers::sql_tests::cte_rename::defined_cte_keys;
use crate::compiler::_helpers::sql_tests::cte_rename::{CteRename, rename_ctes};
use crate::compiler::_helpers::sql_tests::cte_slices::{
    SliceDialect, WithSlices, identifier_keys, split_top_level_with, strip_statement_terminators,
    used_ctes,
};
use crate::compiler::_helpers::sql_tests::cte_sql::{cte_definition_sql, leading_with_prefix_end};
use crate::compiler::_helpers::sql_tests::helper_names::HELPER_PREFIX;
use crate::compiler::_helpers::sql_tests::planning::REF_PREFIX;
use crate::compiler::models::{SqlTestAssertionStep, SqlTestChainStep};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

const DEFAULT_WORKERS: usize = 4;
const STEP_BODY_CTE: &str = "__sqb_step_body";
const MAX_WORKERS: usize = 4;
const WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBatchRequest {
    requests: Vec<RenderRequest>,
    #[serde(default = "default_workers")]
    workers: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DifferenceSampleRequest {
    step: SqlTestChainStep,
    #[serde(default = "default_true")]
    sql_analysis_enabled: bool,
    #[serde(default = "default_set_difference")]
    set_difference_operator: String,
    #[serde(default)]
    sql_analysis_dialect: Option<String>,
    direction: DifferenceDirection,
    sample_limit: usize,
    #[serde(default)]
    use_top_clause: bool,
    #[serde(default)]
    actual_relation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DifferenceDirection {
    Unexpected,
    Missing,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RenderRequest {
    #[serde(default)]
    pub(crate) chain: Vec<SqlTestChainStep>,
    #[serde(default)]
    pub(crate) assertions: Vec<SqlTestAssertionStep>,
    #[serde(default = "default_true")]
    pub(crate) sql_analysis_enabled: bool,
    #[serde(default = "default_set_difference")]
    pub(crate) set_difference_operator: String,
    #[serde(default)]
    pub(crate) sql_analysis_dialect: Option<String>,
    #[serde(default)]
    pub(crate) probe_step_index: Option<usize>,
    #[serde(default)]
    pub(crate) actual_relations: HashMap<usize, String>,
    #[serde(default)]
    pub(crate) output_step_index: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderResponse {
    sql: String,
}

/// One CTE placed in the shared top-level WITH, keeping its authored header and body text.
struct LiftedCte {
    key: String,
    header: String,
    body: String,
    authored: String,
    origin: String,
}

impl LiftedCte {
    fn conflicts(&self, key: &str, header: &str, authored: &str) -> bool {
        self.key == key
            && (!same_header(&self.header, header) || self.authored.trim() != authored.trim())
    }
}

/// Headers name the same CTE when equal, or equal up to case when nothing in them is quoted.
fn same_header(left: &str, right: &str) -> bool {
    left == right || (left.eq_ignore_ascii_case(right) && !left.contains(['"', '`', '[', '\'']))
}

struct RenderCteState {
    dialect: SliceDialect,
    nested_test_sql: bool,
    lifted: Vec<LiftedCte>,
    name_counts: CteSuffixCounter,
    renamed_ctes: usize,
}

impl RenderCteState {
    fn new(dialect: SliceDialect, model_test: bool) -> Self {
        Self {
            dialect,
            nested_test_sql: model_test && !dialect.rejects_nested_with(),
            lifted: Vec::new(),
            name_counts: CteSuffixCounter::default(),
            renamed_ctes: 0,
        }
    }

    /// Lift a query's CTEs verbatim and return its body, nesting it unchanged on a name clash.
    fn lift(&mut self, sql: &str, enabled: bool, origin: &str) -> Result<String, String> {
        let sql = strip_statement_terminators(sql, self.dialect);
        if !enabled || self.nested_test_sql || leading_with_prefix_end(sql).is_none() {
            return Ok(sql.to_string());
        }
        let split = match split_top_level_with(sql, self.dialect) {
            Ok(Some(split)) => split,
            Ok(None) | Err(_) => return self.nested(sql, enabled, origin, &[]),
        };
        let colliding = self.colliding_indices(&split, &vec![true; split.ctes.len()]);
        if !colliding.is_empty() {
            if let Some(renamed) = self.rename_colliding(sql, &split, &colliding) {
                return self.lift(&renamed, enabled, origin);
            }
            let message = self.collision_description(origin, &split, &colliding);
            return self.nested(sql, enabled, origin, &[message]);
        }
        if has_duplicate_keys(split.ctes.iter().map(|cte| cte.key.as_str())) {
            return self.nested(sql, enabled, origin, &[]);
        }
        for cte in &split.ctes {
            self.push(cte.key.clone(), cte.header, cte.body, origin);
        }
        Ok(split.body.to_string())
    }

    /// Return SQL that cannot be lifted, refusing it where the dialect rejects a nested WITH.
    fn nested(
        &self,
        sql: &str,
        enabled: bool,
        origin: &str,
        collisions: &[String],
    ) -> Result<String, String> {
        let sql = strip_statement_terminators(sql, self.dialect);
        if !(enabled
            && self.dialect.rejects_nested_with()
            && leading_with_prefix_end(sql).is_some())
        {
            return Ok(sql.to_string());
        }
        if collisions.is_empty() {
            return Err(format!(
                "the WITH clause of {origin} cannot be lifted into the test query, and T-SQL \
                 does not allow a nested WITH; use plain `name AS (...)` CTEs without RECURSIVE \
                 or MATERIALIZED"
            ));
        }
        Err(format!(
            "{}; T-SQL does not allow a nested WITH, so rename the CTE in the model or the \
             fixture so the names are unique",
            collisions.join("; ")
        ))
    }

    /// Define a step's inputs and its model once at top level, the model as a nested `__ref__` CTE.
    fn actual_step_sql(
        &mut self,
        step: &SqlTestChainStep,
        suffix: &str,
        enabled: bool,
    ) -> Result<String, String> {
        let origin = format!("model '{}'", step.model_name);
        let Some(body) = step.comparison_body_sql.as_deref() else {
            return self.lift(&step.resolved_sql, enabled, &origin);
        };
        let body = strip_statement_terminators(body, self.dialect);
        let model_cte = model_cte_name(&step.model_name, suffix);
        let mut ctes = step.lifted_ctes.clone();
        ctes.push((model_cte.clone(), body.to_string()));
        self.merge_inputs(
            &ctes,
            &format!("SELECT * FROM {model_cte}"),
            enabled,
            &origin,
        )
    }

    /// Place an expected step's helper CTEs at top level and return its comparison body.
    fn expected_step_sql(
        &mut self,
        step: &SqlTestChainStep,
        expected_sql: &str,
        enabled: bool,
    ) -> Result<String, String> {
        let origin = format!("the expected rows of model '{}'", step.model_name);
        let body = self.merge_inputs(&step.expected_lifted_ctes, expected_sql, enabled, &origin)?;
        self.lift(&body, enabled, &origin)
    }

    /// Place an assertion's helper CTEs at top level and return its comparison body.
    fn assertion_sql(
        &mut self,
        assertion: &SqlTestAssertionStep,
        enabled: bool,
    ) -> Result<String, String> {
        let origin = format!("assertion '{}'", assertion.name);
        let body = assertion
            .comparison_body_sql
            .as_deref()
            .unwrap_or(&assertion.resolved_sql);
        let body = self.merge_inputs(&assertion.lifted_ctes, body, enabled, &origin)?;
        self.lift(&body, enabled, &origin)
    }

    /// Merge inputs at top level, renaming any whose name is taken by other text, and return the body.
    fn merge_inputs(
        &mut self,
        inputs: &[(String, String)],
        body: &str,
        enabled: bool,
        origin: &str,
    ) -> Result<String, String> {
        let Some(collisions) = self.merge(inputs, enabled, origin)? else {
            return Ok(body.to_string());
        };
        let refused = || {
            format!(
                "{collisions}; rename the CTE in the model or the fixture so the names are unique"
            )
        };
        let (renamed_inputs, renamed_body) =
            self.renamed_inputs(inputs, body).ok_or_else(refused)?;
        if self.merge(&renamed_inputs, enabled, origin)?.is_some() {
            return Err(refused());
        }
        Ok(renamed_body)
    }

    /// Inputs whose names collide with lifted CTEs renamed by token span, with the body's references.
    fn renamed_inputs(
        &mut self,
        inputs: &[(String, String)],
        body: &str,
    ) -> Option<(Vec<(String, String)>, String)> {
        let definitions: Vec<String> = inputs
            .iter()
            .map(|(name, sql)| cte_definition_sql(name, sql))
            .chain(std::iter::once(cte_definition_sql(STEP_BODY_CTE, body)))
            .collect();
        let statement = format!("WITH {}\nSELECT 1", definitions.join(",\n"));
        let Ok(Some(split)) = split_top_level_with(&statement, self.dialect) else {
            return None;
        };
        let colliding = self.colliding_indices(&split, &vec![true; split.ctes.len()]);
        let taken = identifier_keys(&statement, self.dialect);
        let names: Vec<String> = colliding.iter().map(|_| self.fresh_name(&taken)).collect();
        let renames: Vec<CteRename<'_>> = colliding
            .iter()
            .zip(&names)
            .map(|(index, name)| CteRename {
                index: *index,
                name,
            })
            .collect();
        let renamed = rename_ctes(&statement, &split, &renames, self.dialect)?;
        let Ok(Some(split)) = split_top_level_with(&renamed, self.dialect) else {
            return None;
        };
        let (body_cte, input_ctes) = split.ctes.split_last()?;
        Some((
            input_ctes
                .iter()
                .map(|cte| (cte.header.to_string(), cte.body.to_string()))
                .collect(),
            body_cte.body.to_string(),
        ))
    }

    /// Add generated CTEs verbatim, describing any name already lifted with different text.
    fn merge(
        &mut self,
        ctes: &[(String, String)],
        enabled: bool,
        step_origin: &str,
    ) -> Result<Option<String>, String> {
        let keys: Vec<String> = ctes.iter().map(|(name, _)| cte_key(name)).collect();
        let mut collisions: Vec<(&str, &LiftedCte)> = Vec::new();
        for ((name, sql), key) in ctes.iter().zip(&keys) {
            if let Some(existing) = self.conflict(key, name, sql) {
                collisions.push((name.as_str(), existing));
            }
        }
        if !collisions.is_empty() {
            return Ok(Some(collision_message(step_origin, &collisions)));
        }
        for ((name, sql), key) in ctes.iter().zip(keys) {
            if self.lifted.iter().any(|existing| existing.key == key) {
                continue;
            }
            let origin = generated_cte_origin(name);
            let body = if enabled && self.dialect.rejects_nested_with() {
                self.flatten(sql, &origin)?
            } else {
                sql.clone()
            };
            if let Some(existing) = self.conflict(&key, name, sql) {
                let message = collision_message(&origin, &[(name.as_str(), existing)]);
                self.nested(sql, enabled, &origin, std::slice::from_ref(&message))?;
                return Ok(Some(message));
            }
            self.lifted.push(LiftedCte {
                key,
                header: name.clone(),
                body,
                authored: sql.clone(),
                origin,
            });
        }
        Ok(None)
    }

    /// Lift the CTEs a generated body reads ahead of it; unread CTEs are dead and left out.
    fn flatten(&mut self, sql: &str, origin: &str) -> Result<String, String> {
        if leading_with_prefix_end(sql).is_none() {
            return Ok(sql.to_string());
        }
        let split = match split_top_level_with(sql, self.dialect) {
            Ok(Some(split)) => split,
            Ok(None) | Err(_) => return self.nested(sql, true, origin, &[]),
        };
        let used = used_ctes(&split, self.dialect);
        let colliding = self.colliding_indices(&split, &used);
        if !colliding.is_empty() {
            if let Some(renamed) = self.rename_colliding(sql, &split, &colliding) {
                return self.flatten(&renamed, origin);
            }
            let message = self.collision_description(origin, &split, &colliding);
            return self.nested(sql, true, origin, &[message]);
        }
        for (cte, _) in split.ctes.iter().zip(used).filter(|(_, used)| *used) {
            self.push(cte.key.clone(), cte.header, cte.body, origin);
        }
        Ok(split.body.to_string())
    }

    /// Indices of the selected CTEs whose names are already lifted with different text.
    fn colliding_indices(&self, split: &WithSlices<'_>, selected: &[bool]) -> Vec<usize> {
        let mut indices: Vec<usize> = Vec::new();
        for (index, (cte, selected)) in split.ctes.iter().zip(selected).enumerate() {
            if *selected && self.conflict(&cte.key, cte.header, cte.body).is_some() {
                indices.push(index);
            }
        }
        indices
    }

    fn collision_description(
        &self,
        origin: &str,
        split: &WithSlices<'_>,
        indices: &[usize],
    ) -> String {
        let mut collisions: Vec<(&str, &LiftedCte)> = Vec::new();
        for cte in indices.iter().filter_map(|index| split.ctes.get(*index)) {
            if let Some(existing) = self.conflict(&cte.key, cte.header, cte.body) {
                collisions.push((cte.header, existing));
            }
        }
        collision_message(origin, &collisions)
    }

    /// Where a nested WITH is rejected, rename colliding CTEs by token span to fresh names.
    fn rename_colliding(
        &mut self,
        sql: &str,
        split: &WithSlices<'_>,
        indices: &[usize],
    ) -> Option<String> {
        if !self.dialect.rejects_nested_with() {
            return None;
        }
        let taken = identifier_keys(sql, self.dialect);
        let names: Vec<String> = indices.iter().map(|_| self.fresh_name(&taken)).collect();
        let renames: Vec<CteRename<'_>> = indices
            .iter()
            .zip(&names)
            .map(|(index, name)| CteRename {
                index: *index,
                name,
            })
            .collect();
        rename_ctes(sql, split, &renames, self.dialect)
    }

    fn fresh_name(&mut self, taken: &HashSet<String>) -> String {
        loop {
            let name = format!("__sqb_cte_{}", self.renamed_ctes);
            self.renamed_ctes += 1;
            if !taken.contains(&name) && !self.lifted.iter().any(|cte| cte.key == name) {
                return name;
            }
        }
    }

    fn conflict(&self, key: &str, header: &str, body: &str) -> Option<&LiftedCte> {
        self.lifted
            .iter()
            .find(|existing| existing.conflicts(key, header, body))
    }

    fn push(&mut self, key: String, header: &str, body: &str, origin: &str) {
        if self.lifted.iter().any(|existing| existing.key == key) {
            return;
        }
        self.lifted.push(LiftedCte {
            key,
            header: header.to_string(),
            body: body.to_string(),
            authored: body.to_string(),
            origin: origin.to_string(),
        });
    }

    fn unique_suffix(&mut self, model_name: &str) -> String {
        self.name_counts.next(model_name)
    }

    /// Refuse a query where a nested CTE reuses a top-level name, which dialects resolve differently.
    fn verify_isolated_names(&self, comparison_ctes: &[(String, String)]) -> Result<(), String> {
        let top_level: Vec<(&str, &str, String)> = self
            .lifted
            .iter()
            .map(|cte| (cte.header.as_str(), cte.body.as_str(), cte.origin.clone()))
            .chain(
                comparison_ctes
                    .iter()
                    .map(|(name, sql)| (name.as_str(), sql.as_str(), format!("CTE '{name}'"))),
            )
            .collect();
        let keys: HashSet<String> = top_level.iter().map(|(name, _, _)| cte_key(name)).collect();
        for (_, body, origin) in &top_level {
            let nested = defined_cte_keys(body, self.dialect);
            if let Some(name) = nested.iter().find(|name| keys.contains(*name)) {
                return Err(format!(
                    "{origin} defines CTE '{name}', which is also a CTE SQLBuild generates at the \
                     top level of the test query; rename the CTE"
                ));
            }
        }
        Ok(())
    }

    /// Read already materialised model outputs instead of recomputing their CTEs.
    fn substitute(&mut self, substitutions: &[(String, String)]) {
        for (key, body) in substitutions {
            if let Some(cte) = self.lifted.iter_mut().find(|cte| cte.key == *key) {
                cte.body.clone_from(body);
            }
        }
    }

    fn definitions(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lifted)
            .iter()
            .map(|cte| cte_definition_sql(&cte.header, &cte.body))
            .collect()
    }
}

fn collision_message(origin: &str, collisions: &[(&str, &LiftedCte)]) -> String {
    collisions
        .iter()
        .map(|(header, existing)| {
            format!(
                "CTE '{}' of {origin} collides with CTE '{}' of {}",
                display_cte_name(header),
                display_cte_name(&existing.header),
                existing.origin
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// A CTE name as its author wrote it, without SQLBuild's reserved helper prefix.
fn display_cte_name(name: &str) -> &str {
    name.strip_prefix(HELPER_PREFIX).unwrap_or(name)
}

/// The `__ref__` CTE of a step's model, under the name downstream steps reference it by.
fn model_cte_name(model_name: &str, suffix: &str) -> String {
    let mut characters = model_name.chars();
    let plain = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_');
    format!("{REF_PREFIX}{}", if plain { model_name } else { suffix })
}

/// Describe a generated CTE by what it stands in for: an upstream model, source or seed.
fn generated_cte_origin(name: &str) -> String {
    for (prefix, kind) in [
        (HELPER_PREFIX, "helper"),
        ("__ref__", "model"),
        ("__source__", "source"),
        ("__seed__", "seed"),
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return format!("{kind} '{rest}'");
        }
    }
    format!("CTE '{name}'")
}

fn has_duplicate_keys<'k>(keys: impl Iterator<Item = &'k str>) -> bool {
    let mut seen: HashSet<&str> = HashSet::new();
    keys.into_iter().any(|key| !seen.insert(key))
}

/// Case-insensitive CTE identity with identifier quotes removed.
fn cte_key(name: &str) -> String {
    let trimmed = name.trim();
    [('"', '"'), ('`', '`'), ('[', ']')]
        .iter()
        .find_map(|(open, close)| trimmed.strip_prefix(*open)?.strip_suffix(*close))
        .unwrap_or(trimmed)
        .to_lowercase()
}

/// Readable, collision-free comparison CTE suffixes in step order.
#[derive(Default)]
struct CteSuffixCounter {
    counts: HashMap<String, usize>,
}

impl CteSuffixCounter {
    fn next(&mut self, model_name: &str) -> String {
        let base = sanitize_cte_suffix(model_name);
        let count = self.counts.entry(base.clone()).or_default();
        *count += 1;
        if *count == 1 {
            base
        } else {
            format!("{base}_{count}")
        }
    }
}

/// Mark the chain steps whose actual SQL the comparison renderer emits.
pub(crate) fn rendered_chain_steps(
    chain: &[SqlTestChainStep],
    assertions: &[SqlTestAssertionStep],
) -> Vec<bool> {
    let mut suffixes = CteSuffixCounter::default();
    chain
        .iter()
        .map(|step| {
            let actual_cte = format!("__actual__{}", suffixes.next(&step.model_name));
            step.expected_cte_sql.is_some() || assertions_use_actual(assertions, &actual_cte)
        })
        .collect()
}

pub(crate) fn render_json(request_json: &str) -> Result<String, String> {
    let request: RenderBatchRequest =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    let workers = request.workers.clamp(1, MAX_WORKERS);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers.min(request.requests.len().max(1)))
        .stack_size(WORKER_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())?;
    let responses: Vec<RenderResponse> = pool.install(|| {
        request
            .requests
            .into_par_iter()
            .map(render_response)
            .collect::<Result<_, _>>()
    })?;
    serde_json::to_string(&responses).map_err(|error| error.to_string())
}

fn render_response(request: RenderRequest) -> Result<RenderResponse, String> {
    Ok(RenderResponse {
        sql: render_comparison_sql(&request)?,
    })
}

/// The native parser that reads SQL tests in analysis dialect `name`; `None` leaves them unavailable.
pub(crate) fn sql_test_parser(name: Option<&str>) -> Option<&'static dyn NativeDialect> {
    match analysis_route(name) {
        AnalysisRoute::Native(parser) => Some(parser),
        _ => None,
    }
}

pub(crate) fn render_difference_sample_json(request_json: &str) -> Result<String, String> {
    let request: DifferenceSampleRequest =
        serde_json::from_str(request_json).map_err(|error| error.to_string())?;
    serde_json::to_string(&RenderResponse {
        sql: render_difference_sample_sql(&request)?,
    })
    .map_err(|error| error.to_string())
}

fn render_difference_sample_sql(request: &DifferenceSampleRequest) -> Result<String, String> {
    let step = &request.step;
    let Some(expected_input) = step.expected_cte_sql.as_deref() else {
        return Ok(String::new());
    };
    let enabled = request.sql_analysis_enabled;
    let mut cte_state = RenderCteState::new(
        SliceDialect::new(request.sql_analysis_dialect.as_deref()),
        step.comparison_body_sql.is_some(),
    );
    let suffix = cte_state.unique_suffix(&step.model_name);
    let actual_sql = match request.actual_relation.as_deref() {
        Some(relation) => relation_select(relation),
        None => cte_state.actual_step_sql(step, &suffix, enabled)?,
    };
    let expected_sql = cte_state.expected_step_sql(step, expected_input, enabled)?;
    let mut cte_parts = cte_state.definitions();
    cte_parts.push(cte_definition_sql("__actual", &actual_sql));
    cte_parts.push(cte_definition_sql("__expected", &expected_sql));
    let (left, right) = match request.direction {
        DifferenceDirection::Unexpected => ("__actual", "__expected"),
        DifferenceDirection::Missing => ("__expected", "__actual"),
    };
    let (bounded_select, limit_clause) = if request.use_top_clause {
        (
            format!("SELECT TOP {} *", request.sample_limit),
            String::new(),
        )
    } else {
        (
            "SELECT *".to_string(),
            format!(" LIMIT {}", request.sample_limit),
        )
    };
    let projection = compared_projection(step);
    let statement = format!(
        "WITH {}\n{bounded_select} FROM (SELECT {projection} FROM {left} {} SELECT {projection} FROM {right}) AS __sqlbuild_difference{limit_clause}",
        cte_parts.join(",\n"),
        request.set_difference_operator,
    );
    if request.actual_relation.is_none() {
        return Ok(statement);
    }
    Ok(pruned_statement(&statement, cte_state.dialect))
}

/// A query reading every row of an already materialised actual output.
fn relation_select(relation: &str) -> String {
    format!("SELECT * FROM {relation}")
}

/// The statement without top-level CTEs nothing reads, or unchanged when it cannot be sliced.
fn pruned_statement(statement: &str, dialect: SliceDialect) -> String {
    let Ok(Some(split)) = split_top_level_with(statement, dialect) else {
        return statement.to_string();
    };
    let used = used_ctes(&split, dialect);
    if used.iter().all(|used| *used) {
        return statement.to_string();
    }
    let definitions: Vec<String> = split
        .ctes
        .iter()
        .zip(used)
        .filter(|(_, used)| *used)
        .map(|(cte, _)| cte_definition_sql(cte.header, cte.body))
        .collect();
    if definitions.is_empty() {
        return split.body.trim_start().to_string();
    }
    format!(
        "WITH {}\n{}",
        definitions.join(",\n"),
        split.body.trim_start()
    )
}

/// Columns compared for an expected-output step: the listed expected columns, else every column.
fn compared_projection(step: &SqlTestChainStep) -> String {
    compared_columns_projection(step.expected_columns.as_deref())
}

/// The projection comparing `columns` by name, or every column positionally without a list.
pub(crate) fn compared_columns_projection(columns: Option<&[String]>) -> String {
    columns.map_or_else(|| "*".to_string(), |columns| columns.join(", "))
}

/// Row counts of `actual` and `expected` and of the rows each has that the other lacks.
pub(crate) fn comparison_counts_sql(
    (actual, expected): (&str, &str),
    projection: &str,
    set_difference_operator: &str,
) -> String {
    format!(
        "(SELECT COUNT(*) FROM {actual}) AS actual_count, \
         (SELECT COUNT(*) FROM {expected}) AS expected_count, \
         (SELECT COUNT(*) FROM (SELECT {projection} FROM {actual} {set_difference_operator} SELECT {projection} FROM {expected}) AS __sqlbuild_mismatch) AS unexpected_count, \
         (SELECT COUNT(*) FROM (SELECT {projection} FROM {expected} {set_difference_operator} SELECT {projection} FROM {actual}) AS __sqlbuild_missing) AS missing_count"
    )
}

/// Render one planned test's comparison query, lifting authored CTE text verbatim.
pub(crate) fn render_comparison_sql(request: &RenderRequest) -> Result<String, String> {
    if request.chain.is_empty() && request.assertions.is_empty() {
        return Ok(String::new());
    }
    let enabled = request.sql_analysis_enabled;
    let model_test = !request.assertions.is_empty()
        || request
            .chain
            .iter()
            .any(|step| step.comparison_body_sql.is_some());
    let mut cte_state = RenderCteState::new(
        SliceDialect::new(request.sql_analysis_dialect.as_deref()),
        model_test,
    );
    let mut comparison_ctes: Vec<(String, String)> = Vec::new();
    let mut select_parts: Vec<String> = Vec::new();
    let rendered_steps = rendered_chain_steps(&request.chain, &request.assertions);
    let mut probe_actual_cte: Option<String> = None;
    let mut output_actual_cte: Option<String> = None;
    let mut substitutions: Vec<(String, String)> = Vec::new();

    for (step_index, step) in request.chain.iter().enumerate() {
        let suffix = cte_state.unique_suffix(&step.model_name);
        let actual_cte = format!("__actual__{suffix}");
        let expected_cte = format!("__expected__{suffix}");
        if !rendered_steps[step_index] {
            continue;
        }
        let actual_sql = match request.actual_relations.get(&step_index) {
            Some(relation) => {
                substitutions.push((
                    cte_key(&model_cte_name(&step.model_name, &suffix)),
                    relation_select(relation),
                ));
                relation_select(relation)
            }
            None => cte_state.actual_step_sql(step, &suffix, enabled)?,
        };
        comparison_ctes.push((actual_cte.clone(), actual_sql));
        if request.probe_step_index == Some(step_index) {
            probe_actual_cte = Some(actual_cte.clone());
        }
        if request.output_step_index == Some(step_index) {
            output_actual_cte = Some(actual_cte.clone());
            break;
        }
        let Some(expected_input) = step.expected_cte_sql.as_deref() else {
            continue;
        };
        let expected_sql = cte_state.expected_step_sql(step, expected_input, enabled)?;
        comparison_ctes.push((expected_cte.clone(), expected_sql));
        select_parts.push(format!(
            "SELECT {step_index} AS step_index, '{}' AS model_name, {}",
            escape_sql_string(&step.model_name),
            comparison_counts_sql(
                (&actual_cte, &expected_cte),
                &compared_projection(step),
                &request.set_difference_operator,
            ),
        ));
    }

    if request.output_step_index.is_some() {
        let Some(actual_cte) = output_actual_cte else {
            return Ok(String::new());
        };
        cte_state.verify_isolated_names(&comparison_ctes)?;
        cte_state.substitute(&substitutions);
        let mut cte_parts = cte_state.definitions();
        cte_parts.extend(
            comparison_ctes
                .iter()
                .map(|(name, sql)| cte_definition_sql(name, sql)),
        );
        let statement = format!("WITH {}\nSELECT * FROM {actual_cte}", cte_parts.join(",\n"));
        return Ok(pruned_statement(&statement, cte_state.dialect));
    }

    for (assertion_index, assertion) in request
        .assertions
        .iter()
        .enumerate()
        .map(|(index, assertion)| (index + request.chain.len(), assertion))
    {
        let suffix = cte_state.unique_suffix(&assertion.name);
        let assertion_cte = format!("__assert__{suffix}");
        let assertion_sql = cte_state.assertion_sql(assertion, enabled)?;
        comparison_ctes.push((assertion_cte.clone(), assertion_sql));
        select_parts.push(format!(
            "SELECT {assertion_index} AS step_index, 'assertion {}' AS model_name, \
             (SELECT COUNT(*) FROM {assertion_cte}) AS actual_count, \
             0 AS expected_count, \
             (SELECT COUNT(*) FROM {assertion_cte}) AS unexpected_count, \
             0 AS missing_count",
            escape_sql_string(&assertion.name),
        ));
    }

    if select_parts.is_empty() {
        return Ok(String::new());
    }
    cte_state.verify_isolated_names(&comparison_ctes)?;
    cte_state.substitute(&substitutions);
    let dialect = cte_state.dialect;
    let mut cte_parts = cte_state.definitions();
    cte_parts.extend(
        comparison_ctes
            .iter()
            .map(|(name, sql)| cte_definition_sql(name, sql)),
    );
    let single_read = !request.actual_relations.is_empty();
    if request.probe_step_index.is_some() {
        return Ok(probe_actual_cte.map_or_else(String::new, |actual_cte| {
            let statement = format!(
                "WITH {}\nSELECT * FROM {actual_cte} WHERE 1 = 0",
                cte_parts.join(",\n")
            );
            if single_read {
                pruned_statement(&statement, dialect)
            } else {
                statement
            }
        }));
    }
    let statement = format!(
        "WITH {}\n{}",
        cte_parts.join(",\n"),
        select_parts.join("\nUNION ALL\n")
    );
    if single_read {
        return Ok(pruned_statement(&statement, dialect));
    }
    Ok(statement)
}

fn sanitize_cte_suffix(model_name: &str) -> String {
    let mut suffix = String::with_capacity(model_name.len());
    let mut previous_separator = false;
    for character in model_name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            suffix.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator {
            suffix.push('_');
            previous_separator = true;
        }
    }
    let suffix = suffix.trim_matches('_');
    if suffix.is_empty() {
        "model".to_string()
    } else if suffix.as_bytes()[0].is_ascii_digit() {
        format!("model_{suffix}")
    } else {
        suffix.to_string()
    }
}

fn assertions_use_actual(assertions: &[SqlTestAssertionStep], actual_cte: &str) -> bool {
    let needle = actual_cte.to_ascii_lowercase();
    assertions.iter().any(|assertion| {
        assertion
            .resolved_sql
            .to_ascii_lowercase()
            .contains(&needle)
    })
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn default_workers() -> usize {
    DEFAULT_WORKERS
}

fn default_true() -> bool {
    true
}

fn default_set_difference() -> String {
    "EXCEPT".to_string()
}
