//! Which file runs which rules, and the faults each `evaluate_core_rules` call reports.

use std::collections::HashSet;
use std::path::Path;

use fensu_facts::snapshot::models::RepositoryObservationIndex;

use fensu_native::{
    NativeExecutionRule, NativeExecutionTarget, NativeProjectPlane, NativeProjectQuery,
    NativeRuleContext, evaluate_core_rules, plan_core_rule_queries, plan_execution_owners,
};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};

use crate::models::{Fault, RuleMetadata};
use crate::python_rules::_helpers::contexts::{rule_context, threshold_calls};
use crate::python_rules::_helpers::faults::python_fault;
use crate::python_rules::_helpers::observations::{observations, repository_index};
use crate::python_rules::constants::{PACKAGE_ROOT, PYTHON_RULES, ROOT_SCOPE};
use crate::python_rules::models::{
    CallInputs, PythonFiles, PythonRuleSpec, PythonTarget, RuleCall,
};

/// The selected rules that Fensu evaluates.
pub(crate) fn selected_rules(selected: &[&RuleMetadata]) -> Vec<&'static PythonRuleSpec> {
    let codes: HashSet<&str> = selected.iter().map(|rule| rule.code.as_str()).collect();
    PYTHON_RULES
        .iter()
        .filter(|rule| rule.fensu_code.is_some() && codes.contains(rule.code))
        .collect()
}

/// Every call of the run, with the project questions each one asks.
pub(crate) fn rule_calls<'a>(
    targets: &'a [PythonTarget],
    rules: &[&PythonRuleSpec],
    inputs: &CallInputs<'_>,
) -> Result<Vec<RuleCall<'a>>, String> {
    Ok(targets
        .par_iter()
        .zip(file_rules(targets, rules)?.into_par_iter())
        .map(|(target, rules)| planned_calls(target, &rules, inputs))
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .flatten()
        .collect())
}

/// The faults of `calls`, answering their project questions from one walk of the project.
pub(crate) fn call_faults(
    calls: Vec<RuleCall<'_>>,
    inputs: &CallInputs<'_>,
    plane: &NativeProjectPlane,
) -> Result<Vec<Fault>, String> {
    let queries: Vec<&NativeProjectQuery> =
        calls.iter().flat_map(|call| call.queries.iter()).collect();
    let index = repository_index(Path::new(&inputs.request.project_dir), &queries)?;
    Ok(calls
        .into_par_iter()
        .map(|call| call_rows(call, inputs, &index, plane))
        .collect::<Result<Vec<Vec<Fault>>, String>>()?
        .into_iter()
        .flatten()
        .collect())
}

fn call_rows(
    call: RuleCall<'_>,
    inputs: &CallInputs<'_>,
    index: &RepositoryObservationIndex,
    plane: &NativeProjectPlane,
) -> Result<Vec<Fault>, String> {
    let context = NativeRuleContext {
        observations: observations(&call.queries, inputs.programs, index)?,
        ..call.context
    };
    let program = inputs.programs.program(call.target.source)?;
    let mut faults: Vec<Fault> = Vec::new();
    for row in evaluate_core_rules(program, &call.codes, &context, plane)? {
        faults.extend(python_fault(row, call.target, inputs.programs)?);
    }
    Ok(faults)
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

fn planned_calls<'a>(
    target: &'a PythonTarget,
    rules: &[&PythonRuleSpec],
    inputs: &CallInputs<'_>,
) -> Result<Vec<RuleCall<'a>>, String> {
    if rules.is_empty() {
        return Ok(Vec::new());
    }
    let program = inputs.programs.program(target.source)?;
    let base: NativeRuleContext = rule_context(target, &inputs.settings);
    Ok(
        threshold_calls(rules, &inputs.request.config, target.is_main_module)
            .into_iter()
            .map(|(codes, thresholds)| {
                let context = NativeRuleContext {
                    thresholds,
                    ..base.clone()
                };
                RuleCall {
                    target,
                    queries: plan_core_rule_queries(program, &codes, &context),
                    codes,
                    context,
                }
            })
            .collect(),
    )
}
