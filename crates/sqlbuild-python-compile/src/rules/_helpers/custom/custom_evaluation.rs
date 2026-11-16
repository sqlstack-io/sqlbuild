//! Incremental custom-rule evaluation keyed on the compiler facts each invocation read.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_cache::digest::main::sha256_hex::sha256_hex;
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{JsonDialect, JsonValue, OrjsonOptions};
use sqlbuild_rules::custom_host_pool::main::custom_rule_timings::custom_rule_timings;
use sqlbuild_rules::custom_host_pool::models::{HostRuleSubject, HostRun, HostTimings};
use sqlbuild_rules::custom_host_pool::types::{FactKey, HostPlan};
use sqlbuild_rules::custom_rule_cache::models::CustomEvaluation;
use sqlbuild_rules::custom_rule_cache::types::ReadSet;

use crate::rules::_helpers::custom::custom_hosts::HostCancel;
use crate::rules::_helpers::custom::custom_hosts::HostRequest;
use crate::rules::_helpers::custom::custom_hosts::run_custom_hosts;
use crate::rules::_helpers::custom::custom_rule_cache::CustomRuleCache;
use crate::rules::_helpers::custom::custom_rule_cache::custom_rule_identity;
use crate::rules::_helpers::custom::custom_rule_evidence::custom_rule_implementation_fingerprint;
use crate::rules::_helpers::custom::custom_rule_evidence::custom_rule_import_closure;
use crate::rules::_helpers::fact::fact_encoder::Encoded;
use crate::rules::_helpers::fact::fact_encoder::reproducible;
use crate::rules::_helpers::fact::fact_replay::FactReplay;
use crate::rules::_helpers::fact::fact_replay::delta_safe_fact;
use crate::rules::_helpers::fact::fact_replay::is_tree_fact;
use crate::rules::_helpers::rule::rule_run::bulk_cache_file;
use crate::rules::constants::CUSTOM_HOST_RUNTIME_VERSION;
use crate::rules::constants::CUSTOM_RULES_CACHE_FILE;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::cache::main::current_code_identity::current_code_identity;
use sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported;
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use sqlbuild_python_base::compile_context::models::DeltaInjection;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const PROJECT_SUBJECT: &str = "project";
/// Why a delta compile cannot rerun a custom rule that reads facts of models it does not compile.
pub(crate) const READ_MORE: &str = "a custom rule read more than the changed models";
pub(crate) const PROJECT_WIDE: &str = "a project-wide custom rule must run again";
pub(crate) const OUTSIDE_DELTA: &str =
    "a custom rule must run again for a model the delta does not compile";
/// The custom-rule cache's subject key of project-wide rules.
pub(crate) const PROJECT_SUBJECT_KEY: &str = "";
const MODEL_SUBJECT: &str = "model";
/// The run one custom-rule evaluation describes.
pub(crate) struct CustomRun<'a, 'py> {
    pub(crate) project: &'a Bound<'py, PyAny>,
    pub(crate) config: &'a Bound<'py, PyAny>,
    pub(crate) project_dir: &'a Bound<'py, PyAny>,
    pub(crate) rules: Vec<Bound<'py, PyAny>>,
    pub(crate) dialect: &'a str,
    pub(crate) verify_determinism: bool,
    pub(crate) implementation_fingerprints: HashMap<String, String>,
    pub(crate) cancel: HostCancel,
    /// The bulk cache subdirectory of the result cache, the bulk cache itself when `None`.
    pub(crate) cache_directory: Option<&'a str>,
}

