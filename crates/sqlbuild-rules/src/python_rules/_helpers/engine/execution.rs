//! Which file runs which rules, and each file's findings, from the cache when its answers still hold.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use fensu_facts::snapshot::models::RepositoryObservationIndex;
use fensu_native::rules::constants::DEAD_DEFINITION_CODE;
use fensu_native::{
    NativeDeadCodeContext, NativeExecutionRule, NativeExecutionTarget, NativeProjectPlane,
    NativeProjectQuery, NativeRuleContext, evaluate_core_rules, plan_core_rule_queries,
    plan_execution_owners,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::models::{Fault, RuleMetadata};
use crate::python_rules::_helpers::engine::contexts::{rule_context, threshold_calls};
use crate::python_rules::_helpers::engine::faults::python_fault;
use crate::python_rules::_helpers::engine::file_cache::{answers_digest, file_identity};
use crate::python_rules::_helpers::engine::observations::{
    observations, relative_queries, repository_index,
};
use crate::python_rules::constants::{PACKAGE_ROOT, PYTHON_RULES, ROOT_SCOPE};
use crate::python_rules::models::{
    CacheSplit, CallInputs, FilePlan, PythonCacheEntry, PythonEvaluation, PythonFiles,
    PythonRuleSpec, PythonTarget, RuleCall,
};

/// The selected rules that Fensu evaluates.
pub(crate) fn selected_rules(selected: &[&RuleMetadata]) -> Vec<&'static PythonRuleSpec> {
    let codes: HashSet<&str> = selected.iter().map(|rule| rule.code.as_str()).collect();
    PYTHON_RULES
        .iter()
        .filter(|rule| rule.fensu_code.is_some() && codes.contains(rule.code))
        .collect()
}

/// Each file's calls and cache identity, worked out before any file is parsed.
pub(crate) fn file_plans<'a>(
    targets: &'a [PythonTarget],
    rules: &[&PythonRuleSpec],
    inputs: &CallInputs<'_>,
) -> Result<Vec<FilePlan<'a>>, String> {
    Ok(targets
        .iter()
        .zip(file_rules(targets, rules)?)
        .filter(|(_, rules)| !rules.is_empty())
        .map(|(target, rules)| {
            let calls = threshold_calls(&rules, &inputs.request.config, target.is_main_module);
            FilePlan {
                target,
                identity: file_identity(target, &calls, inputs),
                calls,
            }
        })
        .collect())
}

/// Every planned file's findings: cached ones whose project answers still hold, the rest evaluated.
pub(crate) fn evaluated_files(
    plans: Vec<FilePlan<'_>>,
    inputs: &CallInputs<'_>,
    plane: &NativeProjectPlane,
) -> Result<PythonEvaluation, String> {
    let split: CacheSplit<'_> = cached_files(plans, inputs)?;
    let hits: usize = split.fresh.len();
    let misses: usize = split.stale.len();
    let mut entries: BTreeMap<String, PythonCacheEntry> = split.fresh.into_iter().collect();
    entries.extend(evaluate_files(split.stale, inputs, plane)?);
    Ok(PythonEvaluation {
        faults: entries
            .values()
            .flat_map(|entry| entry.faults.iter().cloned())
            .collect(),
        entries,
        hits,
        misses,
    })
}

/// Split `plans` into cached entries that still hold and files to evaluate again.
fn cached_files<'a>(
    plans: Vec<FilePlan<'a>>,
    inputs: &CallInputs<'_>,
) -> Result<CacheSplit<'a>, String> {
    let cached = |plan: &FilePlan<'_>| {
        inputs
            .cached
            .get(&plan.target.repository_path)
            .filter(|entry| entry.identity == plan.identity)
    };
    let queries: Vec<NativeProjectQuery> = plans
        .iter()
        .filter_map(cached)
        .flat_map(|entry| entry.queries.iter().map(project_query))
        .collect();
    let index: Option<RepositoryObservationIndex> = (!queries.is_empty())
        .then(|| {
            let borrowed: Vec<&NativeProjectQuery> = queries.iter().collect();
            repository_index(Path::new(&inputs.request.project_dir), &borrowed)
        })
        .transpose()?;
    let mut split: CacheSplit<'a> = CacheSplit::default();
    for plan in plans {
        match cached(&plan) {
            Some(entry) if still_holds(entry, inputs, index.as_ref())? => {
                split
                    .fresh
                    .push((plan.target.repository_path.clone(), entry.clone()));
            }
            _ => split.stale.push(plan),
        }
    }
    Ok(split)
}

/// Whether `entry`'s recorded project questions still have the answers it was computed from.
fn still_holds(
    entry: &PythonCacheEntry,
    inputs: &CallInputs<'_>,
    index: Option<&RepositoryObservationIndex>,
) -> Result<bool, String> {
    let queries: Vec<NativeProjectQuery> = entry.queries.iter().map(project_query).collect();
    let answers: BTreeMap<String, Vec<String>> = match index {
        Some(index) => observations(&queries, inputs.programs, index)?
            .into_iter()
            .collect(),
        None => BTreeMap::new(),
    };
    Ok(answers_digest(&answers) == entry.answers)
}

