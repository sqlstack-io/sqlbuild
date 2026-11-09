use crate::configuration::main::validate as config;
use crate::constants::{API_VERSION, NATIVE_BUILD_IDENTITY, RELATION_EXCEPTION_RULES};
use crate::engine::_helpers::cache::{Cache, PendingBucketWrite, RuleCacheEntry};
use crate::lint_text::constants::LINT_UNAVAILABLE_CODE;
use crate::models::{
    EvaluateRequest, EvaluateResponse, Fault, Model, RuleException, RuleMetadata, RuleScope,
    RulesCodeGrammar, RulesConfig,
};
use crate::rules::main::{
    assemble_catalogue, evaluate as rules, evaluate_project, fingerprint,
    resolve_threshold_overrides, select, sql_test_coverage,
};
use crate::rules::models::{
    ModelEvaluationRequest, ProjectEvaluationRequest, QueryShapes, ResolvedThresholdOverride,
};
use fensu_policy::apply_suppressions;
use fensu_policy::lifecycle::errors::LifecycleError;
use fensu_policy::lifecycle::models::{
    ApplySuppressionsRequest, ExactSuppression, Finding, FindingSeverity, ScopedIgnore,
};
#[cfg(test)]
use rayon::iter::IndexedParallelIterator;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use sqlclean::sql_lint::main::cross_join_subject::cross_join_subject;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const NATIVE_RULE_WORKERS: usize = 4;
const NATIVE_RULE_STACK_BYTES: usize = 16 * 1024 * 1024;
const MODEL_RULES_IDENTITY_SEED: &[u8] = b"sqlbuild-rules-model-v2";
const CANCELLED: &str = "native rules evaluation was cancelled";

/// How one evaluation publishes its bulk rules cache bucket and whether it may stop early.
pub(crate) struct EvaluationControl<'a> {
    /// `None` writes the bucket at once; `Some` holds the write for the caller to publish.
    pub(crate) deferred_bucket: Option<&'a RefCell<Option<PendingBucketWrite>>>,
    /// Stops model evaluation between models once set.
    pub(crate) cancel: Option<&'a AtomicBool>,
}

impl EvaluationControl<'_> {
    /// Write the cache at once and never stop early, as an ordinary evaluation does.
    pub(crate) fn immediate() -> Self {
        Self {
            deferred_bucket: None,
            cancel: None,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel
            .is_some_and(|cancel| cancel.load(Ordering::Acquire))
    }
}

struct PendingModelRule<'a> {
    index: usize,
    model: &'a Model,
    identity: Option<String>,
}

/// The error a delta compile's rules request raises when a row without SQL needs evaluating.
pub(crate) const STRIPPED_ROW_MISSED: &str =
    "delta compile unsupported: a model's rule findings are no longer cached";

struct CompletedModelRule {
    index: usize,
    identity: Option<String>,
    faults: Result<Vec<Fault>, String>,
}

struct ModelRuleBatchContext<'a> {
    request: &'a EvaluateRequest,
    selected: &'a BTreeMap<String, &'a RuleMetadata>,
    threshold_overrides: &'a [ResolvedThresholdOverride],
    cancel: Option<&'a AtomicBool>,
}

struct NativeModelRuleRequest<'a> {
    context: ModelRuleBatchContext<'a>,
    cache: Option<&'a Cache>,
    model_rules_fingerprint: &'a str,
}

#[derive(Default)]
struct NativeModelEvaluation {
    faults: Vec<Fault>,
    hits: usize,
    misses: usize,
}

