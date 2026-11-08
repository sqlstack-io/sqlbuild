use crate::models::{
    DeclarationKind, EvaluateRequest, Fault, Model, ResourceKind, RuleMetadata, RulesConfig,
    ScopeResource, SqlScenarioFact, SqlTestCteFact, SqlTestFact, SqlTestMode,
};
use crate::query_facts::main::facts_available::facts_available;
use crate::query_facts::main::model_shape::model_shape;
use crate::query_facts::main::parse_fixture::parse_fixture;
use crate::query_facts::main::stub_call::stub_call;
use crate::query_facts::models::{CallKind, ParsedFixture};
use crate::rules::_helpers::evaluation::{path_fault, rules_unavailable};
use crate::rules::models::ProjectEvaluationRequest;
use serde::{Deserialize, Serialize};
use sqlbuild_analysis::compiler::main::sql_test_fixture_facts::fixture_facts;
use sqlquack_syntax::Expression;
use sqlquack_syntax::expressions::names::models::Literal;
use sqlquack_syntax::expressions::queries::models::Select;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::rc::Rc;

use crate::constants::NATIVE_BUILD_IDENTITY;

const UNIT_ROOT: &str = "tests/unit";
const SCENARIO_ROOT: &str = "tests/scenarios";
const EMPTY_INPUT_RULE_CODE: &str = "SQBRTEST203";
const ALLOWED_TESTS_OPTION: &str = "allowed_tests";
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME as PROJECT_CONFIG_PATH;

pub(crate) fn evaluate_project(
    evaluation: ProjectEvaluationRequest<'_>,
) -> Result<Vec<Fault>, String> {
    let resources: &[ScopeResource] = &evaluation.request.scope_index.resources;
    with_scope_lookup(resources, || evaluated_project(evaluation))
}

fn evaluated_project(evaluation: ProjectEvaluationRequest<'_>) -> Result<Vec<Fault>, String> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut tests: Vec<&SqlTestFact> = evaluation.request.sql_tests.iter().collect();
    tests.sort_by(|left, right| {
        (&left.source_path, left.block_index).cmp(&(&right.source_path, right.block_index))
    });
    let mut scenarios: Vec<&SqlScenarioFact> = evaluation.request.sql_scenarios.iter().collect();
    scenarios.sort_by(|left, right| left.source_path.cmp(&right.source_path));
    let mut fixture_eligibility: HashMap<&str, Result<(), String>> = HashMap::new();

    let fixture_rules: Vec<&RuleMetadata> = evaluation
        .selected
        .values()
        .copied()
        .filter(|rule| {
            matches!(
                rule.code.as_str(),
                "SQBRTEST201" | "SQBRTEST202" | EMPTY_INPUT_RULE_CODE
            )
        })
        .collect();
    let dialect: &str = &evaluation.request.dialect;
    if !fixture_rules.is_empty() && !facts_available(dialect) {
        let paths: BTreeSet<&str> = tests
            .iter()
            .filter(|test| reads_fixtures(test))
            .map(|test| test.source_path.as_str())
            .collect();
        let codes: Vec<&str> = fixture_rules
            .iter()
            .map(|rule| rule.code.as_str())
            .collect();
        for path in paths {
            faults.push(rules_unavailable(path, codes.iter().copied(), dialect));
        }
    }
    for rule in fixture_rules.iter().filter(|_| facts_available(dialect)) {
        for test in &tests {
            if !reads_fixtures(test) {
                continue;
            }
            for cte in test
                .authored_ctes
                .iter()
                .filter(|cte| fixture_facts(&cte.name, &cte.sql).mock)
                .chain(test.expected_ctes.iter())
                .chain(test.assertion_ctes.iter())
            {
                if fixture_facts(&cte.name, &cte.sql).empty_fixture_marker {
                    continue;
                }
                if let Err(reason) = fixture_eligibility.entry(&cte.sql).or_insert_with(|| {
                    match fixture_summary(&cte.sql, &evaluation.request.dialect)
                        .error
                        .clone()
                    {
                        Some(error) => Err(error),
                        None => Ok(()),
                    }
                }) {
                    let mut fault = path_fault(
                        rule,
                        &test.source_path,
                        format!(
                            "Rule {} could not be evaluated for test block {}: {reason}",
                            rule.code, test.block_index
                        ),
                        "Use supported fixture SQL or explicitly ignore the affected Rule."
                            .to_owned(),
                    );
                    fault.unevaluated = true;
                    faults.push(fault);
                    break;
                }
            }
        }
    }

    if let Some(rule) = evaluation.selected.get("SQBRTEST101") {
        faults.extend(canonical_roots(rule, &tests, &scenarios));
    }
    if let Some(rule) = evaluation.selected.get("SQBRTEST102") {
        faults.extend(filenames(rule, &evaluation, &tests, &scenarios));
    }
    if let Some(rule) = evaluation.selected.get("SQBRTEST103") {
        faults.extend(mirroring(rule, &evaluation, &tests));
    }
    if let Some(rule) = evaluation.selected.get("SQBRTEST104") {
        faults.extend(structured_names(rule, &evaluation, &tests));
    }
    if let Some(rule) = evaluation.selected.get(EMPTY_INPUT_RULE_CODE) {
        faults.extend(empty_input_only_faults(
            rule,
            &evaluation.request.config,
            &tests,
        ));
    }
    Ok(faults)
}

