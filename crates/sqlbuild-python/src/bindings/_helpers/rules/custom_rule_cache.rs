//! Custom-rule result cache identities, read-set lookup, result merging and the cache file.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_rules::custom_rule_cache::main::custom_rule_cache_text::custom_rule_cache_text;
use sqlbuild_rules::custom_rule_cache::main::lookup_custom_rule_cache::lookup_custom_rule_cache;
use sqlbuild_rules::custom_rule_cache::main::ordered_custom_findings::ordered_custom_findings;
use sqlbuild_rules::custom_rule_cache::main::parse_custom_rule_cache::parse_custom_rule_cache;
use sqlbuild_rules::custom_rule_cache::main::pending_readsets::pending_readsets;
use sqlbuild_rules::custom_rule_cache::main::rule_identity::rule_identity;
use sqlbuild_rules::custom_rule_cache::main::rule_subjects::rule_subjects;
use sqlbuild_rules::custom_rule_cache::main::store_custom_results::store_custom_results;
use sqlbuild_rules::custom_rule_cache::main::validate_custom_findings::validate_custom_findings;
use sqlbuild_rules::custom_rule_cache::models::{
    CustomCacheLookup, CustomEvaluation, CustomRule, ModelSubjectInput, RuleIdentityInput,
    SubjectsInput,
};
use sqlbuild_rules::custom_rule_cache::types::{ReadSet, StoredRules, SubjectResults};

use crate::bindings::_helpers::compiled_project::values::plain_value;
use crate::bindings::_helpers::rules::findings::{FindingClasses, invalid_finding};
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// One custom rule's cache identity from its metadata, source fingerprint and settings.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn custom_rule_identity(request: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("custom_rule_identity", request)?;
    Ok(rule_identity(&RuleIdentityInput {
        code_identity: request.get("code_identity")?.extract()?,
        runtime_version: request.get("runtime_version")?.extract()?,
        code: request.get("code")?.extract()?,
        family: request.get("family")?.extract()?,
        slug: request.get("slug")?.extract()?,
        message: request.get("message")?.extract()?,
        remediation: request.get("remediation")?.extract()?,
        subject: request.get("subject")?.extract()?,
        implementation_fingerprint: request.get("implementation_fingerprint")?.extract()?,
        config_json: plain_value(&request.get("config")?)?.to_string(),
        dialect: request.get("dialect")?.extract()?,
        project_dir: request.get("project_dir")?.extract()?,
    }))
}

fn rules_error(py: Python<'_>, message: String) -> PyResult<pyo3::PyErr> {
    let class: Bound<'_, PyAny> = error_class(py, ErrorId::Rules)?;
    Ok(pyo3::PyErr::from_value(class.call1((message,))?))
}

pub(crate) fn stored_rules(path: &std::path::Path) -> StoredRules {
    let Ok(text) = std::fs::read_to_string(path) else {
        return StoredRules::new();
    };
    parse_custom_rule_cache(&text)
}

/// One run's custom-rule cache: stored results, pending read sets and accounting.
pub(crate) struct CustomRuleCache {
    path: Option<PathBuf>,
    rules: Vec<CustomRule>,
    project: SubjectsInput,
    stored: StoredRules,
    pending: Vec<ReadSet>,
    results: SubjectResults,
    hits: usize,
    misses: usize,
    counts_by_code: BTreeMap<String, (usize, usize)>,
}

impl CustomRuleCache {
    /// Load the cache at `path` (`None` when caching is off) for `(code, identity, subject)` rules.
    pub(crate) fn new(
        path: Option<PathBuf>,
        rules: Vec<(String, Option<String>, Option<String>)>,
        target_name: Option<String>,
        models: Vec<(String, String, Option<String>)>,
    ) -> Self {
        let project = SubjectsInput {
            target_name,
            models: models
                .into_iter()
                .map(|(path, name, materialized)| ModelSubjectInput {
                    path,
                    name,
                    materialized,
                })
                .collect(),
        };
        let mut rules: Vec<CustomRule> = rules
            .into_iter()
            .map(|(code, identity, subject)| CustomRule {
                subjects: rule_subjects(subject.as_deref(), &project),
                code,
                identity,
                subject,
            })
            .collect();
        rules.sort_by(|left, right| left.code.cmp(&right.code));
        let stored: StoredRules = path.as_deref().map(stored_rules).unwrap_or_default();
        let pending: Vec<ReadSet> = pending_readsets(&stored, &rules);
        Self {
            path,
            rules,
            project,
            stored,
            pending,
            results: SubjectResults::new(),
            hits: 0,
            misses: 0,
            counts_by_code: BTreeMap::new(),
        }
    }

