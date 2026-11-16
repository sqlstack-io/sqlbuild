use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::constants::{
    ALLOWED_PATHS_KEY, ALLOWED_QUALIFIERS_OPTION, DEFAULT_PUBLISHED_LAYERS,
    INTERMEDIATE_LAYER_DIRECTORY, MART_LAYER_DIRECTORY, MODEL_SCHEMA_CONFIG_KEY,
    PUBLISHED_LAYERS_OPTION, REFERENCE_KIND, STAGING_LAYER_DIRECTORY,
};
use crate::models::{EvaluateRequest, Fault, Model, RuleMetadata, RulesConfig};
use crate::rules::_helpers::dispatch::evaluation::{parse_name, path_fault};
use crate::rules::models::{FaultCollector, ProjectEvaluationRequest};
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_core::text::main::glob_match::glob_match;

const NO_CANONICAL_LAYER_FOLDER: &str = "no canonical layer folder";
/// Where an intermediate model that replaces a published one lives.
const INTERMEDIATE_FOLDER: &str = "intermediate/enriched";
const ENTITY_SHAPE: &str = "<domain>__<layer>__<entity>";
const QUALIFIED_SHAPE: &str = "<domain>__<layer>__<entity>__<qualifier>";
const MODEL_FILE_SUFFIX: &str = ".sql";
/// The rule whose published layers and view-of-own-entity test define a published view.
const PUBLISHED_READS_RULE: &str = "SQBRGRAPH103";

pub(crate) fn explicit_model_schema(model: &Model, rule: &RuleMetadata, faults: &FaultCollector) {
    if model
        .authored_config_keys
        .iter()
        .any(|key| key == MODEL_SCHEMA_CONFIG_KEY)
    {
        let schema = model
            .config
            .get(MODEL_SCHEMA_CONFIG_KEY)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<resolved>");
        faults.push(model_fault(
            model,
            rule,
            format!(
                "model declares schema {schema:?} explicitly; model schemas must come from path defaults"
            ),
            None,
        ));
    }
}

pub(crate) fn internal_forward_refs(
    evaluation: &ProjectEvaluationRequest<'_>,
    rule: &RuleMetadata,
    faults: &FaultCollector,
) {
    let configured: BTreeSet<(String, String)> = evaluation
        .request
        .config
        .graph_edge_exceptions
        .iter()
        .map(|entry| (entry.consumer.clone(), entry.dependency.clone()))
        .collect();
    let mut used: BTreeSet<(String, String)> = BTreeSet::new();
    for model in &evaluation.request.models {
        let Some(current) = parse_name(&model.name) else {
            continue;
        };
        let Some(current_rank) = internal_layer_order(&current.layer) else {
            continue;
        };
        for reference in &model.references {
            if reference.ref_kind != REFERENCE_KIND {
                continue;
            }
            let Some(upstream) = parse_name(&reference.ref_name) else {
                continue;
            };
            let Some(upstream_rank) = internal_layer_order(&upstream.layer) else {
                continue;
            };
            if upstream_rank <= current_rank {
                continue;
            }
            let edge = (model.name.clone(), reference.ref_name.clone());
            if configured.contains(&edge) {
                used.insert(edge);
                continue;
            }
            faults.push(model_fault(
                model,
                rule,
                format!(
                    "internal {} model {:?} depends on later internal {} model {:?}",
                    current.layer, model.name, upstream.layer, reference.ref_name
                ),
                Some("Keep the consumer in its semantic layer. Move the required logic into the same or an earlier internal layer, or expose it through an approved published mart interface. For temporary debt, configure this exact consumer/dependency edge with a reason; do not rename or move the consumer merely to satisfy this rule.".into()),
            ));
        }
    }
    for exception in configured.difference(&used) {
        faults.push(Fault {
            unevaluated: false,
            code: rule.code.clone(),
            path: PROJECT_CONFIG_FILENAME.into(),
            line: 1,
            column: 1,
            message: format!(
                "graph edge exception is stale: {} -> {}",
                exception.0, exception.1
            ),
            remediation: "Remove the stale graph_edge_exceptions entry.".into(),
        });
    }
}

pub(crate) fn name_folder_alignment(model: &Model, rule: &RuleMetadata, faults: &FaultCollector) {
    let Some(parts) = parse_name(&model.name) else {
        return;
    };
    let expected = canonical_folder_for_layer(&parts.layer);
    let actual = canonical_folder_in_path(&model.relative_path);
    if actual != Some(expected) {
        faults.push(model_fault(
            model,
            rule,
            format!(
                "model {:?} declares layer {:?} but is stored under {:?}; expected canonical folder {:?}",
                model.name,
                parts.layer,
                actual.unwrap_or(NO_CANONICAL_LAYER_FOLDER),
                expected
            ),
            None,
        ));
    }
}

