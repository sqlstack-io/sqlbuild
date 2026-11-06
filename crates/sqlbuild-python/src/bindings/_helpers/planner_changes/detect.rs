//! Detect how each selected model and function changed since its recorded build.

use sqlbuild_analysis::planner_scope::constants::MODEL;
use sqlbuild_host::entries::models::{ExecutorHost, UserCodeHost};

use crate::bindings::_helpers::compiled_views::function_reads;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::contracts::spec_resolution::matching_dynamic_families;
use crate::bindings::_helpers::model_config::typed_config::{config_text, model_values};
use crate::bindings::_helpers::planner_changes::results::native_enum;
use crate::bindings::_helpers::records::construct::new_record_with;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_changes::constants::{
    CASE_FOLDING_DIALECT, EMPTY_FINGERPRINT_METADATA_JSON,
};
use sqlbuild_analysis::planner_changes::main::detect_schema_changes::detect_schema_changes;
use sqlbuild_analysis::planner_changes::main::metadata_changes::metadata_changes;
use sqlbuild_analysis::planner_changes::main::model_backfill::model_backfill;
use sqlbuild_analysis::planner_changes::main::model_change_kind::model_change_kind;
use sqlbuild_analysis::planner_changes::main::recorded_declared_columns_hash::recorded_declared_columns_hash;
use sqlbuild_analysis::planner_changes::main::replay_policy_needed::replay_policy_needed;
use sqlbuild_analysis::planner_changes::main::resolve_replay_on_change::resolve_replay_on_change;
use sqlbuild_analysis::planner_changes::main::with_declared_columns_hash::with_declared_columns_hash;
use sqlbuild_analysis::planner_changes::models::{
    Backfill, BackfillAction, BackfillFacts, MetadataChanges, SchemaColumns, SchemaComparison,
    SchemaFindingRow,
};

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, imported};
use crate::bindings::_helpers::compiled_views::model_reads::{inferred_rows, star_flags};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_changes::renames::origin_names;
use crate::bindings::_helpers::planner_changes::results::{
    backfill_result, change_kind, planner_enum, replay_policy_error, schema_findings,
};
use crate::bindings::_helpers::planner_identity::hashing::{
    function_definition_hash, model_query_hash,
};
use crate::bindings::_helpers::planner_identity::metadata::{
    compiled_function_fingerprint_sql, function_local_hashes, model_declared_columns_hash,
    model_version_identity_metadata_json,
};
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::semantic_shapes::shapes::snapshot_validity_columns;
use crate::bindings::_helpers::type_system::equality::types_equal as native_types_equal;
use sqlbuild_project::types::InferredRow;

const CHANGES_LOGGER: &str = "sqlbuild.planner.changes";
const REPLAY_CONFIG_KEY: &str = "replay_on_change";

/// Inputs shared by every model of one detection pass.
pub(crate) struct ModelPass<'py> {
    pub(crate) snapshot: Bound<'py, PyAny>,
    pub(crate) query_change_tracking: bool,
    pub(crate) full_refresh: bool,
    pub(crate) function_local_hashes: Bound<'py, PyAny>,
    pub(crate) hook_version_hashes: Bound<'py, PyAny>,
}

/// Selected model and function changes.
#[pyfunction(name = "detect_changes")]
#[pyo3(signature = (**options))]
fn detect_changes_of<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "detect_changes",
        options,
        (
            &["project", "scope", "snapshot", "full_refresh"],
            &["expected_version_hashes", "expected_metadata_jsons"],
        ),
    )?;
    detect_changes(
        (
            &arguments.value("project")?,
            &arguments.value("scope")?,
            &arguments.value("snapshot")?,
        ),
        arguments.flag("full_refresh")?,
        (
            arguments.value("expected_version_hashes")?,
            arguments.value("expected_metadata_jsons")?,
        ),
    )
}