/// Whether the fixture rules read `test`'s fixture SQL: a model test without macro mocks or query overrides.
fn reads_fixtures(test: &SqlTestFact) -> bool {
    matches!(test.mode, SqlTestMode::Model)
        && !test.has_macro_mocks
        && !test.has_model_query_overrides
}

fn canonical_roots(
    rule: &RuleMetadata,
    tests: &[&SqlTestFact],
    scenarios: &[&SqlScenarioFact],
) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    for test in tests {
        if test.ownership_root != UNIT_ROOT || !is_beneath(&test.source_path, UNIT_ROOT) {
            faults.push(path_fault(
                rule,
                &test.source_path,
                format!(
                    "unit test block {} is outside the canonical {UNIT_ROOT}/ root",
                    test.block_index
                ),
                format!("Move this unit test beneath {UNIT_ROOT}/."),
            ));
        }
    }
    for scenario in scenarios {
        if scenario.ownership_root != SCENARIO_ROOT
            || !is_beneath(&scenario.source_path, SCENARIO_ROOT)
        {
            faults.push(path_fault(
                rule,
                &scenario.source_path,
                format!("scenario is outside the canonical {SCENARIO_ROOT}/ root"),
                format!("Move this scenario beneath {SCENARIO_ROOT}/."),
            ));
        }
    }
    faults
}

fn filenames(
    rule: &RuleMetadata,
    evaluation: &ProjectEvaluationRequest<'_>,
    tests: &[&SqlTestFact],
    scenarios: &[&SqlScenarioFact],
) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    for test in tests {
        let stem = file_stem(&test.source_path);
        let allowed = allowed_subjects(evaluation, test);
        let valid = stem.strip_prefix("test_").is_some_and(valid_unit_filename);
        let shares_file: bool = tests.iter().any(|other| {
            other.source_path == test.source_path && other.block_index != test.block_index
        });
        let behavior_matches = shares_file
            || test.explicit_name.as_deref().is_none_or(|name| {
                filename_behavior(stem, &allowed)
                    .zip(structured_name(name, &allowed).map(|(_, behavior)| behavior))
                    .is_none_or(|(slug, behavior)| behavior_slug_matches(slug, behavior))
            });
        if !valid || !behavior_matches {
            faults.push(path_fault(
                rule,
                &test.source_path,
                format!(
                    "unit test block {} filename does not follow test_<subject>__<behavior>.sql",
                    test.block_index
                ),
                test.explicit_name
                    .as_deref()
                    .and_then(|name| structured_name(name, &allowed))
                    .map_or_else(
                        || "Rename this file to test_<subject>__<behavior>.sql.".to_owned(),
                        |(subject, behavior)| {
                            format!(
                                "Rename this file to test_{}__{}.sql.",
                                slug(subject),
                                slug(behavior)
                            )
                        },
                    ),
            ));
        }
    }
    for scenario in scenarios {
        if !valid_filename_parts(file_stem(&scenario.source_path)) {
            faults.push(path_fault(
                rule,
                &scenario.source_path,
                "scenario filename does not follow <subject>__<behavior>.sql".into(),
                "Rename this file to <business subject>__<behavior>.sql.".into(),
            ));
        }
    }
    faults
}

