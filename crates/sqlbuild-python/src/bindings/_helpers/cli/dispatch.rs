//! `sqb` command dispatch: the native route table applied to a parsed namespace.

use std::path::PathBuf;

use crate::bindings::_helpers::audit_command::command::run_audit_command;
use crate::bindings::_helpers::build_command::command::run_build_command;
use crate::bindings::_helpers::check::check_command::run_check_command;
use crate::bindings::_helpers::cli::launch_state::withhold_launch_record;
use crate::bindings::_helpers::clone::clone_command::run_clone_command;
use crate::bindings::_helpers::command_runtime::warehouse_scope::command_warehouse_scope;
use crate::bindings::_helpers::command_runtime::warehouses::validate_cli_warehouse;
use crate::bindings::_helpers::compile_command::reuse_attempt::run_compile;
use crate::bindings::_helpers::contract_command::command::run_contract_command;
use crate::bindings::_helpers::cost_command::command::run_cost_command;
use crate::bindings::_helpers::dbt_command::command::run_dbt_command;
use crate::bindings::_helpers::dbt_command::init::run_dbt_init_command;
use crate::bindings::_helpers::debug_command::run_debug::run_debug_command;
use crate::bindings::_helpers::diff_report::command::run_diff_command;
use crate::bindings::_helpers::diff_report::support::cli_error;
use crate::bindings::_helpers::format_command::command::run_format_command;
use crate::bindings::_helpers::freshness::freshness_command::run_freshness_command;
use crate::bindings::_helpers::init_command::command::run_init_command;
use crate::bindings::_helpers::janitor::janitor_command::run_janitor_command;
use crate::bindings::_helpers::lineage::lineage_command::run_lineage_command;
use crate::bindings::_helpers::load::load_command::run_load_command;
use crate::bindings::_helpers::location_command::command::run_location;
use crate::bindings::_helpers::plan_command::plan::run_plan_command;
use crate::bindings::_helpers::plan_command::rules::run_rules_command;
use crate::bindings::_helpers::query_command::dag::run_dag_command;
use crate::bindings::_helpers::query_command::query::run_query_command;
use crate::bindings::_helpers::recall::command::run_recall_command;
use crate::bindings::_helpers::recall::session::InvocationRecall;
use crate::bindings::_helpers::records::construct::{new_record_with, record_class};
use crate::bindings::_helpers::refactor::refactor_command::run_refactor_command;
use crate::bindings::_helpers::review_command::command::run_review_command;
use crate::bindings::_helpers::scenario_command::capture_command::run_scenario_capture_command;
use crate::bindings::_helpers::scenario_command::test_command::run_scenario_test_command;
use crate::bindings::_helpers::scopes::scope_command::run_scope_command;
use crate::bindings::_helpers::seed_command::command::run_seed_command;
use crate::bindings::_helpers::sql_test_command::test_command::run_test_command;
use crate::bindings::_helpers::workspace_commands::commands::{
    run_playground_command, run_skills_command,
};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyAttributeError, PyOSError, PyTypeError, PyUnicodeError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_cli::diff_report::errors::DiffUserError;
use sqlbuild_cli::diff_report::main::parse_diff_name_range::parse_diff_name_range;
use sqlbuild_cli::dispatch::main::command_dispatch::command_dispatch;
use sqlbuild_cli::dispatch::main::selector_file_selectors::selector_file_selectors;
use sqlbuild_cli::dispatch::main::unavailable_handler::unavailable_handler;
use sqlbuild_cli::dispatch::models::{
    ClassRef, CommandFacts, DiffSide, Dispatch, DispatchFailure, Field, Route, Source,
};
use sqlbuild_core::format_command::main::format_path_selectors::format_path_selectors;

use crate::bindings::_helpers::boundary::python_calls::{imported, within};
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::records::registry;

const PATHLIB_MODULE: &str = "pathlib";
const UTF8: &str = "utf-8";
/// The code of a `--select-file` that cannot be read.
const SELECTOR_FILE_CODE: &str = "C114";
const SELECTOR_FILE_HELP: &str =
    "Pass the path of a readable UTF-8 file, relative to the current directory or absolute.";