/// Selected model and function changes against expected version hashes and metadata JSON.
pub(crate) fn detect_changes<'py>(
    (project, scope, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    full_refresh: bool,
    expected: (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let models: Bound<'py, PyDict> =
        scope_model_changes((project, scope, snapshot), None, expected)?;
    let functions: Bound<'py, PyDict> = PyDict::new(py);
    let selected = scope_data(scope)?;
    let recorded: Bound<'py, PyAny> =
        native_field(&native_field(snapshot, "fingerprints")?, "functions")?;
    let query_change_tracking: bool =
        native_field(&native_field(project, "settings")?, "query_change_tracking")?.is_truthy()?;
    let dialect: Option<String> = native_field(snapshot, "column_dialect")?.extract()?;
    for function in project_reads::functions(project)?.try_iter()? {
        let function: Bound<'py, PyAny> = function?;
        let name: Bound<'py, PyAny> = function_reads::name(&function)?;
        let fingerprint_sql: String = compiled_function_fingerprint_sql(&function)?;
        let mut arguments: Vec<(&str, Bound<'py, PyAny>)> = vec![(
            "fingerprint_sql",
            PyString::new(py, &fingerprint_sql).into_any(),
        )];
        if selected
            .selected
            .contains(&crate::bindings::_helpers::graph::keys::native_key(
                &function_reads::key(&function)?,
            )?)
        {
            let reason: &str = if full_refresh {
                "full_refresh"
            } else {
                let fingerprint: Bound<'py, PyAny> = mapping_get(&recorded, &name)?;
                if fingerprint.is_none() {
                    "first_run"
                } else if query_change_tracking
                    && function_definition_hash(
                        py,
                        &name.extract::<String>()?,
                        (
                            &fingerprint_sql,
                            &function_reads::language(&function)?
                                .str()?
                                .extract::<String>()?,
                            dialect.as_deref(),
                        ),
                    )? != native_field(&fingerprint, "definition_hash")?.extract::<String>()?
                {
                    "query_changed"
                } else {
                    "no_change"
                }
            };
            arguments.push(("reason", planner_enum(py, RecordId::PlanReason, reason)?));
        }
        functions.set_item(
            name,
            new_record_with(py, RecordId::FunctionChangeResult, &arguments)?,
        )?;
    }
    new_record_with(
        py,
        RecordId::PlannerChangeResults,
        &[
            ("models", models.into_any()),
            ("functions", functions.into_any()),
        ],
    )
}

/// One model's change and backfill.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn detect_model_changes<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "detect_model_changes",
        options,
        (
            &[
                "model",
                "snapshot",
                "sql_analysis_enabled",
                "query_change_tracking",
                "full_refresh",
            ],
            &[
                "function_local_hashes",
                "hook_version_hashes",
                "expected_version_hash",
                "expected_metadata_json",
            ],
        ),
    )?;
    let pass = ModelPass {
        snapshot: arguments.value("snapshot")?,
        query_change_tracking: arguments.flag("query_change_tracking")?,
        full_refresh: arguments.flag("full_refresh")?,
        function_local_hashes: arguments.value("function_local_hashes")?,
        hook_version_hashes: arguments.value("hook_version_hashes")?,
    };
    model_change(
        &pass,
        &arguments.value("model")?,
        (
            arguments.value("expected_version_hash")?.extract()?,
            arguments.value("expected_metadata_json")?.extract()?,
        ),
    )
}

pub(crate) fn scope_model_changes<'py>(
    (project, scope, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    query_change_tracking: Option<bool>,
    (version_hashes, metadata_jsons): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = project.py();
    let tracking: bool = match query_change_tracking {
        Some(value) => value,
        None => {
            native_field(&native_field(project, "settings")?, "query_change_tracking")?.extract()?
        }
    };
    let function_local_hashes: Bound<'py, PyAny> = function_local_hashes(
        &project_reads::functions(project)?,
        native_field(project, "sql_analysis_dialect")?
            .extract::<Option<String>>()?
            .as_deref(),
    )?
    .into_any();
    let hook_version_hashes: Bound<'py, PyDict> = PyDict::new(py);
    let identities: Bound<'py, PyAny> = host::entry(py, UserCodeHost::BuildHookIdentities)?
        .call1((native_field(project, "hook_functions")?,))?;
    for item in mapping_items(&identities)?.try_iter()? {
        let item: Bound<'py, PyAny> = item?;
        hook_version_hashes.set_item(
            item.get_item(0)?,
            native_field(&item.get_item(1)?, "version_hash")?,
        )?;
    }
    let pass = ModelPass {
        snapshot: snapshot.clone(),
        query_change_tracking: tracking,
        full_refresh: false,
        function_local_hashes,
        hook_version_hashes: hook_version_hashes.into_any(),
    };
    let scope = scope_data(scope)?;
    let changes: Bound<'py, PyDict> = PyDict::new(py);
    for key in &scope.order {
        if !scope.selected.contains(key) || key.0 != MODEL {
            continue;
        }
        let Some(model) = scope.model(py, &key.1) else {
            continue;
        };
        let name: Bound<'py, PyAny> = model_reads::name_object(&model)?;
        let expected: (Option<String>, Option<String>) = (
            lookup(&version_hashes, &name)?,
            lookup(&metadata_jsons, &name)?,
        );
        changes.set_item(&name, model_change(&pass, &model, expected)?)?;
    }
    Ok(changes)
}