fn evaluate_native_models(
    input: NativeModelRuleRequest<'_>,
    control: &EvaluationControl<'_>,
) -> Result<NativeModelEvaluation, String> {
    let NativeModelRuleRequest {
        context,
        cache,
        model_rules_fingerprint,
    } = input;
    if !context
        .selected
        .values()
        .any(|rule| !rule.custom && !rule.code.starts_with("SQLCLEAN"))
    {
        return Ok(NativeModelEvaluation::default());
    }
    let mut models: Vec<_> = context.request.models.iter().collect();
    models.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let all_models_required = context.selected.values().any(|rule| {
        !rule.custom
            && !rule.code.starts_with("SQLCLEAN")
            && !matches!(
                rule.code.as_str(),
                "SQBRDECLARATION201" | "SQBRDECLARATION301" | "SQBRTEST301"
            )
    });
    let model_limit = if all_models_required {
        models.len()
    } else {
        models.len().min(1)
    };
    let mut bucket = cache
        .map(|store| store.native_rules_bucket(model_rules_fingerprint))
        .transpose()?
        .unwrap_or_default();
    let mut bucket_changed = false;
    let mut result = NativeModelEvaluation::default();
    let mut model_results: Vec<Option<Vec<Fault>>> = vec![None; model_limit];
    let mut pending_models: Vec<PendingModelRule<'_>> = Vec::new();
    let mut identities = if cache.is_some() {
        model_cache_identities(&models[..model_limit], model_rules_fingerprint)?
            .into_iter()
            .map(Some)
            .collect()
    } else {
        vec![None; model_limit]
    };
    for (index, model) in models.iter().take(model_limit).enumerate() {
        let identity = identities[index].take();
        if let Some(identity) = &identity
            && let Some(entry) = bucket
                .entries
                .get(&model.relative_path)
                .filter(|entry| entry.identity == *identity)
        {
            model_results[index] = Some(entry.faults.clone());
            result.hits += 1;
            continue;
        }
        result.misses += 1;
        pending_models.push(PendingModelRule {
            index,
            model,
            identity,
        });
    }
    if pending_models.iter().any(|pending| pending.model.stripped) {
        return Err(STRIPPED_ROW_MISSED.to_owned());
    }
    let project_dir = Path::new(&context.request.project_dir);
    let completed = evaluate_pending_models(pending_models, context)?;
    for completed_model in completed {
        let faults = completed_model.faults?;
        if let Some(identity) = completed_model.identity {
            let _ = bucket.entries.insert(
                models[completed_model.index].relative_path.clone(),
                RuleCacheEntry {
                    identity,
                    faults: faults.clone(),
                },
            );
            bucket_changed = true;
        }
        model_results[completed_model.index] = Some(faults);
    }
    result.faults = model_results.into_iter().flatten().flatten().collect();
    if bucket_changed && let Some(cache) = cache {
        let current: BTreeSet<&str> = models[..model_limit]
            .iter()
            .map(|model| model.relative_path.as_str())
            .collect();
        bucket
            .entries
            .retain(|path, _| current.contains(path.as_str()) || project_dir.join(path).is_file());
        match control.deferred_bucket {
            None => cache.put_native_rules_bucket(model_rules_fingerprint, &bucket)?,
            Some(deferred) => {
                *deferred.borrow_mut() =
                    Some(cache.pending_native_rules_bucket(model_rules_fingerprint, bucket));
            }
        }
    }
    Ok(result)
}

fn evaluate_pending_models(
    pending_models: Vec<PendingModelRule<'_>>,
    context: ModelRuleBatchContext<'_>,
) -> Result<Vec<CompletedModelRule>, String> {
    let shapes = QueryShapes::default();
    let evaluate_pending = |pending: PendingModelRule<'_>| CompletedModelRule {
        index: pending.index,
        identity: pending.identity,
        faults: if context
            .cancel
            .is_some_and(|cancel| cancel.load(Ordering::Acquire))
        {
            Err(CANCELLED.to_owned())
        } else {
            rules::evaluate_model(ModelEvaluationRequest {
                model: pending.model,
                config: &context.request.config,
                selected: context.selected,
                request: context.request,
                threshold_overrides: context.threshold_overrides,
                shapes: &shapes,
            })
            .and_then(|faults| scoped_faults(context.selected, faults, RuleScope::evaluates_models))
        },
    };
    if pending_models.len() <= 1 {
        return Ok(pending_models.into_iter().map(evaluate_pending).collect());
    }
    Ok(native_rule_pool(pending_models.len())?.install(|| {
        pending_models
            .into_par_iter()
            .map(evaluate_pending)
            .collect()
    }))
}

#[cfg(test)]
pub(crate) fn evaluate_json(request_json: &str) -> Result<String, String> {
    let request: EvaluateRequest = serde_json::from_str(request_json)
        .map_err(|error| format!("invalid rules request: {error}"))?;
    evaluate_request(request)
}