/// The `diff` arguments that compare raw queries instead of project models.
const RAW_QUERY_ATTRIBUTES: [&str; 4] = [
    "left_query",
    "left_query_file",
    "right_query",
    "right_query_file",
];

/// One invocation being dispatched: its namespace and the values derived from it once.
pub(crate) struct Invocation<'py> {
    args: Bound<'py, PyAny>,
    project_dir: Bound<'py, PyAny>,
    select: Bound<'py, PyTuple>,
    selector_files: Bound<'py, PyTuple>,
}

impl<'py> Invocation<'py> {
    pub(crate) fn new(args: &Bound<'py, PyAny>) -> PyResult<Self> {
        let raw_project_dir = native_field(args, "project_dir")?;
        let project_dir = if raw_project_dir.is_none() {
            raw_project_dir
        } else {
            path_class(args.py())?
                .call1((raw_project_dir,))?
                .call_method0("resolve")?
        };
        let (select, selector_files) = selector_inputs(args)?;
        if native_field(args, "select_file")?.is_truthy()? {
            withhold_launch_record(args.py())?;
        }
        Ok(Self {
            args: args.clone(),
            project_dir,
            select,
            selector_files,
        })
    }

    fn py(&self) -> Python<'py> {
        self.args.py()
    }

    fn attr(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.args.getattr(name)
    }

    fn flag(&self, name: &str) -> PyResult<bool> {
        self.attr(name)?.is_truthy()
    }

    /// The value of one handler argument or request field.
    pub(crate) fn value(&self, source: Source) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.py();
        match source {
            Source::Attr(name) => self.attr(name),
            Source::TupleOf(name) => tuple_of(&self.attr(name)?),
            Source::Text(name) => Ok(self.attr(name)?.str()?.into_any()),
            Source::OrDefault(name, default) => {
                let value: Bound<'py, PyAny> = self.attr(name)?;
                if value.is_truthy()? {
                    Ok(value)
                } else {
                    Ok(PyString::new(py, default).into_any())
                }
            }
            Source::OptionalPath(name) => {
                let value: Bound<'py, PyAny> = self.attr(name)?;
                if value.is_none() {
                    Ok(value)
                } else {
                    path_class(py)?.call1((value,))
                }
            }
            Source::Flag(value) => value.into_bound_py_any(py),
            Source::Member(class, name) => class_object(py, class)?.call1((self.attr(name)?,)),
            Source::Constant(class, value) => class_object(py, class)?.call1((value,)),
            Source::Nested(class, fields) => {
                class_object(py, class)?.call((), Some(&self.fields(fields)?))
            }
            Source::ProjectDir => Ok(self.project_dir.clone()),
            Source::EffectiveProjectDir => {
                if self.project_dir.is_none() {
                    path_class(py)?.call_method0("cwd")
                } else {
                    Ok(self.project_dir.clone())
                }
            }
            Source::Select => Ok(self.select.clone().into_any()),
            Source::SelectorFiles => Ok(self.selector_files.clone().into_any()),
            Source::QueryLimit => {
                if self.flag("query_no_limit")? {
                    Ok(py.None().into_bound(py))
                } else {
                    self.attr("query_limit")
                }
            }
            Source::DiffName(side) => self.diff_name(side),
            Source::FormatSelect => {
                let raw_paths: Bound<'py, PyAny> = self.attr("format_paths")?;
                let raw_paths: Vec<String> = if raw_paths.is_none() {
                    Vec::new()
                } else {
                    raw_paths.extract()?
                };
                let project_dir: Option<PathBuf> = if self.project_dir.is_none() {
                    None
                } else {
                    Some(self.project_dir.extract()?)
                };
                let cwd: PathBuf = path_class(py)?.call_method0("cwd")?.extract()?;
                let selectors: Vec<String> =
                    format_path_selectors(&raw_paths, project_dir.as_deref(), &cwd).map_err(
                        |error| {
                            cli_error(
                                py,
                                ErrorId::CliUser,
                                &DiffUserError {
                                    message: error.message,
                                    code: error.code.to_owned(),
                                    help: error.help,
                                },
                            )
                        },
                    )?;
                concatenated(
                    &self.select.clone().into_any(),
                    &PyTuple::new(py, selectors)?.into_any(),
                )
            }
            Source::ScenarioSelect => concatenated(
                &self.attr("scenario_selector")?,
                &self.select.clone().into_any(),
            ),
        }
    }

    fn diff_name(&self, side: DiffSide) -> PyResult<Bound<'py, PyAny>> {
        let range: Bound<'py, PyAny> = self.attr("target_range")?;
        if range.is_none() {
            return Ok(range);
        }
        let py: Python<'py> = self.py();
        let (from_name, to_name) = parse_diff_name_range(&range.extract::<String>()?)
            .map_err(|error| cli_error(py, ErrorId::CliUser, &error))?;
        match side {
            DiffSide::From => from_name.into_bound_py_any(py),
            DiffSide::To => to_name.into_bound_py_any(py),
        }
    }

    fn fields(&self, fields: &[Field]) -> PyResult<Bound<'py, PyDict>> {
        let values: Bound<'py, PyDict> = PyDict::new(self.py());
        for field in fields {
            values.set_item(field.name, self.value(field.source)?)?;
        }
        Ok(values)
    }
}

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.to_string()))
}