fn lookup(mapping: &Bound<'_, PyAny>, name: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if mapping.is_none() {
        return Ok(None);
    }
    mapping_get(mapping, name)?.extract()
}

pub(crate) fn model_change<'py>(
    pass: &ModelPass<'py>,
    model: &Bound<'py, PyAny>,
    (expected_version_hash, expected_metadata_json): (Option<String>, Option<String>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let snapshot: &Bound<'py, PyAny> = &pass.snapshot;
    let model_name: Bound<'py, PyAny> = model_reads::name_object(model)?;
    let columns_hash: String = model_declared_columns_hash(model)?;
    let base_metadata: String = match expected_metadata_json.filter(|text| !text.is_empty()) {
        Some(text) => text,
        None => model_version_identity_metadata_json(
            model,
            Some(&pass.function_local_hashes),
            Some(&pass.hook_version_hashes),
        )?,
    };
    let metadata_json: String = with_declared_columns_hash(&base_metadata, &columns_hash);
    let mut result = ChangeResult::new(
        py,
        &model_name,
        &metadata_json,
        expected_version_hash.as_deref(),
    )?;
    if pass.full_refresh {
        return result.finish(
            Backfill {
                action: BackfillAction::Full,
                duration: None,
            },
            "no_change",
        );
    }
    let relation_exists: bool =
        native_field(snapshot, "existing_relations")?.contains(&model_name)?;
    let fingerprint: Bound<'py, PyAny> =
        native_field(&native_field(snapshot, "fingerprints")?, "models")?
            .call_method1("get", (&model_name,))?;
    let recorded: Option<Recorded> = recorded_fingerprint(&fingerprint)?;
    let dialect: Bound<'py, PyAny> = native_field(snapshot, "column_dialect")?;
    let compiled_query_hash: Option<String> = if pass.query_change_tracking && recorded.is_some() {
        Some(model_query_hash(
            py,
            &model_name.extract::<String>()?,
            (
                &model_reads::query_sql(model)?.extract::<String>()?,
                dialect.extract::<Option<String>>()?.as_deref(),
            ),
        )?)
    } else {
        None
    };
    if let Some(recorded) = &recorded {
        result.previous(recorded);
    }
    let definition_changed: bool = match (&compiled_query_hash, &recorded) {
        (Some(hash), Some(recorded)) => *hash != recorded.definition_hash,
        _ => false,
    };
    if !relation_exists && native_field(snapshot, "renamed_models")?.contains(&model_name)? {
        result.query_changed = definition_changed;
        let backfill: Backfill = if definition_changed {
            replay_policy(model)?
        } else {
            Backfill {
                action: BackfillAction::ForwardOnly,
                duration: None,
            }
        };
        return result.finish(backfill, "renamed");
    }
    if !relation_exists {
        result.clear_previous();
        result.recorded_build_relation_missing = recorded.is_some();
        return result.finish(
            Backfill {
                action: BackfillAction::Full,
                duration: None,
            },
            "first_run",
        );
    }
    let recorded_metadata: Option<&str> = recorded
        .as_ref()
        .map(|recorded| recorded.metadata_json.as_str())
        .filter(|text| *text != EMPTY_FINGERPRINT_METADATA_JSON);
    let renames: Bound<'py, PyAny> = native_field(snapshot, "reference_renames")?;
    let origins: Option<Vec<(String, String)>> = match (&compiled_query_hash, &recorded) {
        (Some(_), Some(recorded)) if definition_changed && renames.is_truthy()? => origin_names(
            (model_reads::query_sql(model)?.extract()?, &fingerprint),
            &renames,
            (&recorded.definition_hash, &dialect),
        )?,
        _ => None,
    };
    let changes: MetadataChanges = metadata_changes(
        (&metadata_json, recorded_metadata),
        origins.as_deref(),
        pass.query_change_tracking,
    );
    let mut query_changed: bool = false;
    if let (Some(hash), Some(recorded)) = (&compiled_query_hash, &recorded) {
        query_changed = origins.is_none() && definition_changed;
        log_query_check(
            (model, &model_name, &fingerprint),
            hash,
            (&recorded.definition_hash, query_changed),
        )?;
    }
    let findings: Vec<SchemaFindingRow> =
        model_schema_findings(model, snapshot, &model_name, &dialect)?;
    let declared_columns_changed: bool =
        recorded_declared_columns_hash(recorded_metadata).is_some_and(|hash| hash != columns_hash);
    let facts = BackfillFacts {
        query_changed,
        functions_changed: !changes.changed_functions.is_empty(),
        config_changed: changes.config_changed,
        declared_columns_changed,
        schema_changed: !findings.is_empty(),
    };
    let replay: Option<Backfill> = if replay_policy_needed(facts) {
        Some(replay_policy(model)?)
    } else {
        None
    };
    let backfill: Backfill = model_backfill(facts, replay);
    result.query_changed = query_changed;
    result.config_changed = changes.config_changed;
    result.findings = findings;
    result.changed_functions = changes.changed_functions;
    result.finish_kind(backfill, change_kind(py, model_change_kind(facts))?)
}