/// Decode a request whose models arrive as separate payloads, each with its exact digest.
#[cfg(test)]
pub(crate) fn parse_parts(
    request_json: &[u8],
    model_jsons: &[&[u8]],
    model_digests: &[String],
) -> Result<EvaluateRequest, String> {
    let mut request: EvaluateRequest = serde_json::from_slice(request_json)
        .map_err(|error| format!("invalid rules request: {error}"))?;
    if !request.models.is_empty() {
        return Err("invalid rules request: models must be sent as separate payloads".to_owned());
    }
    if model_jsons.len() != model_digests.len() {
        return Err("invalid rules request: every model payload needs one digest".to_owned());
    }
    request.models = parse_model_payloads(model_jsons, model_digests)?;
    Ok(request)
}

#[cfg(test)]
fn parse_model_payloads(
    model_jsons: &[&[u8]],
    model_digests: &[String],
) -> Result<Vec<Model>, String> {
    let parse = |(payload, digest): (&&[u8], &String)| -> Result<Model, String> {
        let mut model: Model = serde_json::from_slice(payload)
            .map_err(|error| format!("invalid rules request: {error}"))?;
        model.payload_digest = Some(digest.clone());
        Ok(model)
    };
    let results: Vec<Result<Model, String>> = if model_jsons.len() <= 1 {
        model_jsons.iter().zip(model_digests).map(parse).collect()
    } else {
        native_rule_pool(model_jsons.len())?.install(|| {
            model_jsons
                .par_iter()
                .zip(model_digests.par_iter())
                .map(parse)
                .collect()
        })
    };
    results.into_iter().collect()
}

fn native_rule_pool(items: usize) -> Result<rayon::ThreadPool, String> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(NATIVE_RULE_WORKERS.min(items))
        .stack_size(NATIVE_RULE_STACK_BYTES)
        .build()
        .map_err(|error| error.to_string())
}

/// Reject findings from a phase that the emitting rule's declared scope does not cover.
fn scoped_faults(
    selected: &BTreeMap<String, &RuleMetadata>,
    faults: Vec<Fault>,
    covers: fn(RuleScope) -> bool,
) -> Result<Vec<Fault>, String> {
    if let Some(fault) = faults.iter().find(|fault| {
        selected
            .get(&fault.code)
            .is_some_and(|rule| !covers(rule.scope))
    }) {
        return Err(format!(
            "rule {} emitted a finding outside its declared scope",
            fault.code
        ));
    }
    Ok(faults)
}

/// Fingerprint model-rule inputs; suppressions and graph-edge exceptions never reach model rules.
fn model_rules_fingerprint(
    selected: &[&RuleMetadata],
    config: &RulesConfig,
    dialect: &str,
) -> Result<String, String> {
    let model_config = RulesConfig {
        rule_exceptions: Vec::new(),
        rule_ignores: Vec::new(),
        graph_edge_exceptions: Vec::new(),
        cache: Default::default(),
        ..config.clone()
    };
    fingerprint::fingerprint(selected, &model_config, dialect)
}

#[cfg(test)]
pub(crate) fn evaluate_request(request: EvaluateRequest) -> Result<String, String> {
    evaluate_request_with(request, &EvaluationControl::immediate())
}

/// Evaluate `request`, publishing its cache bucket and stopping early as `control` says.
pub(crate) fn evaluate_request_with(
    request: EvaluateRequest,
    control: &EvaluationControl<'_>,
) -> Result<String, String> {
    let store: Option<PathBuf> = request.config.cache.enabled.then(|| {
        crate::engine::_helpers::cache::fixture_summaries_path(
            Path::new(&request.project_dir),
            &request.rules_cache_path,
        )
    });
    crate::rules::_helpers::sql_test_rules::with_fixture_memo(store.as_deref(), || {
        let evaluated: Result<String, String> = evaluated_request(request, control);
        let keep: bool = match &evaluated {
            Ok(_) => !control.cancelled(),
            Err(_failed) => false,
        };
        (evaluated, keep)
    })
}

