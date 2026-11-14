//! Build and evaluate the built-in rules request natively from compiled-project rows.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use pyo3::exceptions::{PyOSError, PyTypeError, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyStringMethods, PyTuple};
use pyo3::{
    FromPyObject, IntoPyObject, IntoPyObjectExt, pyclass, pyfunction, pymethods, wrap_pyfunction,
};
use serde_json::{Value, json};
use sqlbuild_rules::constants::API_VERSION;
use sqlbuild_rules::engine::main::build_models::build_models;
use sqlbuild_rules::engine::main::evaluate_rows::evaluate_rows;
use sqlbuild_rules::engine::main::finalize_rows::finalize_rows;
use sqlbuild_rules::engine::main::project_model_rows::project_model_rows;
use sqlbuild_rules::errors::RowsError;
use sqlbuild_rules::models::{
    Declaration, DeclarationMember, EvaluateRequest, Fault, Model, ModelRow, ModelRowsContext,
    RowsEvaluation, RowsRequest, RulesConfig,
};

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::boundary::panics::{compiler_guard, value_error};
use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::cache::native_store::current_code_identity;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_project::values::plain_value;
use crate::bindings::_helpers::compiled_views::audit_views::NativeCompiledAudit;
use crate::bindings::_helpers::compiled_views::records::compiled_project_facts;
use crate::bindings::_helpers::compiled_views::records::declaration_rows;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::rules::config_json::asdict_json;
use crate::bindings::_helpers::rules::early_builtin_rules::EarlyBuiltinRules;
use crate::bindings::_helpers::rules::fact_replay::{project_attribute, project_models};
use crate::bindings::_helpers::rules::findings::{FindingClasses, finding_fault};
use crate::bindings::_helpers::rules::rule_run::bulk_cache_file;
use crate::bindings::_helpers::rules::selection::rules_error;
use crate::bindings::_helpers::rules::sql_test_objects::{scenario_fact, sql_test_fact};
use crate::bindings::_helpers::scopes::scope_reports::metadata_projection_value;
use crate::bindings::_helpers::type_system::equality::types_equal;
use crate::bindings::types::CompilerDetach;

const DATACLASSES_MODULE: &str = "dataclasses";
const NATIVE_RULES_CACHE_FILE: &str = "native.json";
const NATIVE_RULES_MEMO_FILE: &str = "native-response.json";

/// Name, relative path, members, value, value type and rendering of one declaration.
#[derive(FromPyObject)]
struct DeclarationRowPy<'py>(
    String,
    String,
    Vec<(String, Bound<'py, PyAny>)>,
    Bound<'py, PyAny>,
    Option<String>,
    Option<String>,
);

/// The code, path, line, column and message that identify one fault.
#[derive(Hash, PartialEq, Eq)]
struct FaultIdentity {
    code: String,
    path: String,
    line: u64,
    column: u64,
    message: String,
}

/// The request's project facts, which models, tests and scenarios are built beside.
struct ProjectRows<'py> {
    memo: Option<(String, String)>,
    sql_analysis_enabled: bool,
    include_type_proof: bool,
    attached_audit_targets: Vec<String>,
    public_enums: Vec<DeclarationRowPy<'py>>,
    public_constants: Vec<DeclarationRowPy<'py>>,
    initial_findings: Vec<Bound<'py, PyAny>>,
    dialect: Bound<'py, PyAny>,
}

/// A natively built rules request held until it is evaluated once.
#[pyclass(module = "sqlbuild._native")]
pub(crate) struct NativeRulesRequest {
    request: Mutex<Option<RowsRequest>>,
}

#[pymethods]
impl NativeRulesRequest {
    /// The SQL test, scenario and scope facts this request carries, as `{"sql_tests", "sql_scenarios", "scope_index"}` JSON.
    fn sql_facts_json(&self) -> PyResult<String> {
        let guard = self
            .request
            .lock()
            .map_err(|_| value_error("rules request lock is poisoned"))?;
        let rows: &RowsRequest = guard
            .as_ref()
            .ok_or_else(|| value_error("rules request was already evaluated"))?;
        serde_json::to_string(&serde_json::json!({
            "sql_tests": rows.request.sql_tests,
            "sql_scenarios": rows.request.sql_scenarios,
            "scope_index": rows.request.scope_index,
        }))
        .map_err(value_error)
    }
}