fn mirroring(
    rule: &RuleMetadata,
    evaluation: &ProjectEvaluationRequest<'_>,
    tests: &[&SqlTestFact],
) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    for test in tests {
        let Some(expected_parent) = expected_parent(evaluation, test) else {
            continue;
        };
        let actual_parent = Path::new(&test.source_path)
            .parent()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if actual_parent != expected_parent {
            faults.push(path_fault(
                rule,
                &test.source_path,
                format!(
                    "unit test block {} resolves to resources mirrored by {expected_parent}/",
                    test.block_index
                ),
                format!("Move this test file beneath {expected_parent}/."),
            ));
        }
    }
    faults
}

fn structured_names(
    rule: &RuleMetadata,
    evaluation: &ProjectEvaluationRequest<'_>,
    tests: &[&SqlTestFact],
) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    for test in tests {
        let Some(name) = test.explicit_name.as_deref() else {
            faults.push(name_fault(
                rule,
                test,
                "the TEST block has no explicit name",
                None,
            ));
            continue;
        };
        let allowed = allowed_subjects(evaluation, test);
        let Some((subject, behavior)) = structured_name(name, &allowed) else {
            faults.push(name_fault(
                rule,
                test,
                "the TEST name must contain a nonempty subject and behavior separated by '__'",
                None,
            ));
            continue;
        };
        if generic(subject) || generic(behavior) {
            faults.push(name_fault(
                rule,
                test,
                "the TEST name uses a generic subject or behavior",
                None,
            ));
            continue;
        }
        if !allowed.is_empty() && !subject_matches(subject, &allowed) {
            faults.push(name_fault(
                rule,
                test,
                &format!(
                    "the TEST subject '{}' does not identify its resolved target; expected {}",
                    subject,
                    allowed.join(" or ")
                ),
                allowed.first().map(String::as_str),
            ));
        }
    }
    faults
}

fn expected_parent(
    evaluation: &ProjectEvaluationRequest<'_>,
    test: &SqlTestFact,
) -> Option<String> {
    match test.mode {
        SqlTestMode::Model => {
            let mut directories: Vec<Vec<String>> = Vec::new();
            for name in &test.target_model_names {
                let resource = scope_resource(evaluation, ResourceKind::Model, name)?;
                directories.push(relative_parent(resource)?);
            }
            if directories.is_empty() {
                return None;
            }
            let common = common_components(&directories);
            Some(if common.is_empty() {
                format!(
                    "{UNIT_ROOT}/{}",
                    evaluation.request.config.sql_tests.pipeline_directory
                )
            } else {
                format!("{UNIT_ROOT}/{}", common.join("/"))
            })
        }
        SqlTestMode::Macro => direct_expected_parent(&macro_parent_components(evaluation, test)?),
        SqlTestMode::Udf | SqlTestMode::TableFn => {
            direct_expected_parent(&function_parent_components(evaluation, test)?)
        }
    }
}

fn macro_parent_components(
    evaluation: &ProjectEvaluationRequest<'_>,
    test: &SqlTestFact,
) -> Option<Vec<Vec<String>>> {
    let mut parents: Vec<Vec<String>> = Vec::new();
    for resource in &test.tested_resources {
        let declaration = evaluation
            .request
            .scope_index
            .declarations
            .iter()
            .find(|item| {
                matches!(item.kind, DeclarationKind::Macro) && item.name == resource.name
            })?;
        parents.push(match &declaration.owning_path {
            Some(owner) => std::iter::once("macros".to_owned())
                .chain(owner.split('/').map(str::to_owned))
                .collect(),
            None => direct_parent_components(&declaration.path, &declaration.ownership_root)?,
        });
    }
    Some(parents)
}

fn function_parent_components(
    evaluation: &ProjectEvaluationRequest<'_>,
    test: &SqlTestFact,
) -> Option<Vec<Vec<String>>> {
    let mut parents: Vec<Vec<String>> = Vec::new();
    for resource in &test.tested_resources {
        let fact = scope_resource(evaluation, ResourceKind::Function, &resource.name)?;
        parents.push(direct_parent_components(&fact.path, &fact.ownership_root)?);
    }
    Some(parents)
}