pub(crate) fn name_schema_alignment(model: &Model, rule: &RuleMetadata, faults: &FaultCollector) {
    let Some(parts) = parse_name(&model.name) else {
        return;
    };
    let Some(actual) = model.logical_schema.as_deref() else {
        return;
    };
    let expected = logical_schema_for_layer(&parts.layer);
    if !actual.eq_ignore_ascii_case(expected) {
        faults.push(model_fault(
            model,
            rule,
            format!(
                "model {:?} resolves to schema {:?}, but layer {:?} requires schema {:?}",
                model.name, actual, parts.layer, expected
            ),
            None,
        ));
    }
}

fn internal_layer_order(layer: &str) -> Option<usize> {
    match layer {
        "stg" | "stg_v" => Some(0),
        "int_clean" => Some(1),
        "int_v" | "int_enriched" => Some(2),
        _ => None,
    }
}

fn canonical_folder_for_layer(layer: &str) -> &'static str {
    match layer {
        "stg" | "stg_v" => STAGING_LAYER_DIRECTORY,
        "int_clean" => "intermediate/clean",
        "int_enriched" | "int_v" => "intermediate/enriched",
        _ => MART_LAYER_DIRECTORY,
    }
}

fn canonical_folder_in_path(relative_path: &str) -> Option<&'static str> {
    let parts: Vec<&str> = relative_path.split('/').collect();
    let parent = &parts[..parts.len().saturating_sub(1)];
    for (index, component) in parent.iter().enumerate().skip(1) {
        if *component == STAGING_LAYER_DIRECTORY {
            return Some(STAGING_LAYER_DIRECTORY);
        }
        if *component == MART_LAYER_DIRECTORY {
            return Some(MART_LAYER_DIRECTORY);
        }
        if *component == INTERMEDIATE_LAYER_DIRECTORY {
            return match parent.get(index + 1).copied() {
                Some("clean") => Some("intermediate/clean"),
                Some("enriched") => Some("intermediate/enriched"),
                _ => None,
            };
        }
    }
    None
}

fn logical_schema_for_layer(layer: &str) -> &'static str {
    match layer {
        "stg" | "stg_v" => STAGING_LAYER_DIRECTORY,
        "int_clean" | "int_enriched" | "int_v" => INTERMEDIATE_LAYER_DIRECTORY,
        _ => MART_LAYER_DIRECTORY,
    }
}

fn model_fault(
    model: &Model,
    rule: &RuleMetadata,
    message: String,
    remediation: Option<String>,
) -> Fault {
    Fault {
        unevaluated: false,
        code: rule.code.clone(),
        path: model.relative_path.clone(),
        line: 1,
        column: 1,
        message,
        remediation: remediation.unwrap_or_else(|| rule.remediation.clone()),
    }
}

/// The published layers configured for `code`, or the defaults.
fn published_layers(config: &RulesConfig, code: &str) -> Vec<String> {
    match config
        .rule_options
        .get(code)
        .and_then(|options| options.get(PUBLISHED_LAYERS_OPTION))
        .and_then(Value::as_array)
    {
        Some(layers) => layers
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        None => DEFAULT_PUBLISHED_LAYERS.map(str::to_owned).to_vec(),
    }
}

/// The parts of a `<domain>__<layer>__<entity>[__<qualifier>]` model name.
struct PublishedName {
    domain: String,
    layer: String,
    subject: String,
    source: Option<String>,
    is_view: bool,
}

fn published_name(name: &str) -> Option<PublishedName> {
    let parts = parse_name(name)?;
    Some(PublishedName {
        domain: parts.domain,
        layer: parts.layer,
        subject: parts.subject,
        source: parts.source,
        is_view: parts.is_view,
    })
}

/// One published model and its parsed name.
struct Published<'a> {
    model: &'a Model,
    name: PublishedName,
}

fn published<'a>(model: &'a Model, layers: &[String]) -> Option<Published<'a>> {
    let name: PublishedName = published_name(&model.name)?;
    layers
        .contains(&name.layer)
        .then_some(Published { model, name })
}

/// Flag every read of a same-domain published model, except a view of its own entity.
pub(crate) fn same_domain_published_reads(
    request: &EvaluateRequest,
    rule: &RuleMetadata,
    faults: &FaultCollector,
) {
    let layers: Vec<String> = published_layers(&request.config, &rule.code);
    for model in &request.models {
        let Some(consumer) = published(model, &layers) else {
            continue;
        };
        let mut reported: BTreeSet<&str> = BTreeSet::new();
        for reference in &model.references {
            if reference.ref_kind != REFERENCE_KIND || !reported.insert(&reference.ref_name) {
                continue;
            }
            let Some(producer) = published_name(&reference.ref_name) else {
                continue;
            };
            if producer.domain != consumer.name.domain
                || !layers.contains(&producer.layer)
                || is_view_of_own_entity(&consumer.name, &producer)
            {
                continue;
            }
            faults.push(path_fault(
                rule,
                &model.relative_path,
                format!(
                    "{} reads {}; published models must not read published models of their own \
                     domain",
                    model.name, reference.ref_name
                ),
                read_remediation(&consumer),
            ));
        }
    }
}