/// The model's resolved `replay_on_change` policy.
fn replay_policy(model: &Bound<'_, PyAny>) -> PyResult<Backfill> {
    let py: Python<'_> = model.py();
    let raw: Option<String> = config_text(py, &model_values(model)?, REPLAY_CONFIG_KEY)?;
    resolve_replay_on_change(raw.as_deref()).map_err(|error| replay_policy_error(py, error))
}

/// The recorded fingerprint fields change detection reads.
struct Recorded {
    definition_hash: String,
    metadata_json: String,
    version_hash: String,
}

fn recorded_fingerprint(fingerprint: &Bound<'_, PyAny>) -> PyResult<Option<Recorded>> {
    if fingerprint.is_none() {
        return Ok(None);
    }
    Ok(Some(Recorded {
        definition_hash: native_field(fingerprint, "definition_hash")?.extract()?,
        metadata_json: native_field(fingerprint, "metadata_json")?.extract()?,
        version_hash: native_field(fingerprint, "version_hash")?.extract()?,
    }))
}

fn log_query_check(
    (model, model_name, fingerprint): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, &Bound<'_, PyAny>),
    compiled_query_hash: &str,
    (definition_hash, query_changed): (&str, bool),
) -> PyResult<()> {
    let py: Python<'_> = model.py();
    let logger: Bound<'_, PyAny> =
        imported(py, "logging", "getLogger")?.call1((CHANGES_LOGGER,))?;
    let text = |value: &str| PyString::new(py, value).into_any();
    call_with(
        &host::entry(py, ExecutorHost::LogDebugEvent)?,
        &[
            ("logger", logger.clone()),
            (
                "message",
                text(&format!(
                    "fingerprint comparison compiled_query_hash={compiled_query_hash} \
                     fingerprint_definition_hash={definition_hash} query_changed={}",
                    if query_changed { "True" } else { "False" }
                )),
            ),
            ("sqlbuild_subject", text("model")),
            ("sqlbuild_name", model_name.clone()),
            ("sqlbuild_event", text("query_change_check")),
            ("sqlbuild_phase", text("planner")),
            (
                "sqlbuild_status",
                text(if query_changed {
                    "changed"
                } else {
                    "unchanged"
                }),
            ),
        ],
    )?;
    let log_sql: Bound<'_, PyAny> = host::entry(py, ExecutorHost::LogSql)?;
    call_with(
        &log_sql,
        &[
            ("logger", logger.clone()),
            ("sql", model_reads::query_sql(model)?),
            ("action", text("compiled_query")),
        ],
    )?;
    call_with(
        &log_sql,
        &[
            ("logger", logger),
            ("sql", native_field(fingerprint, "definition")?),
            ("action", text("fingerprint_definition")),
        ],
    )?;
    Ok(())
}