fn direct_parent_components(path: &str, root: &str) -> Option<Vec<String>> {
    let path: Vec<&str> = path.split('/').collect();
    let root: Vec<&str> = root.split('/').collect();
    if path.len() <= root.len() || path.get(..root.len()) != Some(root.as_slice()) {
        return None;
    }
    Some(
        path[..path.len() - 1]
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    )
}

fn direct_expected_parent(parents: &[Vec<String>]) -> Option<String> {
    let common = common_components(parents);
    (!common.is_empty()).then(|| format!("{UNIT_ROOT}/{}", common.join("/")))
}

fn allowed_subjects(evaluation: &ProjectEvaluationRequest<'_>, test: &SqlTestFact) -> Vec<String> {
    if !matches!(test.mode, SqlTestMode::Model) {
        return test
            .tested_resources
            .iter()
            .map(|resource| resource.name.clone())
            .collect();
    }
    if test.target_model_names.len() == 1 {
        return test.target_model_names.clone();
    }
    if test.target_model_names.len() > 1 {
        let mut directories: Vec<Vec<String>> = Vec::new();
        for name in &test.target_model_names {
            let Some(resource) = scope_resource(evaluation, ResourceKind::Model, name) else {
                return Vec::new();
            };
            let Some(parent) = relative_parent(resource) else {
                return Vec::new();
            };
            directories.push(parent);
        }
        let common = common_components(&directories);
        return common
            .last()
            .map_or_else(|| vec!["pipeline".into()], |domain| vec![domain.clone()]);
    }
    Vec::new()
}

/// Each `(is a model, name)`'s first scope resource index, for the resources it was built from.
type ScopeLookup = (usize, HashMap<(bool, String), usize>);

thread_local! {
    static SCOPE_LOOKUP: RefCell<Option<ScopeLookup>> = const { RefCell::new(None) };
}

/// Run `body` with the scope resources indexed by kind and name on this thread.
fn with_scope_lookup<T>(resources: &[ScopeResource], body: impl FnOnce() -> T) -> T {
    let mut index: HashMap<(bool, String), usize> = HashMap::new();
    for (position, item) in resources.iter().enumerate() {
        let key: Option<bool> = match item.kind {
            ResourceKind::Model => Some(true),
            ResourceKind::Function => Some(false),
            _ => None,
        };
        if let Some(model) = key {
            index.entry((model, item.name.clone())).or_insert(position);
        }
    }
    let outer = SCOPE_LOOKUP.with(|slot| slot.borrow_mut().replace((resources.len(), index)));
    let result: T = body();
    SCOPE_LOOKUP.with(|slot| *slot.borrow_mut() = outer);
    result
}

fn scope_resource<'a>(
    evaluation: &'a ProjectEvaluationRequest<'_>,
    kind: ResourceKind,
    name: &str,
) -> Option<&'a ScopeResource> {
    let resources: &'a [ScopeResource] = &evaluation.request.scope_index.resources;
    let model: Option<bool> = match kind {
        ResourceKind::Model => Some(true),
        ResourceKind::Function => Some(false),
        _ => None,
    };
    let indexed: Option<Option<usize>> = SCOPE_LOOKUP.with(|slot| {
        let slot = slot.borrow();
        let (count, index) = slot.as_ref()?;
        if *count != resources.len() {
            return None;
        }
        Some(model.and_then(|model| index.get(&(model, name.to_owned())).copied()))
    });
    match indexed {
        Some(position) => position.map(|position| &resources[position]),
        None => resources
            .iter()
            .find(|item| same_resource_kind(&item.kind, &kind) && item.name == name),
    }
}

fn same_resource_kind(left: &ResourceKind, right: &ResourceKind) -> bool {
    matches!(
        (left, right),
        (ResourceKind::Model, ResourceKind::Model)
            | (ResourceKind::Function, ResourceKind::Function)
    )
}