/// Parse and evaluate `plans`, answering every file's project questions from one walk.
fn evaluate_files(
    plans: Vec<FilePlan<'_>>,
    inputs: &CallInputs<'_>,
    plane: &NativeProjectPlane,
) -> Result<Vec<(String, PythonCacheEntry)>, String> {
    if plans.is_empty() {
        return Ok(Vec::new());
    }
    let files: Vec<(FilePlan<'_>, Vec<RuleCall>)> = plans
        .into_par_iter()
        .map(|plan| {
            let calls = file_calls(&plan, inputs)?;
            Ok((plan, calls))
        })
        .collect::<Result<_, String>>()?;
    let mut queries: Vec<&NativeProjectQuery> = Vec::new();
    for call in files.iter().flat_map(|(_, calls)| calls) {
        queries.extend(call.queries.iter());
    }
    let index = repository_index(Path::new(&inputs.request.project_dir), &queries)?;
    files
        .into_par_iter()
        .map(|file| file_entry(file, inputs, &index, plane))
        .collect()
}

/// One file's findings and the cache entry that records what they were computed from.
fn file_entry(
    (plan, calls): (FilePlan<'_>, Vec<RuleCall>),
    inputs: &CallInputs<'_>,
    index: &RepositoryObservationIndex,
    plane: &NativeProjectPlane,
) -> Result<(String, PythonCacheEntry), String> {
    let mut answers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut queries: Vec<(String, String, String)> = Vec::new();
    let mut faults: Vec<Fault> = Vec::new();
    for call in calls {
        let observed = observations(&call.queries, inputs.programs, index)?;
        answers.extend(
            observed
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        queries.extend(call.queries.iter().map(|query| {
            (
                query.kind.clone(),
                query.path.clone(),
                query.argument.clone(),
            )
        }));
        let context = NativeRuleContext {
            observations: observed,
            ..call.context
        };
        let program = inputs.programs.program(plan.target.source)?;
        for row in evaluate_core_rules(program, &call.codes, &context, plane)? {
            faults.extend(python_fault(row, plan.target, inputs.programs)?);
        }
    }
    queries.sort();
    queries.dedup();
    Ok((
        plan.target.repository_path.clone(),
        PythonCacheEntry {
            identity: plan.identity,
            queries,
            answers: answers_digest(&answers),
            faults,
        },
    ))
}

/// The file's calls with their contexts and project questions; parses the file.
fn file_calls<'a>(plan: &FilePlan<'a>, inputs: &CallInputs<'_>) -> Result<Vec<RuleCall>, String> {
    let program = inputs.programs.program(plan.target.source)?;
    let base: NativeRuleContext = rule_context(plan.target, &inputs.settings);
    Ok(plan
        .calls
        .iter()
        .map(|(codes, thresholds)| {
            let dead_code: NativeDeadCodeContext = dead_code_context(codes, inputs);
            let context = NativeRuleContext {
                thresholds: thresholds.clone(),
                dead_code,
                ..base.clone()
            };
            RuleCall {
                queries: relative_queries(
                    plan_core_rule_queries(program, codes, &context),
                    &inputs.request.project_dir,
                ),
                codes: codes.clone(),
                context,
            }
        })
        .collect())
}

/// The loaded definitions for a call running the dead-code rule; other calls skip copying them per file.
fn dead_code_context(codes: &[String], inputs: &CallInputs<'_>) -> NativeDeadCodeContext {
    if codes.iter().any(|code| code == DEAD_DEFINITION_CODE) {
        inputs.settings.dead_code.clone()
    } else {
        NativeDeadCodeContext::default()
    }
}

fn project_query((kind, path, argument): &(String, String, String)) -> NativeProjectQuery {
    NativeProjectQuery {
        kind: kind.clone(),
        path: path.clone(),
        argument: argument.clone(),
    }
}

/// Each file's rules; project-, scope- and package-wide rules run once, on the file Fensu picks.
fn file_rules<'a>(
    targets: &[PythonTarget],
    rules: &[&'a PythonRuleSpec],
) -> Result<Vec<Vec<&'a PythonRuleSpec>>, String> {
    let mut by_file: Vec<Vec<&PythonRuleSpec>> = vec![Vec::new(); targets.len()];
    for package in [true, false] {
        let indexes: Vec<usize> = (0..targets.len())
            .filter(|index| targets[*index].package == package)
            .collect();
        let applicable: Vec<&PythonRuleSpec> = rules
            .iter()
            .copied()
            .filter(|rule| package || rule.applies == PythonFiles::All)
            .collect();
        let execution_targets: Vec<NativeExecutionTarget> = indexes
            .iter()
            .map(|index| execution_target(&targets[*index]))
            .collect();
        let execution_rules: Vec<NativeExecutionRule> =
            applicable.iter().map(|rule| execution_rule(rule)).collect();
        let plans = plan_execution_owners(&execution_targets, &execution_rules)?;
        for (index, plan) in indexes.iter().zip(plans) {
            let codes: HashSet<&str> = plan.codes.iter().map(String::as_str).collect();
            by_file[*index] = applicable
                .iter()
                .copied()
                .filter(|rule| rule.fensu_code.is_some_and(|code| codes.contains(code)))
                .collect();
        }
    }
    Ok(by_file)
}

fn execution_rule(rule: &PythonRuleSpec) -> NativeExecutionRule {
    NativeExecutionRule::new(
        rule.fensu_code.unwrap_or_default().to_owned(),
        rule.family.to_owned(),
        rule.owner.to_owned(),
    )
}

fn execution_target(target: &PythonTarget) -> NativeExecutionTarget {
    NativeExecutionTarget {
        repository_path: target.repository_path.clone(),
        scope: ROOT_SCOPE.to_owned(),
        root: target.root.clone(),
        relative_parts: target.relative_parts.clone(),
        direct: true,
        ownership_root: target.package.then(|| PACKAGE_ROOT.to_owned()),
        ownership_offset: target.package.then_some(0),
    }
}