/// Build the built-in rules request natively from the compiled project's rows.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_rules_request(
    options: Option<&Bound<'_, PyDict>>,
) -> PyResult<NativeRulesRequest> {
    let request = Request::new("build_rules_request", options)?;
    let py: Python<'_> = request.py();
    as_rules_error(
        py,
        built_request(&request, None).map(|request| NativeRulesRequest {
            request: Mutex::new(Some(request)),
        }),
    )
}

/// `build_rules_request`'s request, reading model facts from `facts` instead of the project.
pub(crate) fn rules_request_over(
    options: &Bound<'_, PyDict>,
    facts: &Bound<'_, NativeCompiledProject>,
) -> PyResult<RowsRequest> {
    built_request(
        &Request::new("build_rules_request", Some(options))?,
        Some(facts),
    )
}

/// The built request `request` holds, which only one evaluation may take.
pub(crate) fn taken_rows(request: &Bound<'_, NativeRulesRequest>) -> PyResult<RowsRequest> {
    request
        .borrow()
        .request
        .lock()
        .map_err(|_| value_error("rules request lock is poisoned"))?
        .take()
        .ok_or_else(|| value_error("rules request was already evaluated"))
}

/// The model-free request of a run selecting no rules, which only validates its configuration.
pub(crate) fn selectionless_rules_request(
    config: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
    dialect: &str,
    custom_payloads: &Bound<'_, PyAny>,
) -> PyResult<NativeRulesRequest> {
    let py: Python<'_> = config.py();
    let header: PyResult<Value> = dialect.into_bound_py_any(py).and_then(|dialect| {
        request_header(config, project_dir, &dialect, (None, custom_payloads, None))
    });
    as_rules_error(
        py,
        header.and_then(|header| {
            compiler_guard(|| {
                let request: EvaluateRequest =
                    serde_json::from_value(header).map_err(value_error)?;
                Ok(NativeRulesRequest {
                    request: Mutex::new(Some(RowsRequest {
                        request,
                        memo: None,
                        retain: false,
                    })),
                })
            })
        }),
    )
}

/// The request's version, paths, dialect, config, custom rules, cache files and scope index.
fn request_header(
    config: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
    dialect: &Bound<'_, PyAny>,
    (project, custom_payloads, cache_directory): (
        Option<&Bound<'_, PyAny>>,
        &Bound<'_, PyAny>,
        Option<&str>,
    ),
) -> PyResult<Value> {
    let py: Python<'_> = config.py();
    let resolved: Bound<'_, PyAny> = project_dir.call_method0("resolve")?;
    let resolved_path: PathBuf = resolved.extract()?;
    let mut header: Value = json!({
        "version": API_VERSION,
        "project_dir": resolved.str()?.to_str()?,
        "dialect": dialect.extract::<String>()?,
        "config": config_value(config)?,
    });
    if let Some(project) = project {
        header["scope_index"] =
            metadata_projection_value(&project_attribute(project, "scope_index")?)?;
    }
    let version = py.version_info();
    header["python_version"] = json!([version.major, version.minor]);
    header["defer_suppressions"] = Value::Bool(true);
    header["custom_rules"] = plain_value(custom_payloads)?;
    header["rules_cache_path"] = Value::String(path_text(
        py,
        bulk_cache_file(&resolved_path, cache_directory, NATIVE_RULES_CACHE_FILE),
    )?);
    Ok(header)
}