    /// Stored read sets whose current digest decides whether their results are reused.
    pub(crate) fn pending_readsets(&self) -> &[ReadSet] {
        &self.pending
    }

    /// The read set `code` last recorded for `subject` under its current identity, if any.
    pub(crate) fn stored_readset(&self, code: &str, subject: &str) -> Option<&ReadSet> {
        let rule: &CustomRule = self.rules.iter().find(|rule| rule.code == code)?;
        let cached = self.stored.get(code)?;
        if rule.identity.as_deref() != Some(cached.identity.as_str()) {
            return None;
        }
        cached.readsets.get(cached.entries.get(subject)?.readset)
    }

    /// Reuse results given each pending read set's digest; returns the subjects still to run.
    pub(crate) fn plan(
        &mut self,
        digests: Vec<Option<String>>,
    ) -> BTreeMap<String, Option<Vec<String>>> {
        if self.path.is_none() {
            self.misses = self.rules.len();
            self.counts_by_code = self
                .rules
                .iter()
                .map(|rule| (rule.code.clone(), (0, rule.subjects.len())))
                .collect();
            return self
                .rules
                .iter()
                .map(|rule| (rule.code.clone(), None))
                .collect();
        }
        let current: BTreeMap<ReadSet, String> = self
            .pending
            .iter()
            .zip(digests)
            .filter_map(|(readset, digest)| Some((readset.clone(), digest?)))
            .collect();
        let lookup: CustomCacheLookup =
            lookup_custom_rule_cache(&self.stored, &self.rules, &current);
        self.results.extend(lookup.hits);
        self.hits = lookup.hit_count;
        self.misses = lookup.miss_count;
        self.counts_by_code = lookup.counts_by_code;
        lookup.plan
    }

    /// Record evaluated results; with caching on, validate them, store them and write the file.
    pub(crate) fn record(
        &mut self,
        py: Python<'_>,
        evaluations: Vec<CustomEvaluation>,
    ) -> PyResult<()> {
        let Some(path) = self.path.clone() else {
            self.extend_results(evaluations);
            return Ok(());
        };
        validate_custom_findings(&self.rules, &self.project, &evaluations)
            .map_err(|error| rules_error(py, error.0).unwrap_or_else(|error| error))?;
        self.stored = store_custom_results(&self.stored, &self.rules, &evaluations);
        self.extend_results(evaluations);
        let text: String = custom_rule_cache_text(&self.stored, &self.rules);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary: PathBuf = path.with_extension(format!(
            "tmp-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, &path)?;
        Ok(())
    }

    /// Every finding in rule code, then subject order; a malformed payload is a `RulesError`.
    pub(crate) fn findings<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let classes = FindingClasses::new(py)?;
        ordered_custom_findings(&self.rules, &self.results)
            .iter()
            .map(|finding| {
                classes
                    .finding_of_payload(finding)?
                    .ok_or_else(|| invalid_finding(py))
            })
            .collect()
    }

    pub(crate) fn hits(&self) -> usize {
        self.hits
    }

    pub(crate) fn misses(&self) -> usize {
        self.misses
    }

    /// Each rule's reused and evaluated subject counts; every subject misses with caching off.
    pub(crate) fn counts_by_code(&self) -> &BTreeMap<String, (usize, usize)> {
        &self.counts_by_code
    }

    fn extend_results(&mut self, evaluations: Vec<CustomEvaluation>) {
        for evaluation in evaluations {
            self.results
                .insert((evaluation.code, evaluation.subject), evaluation.findings);
        }
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(custom_rule_identity, module)?)?;
    Ok(())
}
