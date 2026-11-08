use crate::configuration::main::setting_help::setting_help;
use crate::configuration::main::toml_string_array::toml_string_array;
use crate::constants::{
    DECLARATION_DOMAIN_COMPONENTS, ENFORCED_CONTRACT, REFERENCE_KIND, SOURCE_REFERENCE_KIND,
    VIEW_MATERIALIZATION,
};
use crate::constants::{
    MIN_CUSTOM_RULE_TEST_CASES_KEY, MODEL_NAME_LAYERS, PUBLISHED_READ_CODE, PUBLISHED_SHAPE_CODE,
    RULES_UNAVAILABLE_REMEDIATION,
};
use crate::lint_text::constants::LINT_UNAVAILABLE_CODE;
use crate::models::{Declaration, EvaluateRequest, Fault, Model, RuleMetadata, RulesConfig};
use crate::query_facts::main::dependency_texts::dependency_texts;
use crate::query_facts::main::facts_available::facts_available;
use crate::query_facts::main::model_shape::model_shape;
use crate::query_facts::main::query_facts::query_facts;
use crate::query_facts::models::{
    CallKind, ColumnFact, ComparisonFacts, ItemFacts, ModelCall, ModelSource, QueryFacts,
    RelationFacts, SelectFacts,
};
use crate::rules::_helpers::domain_layout::folder_layer_details;
use crate::rules::_helpers::{
    contract_name_types, dynamic_contracts, explicit_output_types, model_layers, numeric_decisions,
    sql_test_rules, typed_contract_columns,
};
use crate::rules::models::{
    FaultCollector, ModelEvaluationRequest, ProjectEvaluationRequest, ResolvedThresholdOverride,
};
use globset::{Glob, GlobSetBuilder};
use sqlbuild_core::identifier_text::main::is_lowercase_identifier::is_lowercase_identifier;
use sqlbuild_sqltext::sql_scan::main::quote_policy::quote_policy as rules_quote_policy;
use sqlbuild_sqltext::sql_scan::models::QuotePolicy;
use std::collections::{BTreeMap, BTreeSet, HashSet};

const MIXED_STAR_REMEDIATION: &str = "SELECT *, a, b mixes a passthrough star with derived columns. The * exemption permits a lone SELECT * only. Move a and b into an earlier CTE so they are computed upstream, leaving the final select a pure SELECT *. If you need those columns in the output, drop the * exemption and enumerate every column explicitly.";
const NUMERIC_VALUE_LIST_REMEDIATION: &str = "Declare the values as a list or set CONSTANT and compare through [NOT] IN @const(\"<name>\"); only -1, 0, and 1 are self-explanatory.";

#[derive(Clone)]
struct Position {
    line: u64,
    column: u64,
}

#[derive(Clone, Default)]
struct SourceContext {
    relations: BTreeSet<String>,
    sole_relation: bool,
}

pub(crate) struct ParsedModel<'a> {
    pub(crate) facts: &'a QueryFacts,
    pub(crate) source: ModelSource<'a>,
    pub(crate) model: &'a Model,
    classification: ModelClassification,
    authored_compact_sql: String,
    quote_policy: QuotePolicy,
}

impl ParsedModel<'_> {
    fn position(&self, offset: Option<usize>) -> Option<Position> {
        offset.map(|offset| {
            let (line, column) = self.source.line_column(offset);
            Position { line, column }
        })
    }
}

struct ModelClassification {
    authored_dependencies: Vec<String>,
    ctes: Vec<CteClassification>,
    passthrough: bool,
}

struct TestRuleEvaluation<'a> {
    parsed: &'a ParsedModel<'a>,
    config: &'a RulesConfig,
    threshold_overrides: &'a [ResolvedThresholdOverride],
    selected: &'a BTreeMap<String, &'a RuleMetadata>,
    faults: &'a FaultCollector,
}

struct CteClassification {
    name: String,
    dependency_import: bool,
    kind: CteKind,
}

enum CteKind {
    Logical,
    Import {
        dependencies: Vec<String>,
        rejections: Vec<ImportRejection>,
    },
}

enum ImportRejection {
    MultipleDependencies,
    AfterLogical,
    Transformation,
}

pub(crate) fn evaluate_model(request: ModelEvaluationRequest<'_>) -> Result<Vec<Fault>, String> {
    if request.model.sql_analysis_disabled {
        return Ok(Vec::new());
    }
    let model = request.model;
    let selected = request.selected;
    match stacker::grow(8 * 1024 * 1024, || evaluate_model_inner(request)) {
        Ok(faults) => Ok(faults),
        Err(reason) => Ok(selected.values().filter(|rule| requires_model_query(rule)).map(|rule| Fault {
            unevaluated: true,
            code: rule.code.clone(),
            path: model.relative_path.clone(),
            line: 1,
            column: 1,
            message: format!("Rule {} could not be evaluated: {reason}", rule.code),
            remediation: "Use supported SQL, disable SQL analysis, or explicitly ignore the affected Rule.".to_owned(),
        }).collect()),
    }
}