fn built_request(
    options: &Request<'_, '_>,
    facts: Option<&Bound<'_, NativeCompiledProject>>,
) -> PyResult<RowsRequest> {
    let py: Python<'_> = options.py();
    let project: Bound<'_, PyAny> = options.get("project")?;
    let config: Bound<'_, PyAny> = options.get("config")?;
    let project_dir: Bound<'_, PyAny> = options.get("project_dir")?;
    let dialect: Bound<'_, PyAny> = options.get("dialect")?;
    let cache_directory: Option<String> = match options.optional("cache_directory")? {
        Some(directory) if !directory.is_none() => Some(directory.extract()?),
        _ => None,
    };
    let header: Value = request_header(
        &config,
        &project_dir,
        &dialect,
        (
            Some(&project),
            &options.get("custom_payloads")?,
            cache_directory.as_deref(),
        ),
    )?;
    let (public_enums, public_constants) = declaration_rows(
        PyTuple::new(
            py,
            mapping_values(&project_attribute(&project, "public_enums")?)?,
        )?
        .as_any(),
        PyTuple::new(
            py,
            mapping_values(&project_attribute(&project, "public_constants")?)?,
        )?
        .as_any(),
    )?;
    let enabled: bool = field(&field(&config, "cache")?, "enabled")?.is_truthy()?;
    let memo: Option<(String, String)> = if enabled {
        let unresolved: PathBuf = project_dir.extract()?;
        Some((
            current_code_identity(py)?,
            path_text(
                py,
                bulk_cache_file(
                    &unresolved,
                    cache_directory.as_deref(),
                    NATIVE_RULES_MEMO_FILE,
                ),
            )?,
        ))
    } else {
        None
    };
    let mut attached_audit_targets: Vec<String> = Vec::new();
    for audit in project_attribute(&project, "audits")?.try_iter()? {
        let audit: Bound<'_, PyAny> = audit?;
        let target: Option<String> = match audit.cast::<NativeCompiledAudit>() {
            Ok(view) => view.get().attached_target_name(py)?,
            Err(_) => field(&audit, "attached_target_name")?.extract()?,
        };
        attached_audit_targets.extend(target);
    }
    let mut rows = ProjectRows {
        memo,
        sql_analysis_enabled: field(&project_attribute(&project, "settings")?, "sql_analysis")?
            .is_truthy()?,
        include_type_proof: options.get("include_type_proof")?.is_truthy()?,
        attached_audit_targets,
        public_enums: public_enums.extract()?,
        public_constants: public_constants.extract()?,
        initial_findings: options
            .get("initial_findings")?
            .try_iter()?
            .collect::<PyResult<_>>()?,
        dialect,
    };
    let facts: Bound<'_, NativeCompiledProject> = match facts {
        Some(facts) => facts.clone(),
        None => compiled_project_facts(&project)?,
    };
    let names: Vec<String> = project_models(&project)?
        .into_iter()
        .map(|model| model.name)
        .collect();
    let sql_tests: Vec<Bound<'_, PyAny>> = project_attribute(&project, "sql_tests")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let scenarios: Vec<Bound<'_, PyAny>> = project_attribute(&project, "sql_scenarios")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    compiler_guard(|| {
        let mut request: EvaluateRequest = serde_json::from_value(header).map_err(value_error)?;
        request.sql_tests = sql_tests
            .iter()
            .map(sql_test_fact)
            .collect::<PyResult<_>>()?;
        request.sql_scenarios = scenarios
            .iter()
            .map(scenario_fact)
            .collect::<PyResult<_>>()?;
        request.public_enums = declarations(std::mem::take(&mut rows.public_enums))?;
        request.public_constants = declarations(std::mem::take(&mut rows.public_constants))?;
        request.initial_findings = rows
            .initial_findings
            .iter()
            .map(finding_fault)
            .collect::<PyResult<_>>()?;
        let model_rows: Vec<ModelRow> =
            project_model_rows(facts.borrow().facts(), &names).map_err(value_error)?;
        request.models = built_models(&rows, &request, model_rows)?;
        let request: EvaluateRequest =
            crate::bindings::_helpers::compile_delta::rules_rows::whole_project_request(request)?;
        let delta: bool = crate::bindings::_helpers::compile_context::delta::injection().is_some();
        let memo: Option<(String, PathBuf)> = rows
            .memo
            .take()
            .filter(|_| !delta)
            .map(|(identity, path)| (identity, PathBuf::from(path)));
        Ok(RowsRequest {
            request,
            memo,
            retain: crate::bindings::_helpers::compile_context::delta::capturing(),
        })
    })
}