/// Reuse results whose recorded fact reads are unchanged and evaluate only the rest.
pub(crate) fn evaluate_custom_rules_cached<'py>(
    run: CustomRun<'_, 'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = run.project.py();
    let started: Instant = Instant::now();
    let started_cpu: f64 = cpu_seconds(py)?;
    let mut custom: Vec<(String, Bound<'py, PyAny>)> = run
        .rules
        .iter()
        .map(|rule| {
            Ok((
                native_field(rule, "code")?.extract::<String>()?,
                rule.clone(),
            ))
        })
        .collect::<PyResult<_>>()?;
    custom.sort_by(|left, right| left.0.cmp(&right.0));
    if custom.is_empty() {
        return outcome(py, (Vec::new(), 0, 0), (0, 0), &[]);
    }
    let enabled: bool =
        native_field(&native_field(run.config, "cache")?, "enabled")?.is_truthy()?;
    let mut rows: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    for (code, rule) in &custom {
        let identity: Option<String> = if enabled {
            Some(rule_identity(
                &run,
                rule,
                run.implementation_fingerprints.get(code),
            )?)
        } else {
            None
        };
        rows.push((code.clone(), identity, subject_value(rule)?));
    }
    let cache_path: Option<PathBuf> = if enabled {
        Some(bulk_cache_file(
            &run.project_dir.extract::<PathBuf>()?,
            run.cache_directory,
            CUSTOM_RULES_CACHE_FILE,
        ))
    } else {
        None
    };
    let project_dir: PathBuf = run.project_dir.extract()?;
    let mut digests: FactDigests<'py> = FactDigests::new(run.project, &project_dir)?;
    let models: Vec<(String, String, Option<String>)> =
        match sqlbuild_python_base::compile_context::main::delta::injection::injection() {
            Some(injection) => {
                let Some(mut rows) = injection.model_subjects.clone() else {
                    return Err(
                        sqlbuild_python_base::compile_context::main::delta::delta_unsupported::delta_unsupported(
                            "the custom rules' subjects were not recorded",
                        ),
                    );
                };
                let fresh: Vec<(String, String, Option<String>)> = digests.replay.model_rows()?;
                for row in &mut rows {
                    if let Some(found) = fresh.iter().find(|candidate| candidate.0 == row.0) {
                        row.clone_from(found);
                    }
                }
                rows
            }
            None => {
                let rows: Vec<(String, String, Option<String>)> = digests.replay.model_rows()?;
                sqlbuild_python_base::compile_context::main::delta::capture::capture(|capture| {
                    capture.model_subjects = Some(rows.clone());
                });
                rows
            }
        };
    let mut model_paths: Vec<String> = models.iter().map(|(path, _, _)| path.clone()).collect();
    model_paths.sort();
    let mut cache: CustomRuleCache = CustomRuleCache::new(
        cache_path,
        rows,
        native_field(run.project, "effective_target_name")?.extract()?,
        models,
    );
    let pending: Vec<ReadSet> = cache.pending_readsets().to_vec();
    let mut readset_digests: Vec<Option<String>> = Vec::new();
    for readset in &pending {
        readset_digests.push(reproducible(digests.combined(readset))?);
    }
    let plan: HostPlan = cache.plan(readset_digests);
    let mut host_timings: HostTimings = HostTimings::default();
    let delta: Option<std::sync::Arc<DeltaInjection>> = injection();
    if let Some(injection) = &delta
        && let Some(reason) = delta_plan_refusal(&plan, &cache, injection)
    {
        return Err(delta_unsupported(reason));
    }
    if !plan.is_empty() {
        let rules: Vec<(String, HostRuleSubject)> = custom
            .iter()
            .map(|(code, rule)| Ok((code.clone(), host_subject(rule)?)))
            .collect::<PyResult<_>>()?;
        let mut host_run: HostRun = run_custom_hosts(
            py,
            &HostRequest {
                project: run.project,
                config: run.config,
                project_dir: &project_dir,
                dialect: run.dialect,
                rules: &rules,
                model_paths: &model_paths,
                track_reads: enabled,
                verify_determinism: run.verify_determinism,
                cancel: &run.cancel,
            },
            &plan,
        )?;
        host_timings = std::mem::take(&mut host_run.timings);
        let evaluations: Vec<CustomEvaluation> = evaluations(host_run, &mut digests, enabled)?;
        if let Some(injection) = &delta
            && !evaluations
                .iter()
                .all(|evaluation| delta_safe_evaluation(evaluation, injection))
        {
            return Err(delta_unsupported(READ_MORE));
        }
        cache.record(py, evaluations)?;
    }
    sqlbuild_python_base::compile_context::main::delta::capture::capture(|capture| {
        capture.fact_digests.extend(
            digests
                .digests
                .iter()
                .filter(|(key, _)| !is_tree_fact(key))
                .map(|(key, digest)| (key.clone(), digest.clone())),
        );
    });
    let elapsed_ms: i64 = (started.elapsed().as_secs_f64() * 1000.0).round_ties_even() as i64;
    let cpu_ms: i64 = ((cpu_seconds(py)? - started_cpu) * 1000.0) as i64;
    outcome(
        py,
        (cache.findings(py)?, cache.hits(), cache.misses()),
        (elapsed_ms, cpu_ms),
        &custom_rule_timings(&host_timings, cache.counts_by_code()),
    )
}