fn evaluate_model_inner(request: ModelEvaluationRequest<'_>) -> Result<Vec<Fault>, String> {
    let ModelEvaluationRequest {
        model,
        config,
        selected,
        request,
        threshold_overrides,
        shapes,
    } = request;
    let metadata = |code: &str| selected.get(code).copied();
    let faults = FaultCollector::default();
    if let Some(dynamic_faults) = dynamic_contracts::evaluate_without_query(model, config, selected)
    {
        return Ok(dynamic_faults);
    }
    if !requires_parsed_model(selected) {
        return Ok(faults.into_inner());
    }
    if !facts_available(&request.dialect) {
        let codes = selected
            .values()
            .filter(|rule| requires_model_query(rule))
            .map(|rule| rule.code.as_str());
        return Ok(vec![rules_unavailable(
            &model.relative_path,
            codes,
            &request.dialect,
        )]);
    }
    let unparsed =
        |error: &str| format!("could not parse {} for rules: {error}", model.relative_path);
    let shape =
        model_shape(&model.query_sql, &request.dialect).map_err(|error| unparsed(&error))?;
    let cell = shapes.entry(&request.dialect, &shape.sql);
    let facts = cell
        .get_or_init(|| query_facts(&shape.sql, &request.dialect))
        .as_ref()
        .map_err(|error| unparsed(error))?;
    let quote_policy = rules_quote_policy(&request.dialect);
    let source = ModelSource::new(
        &model.query_sql,
        &shape.offsets,
        (model.query_line_offset, model.query_column_offset),
    )
    .with_authored(&model.authored_sql, model.authored_query.as_ref());
    let classification = classify_model(facts, model, &shape.calls);
    let parsed = ParsedModel {
        facts,
        source,
        model,
        classification,
        authored_compact_sql: numeric_decisions::compact_sql(&model.authored_sql, quote_policy),
        quote_policy,
    };
    if let Some(rule) = metadata("SQBRMODEL101") {
        import_ctes(&parsed, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRMODEL102") {
        select_star(&parsed, config, rule, &faults)?;
    }
    if let Some(rule) = metadata("SQBRMODEL103") {
        view_marker(&parsed, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRMODEL104") {
        model_layers::explicit_model_schema(parsed.model, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRGRAPH102") {
        raw_qualified_tables(&parsed, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRPROJECT101") {
        name_grammar(&parsed, config, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRPROJECT102") {
        folder_layer(&parsed, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRPROJECT103") {
        source_token_rule(&parsed, config, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRPROJECT104") {
        reference_name_rule(&parsed, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRPROJECT105") {
        model_layers::name_folder_alignment(parsed.model, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRPROJECT106") {
        model_layers::name_schema_alignment(parsed.model, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRCONTRACT101") {
        contract_required(&parsed, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRCONTRACT106") {
        typed_contract_columns::evaluate(parsed.model, rule, &faults);
    }
    if let Some(rule) = metadata("SQBRCONTRACT105") {
        explicit_output_types::evaluate(&parsed, rule, &faults, &request.dialect);
    }
    evaluate_naming_rules(&parsed, config, selected, &faults);
    evaluate_literal_rules(&parsed, selected, &faults);
    evaluate_test_rules(TestRuleEvaluation {
        parsed: &parsed,
        config,
        threshold_overrides,
        selected,
        faults: &faults,
    });

    Ok(faults.into_inner())
}

fn fault(model: &Model, rule: &RuleMetadata, position: Option<&Position>) -> Fault {
    Fault {
        unevaluated: false,
        code: rule.code.clone(),
        path: model.relative_path.clone(),
        line: position.map_or(1, |value| value.line),
        column: position.map_or(1, |value| value.column),
        message: rule.message.clone(),
        remediation: rule.remediation.clone(),
    }
}

fn custom_fault_impl(
    model: &Model,
    rule: &RuleMetadata,
    position: Option<&Position>,
    details: (String, Option<String>),
) -> Fault {
    let (message, remediation) = details;
    let mut result = fault(model, rule, position);
    result.message = message;
    if let Some(value) = remediation {
        result.remediation = value;
    }
    result
}

macro_rules! custom_fault {
    ($model:expr, $rule:expr, $position:expr, $message:expr, $remediation:expr $(,)?) => {
        custom_fault_impl($model, $rule, $position, ($message, $remediation))
    };
}

pub(super) fn path_fault(
    rule: &RuleMetadata,
    path: &str,
    message: String,
    remediation: String,
) -> Fault {
    Fault {
        unevaluated: false,
        code: rule.code.clone(),
        path: path.into(),
        line: 1,
        column: 1,
        message,
        remediation,
    }
}

pub(crate) fn evaluate_project_rules(request: &ProjectEvaluationRequest<'_>) -> Vec<Fault> {
    let faults = FaultCollector::default();
    if let Some(rule) = request.selected.get("SQBRGRAPH101") {
        model_layers::internal_forward_refs(request, rule, &faults);
    }
    if let Some(rule) = request.selected.get(PUBLISHED_READ_CODE) {
        model_layers::same_domain_published_reads(request.request, rule, &faults);
    }
    if let Some(rule) = request.selected.get(PUBLISHED_SHAPE_CODE) {
        model_layers::published_name_shapes(request.request, rule, &faults);
    }
    if let Some(rule) = request.selected.get("SQBRDECLARATION201") {
        duplicate_enums(request.request, rule, &faults);
    }
    if let Some(rule) = request.selected.get("SQBRDECLARATION301") {
        declaration_domain_placement(request.request, &request.request.config, rule, &faults);
    }
    if let Some(rule) = request.selected.get("SQBRTEST301") {
        custom_rule_test_coverage(
            request.request,
            &request.request.config,
            (request.selected, rule),
            &faults,
        );
    }
    faults.into_inner()
}

/// One non-failing warning that `dialect` SQL is not parsed, so the built-in rules `codes` did not run on `path`.
pub(crate) fn rules_unavailable<'a>(
    path: &str,
    codes: impl Iterator<Item = &'a str>,
    dialect: &str,
) -> Fault {
    Fault {
        unevaluated: false,
        code: LINT_UNAVAILABLE_CODE.to_owned(),
        path: path.to_owned(),
        line: 1,
        column: 1,
        message: format!(
            "SQLBuild cannot parse {dialect} SQL for built-in rules yet, so these rules were not evaluated: {}",
            codes.collect::<Vec<_>>().join(", ")
        ),
        remediation: RULES_UNAVAILABLE_REMEDIATION.to_owned(),
    }
}

fn requires_parsed_model(selected: &BTreeMap<String, &RuleMetadata>) -> bool {
    selected.values().any(|rule| requires_model_query(rule))
}

fn requires_model_query(rule: &RuleMetadata) -> bool {
    !rule.custom
        && (rule.code.starts_with("SQBRMODEL")
            || rule.code.starts_with("SQBRCONTRACT")
            || matches!(
                rule.code.as_str(),
                "SQBRGRAPH102"
                    | "SQBRPROJECT101"
                    | "SQBRPROJECT102"
                    | "SQBRPROJECT103"
                    | "SQBRPROJECT104"
                    | "SQBRPROJECT105"
                    | "SQBRPROJECT106"
                    | "SQBRDECLARATION101"
                    | "SQBRDECLARATION102"
                    | "SQBRTEST201"
                    | "SQBRTEST202"
            ))
}

pub(super) fn plain_select(select: &SelectFacts, allow_star: bool) -> bool {
    if select.items.is_empty() || select.filtered || !select.joins.is_empty() {
        return false;
    }
    if select.items.len() == 1 && matches!(select.items[0], ItemFacts::Star { .. }) {
        return allow_star;
    }
    select.items.iter().all(
        |item| matches!(item, ItemFacts::Expression(expression) if expression.direct_projection),
    )
}

fn plain_projection(query: &QueryFacts, allow_star: bool) -> bool {
    if !query.ctes.is_empty() || query.has_order_by || query.has_limit {
        return false;
    }
    query
        .root_select()
        .is_some_and(|select| plain_select(select, allow_star))
}

fn dependency_kind(relation: &RelationFacts) -> Option<CallKind> {
    match relation {
        RelationFacts::Call { kind, .. } if *kind != CallKind::Other => Some(*kind),
        _ => None,
    }
}

pub(super) fn reads_dependency(select: &SelectFacts) -> bool {
    select.from.len() == 1 && select.joins.is_empty() && dependency_kind(&select.from[0]).is_some()
}

fn dependency_import(query: &QueryFacts) -> bool {
    query.root_select().is_some_and(reads_dependency) && plain_projection(query, true)
}

pub(super) fn sole_table_name(query: &QueryFacts) -> Option<&str> {
    let select = query.root_select()?;
    if select.from.len() != 1 || !select.joins.is_empty() {
        return None;
    }
    match &select.from[0] {
        RelationFacts::Table { display, .. } => Some(display),
        _ => None,
    }
}

fn call_texts(calls: &[ModelCall], indexes: &[usize]) -> Vec<String> {
    let mut texts: Vec<String> = Vec::with_capacity(indexes.len());
    for index in indexes {
        if let Some(call) = calls.get(*index) {
            texts.push(call.text.clone());
        }
    }
    texts
}

fn classify_model(query: &QueryFacts, model: &Model, calls: &[ModelCall]) -> ModelClassification {
    let authored_dependencies = dependency_texts(calls);
    let mut logical_seen = false;
    let ctes = query
        .ctes
        .iter()
        .map(|cte| {
            let dependencies: Vec<String> = call_texts(calls, &cte.dependency_calls);
            let dependency_import = dependency_import(&cte.query);
            let kind = if dependencies.is_empty() {
                logical_seen = true;
                CteKind::Logical
            } else {
                let mut rejections: Vec<ImportRejection> = Vec::new();
                if dependencies.len() > 1 {
                    rejections.push(ImportRejection::MultipleDependencies);
                }
                if logical_seen {
                    rejections.push(ImportRejection::AfterLogical);
                }
                if !dependency_import {
                    rejections.push(ImportRejection::Transformation);
                }
                CteKind::Import {
                    dependencies,
                    rejections,
                }
            };
            CteClassification {
                name: cte.name.clone(),
                dependency_import,
                kind,
            }
        })
        .collect::<Vec<_>>();

    let passthrough = model.references.len() == 1
        && authored_dependencies.len() == 1
        && ctes.len() == 1
        && ctes.first().is_some_and(|cte| {
            cte.dependency_import
                && sole_table_name(query) == Some(cte.name.as_str())
                && query
                    .root_select()
                    .is_some_and(|select| plain_select(select, true))
        });

    ModelClassification {
        authored_dependencies,
        ctes,
        passthrough,
    }
}

fn import_ctes(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    let mut imported: Vec<String> = Vec::new();
    for (cte, facts) in parsed.classification.ctes.iter().zip(&parsed.facts.ctes) {
        let CteKind::Import {
            dependencies,
            rejections,
        } = &cte.kind
        else {
            continue;
        };
        if dependencies.len() == 1 {
            imported.extend(dependencies.iter().cloned());
        }
        let position = parsed.position(facts.name_start);
        for rejection in rejections {
            let message = match rejection {
                ImportRejection::MultipleDependencies => {
                    format!("import CTE {:?} reads multiple dependencies", cte.name)
                }
                ImportRejection::AfterLogical => {
                    format!("import CTE {:?} appears after logical CTEs", cte.name)
                }
                ImportRejection::Transformation => {
                    format!("import CTE {:?} contains transformation logic", cte.name)
                }
            };
            faults.push(custom_fault!(
                parsed.model,
                rule,
                position.as_ref(),
                message,
                None,
            ));
        }
    }
    let duplicate = parsed
        .classification
        .authored_dependencies
        .iter()
        .collect::<HashSet<_>>()
        .len()
        != parsed.classification.authored_dependencies.len();
    let mut authored_sorted = parsed.classification.authored_dependencies.clone();
    let mut imported_sorted = imported;
    authored_sorted.sort();
    imported_sorted.sort();
    if duplicate || authored_sorted != imported_sorted {
        faults.push(custom_fault!(
            parsed.model,
            rule,
            None,
            "each __ref/__source must appear once in its own top-level import CTE".into(),
            None,
        ));
    }
}

fn select_star(
    parsed: &ParsedModel<'_>,
    config: &RulesConfig,
    rule: &RuleMetadata,
    faults: &FaultCollector,
) -> Result<(), String> {
    let mut builder = GlobSetBuilder::new();
    for entry in &config.select_star_allow {
        for pattern in &entry.paths {
            builder.add(Glob::new(pattern).map_err(|error| error.to_string())?);
        }
    }
    let allowed = builder
        .build()
        .map_err(|error| error.to_string())?
        .is_match(&parsed.model.relative_path);
    let mut import_queries: Vec<&QueryFacts> = Vec::new();
    for (cte, classified) in parsed.facts.ctes.iter().zip(&parsed.classification.ctes) {
        if classified.dependency_import {
            import_queries.extend(cte.query.all_queries());
        }
    }
    for query in parsed.facts.all_queries() {
        if import_queries
            .iter()
            .any(|imported| std::ptr::eq(*imported, query))
        {
            continue;
        }
        let dynamic_output_star =
            dynamic_contracts::allows_output_star(query, parsed.facts, parsed.model);
        let mut stars: Vec<(&SelectFacts, Option<usize>)> = Vec::new();
        for select in query.body_selects() {
            stars.extend(
                stars_in_select(select)
                    .into_iter()
                    .map(|star| (select, star)),
            );
        }
        for (select, star) in stars {
            if dynamic_output_star || dynamic_contracts::is_dynamic_pivot_star(select) {
                continue;
            }
            let lone = select.items.len() == 1;
            if allowed && lone {
                continue;
            }
            let remediation = (allowed && !lone).then(|| MIXED_STAR_REMEDIATION.into());
            faults.push(custom_fault!(
                parsed.model,
                rule,
                parsed.position(star).as_ref(),
                rule.message.clone(),
                remediation,
            ));
        }
    }
    Ok(())
}

#[derive(Clone)]
pub(super) struct NameParts {
    pub(super) domain: String,
    pub(super) layer: String,
    pub(super) subject: String,
    pub(super) source: Option<String>,
    pub(super) is_view: bool,
}

pub(super) fn parse_name(name: &str) -> Option<NameParts> {
    let parts: Vec<&str> = name.split("__").collect();
    if !(3..=4).contains(&parts.len()) {
        return None;
    }
    if !is_lowercase_identifier(parts[0])
        || !MODEL_NAME_LAYERS.contains(&parts[1])
        || !is_lowercase_identifier(parts[2])
        || parts
            .get(3)
            .is_some_and(|value| !is_lowercase_identifier(value))
    {
        return None;
    }
    Some(NameParts {
        domain: parts[0].into(),
        layer: parts[1].into(),
        subject: parts[2].into(),
        source: parts.get(3).map(|value| (*value).into()),
        is_view: parts[1].ends_with("_v"),
    })
}

fn view_marker(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    let Some(parts) = parse_name(&parsed.model.name) else {
        return;
    };
    let materialized_view = parsed
        .model
        .config
        .get("materialized")
        .and_then(serde_json::Value::as_str)
        == Some(VIEW_MATERIALIZATION);
    if parts.is_view != materialized_view {
        faults.push(fault(parsed.model, rule, None));
    }
}

fn raw_qualified_tables(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    for query in parsed.facts.all_queries() {
        let Some(select) = query.root_select() else {
            continue;
        };
        for relation in select.from.iter().chain(&select.joins) {
            if let RelationFacts::Table {
                display,
                parts,
                start,
                ..
            } = relation
                && *parts > 1
            {
                faults.push(custom_fault!(
                    parsed.model,
                    rule,
                    parsed.position(*start).as_ref(),
                    format!("raw qualified table {display:?} bypasses the SQLBuild graph"),
                    None,
                ));
            }
        }
    }
}

fn name_grammar(
    parsed: &ParsedModel<'_>,
    config: &RulesConfig,
    rule: &RuleMetadata,
    faults: &FaultCollector,
) {
    if let Some(parts) = parse_name(&parsed.model.name) {
        if let Some(marker) = duplicate_layer_marker(&parts.subject) {
            faults.push(custom_fault!(
                parsed.model,
                rule,
                None,
                format!(
                    "model {:?} declares layer {:?} but its subject starts with a second layer marker {:?}",
                    parsed.model.name, parts.layer, marker
                ),
                Some("Rename the model so exactly one layer appears immediately after the domain. Do not preserve the previous layer inside the subject; resolve any resulting identity collision as a semantic design decision.".into()),
            ));
        }
        if !config.domains.is_empty() && !config.domains.contains(&parts.domain) {
            faults.push(custom_fault!(
                parsed.model,
                rule,
                None,
                format!(
                    "model {:?} uses unknown domain {:?}",
                    parsed.model.name, parts.domain
                ),
                Some(format!(
                    "Rename the model into a configured rules domain. The current value is [rules] domains = {}. {}",
                    toml_string_array(config.domains.iter().map(String::as_str)),
                    setting_help(
                        "If this domain is an intentional project owner, add it",
                        "rules",
                        "domains",
                        &toml_string_array(
                            config
                                .domains
                                .iter()
                                .map(String::as_str)
                                .chain([parts.domain.as_str()]),
                        ),
                    )
                )),
            ));
        }
        return;
    }
    let apparent = parsed.model.name.split("__").nth(1);
    let message = if apparent.is_some_and(|value| value.starts_with("int")) {
        format!(
            "model {:?} uses unsupported intermediate layer {:?}",
            parsed.model.name,
            apparent.unwrap_or_default()
        )
    } else {
        format!(
            "model {:?} does not follow <domain>__<layer>__<entity>[__<source>]",
            parsed.model.name
        )
    };
    faults.push(custom_fault!(parsed.model, rule, None, message, None));
}

fn duplicate_layer_marker(subject: &str) -> Option<&'static str> {
    [
        "int_enriched",
        "int_clean",
        "mart_v",
        "stg_v",
        "int_v",
        "mart",
        "stg",
        "v",
    ]
    .into_iter()
    .find(|marker| subject.starts_with(&format!("{marker}_")))
}

fn folder_layer(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    let Some(parts) = parse_name(&parsed.model.name) else {
        return;
    };
    if let Some((message, remediation)) = folder_layer_details(
        &parsed.model.name,
        &parsed.model.relative_path,
        &parts.layer,
    ) {
        faults.push(custom_fault!(
            parsed.model,
            rule,
            None,
            message,
            Some(remediation),
        ));
    }
}

fn source_token_rule(
    parsed: &ParsedModel<'_>,
    config: &RulesConfig,
    rule: &RuleMetadata,
    faults: &FaultCollector,
) {
    let mut tokens: Vec<String> = Vec::new();
    if let Some(source) = parse_name(&parsed.model.name).and_then(|parts| parts.source) {
        tokens.push(source);
    }
    tokens.extend(
        parsed
            .model
            .references
            .iter()
            .filter(|reference| reference.ref_kind == SOURCE_REFERENCE_KIND)
            .map(|reference| reference.ref_name.clone()),
    );
    let mut retired_token: Option<(&String, &String)> = None;
    for token in &tokens {
        if let Some(replacement) = config.retired_source_tokens.get(token) {
            retired_token = Some((token, replacement));
            break;
        }
    }
    if let Some((retired, replacement)) = retired_token {
        faults.push(custom_fault!(
            parsed.model,
            rule,
            None,
            format!(
                "model {:?} uses retired source token {:?}",
                parsed.model.name, retired
            ),
            Some(format!(
                "Rename the model's source token to {replacement:?}; update references at the same model path."
            )),
        ));
    } else if !config.approved_source_tokens.is_empty()
        && let Some(token) = tokens
            .iter()
            .find(|token| !config.approved_source_tokens.contains(token))
    {
        faults.push(custom_fault!(
                parsed.model,
                rule,
                None,
                format!(
                    "model {:?} uses unapproved source token {:?}",
                    parsed.model.name, token
                ),
                Some(format!(
                    "Rename the source suffix to an approved token. The current value is [rules] approved_source_tokens = {}. {}",
                    toml_string_array(config.approved_source_tokens.iter().map(String::as_str)),
                    setting_help(
                        "To approve this token",
                        "rules",
                        "approved_source_tokens",
                        &toml_string_array(
                            config
                                .approved_source_tokens
                                .iter()
                                .map(String::as_str)
                                .chain([token.as_str()]),
                        ),
                    )
                )),
            ));
    }
}

fn reference_name_rule(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    for reference in &parsed.model.references {
        let invalid = parse_name(&reference.ref_name)
            .is_none_or(|parts| duplicate_layer_marker(&parts.subject).is_some());
        if reference.ref_kind == REFERENCE_KIND && invalid {
            faults.push(custom_fault!(
                parsed.model,
                rule,
                None,
                format!(
                    "reference {:?} does not follow Rules model grammar",
                    reference.ref_name
                ),
                Some("Rename the referenced model to <domain>__<layer>__<entity>[__<source>] and update this __ref at the current model path.".into()),
            ));
        }
    }
}

fn contract_required(parsed: &ParsedModel<'_>, rule: &RuleMetadata, faults: &FaultCollector) {
    if parsed
        .model
        .config
        .get("contract")
        .and_then(serde_json::Value::as_str)
        != Some(ENFORCED_CONTRACT)
    {
        faults.push(fault(parsed.model, rule, None));
    }
}

fn evaluate_naming_rules(
    parsed: &ParsedModel<'_>,
    config: &RulesConfig,
    selected: &BTreeMap<String, &RuleMetadata>,
    faults: &FaultCollector,
) {
    for column in &parsed.model.columns {
        if let Some((code, message)) = contract_name_types::finding(column, config)
            && let Some(rule) = selected.get(code)
        {
            faults.push(custom_fault!(parsed.model, rule, None, message, None));
        }
    }
}

fn evaluate_literal_rules(
    parsed: &ParsedModel<'_>,
    selected: &BTreeMap<String, &RuleMetadata>,
    faults: &FaultCollector,
) {
    let source_imports = source_import_names(parsed.facts);
    for query in parsed.facts.all_queries() {
        let Some(select) = query.root_select() else {
            continue;
        };
        let source_context = select_source_context(select, &source_imports);
        for comparison in &select.comparisons {
            evaluate_comparison(parsed, selected, (comparison, &source_context), faults);
        }
    }
}

fn evaluate_comparison(
    parsed: &ParsedModel<'_>,
    selected: &BTreeMap<String, &RuleMetadata>,
    (comparison, source_context): (&ComparisonFacts, &SourceContext),
    faults: &FaultCollector,
) {
    let position = parsed.position(comparison.span.and_then(|span| span.position));
    let sql = comparison
        .span
        .map_or(comparison.generated_sql.as_str(), |span| {
            parsed.source.text(span.start, span.end)
        });
    if comparison.binary
        && let Some(rule) = selected.get("SQBRDECLARATION101")
    {
        let enum_column = comparison
            .columns
            .iter()
            .any(|name| parsed.model.enum_columns.contains(name));
        let bare_string = comparison.string_literals.iter().any(|literal| {
            let text = literal
                .range
                .map_or(literal.generated.as_str(), |(start, end)| {
                    parsed.source.text(start, end)
                });
            parsed.model.authored_sql.contains(text)
        });
        let modified_controlled_column =
            comparison
                .modified_columns
                .iter()
                .any(|column: &ColumnFact| {
                    parsed.model.enum_columns.contains(&column.name)
                        && !column
                            .qualifier
                            .as_ref()
                            .is_some_and(|qualifier| source_context.relations.contains(qualifier))
                        && !(column.qualifier.is_none() && source_context.sole_relation)
                });
        if enum_column
            && (bare_string || modified_controlled_column || comparison.modified_string_literal)
        {
            faults.push(fault(parsed.model, rule, position.as_ref()));
        }
    }
    if let Some(rule) = selected.get("SQBRDECLARATION102") {
        let magic = comparison.numeric_literals.iter().any(|literal| {
            numeric_decisions::is_authored_decision(numeric_decisions::AuthoredDecision {
                authored_compact_sql: &parsed.authored_compact_sql,
                comparison_sql: sql,
                equality: comparison.equality,
                output_name: comparison.output_name.as_deref(),
                literal,
                quote_policy: parsed.quote_policy,
            })
        });
        if magic {
            faults.push(custom_fault!(
                parsed.model,
                rule,
                position.as_ref(),
                format!(
                    "non-canonical numeric comparison: {}",
                    numeric_decisions::single_spaced(sql, parsed.quote_policy)
                ),
                comparison
                    .value_list
                    .then(|| NUMERIC_VALUE_LIST_REMEDIATION.to_owned()),
            ));
        }
    }
}

fn evaluate_test_rules(evaluation: TestRuleEvaluation<'_>) {
    if evaluation.parsed.classification.passthrough {
        return;
    }
    if let Some(rule) = evaluation.selected.get("SQBRTEST201") {
        let minimum = effective_threshold(&evaluation, "min_audits_per_model", 1);
        if evaluation.parsed.model.declared_audit_count < minimum {
            evaluation.faults.push(custom_fault!(
                evaluation.parsed.model,
                rule,
                None,
                format!(
                    "model {:?} has {} audits; {} required",
                    evaluation.parsed.model.name,
                    evaluation.parsed.model.declared_audit_count,
                    minimum
                ),
                None,
            ));
        }
    }
    if let Some(rule) = evaluation.selected.get("SQBRTEST202") {
        let minimum = effective_threshold(&evaluation, "min_tests_per_model", 1);
        if let Some(message) =
            sql_test_rules::minimum_tests_shortfall(evaluation.parsed.model, minimum)
        {
            evaluation.faults.push(custom_fault!(
                evaluation.parsed.model,
                rule,
                None,
                message,
                None
            ));
        }
    }
}

fn effective_threshold(evaluation: &TestRuleEvaluation<'_>, name: &str, default: u32) -> u32 {
    let mut value = evaluation
        .config
        .thresholds
        .get(name)
        .copied()
        .unwrap_or(default);
    for entry in evaluation.threshold_overrides {
        if entry.matches(&evaluation.parsed.model.relative_path)
            && let Some(overridden) = entry.threshold(name)
        {
            value = overridden;
        }
    }
    value
}

pub(crate) fn resolve_threshold_overrides(
    config: &RulesConfig,
) -> Result<Vec<ResolvedThresholdOverride>, String> {
    config
        .threshold_overrides
        .iter()
        .map(|entry| {
            let mut builder = GlobSetBuilder::new();
            for pattern in &entry.paths {
                builder.add(Glob::new(pattern).map_err(|error| error.to_string())?);
            }
            Ok(ResolvedThresholdOverride {
                matcher: builder.build().map_err(|error| error.to_string())?,
                thresholds: entry.thresholds.clone(),
            })
        })
        .collect()
}

fn duplicate_enums(request: &EvaluateRequest, rule: &RuleMetadata, faults: &FaultCollector) {
    let declarations = request.public_enums.iter().chain(
        request
            .models
            .iter()
            .flat_map(|model| model.enum_declarations.iter()),
    );
    let mut signatures: BTreeMap<String, &Declaration> = BTreeMap::new();
    for declaration in declarations {
        let mut members: Vec<_> = declaration
            .members
            .iter()
            .map(|member| (member.name.clone(), format!("{:?}", member.value)))
            .collect();
        members.sort();
        let signature = serde_json::to_string(&members).unwrap_or_default();
        if let Some(previous) = signatures.get(&signature) {
            faults.push(path_fault(
                rule,
                &declaration.relative_path,
                format!("enum {:?} duplicates {:?}", declaration.name, previous.name),
                rule.remediation.clone(),
            ));
        }
        signatures.insert(signature, declaration);
    }
}

fn declaration_domain_placement(
    request: &EvaluateRequest,
    config: &RulesConfig,
    rule: &RuleMetadata,
    faults: &FaultCollector,
) {
    let declarations: Vec<(&str, &str)> = if request.scope_index.completeness.placement {
        request
            .scope_index
            .declarations
            .iter()
            .filter(|declaration| {
                matches!(declaration.scope, crate::models::ScopeKind::Global)
                    && matches!(
                        declaration.kind,
                        crate::models::DeclarationKind::Enum
                            | crate::models::DeclarationKind::Constant
                    )
            })
            .map(|declaration| (declaration.name.as_str(), declaration.path.as_str()))
            .collect()
    } else {
        request
            .public_enums
            .iter()
            .chain(request.public_constants.iter())
            .map(|declaration| {
                (
                    declaration.name.as_str(),
                    declaration.relative_path.as_str(),
                )
            })
            .collect()
    };
    for (name, path) in declarations {
        let parts: Vec<_> = path.split('/').collect();
        let domain = (parts.len() >= DECLARATION_DOMAIN_COMPONENTS).then(|| parts[1]);
        if domain.is_some_and(|value| {
            config.domains.is_empty() || config.domains.contains(&value.into())
        }) {
            continue;
        }
        let expected = if config.domains.is_empty() {
            "<domain>".into()
        } else {
            config.domains.join("|")
        };
        let root = parts.first().copied().unwrap_or("declarations");
        faults.push(path_fault(
            rule,
            path,
            format!(
                "public declaration {:?} has no configured domain folder",
                name
            ),
            format!("Move this declaration under {root}/{expected}/ at this file path."),
        ));
    }
}

fn custom_rule_test_coverage(
    request: &EvaluateRequest,
    config: &RulesConfig,
    policy: (&BTreeMap<String, &RuleMetadata>, &RuleMetadata),
    faults: &FaultCollector,
) {
    let (selected, rule) = policy;
    let minimum = config
        .thresholds
        .get(MIN_CUSTOM_RULE_TEST_CASES_KEY)
        .copied()
        .unwrap_or(1);
    if minimum == 0 {
        return;
    }
    for custom in &request.custom_rules {
        if !selected.contains_key(&custom.code) {
            continue;
        }
        if custom.test_case_count < minimum {
            faults.push(Fault {
                unevaluated: false,
                code: rule.code.clone(),
                path: custom.source.clone().unwrap_or_else(|| "rules".into()),
                line: custom.source_line.max(1),
                column: custom.source_column.max(1),
                message: format!(
                    "custom rule {} has {} harness cases; {} required",
                    custom.code, custom.test_case_count, minimum
                ),
                remediation: rule.remediation.clone(),
            });
        }
    }
}

fn stars_in_select(select: &SelectFacts) -> Vec<Option<usize>> {
    select
        .items
        .iter()
        .filter_map(|item| match item {
            ItemFacts::Star { star_start, .. } => Some(*star_start),
            ItemFacts::Expression(_) => None,
        })
        .collect()
}

fn source_import_names(query: &QueryFacts) -> BTreeSet<String> {
    query
        .ctes
        .iter()
        .filter(|cte| {
            dependency_import(&cte.query)
                && cte.query.root_select().is_some_and(|select| {
                    reads_dependency(select)
                        && dependency_kind(&select.from[0]) == Some(CallKind::Source)
                })
        })
        .map(|cte| cte.name.to_ascii_lowercase())
        .collect()
}

fn select_source_context(select: &SelectFacts, source_imports: &BTreeSet<String>) -> SourceContext {
    let relations: BTreeSet<String> = select
        .from
        .iter()
        .chain(&select.joins)
        .filter_map(|relation| source_relation_name(relation, source_imports))
        .collect();
    let sole_relation = select.from.len() == 1
        && select.joins.is_empty()
        && source_relation_name(&select.from[0], source_imports).is_some();
    SourceContext {
        relations,
        sole_relation,
    }
}

fn source_relation_name(
    relation: &RelationFacts,
    source_imports: &BTreeSet<String>,
) -> Option<String> {
    match relation {
        RelationFacts::Table { last, alias, .. } if source_imports.contains(last) => Some(
            alias
                .as_ref()
                .map_or_else(|| last.clone(), |value| value.to_ascii_lowercase()),
        ),
        RelationFacts::Call {
            kind: CallKind::Source,
            alias,
            ..
        } => Some(
            alias
                .as_ref()
                .map_or_else(|| "__source".to_owned(), |value| value.to_ascii_lowercase()),
        ),
        _ => None,
    }
}