/// Answer the request from the response memo or evaluate it, decoding findings natively.
#[pyfunction]
pub(crate) fn evaluate_rules_request<'py>(
    py: Python<'py>,
    request: &Bound<'py, NativeRulesRequest>,
) -> PyResult<Bound<'py, PyAny>> {
    as_rules_error(py, evaluated(py, request, None))
}

/// `evaluate_rules_request`, answered by `early` when identical; `interrupted` stops the wait.
pub(crate) fn evaluate_rules_request_with<'py>(
    py: Python<'py>,
    request: &Bound<'py, NativeRulesRequest>,
    early: EarlyBuiltinRules,
    interrupted: &AtomicBool,
) -> PyResult<Bound<'py, PyAny>> {
    as_rules_error(py, evaluated(py, request, Some((early, interrupted))))
}

fn evaluated<'py>(
    py: Python<'py>,
    request: &Bound<'py, NativeRulesRequest>,
    early: Option<(EarlyBuiltinRules, &AtomicBool)>,
) -> PyResult<Bound<'py, PyAny>> {
    let request: RowsRequest = taken_rows(request)?;
    let evaluation: Result<RowsEvaluation, RowsError> = match early {
        None => py
            .compiler_detach(|| Ok(evaluate_rows(request)))
            .map_err(value_error)?,
        Some((early, interrupted)) => early.answer(py, request, interrupted)?,
    };
    let mut evaluation: RowsEvaluation = evaluation.map_err(|error| match error {
        RowsError::Rules(message) => value_error(message),
        RowsError::Io(error) => PyOSError::new_err(error.to_string()),
    })?;
    if let Some(retained) = evaluation.retained.take() {
        crate::bindings::_helpers::compile_context::delta::capture(|capture| {
            capture.rules =
                Some(crate::bindings::_helpers::compile_context::delta::CapturedRules { retained });
        });
    }
    let classes = FindingClasses::new(py)?;
    let findings: Vec<Bound<'py, PyAny>> = evaluation
        .faults
        .into_iter()
        .map(|fault| classes.finding_of_fault(fault, false))
        .collect::<PyResult<_>>()?;
    new_record_with(
        py,
        RecordId::NativeRulesEvaluation,
        &[
            ("findings", PyTuple::new(py, findings)?.into_any()),
            (
                "selected_codes",
                PyTuple::new(py, evaluation.selected_codes)?.into_any(),
            ),
            (
                "evaluated_models",
                evaluation.evaluated_models.into_bound_py_any(py)?,
            ),
            ("cache_hits", evaluation.cache_hits.into_bound_py_any(py)?),
            (
                "cache_misses",
                evaluation.cache_misses.into_bound_py_any(py)?,
            ),
            ("built_in_ms", evaluation.built_in_ms.into_bound_py_any(py)?),
            ("reused", evaluation.reused.into_bound_py_any(py)?),
        ],
    )
}

/// Apply the exception policy natively, keeping each original finding's fixable mark.
#[pyfunction]
#[pyo3(signature = (*, config, project_dir, evaluated_codes, findings))]
pub(crate) fn finalize_rule_findings_rows<'py>(
    config: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    evaluated_codes: Vec<String>,
    findings: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = config.py();
    as_rules_error(
        py,
        finalized(config, project_dir, (evaluated_codes, findings)),
    )
}