fn path_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, PATHLIB_MODULE, "Path")
}

fn class_object<'py>(py: Python<'py>, class: ClassRef) -> PyResult<Bound<'py, PyAny>> {
    record_class(py, registry::record_named(class.name)?)
}

fn tuple_of<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(value.py(), value.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any())
}

fn concatenated<'py>(
    first: &Bound<'py, PyAny>,
    second: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let items: Vec<Bound<'py, PyAny>> = first
        .try_iter()?
        .chain(second.try_iter()?)
        .collect::<PyResult<_>>()?;
    Ok(PyTuple::new(first.py(), items)?.into_any())
}

/// `CliUserError(message, code=..., help=...)`.
fn user_error(py: Python<'_>, failure: DispatchFailure) -> PyErr {
    let raised = (|| {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("code", failure.code)?;
        keywords.set_item("help", failure.help)?;
        error_class(py, ErrorId::CliUser)?.call((failure.message,), Some(&keywords))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// `--select` and the selectors of every `--select-file`, with each file's selector count.
fn selector_inputs<'py>(
    args: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyTuple>, Bound<'py, PyTuple>)> {
    let py: Python<'py> = args.py();
    let mut selectors: Vec<Bound<'py, PyAny>> = native_field(args, "select")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let mut files: Vec<Bound<'py, PyAny>> = Vec::new();
    for raw_path in native_field(args, "select_file")?.try_iter()? {
        let path: Bound<'py, PyAny> = path_class(py)?.call1((raw_path?,))?;
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("encoding", UTF8)?;
        let text: Bound<'py, PyAny> = match path.call_method("read_text", (), Some(&keywords)) {
            Ok(text) => text,
            Err(error) => return Err(selector_file_error(py, &path, &error)?),
        };
        let listed: Vec<String> = selector_file_selectors(&text.extract::<String>()?);
        let count: usize = listed.len();
        for selector in &listed {
            selectors.push(PyString::new(py, selector).into_any());
        }
        files.push(new_record_with(
            py,
            RecordId::SelectorFileSummary,
            &[
                ("path", path),
                ("selector_count", count.into_bound_py_any(py)?),
            ],
        )?);
    }
    Ok((PyTuple::new(py, selectors)?, PyTuple::new(py, files)?))
}

/// The user error for a `--select-file` that cannot be read, or `error` when it is unexpected.
fn selector_file_error(py: Python<'_>, path: &Bound<'_, PyAny>, error: &PyErr) -> PyResult<PyErr> {
    let readable =
        error.is_instance_of::<PyOSError>(py) || error.is_instance_of::<PyUnicodeError>(py);
    if !readable {
        return Ok(error.clone_ref(py));
    }
    let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
    let reason: Bound<'_, PyAny> = value
        .getattr_opt("strerror")?
        .unwrap_or_else(|| value.clone());
    let reason: String = if reason.is_none() {
        value.str()?.to_string()
    } else {
        reason.str()?.to_string()
    };
    Ok(user_error(
        py,
        DispatchFailure {
            message: format!("cannot read selector file {}: {reason}", path.str()?),
            code: SELECTOR_FILE_CODE,
            help: Some(SELECTOR_FILE_HELP),
        },
    ))
}

/// Call the route's command implementation, or the handler a test injected for its slot.
fn call_route<'py>(
    invocation: &Invocation<'py>,
    route: &Route,
    handler: Option<Bound<'py, PyAny>>,
    recall: &InvocationRecall,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    let mut positional: Vec<Bound<'py, PyAny>> = Vec::new();
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    let positional_count: usize = match &handler {
        Some(_) => route.shape.slot_positional,
        None => {
            if let Some(extra) = route.implementation_extra {
                keywords.set_item(extra.name, invocation.value(extra.source)?)?;
            }
            if route.shape.implementation_keywords {
                0
            } else {
                route.shape.slot_positional
            }
        }
    };
    for (index, field) in route.arguments.iter().enumerate() {
        let value: Bound<'py, PyAny> = invocation.value(field.source)?;
        if index < positional_count {
            positional.push(value);
        } else {
            keywords.set_item(field.name, value)?;
        }
    }
    match handler {
        Some(handler) => handler.call(PyTuple::new(py, positional)?, Some(&keywords)),
        None => run_implementation(route.implementation, (positional, keywords), recall),
    }
}