fn relative_parent(resource: &ScopeResource) -> Option<Vec<String>> {
    let path: Vec<&str> = resource.path.split('/').collect();
    let root: Vec<&str> = resource.ownership_root.split('/').collect();
    if path.len() <= root.len() || path.get(..root.len()) != Some(root.as_slice()) {
        return None;
    }
    Some(
        path[root.len()..path.len() - 1]
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    )
}

fn common_components(values: &[Vec<String>]) -> Vec<String> {
    let Some(first) = values.first() else {
        return Vec::new();
    };
    let mut common: Vec<String> = Vec::new();
    for (index, value) in first.iter().enumerate() {
        let mut shared = true;
        for candidate in values {
            if candidate.get(index) != Some(value) {
                shared = false;
                break;
            }
        }
        if !shared {
            break;
        }
        common.push(value.clone());
    }
    common
}

fn name_fault(
    rule: &RuleMetadata,
    test: &SqlTestFact,
    detail: &str,
    proposed_subject: Option<&str>,
) -> Fault {
    path_fault(
        rule,
        &test.source_path,
        format!("unit test block {} {detail}", test.block_index),
        format!(
            "Add name \"{}__<expected_behavior>\" to this TEST header.",
            proposed_subject.unwrap_or("<resolved_subject>")
        ),
    )
}

fn is_beneath(path: &str, root: &str) -> bool {
    path.strip_prefix(root)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn file_stem(path: &str) -> &str {
    let filename = path.rsplit('/').next().unwrap_or(path);
    filename.strip_suffix(".sql").unwrap_or(filename)
}

fn valid_filename_parts(value: &str) -> bool {
    value.contains("__") && canonical_identity(value)
}

fn valid_unit_filename(value: &str) -> bool {
    canonical_identity(value)
}

fn canonical_identity(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_lowercase())
        && value
            .chars()
            .last()
            .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn filename_behavior<'a>(stem: &'a str, allowed_subjects: &[String]) -> Option<&'a str> {
    structured_name(stem.strip_prefix("test_").unwrap_or(stem), allowed_subjects)
        .map(|(_, behavior)| behavior)
}

fn structured_name<'a>(name: &'a str, allowed_subjects: &[String]) -> Option<(&'a str, &'a str)> {
    let matched_subject_length = longest_subject_prefix(name, allowed_subjects);
    let (subject, behavior) = matched_subject_length.map_or_else(
        || name.split_once("__"),
        |length| Some((&name[..length], &name[length + 2..])),
    )?;
    let subject = subject.trim();
    let behavior = behavior.trim();
    (!subject.is_empty() && !behavior.is_empty()).then_some((subject, behavior))
}

fn longest_subject_prefix(name: &str, allowed_subjects: &[String]) -> Option<usize> {
    let mut longest: Option<usize> = None;
    for subject in allowed_subjects {
        let Some(suffix) = name.strip_prefix(subject) else {
            continue;
        };
        let Some(behavior) = suffix.strip_prefix("__") else {
            continue;
        };
        if behavior.is_empty() {
            continue;
        }
        if longest.is_none_or(|length| subject.len() > length) {
            longest = Some(subject.len());
        }
    }
    longest
}

fn behavior_slug_matches(filename: &str, behavior: &str) -> bool {
    let behavior = slug(behavior);
    behavior == filename || behavior.starts_with(&format!("{filename}_"))
}

fn subject_matches(subject: &str, allowed: &[String]) -> bool {
    let normalized = slug(subject);
    for value in allowed {
        let candidate = slug(value);
        if normalized == candidate {
            return true;
        }
        if candidate == crate::constants::PIPELINE_SUBJECT
            && normalized
                .split('_')
                .any(|component| component == crate::constants::PIPELINE_SUBJECT)
        {
            return true;
        }
    }
    false
}

fn generic(value: &str) -> bool {
    let normalized = slug(value);
    matches!(
        normalized.as_str(),
        crate::constants::GENERIC_TEST_NAME
            | crate::constants::GENERIC_WORKS_NAME
            | crate::constants::GENERIC_BASIC_NAME
            | crate::constants::GENERIC_SCENARIO_NAME
            | crate::constants::GENERIC_CASE_NAME
    ) || normalized
        .strip_prefix("case_")
        .is_some_and(|suffix| suffix.chars().all(|character| character.is_ascii_digit()))
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            if separator && !output.is_empty() {
                output.push('_');
            }
            output.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    output
}