fn model_schema_findings<'py>(
    model: &Bound<'py, PyAny>,
    snapshot: &Bound<'py, PyAny>,
    model_name: &Bound<'py, PyAny>,
    dialect: &Bound<'py, PyAny>,
) -> PyResult<Vec<SchemaFindingRow>> {
    let warehouse: Bound<'py, PyAny> =
        native_field(snapshot, "existing_columns")?.call_method1("get", (model_name,))?;
    if warehouse.is_none() {
        return Ok(Vec::new());
    }
    let validity: Vec<String> = snapshot_validity_columns(model)?;
    let mut warehouse: Vec<(String, String)> = column_pairs(&warehouse)?;
    for column in &validity {
        warehouse.retain(|(name, _)| !column.eq_ignore_ascii_case(name));
    }
    let schema_entry: Bound<'py, PyAny> = model_reads::schema_entry(model)?;
    let mut yml: Vec<(String, String)> = Vec::new();
    let mut type_enforcement: bool = false;
    let mut dynamic: Option<Bound<'py, PyAny>> = None;
    if !schema_entry.is_none() {
        for column in native_field(&schema_entry, "columns")?.try_iter()? {
            let column: Bound<'py, PyAny> = column?;
            if let Some(kind) = native_field(&column, "type")?.extract::<Option<String>>()? {
                yml.push((native_field(&column, "name")?.extract()?, kind));
            }
        }
        type_enforcement = native_field(&schema_entry, "type_enforcement")?
            .extract::<Option<bool>>()?
            .unwrap_or(false);
        let families: Bound<'py, PyAny> = native_field(&schema_entry, "dynamic_columns")?;
        if families.is_truthy()? {
            dynamic = Some(families);
        }
    }
    let inferred: Option<Vec<(String, Option<String>)>> = inferred_rows(model)?.map(typed_names);
    if yml.is_empty() && inferred.as_ref().is_none_or(Vec::is_empty) {
        return Ok(Vec::new());
    }
    let inferred_schema_complete: bool = !star_flags(model)?.0;
    schema_findings_of(
        SchemaColumns {
            yml,
            inferred,
            warehouse,
        },
        (type_enforcement, inferred_schema_complete),
        (dynamic, dialect),
    )
}

fn typed_names(rows: Vec<InferredRow>) -> Vec<(String, Option<String>)> {
    let mut columns: Vec<(String, Option<String>)> = Vec::with_capacity(rows.len());
    for (name, column_type, _) in rows {
        columns.push((name, column_type));
    }
    columns
}

fn column_pairs(columns: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for column in columns.try_iter()? {
        let column: Bound<'_, PyAny> = column?;
        pairs.push((
            native_field(&column, "name")?.extract()?,
            native_field(&column, "type")?.extract()?,
        ));
    }
    Ok(pairs)
}

fn schema_findings_of<'py>(
    columns: SchemaColumns,
    (type_enforcement, inferred_schema_complete): (bool, bool),
    (dynamic, dialect): (Option<Bound<'py, PyAny>>, &Bound<'py, PyAny>),
) -> PyResult<Vec<SchemaFindingRow>> {
    let _py: Python<'py> = dialect.py();
    let dialect_name: Option<String> = dialect.extract()?;
    let mut types_equal =
        |left: &str, right: &str| -> PyResult<bool> { native_types_equal(dialect, left, right) };
    let mut single_family_type = |name: &str| -> PyResult<Option<String>> {
        let Some(families) = &dynamic else {
            return Ok(None);
        };
        let matching = matching_dynamic_families(families, name)?;
        if matching.len()? != 1 {
            return Ok(None);
        }
        native_field(&matching.get_item(0)?, "type")?.extract()
    };
    detect_schema_changes(
        columns,
        SchemaComparison {
            type_enforcement,
            inferred_schema_complete,
            case_folding: dialect_name.as_deref() == Some(CASE_FOLDING_DIALECT),
        },
        (&mut types_equal, &mut single_family_type),
    )
}