fn evaluated_request(
    request: EvaluateRequest,
    control: &EvaluationControl<'_>,
) -> Result<String, String> {
    if request.version != API_VERSION {
        return Err(format!(
            "unsupported rules native API version {}; expected {API_VERSION}",
            request.version
        ));
    }
    config::validate(&request.config)?;
    let all_rules = assemble_catalogue::assemble_catalogue(&request.custom_rules)?;
    let selected = select::select(
        &all_rules,
        request.config.select.as_deref(),
        &request.config.ignore,
    )?;
    let request = if selected
        .iter()
        .any(|rule| matches!(rule.code.as_str(), "SQBRTEST202" | "SQBRTEST203"))
    {
        sql_test_coverage::annotate(request)
    } else {
        request
    };
    validate_suppression_codes(&request, &all_rules)?;
    let ruleset_fingerprint =
        fingerprint::fingerprint(&selected, &request.config, &request.dialect)?;
    let selected_by_code: BTreeMap<String, &RuleMetadata> = selected
        .iter()
        .map(|rule| (rule.code.clone(), *rule))
        .collect();
    let threshold_overrides =
        resolve_threshold_overrides::resolve_threshold_overrides(&request.config)?;
    if selected.is_empty() {
        validate_exception_paths(&request)?;
        if !request.defer_suppressions {
            let _ = suppress_faults(&request, &selected, Vec::new())?;
        }
        return serde_json::to_string(&EvaluateResponse {
            version: API_VERSION,
            faults: Vec::new(),
            evaluated_models: 0,
            cache_hits: 0,
            cache_misses: 0,
            ruleset_fingerprint,
            selected_codes: Vec::new(),
            built_in_ms: 0,
        })
        .map_err(|error| error.to_string());
    }

    let cache_enabled = request.config.cache.enabled;
    let model_rules_fingerprint = cache_enabled
        .then(|| model_rules_fingerprint(&selected, &request.config, &request.dialect))
        .transpose()?
        .unwrap_or_default();
    let cache = cache_enabled
        .then(|| Cache::open(Path::new(&request.project_dir), &request.rules_cache_path))
        .transpose()?;
    let built_in_started = Instant::now();
    let mut raw_faults = request.initial_findings.clone();
    raw_faults.extend(scoped_faults(
        &selected_by_code,
        evaluate_project::evaluate_project(ProjectEvaluationRequest {
            selected: &selected_by_code,
            request: &request,
        })?,
        RuleScope::evaluates_project,
    )?);
    if control.cancelled() {
        return Err(CANCELLED.to_owned());
    }
    let cancel: Option<&AtomicBool> = control.cancel;
    let native = evaluate_native_models(
        NativeModelRuleRequest {
            context: ModelRuleBatchContext {
                request: &request,
                selected: &selected_by_code,
                threshold_overrides: &threshold_overrides,
                cancel,
            },
            cache: cache.as_ref(),
            model_rules_fingerprint: &model_rules_fingerprint,
        },
        control,
    )?;
    let cache_hits = native.hits;
    let cache_misses = native.misses;
    raw_faults.extend(native.faults);
    let built_in_ms = built_in_started.elapsed().as_millis() as u64;
    validate_exception_paths(&request)?;
    let faults = if request.defer_suppressions {
        raw_faults
    } else {
        suppress_faults(&request, &selected, raw_faults)?
    };
    serde_json::to_string(&EvaluateResponse {
        version: API_VERSION,
        faults,
        evaluated_models: request.models.len(),
        cache_hits,
        cache_misses,
        ruleset_fingerprint,
        selected_codes: selected.iter().map(|rule| rule.code.clone()).collect(),
        built_in_ms,
    })
    .map_err(|error| error.to_string())
}

fn model_cache_identities(
    models: &[&crate::models::Model],
    model_rules: &str,
) -> Result<Vec<String>, String> {
    let identity = |model: &&crate::models::Model| model_cache_identity(model, model_rules);
    let results: Vec<Result<String, String>> = if models.len() <= 1 {
        models.iter().map(identity).collect()
    } else {
        native_rule_pool(models.len())?.install(|| models.par_iter().map(identity).collect())
    };
    results.into_iter().collect()
}