pub(crate) fn annotate_empty_input_only_tests(mut request: EvaluateRequest) -> EvaluateRequest {
    for test in &mut request.sql_tests {
        test.empty_input_only = is_empty_input_only(test, &request.dialect);
    }
    let allowed = allowed_tests(&request.config);
    let mut excluded: BTreeMap<String, u32> = BTreeMap::new();
    for test in &request.sql_tests {
        if !test.empty_input_only || is_allowlisted(test, &allowed) {
            continue;
        }
        for name in &test.target_model_names {
            *excluded.entry(name.clone()).or_default() += 1;
        }
    }
    for model in &mut request.models {
        model.empty_input_only_test_count = excluded.get(&model.name).copied().unwrap_or(0);
    }
    request
}

pub(crate) fn minimum_tests_shortfall(model: &Model, minimum: u32) -> Option<String> {
    let excluded = model.empty_input_only_test_count;
    let counted = model.targeting_test_count.saturating_sub(excluded);
    if counted >= minimum {
        return None;
    }
    let mut message = format!(
        "model {:?} has {counted} tests; {minimum} required",
        model.name
    );
    if excluded > 0 {
        let noun = if excluded == 1 { "test" } else { "tests" };
        message.push_str(&format!(
            " ({excluded} empty-input-only {noun} not counted; see {EMPTY_INPUT_RULE_CODE})"
        ));
    }
    Some(message)
}

fn empty_input_only_faults(
    rule: &RuleMetadata,
    config: &RulesConfig,
    tests: &[&SqlTestFact],
) -> Vec<Fault> {
    let allowed = allowed_tests(config);
    let mut faults: Vec<Fault> = Vec::new();
    for test in tests {
        if test.empty_input_only && !is_allowlisted(test, &allowed) {
            faults.push(path_fault(
                rule,
                &test.source_path,
                format!(
                    "unit test block {} ({:?}) mocks only empty inputs and asserts only that no rows are produced",
                    test.block_index, test.name
                ),
                rule.remediation.clone(),
            ));
        }
    }
    for entry in &allowed {
        if !tests.iter().any(|test| test_named(test, entry)) {
            faults.push(path_fault(
                rule,
                PROJECT_CONFIG_PATH,
                format!("stale {ALLOWED_TESTS_OPTION} entry {entry:?} names no existing SQL test"),
                format!(
                    "Remove {entry:?} from [rules.rule_options.{EMPTY_INPUT_RULE_CODE}] {ALLOWED_TESTS_OPTION}, or correct it to the name of an existing test."
                ),
            ));
        }
    }
    faults
}

fn allowed_tests(config: &RulesConfig) -> Vec<String> {
    config.rule_option_strings(EMPTY_INPUT_RULE_CODE, ALLOWED_TESTS_OPTION)
}

fn is_allowlisted(test: &SqlTestFact, allowed: &[String]) -> bool {
    allowed.iter().any(|entry| test_named(test, entry))
}

fn test_named(test: &SqlTestFact, entry: &str) -> bool {
    test.name == entry || (test.case_name.is_some() && test.parent_name.as_deref() == Some(entry))
}

fn is_empty_input_only(test: &SqlTestFact, dialect: &str) -> bool {
    if !matches!(test.mode, SqlTestMode::Model)
        || test.has_macro_mocks
        || test.has_model_query_overrides
        || (test.expected_ctes.is_empty() && test.assertion_ctes.is_empty())
    {
        return false;
    }
    let mut mocks = test
        .authored_ctes
        .iter()
        .filter(|cte| fixture_facts(&cte.name, &cte.sql).mock)
        .peekable();
    mocks.peek().is_some()
        && mocks.all(|cte| empty_relation(cte, dialect))
        && test
            .expected_ctes
            .iter()
            .all(|cte| empty_relation(cte, dialect))
        && test
            .assertion_ctes
            .iter()
            .all(|cte| bare_row_existence(&cte.sql, &test.target_model_names, dialect))
}