/// The argument passed first by position or under `name`, or `None`.
fn argument<'py>(
    (positional, keywords): &(Vec<Bound<'py, PyAny>>, Bound<'py, PyDict>),
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    match positional.first() {
        Some(value) => Ok(Some(value.clone())),
        None => keywords.get_item(name),
    }
}

/// Run the native command implementation `name` with the route's arguments.
fn run_implementation<'py>(
    name: &str,
    arguments: (Vec<Bound<'py, PyAny>>, Bound<'py, PyDict>),
    recall: &InvocationRecall,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = arguments.1.py();
    let request = || -> PyResult<Bound<'py, PyAny>> {
        argument(&arguments, "request")?
            .ok_or_else(|| PyTypeError::new_err(format!("{name}() missing argument 'request'")))
    };
    let options = || Some(arguments.1.clone());
    let code: i32 = match name {
        "run_audit_command" => run_audit_command(py, &request()?)?,
        "run_build_command" => run_build_command(py, &request()?)?,
        "run_check_command" => run_check_command(py, &request()?)?,
        "run_clone_command" => run_clone_command(py, &request()?)?,
        "run_compile" => return run_compile(&request()?),
        "run_contract_command" => run_contract_command(py, &request()?)?,
        "run_cost_command" => run_cost_command(py, &request()?)?,
        "run_dag_command" => run_dag_command(py, options())?,
        "run_dbt_command" => run_dbt_command(py, options())?,
        "run_dbt_init_command" => run_dbt_init_command(py, &request()?)?,
        "run_debug_command" => run_debug_command(py, options())?,
        "run_diff_command" => run_diff_command(py, &request()?)?,
        "run_format_command" => run_format_command(py, options())?,
        "run_freshness_command" => run_freshness_command(py, &request()?)?,
        "run_init_command" => run_init_command(
            py,
            argument(&arguments, "project_dir")?.filter(|value| !value.is_none()),
        )?,
        "run_janitor_command" => run_janitor_command(py, &request()?)?,
        "run_lineage_command" => run_lineage_command(py, &request()?)?,
        "run_load_command" => run_load_command(py, &request()?)?,
        "run_location_command" => run_location(py, options(), Some(recall))?,
        "run_plan_command" => run_plan_command(&request()?)?,
        "run_playground_command" => run_playground_command(py, &request()?)?,
        "run_query_command" => run_query_command(py, options())?,
        "run_recall_command" => run_recall_command(py, options())?,
        "run_refactor_command" => run_refactor_command(py, &request()?)?,
        "run_review_command" => run_review_command(py, &request()?)?,
        "run_rules_command" => run_rules_command(&request()?)?,
        "run_scenario_capture_command" => run_scenario_capture_command(py, &request()?)?,
        "run_scenario_test_command" => run_scenario_test_command(py, &request()?)?,
        "run_scope_command" => run_scope_command(
            py,
            &request()?,
            arguments.1.get_item("load_scope_index")?,
            arguments.1.get_item("output_stream")?,
        )?,
        "run_seed_command" => run_seed_command(py, &request()?)?,
        "run_skills_command" => run_skills_command(py, options())?,
        "run_test_command" => run_test_command(py, &request()?)?,
        _ => {
            return Err(PyAttributeError::new_err(format!(
                "module 'sqlbuild._native' has no attribute '{name}'"
            )));
        }
    };
    code.into_bound_py_any(py)
}