/// Key one model's findings by payload, model-rules fingerprint, and derived test counts.
fn model_cache_identity(model: &crate::models::Model, model_rules: &str) -> Result<String, String> {
    let mut digest = blake3::Hasher::new();
    digest.update(MODEL_RULES_IDENTITY_SEED);
    digest.update(&[0]);
    digest.update(NATIVE_BUILD_IDENTITY.as_bytes());
    digest.update(&[0]);
    digest.update(model_rules.as_bytes());
    digest.update(&[0]);
    match &model.payload_digest {
        Some(payload) => {
            digest.update(b"payload:");
            digest.update(payload.as_bytes());
        }
        None => {
            digest.update(b"model:");
            serde_json::to_writer(&mut digest, model).map_err(|error| error.to_string())?;
        }
    }
    digest.update(&[0]);
    digest.update(&model.empty_input_only_test_count.to_le_bytes());
    Ok(digest.finalize().to_hex().to_string())
}

fn validate_suppression_codes(
    request: &EvaluateRequest,
    catalogue: &[RuleMetadata],
) -> Result<(), String> {
    let codes: BTreeSet<_> = catalogue.iter().map(|rule| rule.code.as_str()).collect();
    let unknown: BTreeSet<_> = request
        .config
        .rule_exceptions
        .iter()
        .map(|entry| entry.rule.as_str())
        .filter(|code| !codes.contains(code))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "rule exceptions target unknown rules: {}",
            unknown.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    for ignore in &request.config.rule_ignores {
        for selector in &ignore.rules {
            if !catalogue.iter().any(|rule| rule.code.starts_with(selector)) {
                return Err(format!("rule-ignore selector matches no rules: {selector}"));
            }
        }
    }
    Ok(())
}

fn validate_exception_paths(request: &EvaluateRequest) -> Result<(), String> {
    let root = Path::new(&request.project_dir);
    for entry in &request.config.rule_exceptions {
        if !root.join(&entry.path).is_file() {
            return Err(format!(
                "rule exception path does not exist: {}",
                entry.path
            ));
        }
    }
    let literal_ignore_paths = request
        .config
        .rule_ignores
        .iter()
        .flat_map(|entry| &entry.paths)
        .filter(|path| !path.contains(['*', '?', '[', '{']));
    for path in literal_ignore_paths {
        if !root.join(path).exists() {
            return Err(format!("rule ignore path does not exist: {path}"));
        }
    }
    Ok(())
}

fn suppress_faults(
    request: &EvaluateRequest,
    selected: &[&RuleMetadata],
    faults: Vec<Fault>,
) -> Result<Vec<Fault>, String> {
    let evaluated_codes = selected
        .iter()
        .map(|rule| rule.code.clone())
        .collect::<Vec<_>>();
    apply_fault_policy(request, &evaluated_codes, faults)
}

/// Apply the exception policy once to completed built-in, SQL and custom findings.
pub(crate) fn finalize_findings(
    project_dir: String,
    config: crate::models::RulesConfig,
    evaluated_codes: &[String],
    findings: Vec<Fault>,
) -> Result<Vec<Fault>, String> {
    config::validate(&config)?;
    let request = EvaluateRequest {
        project_dir,
        config,
        ..Default::default()
    };
    validate_exception_paths(&request)?;
    apply_fault_policy(&request, evaluated_codes, findings)
}

/// Apply rule exceptions to `faults`; lint-unavailable warnings report our gap, not a rule, so they pass through unsuppressed.
fn apply_fault_policy(
    request: &EvaluateRequest,
    evaluated_codes: &[String],
    faults: Vec<Fault>,
) -> Result<Vec<Fault>, String> {
    let (unavailable, faults): (Vec<Fault>, Vec<Fault>) = faults
        .into_iter()
        .partition(|fault| fault.code == LINT_UNAVAILABLE_CODE);
    let unevaluated: BTreeSet<_> = faults
        .iter()
        .filter(|fault| fault.unevaluated)
        .map(|fault| {
            (
                fault.code.clone(),
                fault.path.clone(),
                fault.message.clone(),
            )
        })
        .collect();
    let exceptions: &[RuleException] = &request.config.rule_exceptions;
    let findings = faults
        .into_iter()
        .map(|fault| fault_to_finding(fault, exceptions))
        .collect::<Result<Vec<_>, _>>()?;
    let suppressions = exceptions
        .iter()
        .map(|entry| ExactSuppression {
            code: entry.rule.clone(),
            path: entry.path.clone(),
            symbol: exception_symbol(entry),
            reason: entry.reason.clone(),
        })
        .collect::<Vec<_>>();
    let scoped_ignores = request
        .config
        .rule_ignores
        .iter()
        .map(|entry| ScopedIgnore {
            selectors: entry.rules.clone(),
            paths: entry.paths.clone(),
            reason: entry.reason.clone(),
        })
        .collect::<Vec<_>>();
    let grammar = RulesCodeGrammar;
    let result = apply_suppressions(ApplySuppressionsRequest {
        findings,
        evaluated_codes,
        suppressions: &suppressions,
        scoped_ignores: &scoped_ignores,
        grammar: &grammar,
    })
    .map_err(lifecycle_error)?;
    Ok(result
        .findings
        .into_iter()
        .map(|finding| {
            let mut fault = finding_to_fault(finding);
            fault.unevaluated = unevaluated.contains(&(
                fault.code.clone(),
                fault.path.clone(),
                fault.message.clone(),
            ));
            fault
        })
        .chain(unavailable)
        .collect())
}