/// Whether `readers` is not empty and each one is a published view of `model`'s own entity.
pub(crate) fn backs_published_view(model: &str, readers: &[String], config: &RulesConfig) -> bool {
    let layers: Vec<String> = published_layers(config, PUBLISHED_READS_RULE);
    let Some(producer) = published_name(model).filter(|name| layers.contains(&name.layer)) else {
        return false;
    };
    !readers.is_empty()
        && readers.iter().all(|reader| {
            published_name(reader).is_some_and(|consumer| {
                consumer.domain == producer.domain
                    && layers.contains(&consumer.layer)
                    && is_view_of_own_entity(&consumer, &producer)
            })
        })
}

/// `X__<layer>_v__<entity>[__<qualifier>]` reading `X__<layer>__<entity>` or the same qualifier.
fn is_view_of_own_entity(consumer: &PublishedName, producer: &PublishedName) -> bool {
    consumer.is_view
        && !producer.is_view
        && (producer.source.is_none() || producer.source == consumer.source)
        && consumer.subject == producer.subject
        && consumer.layer == format!("{}_v", producer.layer)
}

fn read_remediation(consumer: &Published<'_>) -> String {
    let domain: &str = &consumer.name.domain;
    let mut remediation: String = format!(
        "Move the shared logic into a {domain}__int_* model that both published models read, or \
         move this model into the domain that consumes it."
    );
    if let Some(command) = intermediate_move(consumer) {
        remediation.push_str(&format!(" To move it into intermediate: {command}"));
    }
    remediation
}

/// `sqb mv` into the domain's intermediate layer, when the model lives in a `mart` folder.
fn intermediate_move(model: &Published<'_>) -> Option<String> {
    let layer: &str = if model.name.is_view {
        "int_v"
    } else {
        "int_enriched"
    };
    let mut name: String = format!("{}__{layer}__{}", model.name.domain, model.name.subject);
    if let Some(qualifier) = &model.name.source {
        name.push_str("__");
        name.push_str(qualifier);
    }
    let parts: Vec<&str> = model.model.relative_path.split('/').collect();
    let (_, folders) = parts.split_last()?;
    let mart: usize = folders
        .iter()
        .position(|part| *part == MART_LAYER_DIRECTORY)?;
    let mut target: Vec<&str> = folders[..mart].to_vec();
    target.push(INTERMEDIATE_FOLDER);
    target.extend(&folders[mart + 1..]);
    Some(format!(
        "sqb mv {} {}/{name}{MODEL_FILE_SUFFIX}",
        model.model.name,
        target.join("/")
    ))
}

/// One `allowed_qualifiers` entry: the published models it leaves out of the name-shape check.
struct AllowedQualifier {
    domain: Option<String>,
    qualifier: Option<String>,
    paths: Vec<String>,
}

impl AllowedQualifier {
    fn matches(&self, entry: &Published<'_>) -> bool {
        self.domain
            .as_ref()
            .is_none_or(|domain| *domain == entry.name.domain)
            && self.qualifier.as_ref().is_none_or(|pattern| {
                entry
                    .name
                    .source
                    .as_ref()
                    .is_some_and(|qualifier| glob_match(qualifier, pattern))
            })
            && (self.paths.is_empty()
                || self
                    .paths
                    .iter()
                    .any(|pattern| glob_match(&entry.model.relative_path, pattern)))
    }
}

fn allowed_qualifiers(config: &RulesConfig, code: &str) -> Vec<AllowedQualifier> {
    let Some(entries) = config
        .rule_options
        .get(code)
        .and_then(|options| options.get(ALLOWED_QUALIFIERS_OPTION))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|entry| {
            let text = |key: &str| entry.get(key).and_then(Value::as_str).map(str::to_owned);
            let allowed = AllowedQualifier {
                domain: text("domain"),
                qualifier: text("qualifier"),
                paths: entry.get(ALLOWED_PATHS_KEY).map(texts).unwrap_or_default(),
            };
            (allowed.qualifier.is_some() || !allowed.paths.is_empty()).then_some(allowed)
        })
        .collect()
}