fn finalized<'py>(
    config: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    (evaluated_codes, findings): (Vec<String>, Vec<Bound<'py, PyAny>>),
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = config.py();
    let config: RulesConfig = serde_json::from_value(config_value(config)?)
        .map_err(|error| value_error(format!("invalid findings request: {error}")))?;
    let project_dir: String = project_dir
        .call_method0("resolve")?
        .str()?
        .to_str()?
        .to_owned();
    let mut fixable: HashSet<FaultIdentity> = HashSet::new();
    let mut faults: Vec<Fault> = Vec::with_capacity(findings.len());
    for finding in &findings {
        let fault: Fault = finding_fault(finding)?;
        if field(finding, "fixable")?.extract::<bool>()? {
            fixable.insert(fault_identity(&fault));
        }
        faults.push(fault);
    }
    let finalized: Vec<Fault> = py
        .compiler_detach(|| finalize_rows(project_dir, config, &evaluated_codes, faults))
        .map_err(value_error)?;
    let classes = FindingClasses::new(py)?;
    let rows: Vec<Bound<'py, PyAny>> = finalized
        .into_iter()
        .map(|fault| {
            let is_fixable: bool = fixable.contains(&fault_identity(&fault));
            classes.finding_of_fault(fault, is_fixable)
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, rows)
}

/// `asdict(config)` as orjson with `default=str` encodes it.
fn config_value(config: &Bound<'_, PyAny>) -> PyResult<Value> {
    if let Some(value) = asdict_json(config)? {
        return Ok(value);
    }
    plain_value(&imported(config.py(), DATACLASSES_MODULE, "asdict")?.call1((config,))?)
}

/// `str(path)` of the Python path built from `path`.
fn path_text(py: Python<'_>, path: PathBuf) -> PyResult<String> {
    Ok(path.into_pyobject(py)?.str()?.to_str()?.to_owned())
}

fn mapping_values<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    mapping.call_method0("values")?.try_iter()?.collect()
}

/// Value and type errors of the request as `RulesError`, as the rules API reports them.
fn as_rules_error<T>(py: Python<'_>, result: PyResult<T>) -> PyResult<T> {
    result.map_err(|error| {
        if error.is_instance_of::<PyValueError>(py) || error.is_instance_of::<PyTypeError>(py) {
            let message: String = error.value(py).to_string();
            rules_error(py, &message, Some(error))
        } else {
            error
        }
    })
}

fn built_models(
    project: &ProjectRows<'_>,
    request: &EvaluateRequest,
    rows: Vec<ModelRow>,
) -> PyResult<Vec<Model>> {
    let mut python_error: Option<PyErr> = None;
    let fallback = |left: &str, right: &str| -> Result<bool, String> {
        match types_equal(&project.dialect, left, right) {
            Ok(equal) => Ok(equal),
            Err(error) => {
                let message: String = error.to_string();
                python_error = Some(error);
                Err(message)
            }
        }
    };
    let built: Result<Vec<Model>, String> = build_models(
        rows,
        ModelRowsContext {
            sql_analysis_enabled: project.sql_analysis_enabled,
            include_type_proof: project.include_type_proof,
            dialect: &request.dialect,
            attached_audit_targets: &project.attached_audit_targets,
            sql_tests: &request.sql_tests,
        },
        fallback,
    );
    built.map_err(|message| python_error.take().unwrap_or_else(|| value_error(message)))
}

fn declarations(rows: Vec<DeclarationRowPy<'_>>) -> PyResult<Vec<Declaration>> {
    rows.into_iter().map(declaration).collect()
}

fn declaration(row: DeclarationRowPy<'_>) -> PyResult<Declaration> {
    let DeclarationRowPy(name, relative_path, members, value, value_type, render_as) = row;
    Ok(Declaration {
        name,
        relative_path,
        members: members
            .into_iter()
            .map(declaration_member)
            .collect::<PyResult<_>>()?,
        value: Some(plain_value(&value)?).filter(|value| !value.is_null()),
        value_type,
        render_as,
    })
}

fn declaration_member((name, value): (String, Bound<'_, PyAny>)) -> PyResult<DeclarationMember> {
    Ok(DeclarationMember {
        name,
        value: plain_value(&value)?,
    })
}

fn fault_identity(fault: &Fault) -> FaultIdentity {
    FaultIdentity {
        code: fault.code.clone(),
        path: fault.path.clone(),
        line: fault.line,
        column: fault.column,
        message: fault.message.clone(),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeRulesRequest>()?;
    module.add_function(wrap_pyfunction!(build_rules_request, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_rules_request, module)?)?;
    module.add_function(wrap_pyfunction!(finalize_rule_findings_rows, module)?)?;
    Ok(())
}