/// The symbol a relation-level exception and the findings it covers share.
fn relation_symbol(relation: &str, cte: Option<&str>) -> String {
    let relation: String = relation.to_lowercase();
    match cte {
        Some(cte) => format!("relation {relation} in CTE {}", cte.to_lowercase()),
        None => format!("relation {relation}"),
    }
}

/// The symbol a relation-level exception covers, `None` for a file-wide exception.
fn exception_symbol(entry: &RuleException) -> Option<String> {
    let relation: &str = entry.relation.as_deref()?;
    Some(relation_symbol(relation, entry.cte.as_deref()))
}

/// The relation-level exception symbol that covers `fault`: the CTE-scoped one first.
fn covering_symbol(fault: &Fault, exceptions: &[RuleException]) -> Option<String> {
    if !RELATION_EXCEPTION_RULES.contains(&fault.code.as_str()) {
        return None;
    }
    let scoped: Vec<&RuleException> = exceptions
        .iter()
        .filter(|entry| {
            entry.rule == fault.code && entry.path == fault.path && entry.relation.is_some()
        })
        .collect();
    if scoped.is_empty() {
        return None;
    }
    let (relation, cte) = cross_join_subject(&fault.message)?;
    let matches = |entry: &&RuleException, cte: Option<&str>| {
        entry
            .relation
            .as_deref()
            .is_some_and(|named| named.eq_ignore_ascii_case(&relation))
            && match (entry.cte.as_deref(), cte) {
                (Some(named), Some(found)) => named.eq_ignore_ascii_case(found),
                (None, _) => true,
                (Some(_), None) => false,
            }
    };
    scoped
        .iter()
        .find(|entry| entry.cte.is_some() && matches(entry, cte.as_deref()))
        .or_else(|| {
            scoped
                .iter()
                .find(|entry| entry.cte.is_none() && matches(entry, None))
        })
        .map(|entry| {
            relation_symbol(
                entry.relation.as_deref().unwrap_or_default(),
                entry.cte.as_deref(),
            )
        })
}

fn fault_to_finding(fault: Fault, exceptions: &[RuleException]) -> Result<Finding, String> {
    let symbol: Option<String> = covering_symbol(&fault, exceptions);
    Ok(Finding {
        code: fault.code,
        path: fault.path,
        line: Some(u32::try_from(fault.line).map_err(|_| "rule finding line exceeds u32")?),
        column: Some(u32::try_from(fault.column).map_err(|_| "rule finding column exceeds u32")?),
        symbol,
        message: fault.message,
        remediation: Some(fault.remediation),
        severity: FindingSeverity::Blocking,
    })
}

fn finding_to_fault(finding: Finding) -> Fault {
    Fault {
        unevaluated: false,
        code: finding.code,
        path: finding.path,
        line: u64::from(finding.line.unwrap_or(1)),
        column: u64::from(finding.column.unwrap_or(1)),
        message: finding.message,
        remediation: finding.remediation.unwrap_or_default(),
    }
}

fn lifecycle_error(error: LifecycleError) -> String {
    match error {
        LifecycleError::StaleSuppression {
            code,
            path,
            symbol: Some(symbol),
            ..
        } => format!("stale rule exception suppresses no finding: {code} at {path} ({symbol})"),
        LifecycleError::StaleSuppression { code, path, .. } => {
            format!("stale rule exception suppresses no finding: {code} at {path}")
        }
        other => format!("invalid rules lifecycle: {other}"),
    }
}