/// The strings of a JSON list; anything else is empty.
fn texts(value: &Value) -> Vec<String> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn allowlisted(allowed: &[AllowedQualifier], entry: &Published<'_>) -> bool {
    allowed.iter().any(|allowed| allowed.matches(entry))
}

/// Flag published models whose name shape differs from their domain's majority shape.
pub(crate) fn published_name_shapes(
    request: &EvaluateRequest,
    rule: &RuleMetadata,
    faults: &FaultCollector,
) {
    let layers: Vec<String> = published_layers(&request.config, &rule.code);
    let allowed: Vec<AllowedQualifier> = allowed_qualifiers(&request.config, &rule.code);
    let mut domains: BTreeMap<String, Vec<Published<'_>>> = BTreeMap::new();
    for model in &request.models {
        let Some(entry) = published(model, &layers) else {
            continue;
        };
        if allowlisted(&allowed, &entry) {
            continue;
        }
        domains
            .entry(entry.name.domain.clone())
            .or_default()
            .push(entry);
    }
    for (domain, models) in &domains {
        let qualified: usize = models
            .iter()
            .filter(|entry| entry.name.source.is_some())
            .count();
        let entities: usize = models.len() - qualified;
        if qualified == 0 || entities == 0 {
            continue;
        }
        let majority_qualified: bool = qualified > entities;
        let shape = DomainShape {
            domain,
            entities,
            qualified,
            family: models
                .iter()
                .filter(|entry| entry.name.source.is_some() == majority_qualified)
                .collect(),
        };
        for entry in models
            .iter()
            .filter(|entry| entry.name.source.is_some() != majority_qualified)
        {
            faults.push(shape_fault(rule, entry, &shape));
        }
    }
}

/// One domain's published name shapes: how many models use each, and the majority family.
struct DomainShape<'a> {
    domain: &'a str,
    entities: usize,
    qualified: usize,
    family: Vec<&'a Published<'a>>,
}

fn shape_fault(rule: &RuleMetadata, entry: &Published<'_>, shape: &DomainShape<'_>) -> Fault {
    let majority: &str = if entry.name.source.is_some() {
        ENTITY_SHAPE
    } else {
        QUALIFIED_SHAPE
    };
    let message: String = format!(
        "domain {} publishes {} {} as {ENTITY_SHAPE} and {} as {QUALIFIED_SHAPE}; {} does not \
         fit the majority shape {majority}",
        shape.domain,
        shape.entities,
        if shape.entities == 1 {
            "model"
        } else {
            "models"
        },
        shape.qualified,
        entry.model.name
    );
    path_fault(
        rule,
        &entry.model.relative_path,
        message,
        shape_remediation(rule, entry, &shape.family),
    )
}

fn shape_remediation(
    rule: &RuleMetadata,
    entry: &Published<'_>,
    family: &[&Published<'_>],
) -> String {
    let domain: &str = &entry.name.domain;
    let mut steps: Vec<String> = Vec::new();
    match &entry.name.source {
        Some(qualifier) => {
            let mut adapt: String = format!(
                "If {} adapts another published model, move that logic into {domain}__int_* or \
                 into the consuming domain",
                entry.model.name
            );
            if let Some(command) = intermediate_move(entry) {
                adapt.push_str(&format!(" ({command})"));
            }
            steps.push(adapt);
            let mut split: String = format!(
                "If {domain} really is published once per qualifier, rename its models without \
                 one into the family"
            );
            if let Some(model) = family.first() {
                split.push_str(&format!(
                    " (for example: sqb rename {} {}__core)",
                    model.model.name, model.model.name
                ));
            }
            steps.push(split);
            steps.push(allowlist_step(rule, domain, qualifier));
        }
        None => {
            steps.push(format!(
                "If the models with a qualifier adapt this one, move their logic into \
                 {domain}__int_* or into the consuming domain"
            ));
            steps.push(format!(
                "If {domain} really is published once per qualifier, rename this model into the \
                 family (for example: sqb rename {} {}__core)",
                entry.model.name, entry.model.name
            ));
            if let Some(qualifier) = family.first().and_then(|model| model.name.source.as_ref()) {
                steps.push(allowlist_step(rule, domain, qualifier));
            }
        }
    }
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| format!("{}. {step}.", index + 1))
        .collect::<Vec<String>>()
        .join(" ")
}

fn allowlist_step(rule: &RuleMetadata, domain: &str, qualifier: &str) -> String {
    format!(
        "If {qualifier} is an intentional, structural qualifier, allowlist it with a reason in \
         {PROJECT_CONFIG_FILENAME}: [rules.rule_options.{}] {ALLOWED_QUALIFIERS_OPTION} = \
         [{{ domain = \"{domain}\", qualifier = \"{qualifier}\", reason = \"<why>\" }}]",
        rule.code
    )
}