fn empty_relation(cte: &SqlTestCteFact, dialect: &str) -> bool {
    if fixture_facts(&cte.name, &cte.sql).empty_fixture_marker {
        return true;
    }
    fixture_summary(&cte.sql, dialect).empty
}

/// Whether a parsed fixture selects no rows.
fn selects_no_rows(fixture: &ParsedFixture) -> bool {
    let Expression::Select(select) = &fixture.statement else {
        return false;
    };
    select.with.is_none()
        && (limited_to_zero_rows(select)
            || (filtered_to_zero_rows(select, fixture) && !fixture.order_by_calls_function()))
}

/// What the SQL-test rules read of one parsed fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FixtureSummary {
    /// Why the fixture does not parse, when it does not.
    error: Option<String>,
    /// Whether it selects no rows.
    empty: bool,
    /// The model a bare row-existence assertion reads, when the fixture is one.
    row_existence_target: Option<String>,
}

/// Fixture summaries by digest of `(dialect, SQL)`, shared by the rules of one evaluation.
#[derive(Default)]
struct FixtureMemo {
    summaries: HashMap<String, Rc<FixtureSummary>>,
    /// The digests this evaluation read, which are the ones saved again.
    used: std::collections::HashSet<String>,
    /// Whether any summary was computed rather than loaded.
    computed: bool,
}

thread_local! {
    static FIXTURE_MEMO: RefCell<Option<FixtureMemo>> = const { RefCell::new(None) };
}

/// The fixture summaries stored at `path`; missing or unreadable ones are summarised again.
fn stored_summaries(path: &Path) -> Option<StoredSummaries> {
    let bytes: Vec<u8> = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_unreadable) => return None,
    };
    let Ok(stored) = serde_json::from_slice::<StoredSummaries>(&bytes) else {
        return None;
    };
    Some(stored)
}

/// The stored fixture summaries, kept only for one native build.
#[derive(Default, Serialize, Deserialize)]
struct StoredSummaries {
    build: String,
    summaries: BTreeMap<String, FixtureSummary>,
}

/// Run `body` summarising each fixture once, from and, when `body` says so, back to `store`.
pub(crate) fn with_fixture_memo<T>(store: Option<&Path>, body: impl FnOnce() -> (T, bool)) -> T {
    let loaded: BTreeMap<String, FixtureSummary> = store
        .and_then(stored_summaries)
        .filter(|stored| stored.build == NATIVE_BUILD_IDENTITY)
        .map(|stored| stored.summaries)
        .unwrap_or_default();
    let loaded_count: usize = loaded.len();
    let memo: FixtureMemo = FixtureMemo {
        summaries: loaded
            .into_iter()
            .map(|(key, summary)| (key, Rc::new(summary)))
            .collect(),
        used: std::collections::HashSet::new(),
        computed: false,
    };
    let outer: Option<FixtureMemo> = FIXTURE_MEMO.with(|slot| slot.borrow_mut().replace(memo));
    let (result, write): (T, bool) = body();
    let memo: Option<FixtureMemo> = FIXTURE_MEMO.with(|slot| {
        let mut slot = slot.borrow_mut();
        let memo = slot.take();
        *slot = outer;
        memo
    });
    if let (Some(path), Some(memo), true) = (store, memo, write)
        && !memo.used.is_empty()
        && (memo.computed || memo.used.len() != loaded_count)
    {
        let FixtureMemo {
            summaries, used, ..
        } = memo;
        let stored: StoredSummaries = StoredSummaries {
            build: NATIVE_BUILD_IDENTITY.to_owned(),
            summaries: summaries
                .into_iter()
                .filter(|(key, _)| used.contains(key))
                .map(|(key, summary)| (key, summary.as_ref().clone()))
                .collect(),
        };
        let _ = write_summaries(path, &stored);
    }
    result
}

fn write_summaries(path: &Path, stored: &StoredSummaries) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, serde_json::to_vec(stored)?)?;
    std::fs::rename(&temporary, path)
}

fn summarised(sql: &str, dialect: &str) -> FixtureSummary {
    let parsed: Result<ParsedFixture, String> = model_shape(sql, dialect)
        .and_then(|shape| parse_fixture(shape, dialect))
        .map_err(|error| format!("could not parse SQL-test fixture: {error}"));
    match parsed {
        Err(error) => FixtureSummary {
            error: Some(error),
            empty: false,
            row_existence_target: None,
        },
        Ok(fixture) => FixtureSummary {
            error: None,
            empty: selects_no_rows(&fixture),
            row_existence_target: row_existence_target(&fixture),
        },
    }
}