/// Schema findings between declared or inferred columns and warehouse columns.
#[pyfunction(name = "detect_schema_changes")]
#[pyo3(signature = (**options))]
fn schema_changes<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "detect_schema_changes",
        options,
        (
            &[
                "yml_columns",
                "inferred_columns",
                "warehouse_columns",
                "type_enforcement",
                "inferred_schema_complete",
            ],
            &["dynamic_columns", "dialect"],
        ),
    )?;
    let yml_columns: Bound<'py, PyAny> = arguments.value("yml_columns")?;
    let inferred_columns: Bound<'py, PyAny> = arguments.value("inferred_columns")?;
    let warehouse_columns: Bound<'py, PyAny> = arguments.value("warehouse_columns")?;
    let type_enforcement: bool = arguments.flag("type_enforcement")?;
    let inferred_schema_complete: bool = arguments.flag("inferred_schema_complete")?;
    let dynamic_columns: Bound<'py, PyAny> = arguments.value("dynamic_columns")?;
    let dialect: Bound<'py, PyAny> = arguments.value("dialect")?;
    let py: Python<'py> = yml_columns.py();
    let inferred: Option<Vec<(String, Option<String>)>> = if inferred_columns.is_none() {
        None
    } else {
        let mut items: Vec<(String, Option<String>)> = Vec::new();
        for column in inferred_columns.try_iter()? {
            let column: Bound<'py, PyAny> = column?;
            items.push((
                native_field(&column, "name")?.extract()?,
                native_field(&column, "type")?.extract()?,
            ));
        }
        Some(items)
    };
    let dynamic: Option<Bound<'py, PyAny>> = if dynamic_columns.is_truthy()? {
        Some(dynamic_columns)
    } else {
        None
    };
    let findings: Vec<SchemaFindingRow> = schema_findings_of(
        SchemaColumns {
            yml: column_pairs(&yml_columns)?,
            inferred,
            warehouse: column_pairs(&warehouse_columns)?,
        },
        (type_enforcement, inferred_schema_complete),
        (dynamic, &dialect),
    )?;
    schema_findings(py, &findings)
}

/// The `ChangeDetectionResult` being assembled for one model.
struct ChangeResult<'py> {
    py: Python<'py>,
    model_name: Bound<'py, PyAny>,
    metadata_json: String,
    version_hash: Option<String>,
    previous_metadata_json: Option<String>,
    previous_version_hash: Option<String>,
    query_changed: bool,
    config_changed: bool,
    recorded_build_relation_missing: bool,
    findings: Vec<SchemaFindingRow>,
    changed_functions: Vec<String>,
}

impl<'py> ChangeResult<'py> {
    fn new(
        py: Python<'py>,
        model_name: &Bound<'py, PyAny>,
        metadata_json: &str,
        version_hash: Option<&str>,
    ) -> PyResult<Self> {
        Ok(Self {
            py,
            model_name: model_name.clone(),
            metadata_json: metadata_json.to_owned(),
            version_hash: version_hash.map(str::to_owned),
            previous_metadata_json: None,
            previous_version_hash: None,
            query_changed: false,
            config_changed: false,
            recorded_build_relation_missing: false,
            findings: Vec::new(),
            changed_functions: Vec::new(),
        })
    }

    fn previous(&mut self, recorded: &Recorded) {
        self.previous_metadata_json = Some(recorded.metadata_json.clone());
        self.previous_version_hash = Some(recorded.version_hash.clone());
    }

    fn clear_previous(&mut self) {
        self.previous_metadata_json = None;
        self.previous_version_hash = None;
    }

    fn finish(self, backfill: Backfill, kind: &str) -> PyResult<Bound<'py, PyAny>> {
        let kind: Bound<'py, PyAny> = native_enum(self.py, RecordId::ChangeKind, kind)?;
        self.finish_kind(backfill, kind)
    }

    fn finish_kind(
        self,
        backfill: Backfill,
        kind: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.py;
        let flag = |value: bool| {
            value
                .into_pyobject(py)
                .map(|flag| flag.to_owned().into_any())
        };
        new_record_with(
            py,
            RecordId::ChangeDetectionResult,
            &[
                ("model_name", self.model_name),
                ("change_kind", kind),
                ("query_changed", flag(self.query_changed)?),
                ("config_changed", flag(self.config_changed)?),
                (
                    "fingerprint_metadata_json",
                    self.metadata_json.into_pyobject(py)?.into_any(),
                ),
                (
                    "previous_metadata_json",
                    self.previous_metadata_json.into_pyobject(py)?.into_any(),
                ),
                (
                    "fingerprint_version_hash",
                    self.version_hash.into_pyobject(py)?.into_any(),
                ),
                (
                    "previous_version_hash",
                    self.previous_version_hash.into_pyobject(py)?.into_any(),
                ),
                (
                    "schema_findings",
                    schema_findings(py, &self.findings)?.into_any(),
                ),
                ("backfill", backfill_result(py, &backfill)?),
                (
                    "recorded_build_relation_missing",
                    flag(self.recorded_build_relation_missing)?,
                ),
                (
                    "changed_functions",
                    pyo3::types::PyTuple::new(py, self.changed_functions)?.into_any(),
                ),
            ],
        )
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(detect_changes_of, module)?)?;
    module.add_function(wrap_pyfunction!(detect_model_changes, module)?)?;
    module.add_function(wrap_pyfunction!(schema_changes, module)?)
}