fn routed_command<'py>(
    args: &Bound<'py, PyAny>,
    handlers: Option<&Bound<'py, PyAny>>,
    recall: &InvocationRecall,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = args.py();
    let invocation = Invocation::new(args)?;
    match command_dispatch(&command_facts(args)?) {
        Dispatch::Nothing => 0.into_bound_py_any(py),
        Dispatch::Fail(failure) => Err(user_error(py, failure)),
        Dispatch::Run(route, failure) => {
            let handler: Option<Bound<'py, PyAny>> = match handlers {
                Some(handlers) => {
                    let slot: Bound<'py, PyAny> = handlers.getattr(route.slot)?;
                    if slot.is_none() {
                        let command: String = native_field(args, "command")?.str()?.to_string();
                        let code: &str = route.unavailable_code.unwrap_or("C000");
                        return Err(user_error(py, unavailable_handler(&command, code)));
                    }
                    Some(slot)
                }
                None => None,
            };
            if let Some(failure) = failure {
                return Err(user_error(py, failure));
            }
            call_route(&invocation, route, handler, recall)
        }
    }
}

/// Run a parsed command's handler, imported only when it runs unless `handlers` injects it.
pub(crate) fn dispatch_command<'py>(
    args: &Bound<'py, PyAny>,
    handlers: Option<Bound<'py, PyAny>>,
    recall: &InvocationRecall,
) -> PyResult<Bound<'py, PyAny>> {
    within(&warehouse_scope(args)?, |_| {
        routed_command(args, handlers.as_ref(), recall)
    })
}

pub(crate) fn warehouse_scope<'py>(args: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = args.py();
    let warehouse: Bound<'py, PyAny> = native_field(args, "warehouse")?;
    if !warehouse.is_none() {
        let raw_project_dir: Bound<'py, PyAny> = native_field(args, "project_dir")?;
        let project_dir: Bound<'py, PyAny> = if raw_project_dir.is_none() {
            path_class(py)?.call_method0("cwd")?
        } else {
            path_class(py)?
                .call1((raw_project_dir,))?
                .call_method0("resolve")?
        };
        validate_cli_warehouse(
            py,
            warehouse.clone().extract::<Option<String>>()?,
            project_dir,
        )
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    }
    Ok(Bound::new(
        py,
        command_warehouse_scope(
            &native_field(args, "command")?,
            &warehouse,
            Some(&native_field(args, "vars")?),
        )?,
    )?
    .into_any())
}

/// The parsed values that select a route, read from the namespace.
pub(crate) fn command_facts(args: &Bound<'_, PyAny>) -> PyResult<CommandFacts> {
    let attr = |name: &str| native_field(args, name);
    let flag = |name: &str| -> PyResult<bool> { attr(name)?.is_truthy() };
    let present = |name: &str| -> PyResult<bool> { Ok(!attr(name)?.is_none()) };
    let command: Option<String> = optional_text(&attr("command")?)?;
    let subcommand_attribute: &str = match command.as_deref() {
        Some("dbt") => "dbt_command",
        Some("scenario") => "scenario_command",
        _ => "command",
    };
    let mut raw_query_diff: bool = false;
    for name in RAW_QUERY_ATTRIBUTES {
        raw_query_diff |= present(name)?;
    }
    Ok(CommandFacts {
        subcommand: optional_text(&attr(subcommand_attribute)?)?,
        command,
        json: flag("json")?,
        from_target: present("from_target")?,
        contract_ready: present("contract_command")? && present("contract_from")?,
        format_diff: flag("format_diff")?,
        format_fix: flag("format_fix")?,
        format_fixtures_only: flag("format_fixtures_only")?,
        scenario_local: flag("scenario_local")?,
        scenario_retain: flag("scenario_retain")?,
        scenario_snapshot_sync: flag("scenario_sync_snapshots")? || flag("scenario_refresh")?,
        raw_query_diff,
    })
}