/// Why a delta can't run `plan`: it runs only its own models' rules that read what it holds.
fn delta_plan_refusal(
    plan: &HostPlan,
    cache: &CustomRuleCache,
    injection: &DeltaInjection,
) -> Option<&'static str> {
    let all_sources: bool = injection.source_names.is_none();
    for (code, subjects) in plan {
        let Some(subjects) = subjects else {
            return Some(PROJECT_WIDE);
        };
        for subject in subjects {
            if !injection.model_paths.contains(subject) {
                return Some(OUTSIDE_DELTA);
            }
            if cache.stored_readset(code, subject).is_some_and(|readset| {
                !readset
                    .iter()
                    .all(|key| delta_safe_fact(key, &injection.model_paths, all_sources))
            }) {
                return Some(READ_MORE);
            }
        }
    }
    None
}

/// Whether a delta compile evaluated a custom rule from facts it holds in full.
fn delta_safe_evaluation(evaluation: &CustomEvaluation, injection: &DeltaInjection) -> bool {
    let all_sources: bool = injection.source_names.is_none();
    evaluation.cacheable.as_ref().is_some_and(|(readset, _)| {
        readset
            .iter()
            .all(|key| delta_safe_fact(key, &injection.model_paths, all_sources))
    })
}

/// `CustomRulesOutcome(findings=..., cache_hits=..., cache_misses=..., custom_ms=..., ...)`.
pub(crate) fn outcome<'py>(
    py: Python<'py>,
    (findings, hits, misses): (Vec<Bound<'py, PyAny>>, usize, usize),
    (custom_ms, custom_cpu_ms): (i64, i64),
    timings: &[(String, u64)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::CustomRulesOutcome,
        &[
            ("findings", PyTuple::new(py, findings)?.into_any()),
            ("cache_hits", hits.into_bound_py_any(py)?),
            ("cache_misses", misses.into_bound_py_any(py)?),
            ("custom_ms", custom_ms.into_bound_py_any(py)?),
            ("custom_cpu_ms", custom_cpu_ms.into_bound_py_any(py)?),
            (
                "custom_rule_timings",
                PyTuple::new(py, timings.iter().cloned())?.into_any(),
            ),
        ],
    )
}

/// Each evaluated invocation with its read set and its digest when it may be cached.
fn evaluations(
    run: HostRun,
    digests: &mut FactDigests<'_>,
    cacheable: bool,
) -> PyResult<Vec<CustomEvaluation>> {
    let mut full_keys: Option<Vec<FactKey>> = None;
    let mut combined: HashMap<ReadSet, Option<String>> = HashMap::new();
    let mut rows: Vec<CustomEvaluation> = Vec::new();
    for evaluation in run.evaluated {
        let mut stored: Option<(ReadSet, Option<String>)> = None;
        if cacheable && !run.uncacheable.contains(&evaluation.code) {
            let reads: Vec<FactKey> = match evaluation.reads {
                Some(reads) => reads,
                None => {
                    if full_keys.is_none() {
                        full_keys = Some(digests.replay.full_fact_keys()?);
                    }
                    let mut reads: Vec<FactKey> = run
                        .untracked_reads
                        .get(&evaluation.code)
                        .cloned()
                        .unwrap_or_default();
                    reads.extend(full_keys.iter().flatten().cloned());
                    reads
                }
            };
            let readset: ReadSet = reads
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if !combined.contains_key(&readset) {
                let digest: Option<String> =
                    reproducible(digests.observed(&readset, &run.observed))?.flatten();
                combined.insert(readset.clone(), digest);
            }
            let digest: Option<String> = combined.get(&readset).cloned().flatten();
            stored = Some((readset, digest));
        }
        rows.push(CustomEvaluation {
            code: evaluation.code,
            subject: evaluation.subject,
            cacheable: stored,
            findings: evaluation.findings,
        });
    }
    Ok(rows)
}

/// Memoized digests of recorded fact keys against one compiled project snapshot.
struct FactDigests<'py> {
    replay: FactReplay<'py>,
    digests: HashMap<FactKey, String>,
}

impl<'py> FactDigests<'py> {
    fn new(project: &Bound<'py, PyAny>, project_dir: &Path) -> PyResult<Self> {
        Ok(Self {
            replay: FactReplay::new(project, project_dir)?,
            digests: HashMap::new(),
        })
    }

    /// One fact digest; `Unreproducible` when it cannot be recomputed.
    fn digest(&mut self, key: &FactKey) -> Encoded<String> {
        if let Some(cached) = self.digests.get(key) {
            return Ok(cached.clone());
        }
        if let Some(injected) =
            sqlbuild_python_base::compile_context::main::delta::injected_fact_digest::injected_fact_digest(key)
        {
            self.digests.insert(key.clone(), injected.clone());
            return Ok(injected);
        }
        let digest: String = self.replay.digest(key)?;
        self.digests.insert(key.clone(), digest.clone());
        Ok(digest)
    }