fn fixture_summary(sql: &str, dialect: &str) -> Rc<FixtureSummary> {
    FIXTURE_MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        let Some(memo) = memo.as_mut() else {
            return Rc::new(summarised(sql, dialect));
        };
        let mut hasher: blake3::Hasher = blake3::Hasher::new();
        hasher.update(dialect.as_bytes());
        hasher.update(b"\0");
        hasher.update(sql.as_bytes());
        let key: String = hasher.finalize().to_hex().to_string();
        if let Some(summary) = memo.summaries.get(&key) {
            let summary: Rc<FixtureSummary> = Rc::clone(summary);
            memo.used.insert(key);
            return summary;
        }
        let summary: Rc<FixtureSummary> = Rc::new(summarised(sql, dialect));
        memo.summaries.insert(key.clone(), Rc::clone(&summary));
        memo.used.insert(key);
        memo.computed = true;
        summary
    })
}

fn limited_to_zero_rows(select: &Select) -> bool {
    select
        .limit
        .as_ref()
        .is_some_and(|limit| numeric_literal(&limit.this) == Some(0.0))
}

fn filtered_to_zero_rows(select: &Select, fixture: &ParsedFixture) -> bool {
    select.having.is_none()
        && select.group_by.is_none()
        && !fixture.select_calls_function()
        && select
            .where_clause
            .as_ref()
            .is_some_and(|clause| constant_false(&clause.this))
}

fn unwrap_paren(mut expression: &Expression) -> &Expression {
    while let Expression::Paren(paren) = expression {
        expression = &paren.this;
    }
    expression
}

fn constant_false(predicate: &Expression) -> bool {
    match unwrap_paren(predicate) {
        Expression::Boolean(value) => !value.value,
        Expression::Eq(comparison) => numeric_literal(&comparison.left)
            .zip(numeric_literal(&comparison.right))
            .is_some_and(|(left, right)| left != right),
        _ => false,
    }
}

fn numeric_literal(expression: &Expression) -> Option<f64> {
    let Expression::Literal(literal) = unwrap_paren(expression) else {
        return None;
    };
    let Literal::Number(number) = literal.as_ref() else {
        return None;
    };
    let Ok(parsed) = number.parse::<f64>() else {
        return None;
    };
    Some(parsed)
}

fn bare_row_existence(sql: &str, targets: &[String], dialect: &str) -> bool {
    fixture_summary(sql, dialect)
        .row_existence_target
        .as_ref()
        .is_some_and(|target| targets.contains(target))
}

/// The model a fixture reads when it is a bare existence check of one model's rows.
fn row_existence_target(fixture: &ParsedFixture) -> Option<String> {
    let Expression::Select(select) = &fixture.statement else {
        return None;
    };
    let [source] = select
        .from
        .as_ref()
        .map_or(&[][..], |from| from.expressions.as_slice())
    else {
        return None;
    };
    let unfiltered = select.where_clause.is_none()
        && select.prewhere.is_none()
        && select.having.is_none()
        && select.qualify.is_none()
        && select.connect.is_none()
        && select.lateral_views.is_empty()
        && select.group_by.is_none();
    let bare: bool = select.with.is_none()
        && unfiltered
        && select.joins.is_empty()
        && !fixture.projection_calls_function()
        && !fixture.order_by_calls_function()
        && select.limit.is_none()
        && select.offset.is_none()
        && select.fetch.is_none()
        && fixture.query_count() == 1;
    if !bare {
        return None;
    }
    reference_target(source, fixture)
}

fn reference_target(source: &Expression, fixture: &ParsedFixture) -> Option<String> {
    let Expression::Table(table) = source else {
        return None;
    };
    if table.schema.is_some() || table.catalog.is_some() {
        return None;
    }
    let (CallKind::Ref, index) = stub_call(&table.name.name)? else {
        return None;
    };
    fixture.calls.get(index)?.target.clone()
}