    /// One read set's digest over its keys in canonical order.
    fn combined(&mut self, keys: &[FactKey]) -> Encoded<String> {
        let mut sorted: Vec<&FactKey> = keys.iter().collect();
        sorted.sort();
        let mut bytes: Vec<u8> = Vec::new();
        for key in sorted {
            bytes.extend(orjson_key(key).into_bytes());
            bytes.extend(self.digest(key)?.into_bytes());
        }
        Ok(sha256_hex(&bytes))
    }

    /// The read set's digest, or `None` when a filesystem fact changed since the rule read it.
    fn observed(
        &mut self,
        readset: &[FactKey],
        observed: &BTreeMap<FactKey, Option<String>>,
    ) -> Encoded<Option<String>> {
        for key in readset {
            if is_tree_fact(key)
                && let Some(seen) = observed.get(key)
                && seen.as_deref() != Some(self.digest(key)?.as_str())
            {
                return Ok(None);
            }
        }
        self.combined(readset).map(Some)
    }
}

/// `orjson.dumps(key)` of one fact key.
fn orjson_key(key: &FactKey) -> String {
    let value: JsonValue = JsonValue::Array(
        key.iter()
            .map(|part| JsonValue::String(part.clone()))
            .collect(),
    );
    dumps(&value, &JsonDialect::Orjson(OrjsonOptions::default())).unwrap_or_default()
}

fn subject_value(rule: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let subject: Bound<'_, PyAny> = native_field(rule, "subject")?;
    if subject.is_none() {
        return Ok(None);
    }
    native_field(&subject, "value")?.extract()
}

fn host_subject(rule: &Bound<'_, PyAny>) -> PyResult<HostRuleSubject> {
    Ok(match subject_value(rule)?.as_deref() {
        Some(PROJECT_SUBJECT) => HostRuleSubject::Project,
        Some(MODEL_SUBJECT) => HostRuleSubject::Model,
        Some(_) | None => HostRuleSubject::Other,
    })
}

fn rule_identity(
    run: &CustomRun<'_, '_>,
    rule: &Bound<'_, PyAny>,
    fingerprint: Option<&String>,
) -> PyResult<String> {
    let py: Python<'_> = rule.py();
    let fingerprint: Bound<'_, PyAny> = match fingerprint {
        Some(fingerprint) => fingerprint.into_bound_py_any(py)?,
        None => {
            let closure: Bound<'_, PyAny> =
                custom_rule_import_closure(rule, run.project_dir)?.into_any();
            custom_rule_implementation_fingerprint(rule, run.project_dir, Some(&closure))?
                .into_bound_py_any(py)?
        }
    };
    let subject: Option<String> = subject_value(rule)?;
    custom_rule_identity(Some(&keywords(
        py,
        &[
            (
                "code_identity",
                current_code_identity(py)?.into_bound_py_any(py)?,
            ),
            (
                "runtime_version",
                pyo3::types::PyString::new(py, CUSTOM_HOST_RUNTIME_VERSION).into_any(),
            ),
            ("code", native_field(rule, "code")?),
            ("family", native_field(rule, "family")?),
            ("slug", native_field(rule, "slug")?),
            ("message", native_field(rule, "message")?),
            ("remediation", native_field(rule, "remediation")?),
            ("subject", subject.into_bound_py_any(py)?),
            ("implementation_fingerprint", fingerprint),
            ("config", run.config.clone()),
            ("dialect", run.dialect.into_bound_py_any(py)?),
            (
                "project_dir",
                run.project_dir.call_method0("resolve")?.str()?.into_any(),
            ),
        ],
    )?))
}

/// This thread's CPU plus that of finished child processes, which run the rule hosts.
fn cpu_seconds(py: Python<'_>) -> PyResult<f64> {
    let thread: f64 = imported(py, "time", "thread_time")?.call0()?.extract()?;
    let times: Bound<'_, PyAny> = imported(py, "os", "times")?.call0()?;
    let user: f64 = native_field(&times, "children_user")?.extract()?;
    let system: f64 = native_field(&times, "children_system")?.extract()?;
    Ok(thread + user + system)
}

/// The digest the custom-rule cache keys one recorded fact on, or `None` when unreproducible.
#[pyfunction]
#[pyo3(signature = (*, project, project_dir, key))]
pub(crate) fn custom_rule_fact_digest(
    project: &Bound<'_, PyAny>,
    project_dir: PathBuf,
    key: FactKey,
) -> PyResult<Option<String>> {
    reproducible(FactReplay::new(project, &project_dir)?.digest(&key))
}
