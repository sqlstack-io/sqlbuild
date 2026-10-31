//! The `sqb` entry: invocation scopes, parsing, observed dispatch and the error projection.

use std::sync::Arc;
use std::time::{Instant, SystemTime};

use pyo3::exceptions::PySystemExit;
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{
    PyBool, PyCFunction, PyDateTime, PyDict, PyDictMethods, PyInt, PyList, PyString,
    PyStringMethods, PyTuple, PyTypeMethods, PyTzInfo,
};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_cli::dispatch::main::failure_report::failure_report;
use sqlbuild_cli::dispatch::main::invocation_policy::invocation_policy;
use sqlbuild_cli::dispatch::models::{CommandFacts, CommandFailure, InvocationPolicy};

use sqlbuild_host::entries::models::{ExecutorHost, FacadeHost};
use sqlbuild_orchestration::runs::models::RunOutcome;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, keywords, within};
use crate::bindings::_helpers::build_progress::transient_lines::shared_transient_line_coordinator;
use crate::bindings::_helpers::cli::argv::parse_cli_invocation;
use crate::bindings::_helpers::cli::dispatch::{command_facts, dispatch_command};
use crate::bindings::_helpers::cli::failure_cause::take_failure_cause;
use crate::bindings::_helpers::cli::launch_state::offer_launch_record;
use crate::bindings::_helpers::cli::machine_output::take_machine_output_written;
use crate::bindings::_helpers::cli::observability::{
    CliObservabilityScope, cli_observability_scope,
};
use crate::bindings::_helpers::cli::termination::{
    CommandTerminated, TERMINATED_MESSAGE, TerminationHandler, handle_sigterm, interrupting_signal,
    signal_exit_code, terminated, terminated_exit,
};
use crate::bindings::_helpers::compile_command::report::format_compile_error_json;
use crate::bindings::_helpers::cost_report::attribution::take_cost_status;
use crate::bindings::_helpers::diff_report::output::render_diff_error_json;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::output_capture::records::invocation_context;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::recall::constants::{JSON_FORMAT, RECALL_SETTINGS_CODE};
use crate::bindings::_helpers::recall::events::escaped_text;
use crate::bindings::_helpers::recall::session::{
    InvocationRecall, effective_selection, finish_invocation_recall, start_invocation_recall,
};
use crate::bindings::_helpers::recall::sinks::{RecallLifecycleSink, publish_lifecycle_event};
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::construct::{error_class, new_error};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};
use crate::bindings::_helpers::workspace_commands::commands::maintain_sqlbuild_skills;
use sqlbuild_cli::launcher::constants::LAUNCH_RECORD_OPTION;
use sqlbuild_core::cli_text::main::format_coded_error::format_coded_error;
use sqlbuild_core::cli_text::models::CliStyle;

const NO_COLOR_OPTION: &str = "--no-color";
const CLI_LOGGER: &str = "sqlbuild.cli";
const UNKNOWN_COMMAND: &str = "unknown";
const DIFF_PUBLICATION_CODE: &str = "C257";
const DIFF_EXECUTION_STATUS: &str = "execution_failed";
const DIFF_OUTCOME_PREFIX: &str = "Query diff outcome  ";
const ERROR_MESSAGE_CHARACTERS: usize = 500;
/// The error type recorded when a raised error cannot be read.
const FALLBACK_ERROR_TYPE: &str = "Exception";
/// Expected staying-Python command failures, loaded only once a command raised.
const EXPECTED_COMMAND_FAILURES: [ExecutorHost; 4] = [
    ExecutorHost::AuditExecutionError,
    ExecutorHost::ExecutionDurationLimitError,
    ExecutorHost::WarehouseConnectionError,
    ExecutorHost::TemporaryTestFunctionError,
];
/// Expected native command failures.
const EXPECTED_NATIVE_FAILURES: [ErrorId; 2] = [ErrorId::Discovery, ErrorId::SpecConfig];

/// One parsed invocation: its namespace, injected handlers and error colour.
struct Invocation<'py> {
    args: Bound<'py, PyAny>,
    handlers: Option<Bound<'py, PyAny>>,
    use_color: bool,
    facts: CommandFacts,
    recall: InvocationRecall,
}

impl<'py> Invocation<'py> {
    fn py(&self) -> Python<'py> {
        self.args.py()
    }

    fn command_is(&self, name: &str) -> bool {
        self.facts.command.as_deref() == Some(name)
    }

    fn flag(&self, name: &str) -> PyResult<bool> {
        self.args.getattr(name)?.is_truthy()
    }

    fn project_dir(&self) -> PyResult<Bound<'py, PyAny>> {
        let path: Bound<'py, PyAny> = imported(self.py(), "pathlib", "Path")?;
        let raw: Bound<'py, PyAny> = self.args.getattr("project_dir")?;
        if raw.is_none() {
            path.call_method0("cwd")
        } else {
            path.call1((raw,))
        }
    }

    fn command_name(&self) -> String {
        self.facts
            .command
            .clone()
            .unwrap_or_else(|| UNKNOWN_COMMAND.to_owned())
    }
}

/// `print(value, file=sys.<stream>, end=<end>)`.
fn print_object(value: &Bound<'_, PyAny>, stream: Option<&str>, end: Option<&str>) -> PyResult<()> {
    let py: Python<'_> = value.py();
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    if let Some(stream) = stream {
        keywords.set_item("file", py.import("sys")?.getattr(stream)?)?;
    }
    if let Some(end) = end {
        keywords.set_item("end", end)?;
    }
    imported(py, "builtins", "print")?.call((value,), Some(&keywords))?;
    Ok(())
}

/// `(code, message, help)` of an expected error as printable strings, lone surrogates escaped.
fn error_parts<'py>(
    error: &Bound<'py, PyAny>,
    fallback_code: &str,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = error.py();
    let printable = |text: Bound<'py, PyString>| -> PyResult<Bound<'py, PyAny>> {
        Ok(PyString::new(py, &escaped_text(&text)?).into_any())
    };
    let code: Bound<'py, PyAny> = match error.getattr_opt("code")? {
        Some(code) => printable(code.str()?)?,
        None => PyString::new(py, fallback_code).into_any(),
    };
    let message: Bound<'py, PyAny> = match error.getattr_opt("message")? {
        Some(message) => printable(message.str()?)?,
        None => printable(error.str()?)?,
    };
    let help: Bound<'py, PyAny> = match error.getattr_opt("help")? {
        Some(help) if !help.is_none() => printable(help.str()?)?,
        _ => py.None().into_bound(py),
    };
    Ok((code, message, help))
}

/// Print the coded error text of `error` to stderr.
fn print_expected_error(
    invocation: &Invocation<'_>,
    error: &Bound<'_, PyAny>,
    fallback_code: &str,
) -> PyResult<()> {
    let py: Python<'_> = invocation.py();
    let (code, message, help) = error_parts(error, fallback_code)?;
    let help: Option<String> = if help.is_none() {
        None
    } else {
        Some(help.extract()?)
    };
    let text: String = format_coded_error(
        (
            &code.extract::<String>()?,
            &message.extract::<String>()?,
            help.as_deref(),
        ),
        true,
        CliStyle {
            use_color: invocation.use_color,
        },
    );
    print_object(
        &PyString::new(py, &format!("{text}\n")).into_any(),
        Some("stderr"),
        None,
    )
}

/// The compile JSON error document `sqb compile --json` prints for a failure.
fn write_compile_machine_error(
    invocation: &Invocation<'_>,
    error: &Bound<'_, PyAny>,
    fallback_code: &str,
) -> PyResult<()> {
    if !invocation.command_is("compile") || !invocation.flag("json")? {
        return Ok(());
    }
    let py: Python<'_> = invocation.py();
    let (code, message, help) = error_parts(error, fallback_code)?;
    let location: Bound<'_, PyAny> = error
        .getattr_opt("location")?
        .unwrap_or_else(|| py.None().into_bound(py));
    let location: Bound<'_, PyAny> =
        if location.is_instance(&record_class(py, RecordId::SourceLocation)?)? {
            location
        } else {
            py.None().into_bound(py)
        };
    let document: Bound<'_, PyAny> = format_compile_error_json(
        code,
        message,
        help,
        Some(location).filter(|value| !value.is_none()),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    print_object(&document, None, None)
}

/// With `sqb recall … --json`, `--json` on `sqb plan`, `sqb rename` or `sqb mv`, or `sqb janitor --json` (before it printed its own document), the failure as `{"error": {"code", "message", "help"}}` on stdout, so callers parsing the output read the error instead of failing to parse it.
fn write_machine_error(
    invocation: &Invocation<'_>,
    error: &Bound<'_, PyAny>,
    fallback_code: &str,
) -> PyResult<()> {
    let json_output: bool = if invocation.command_is("recall") {
        let format: Option<String> = invocation
            .args
            .getattr_opt("recall_format")?
            .map(|format| format.extract::<Option<String>>())
            .transpose()?
            .flatten();
        format.as_deref() == Some(JSON_FORMAT)
    } else if invocation.command_is("janitor") {
        invocation.flag("json")? && !take_machine_output_written()
    } else {
        (invocation.command_is("plan")
            || invocation.command_is("rename")
            || invocation.command_is("mv"))
            && invocation.flag("json")?
    };
    if !json_output {
        return Ok(());
    }
    let (code, message, help) = error_parts(error, fallback_code)?;
    let help: Option<String> = if help.is_none() {
        None
    } else {
        Some(help.str()?.to_string())
    };
    let document = serde_json::json!({
        "error": {
            "code": code.str()?.to_string(),
            "message": message.str()?.to_string(),
            "help": help,
        }
    });
    print_object(
        &PyString::new(invocation.py(), &document.to_string()).into_any(),
        None,
        None,
    )
}

fn render_diff_error<'py>(
    py: Python<'py>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    render_diff_error_json(py, Some(keywords(py, fields)?))
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))
}

/// The diff JSON error document, and the publication error that replaces `error` if any.
fn write_diff_machine_error<'py>(
    invocation: &Invocation<'py>,
    error: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    if !invocation.command_is("diff") {
        return Ok(None);
    }
    let py: Python<'py> = invocation.py();
    let json_stdout: bool = invocation.flag("json")?;
    let json_path: Bound<'py, PyAny> = native_field(&invocation.args, "json_output")?;
    if !json_stdout && json_path.is_none() {
        return Ok(None);
    }
    let status: Bound<'py, PyAny> = match error.getattr_opt("status")? {
        Some(status) => status.str()?.into_any(),
        None => PyString::new(py, DIFF_EXECUTION_STATUS).into_any(),
    };
    let size_guard: Bound<'py, PyAny> = error
        .getattr_opt("details")?
        .unwrap_or_else(|| py.None().into_bound(py));
    let payload: Bound<'py, PyAny> = render_diff_error(
        py,
        &[
            ("status", status),
            ("code", native_field(error, "code")?),
            ("message", native_field(error, "message")?),
            ("help_text", native_field(error, "help")?),
            ("size_guard", size_guard),
        ],
    )?;
    if !json_path.is_none() {
        let published: PyResult<()> = (|| {
            let parent: Bound<'py, PyAny> = native_field(&json_path, "parent")?;
            let options: Bound<'py, PyDict> = PyDict::new(py);
            options.set_item("parents", true)?;
            options.set_item("exist_ok", true)?;
            parent.call_method("mkdir", (), Some(&options))?;
            let encoding: Bound<'py, PyDict> = PyDict::new(py);
            encoding.set_item("encoding", "utf-8")?;
            json_path.call_method("write_text", (payload.add("\n")?,), Some(&encoding))?;
            Ok(())
        })();
        if let Err(output_error) = published {
            let value: Bound<'py, PyAny> = output_error.value(py).clone().into_any();
            if !value.is_instance_of::<pyo3::exceptions::PyOSError>()
                && !value.is_instance_of::<pyo3::exceptions::PyUnicodeError>()
            {
                return Err(output_error);
            }
            let keywords: Bound<'py, PyDict> = PyDict::new(py);
            keywords.set_item("code", DIFF_PUBLICATION_CODE)?;
            let publication: Bound<'py, PyAny> = error_class(py, ErrorId::QueryDiffExecution)?
                .call(
                    (format!(
                        "failed to publish query diff output: {}",
                        value.str()?
                    ),),
                    Some(&keywords),
                )?;
            if json_stdout {
                let document: Bound<'py, PyAny> = render_diff_error(
                    py,
                    &[
                        ("status", native_field(&publication, "status")?),
                        ("code", native_field(&publication, "code")?),
                        ("message", native_field(&publication, "message")?),
                    ],
                )?;
                print_object(&document, None, None)?;
            }
            return Ok(Some(publication));
        }
    }
    if json_stdout {
        print_object(&payload, None, None)?;
    }
    Ok(None)
}

/// Log `error` with its traceback at error level, as `logger.exception` does.
fn log_failure(py: Python<'_>, message: &str, error: &Bound<'_, PyAny>) -> PyResult<()> {
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("exc_info", error)?;
    py.import("logging")?
        .getattr("getLogger")?
        .call1((CLI_LOGGER,))?
        .call_method("error", (message,), Some(&keywords))?;
    Ok(())
}

fn exit<'py>(py: Python<'py>, code: i64) -> PyResult<Bound<'py, PyAny>> {
    code.into_bound_py_any(py)
}

/// The kind of expected failure `error` is, or `None` when it propagates.
fn failure_kind(py: Python<'_>, error: &Bound<'_, PyAny>) -> PyResult<Option<CommandFailure>> {
    if error.is_instance(&error_class(py, ErrorId::CliUser)?)?
        || error.is_instance(&error_class(py, ErrorId::Rules)?)?
    {
        return Ok(Some(CommandFailure::User));
    }
    if error.is_instance(&error_class(py, ErrorId::Lint)?)? {
        return Ok(Some(CommandFailure::Lint));
    }
    if error.is_instance_of::<pyo3::exceptions::PyValueError>() {
        return Ok(Some(CommandFailure::Command));
    }
    for failure in EXPECTED_COMMAND_FAILURES {
        if error.is_instance(&host::entry(py, failure)?)? {
            return Ok(Some(CommandFailure::Command));
        }
    }
    for native in EXPECTED_NATIVE_FAILURES {
        if error.is_instance(&error_class(py, native)?)? {
            return Ok(Some(CommandFailure::Command));
        }
    }
    Ok(None)
}

/// Report one expected failure and return the command's exit code.
fn report_failure<'py>(
    invocation: &Invocation<'py>,
    error: Bound<'py, PyAny>,
    kind: CommandFailure,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    let (fallback_code, log_message) = failure_report(kind);
    log_failure(py, log_message, &error)?;
    let user_error_class: Bound<'py, PyAny> = error_class(py, ErrorId::CliUser)?;
    let is_user_error: bool = error.is_instance(&user_error_class)?;
    let effective: Bound<'py, PyAny> = if is_user_error {
        write_diff_machine_error(invocation, &error)?.unwrap_or(error)
    } else {
        error
    };
    write_compile_machine_error(invocation, &effective, fallback_code)?;
    write_machine_error(invocation, &effective, fallback_code)?;
    if effective.is_instance(&error_class(py, ErrorId::QueryDiffOutcome)?)? {
        let line: Bound<'py, PyAny> = PyString::new(py, DIFF_OUTCOME_PREFIX)
            .into_any()
            .add(native_field(&effective, "status")?.str()?)?;
        print_object(&line, Some("stderr"), None)?;
    }
    print_expected_error(invocation, &effective, fallback_code)?;
    if effective.is_instance(&user_error_class)? {
        let code: Bound<'py, PyAny> = effective
            .getattr_opt("exit_code")?
            .unwrap_or_else(|| PyInt::new(py, 1).into_any());
        return py.get_type::<PyInt>().call1((code,));
    }
    exit(py, 1)
}

/// Check or refresh installed SQLBuild skills, printing what changed.
fn report_skill_freshness(invocation: &Invocation<'_>, policy: InvocationPolicy) -> PyResult<()> {
    if !policy.maintains_skills {
        return Ok(());
    }
    let py: Python<'_> = invocation.py();
    let project_dir: Bound<'_, PyAny> = invocation.project_dir()?;
    let result: Bound<'_, PyAny> =
        match maintain_sqlbuild_skills(py, project_dir.extract::<std::path::PathBuf>()?) {
            Ok(result) => result,
            Err(error) => {
                let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
                if value.is_instance(&error_class(py, ErrorId::CliUser)?)?
                    || value.is_instance_of::<pyo3::exceptions::PyOSError>()
                    || value.is_instance_of::<pyo3::exceptions::PyUnicodeError>()
                {
                    return Ok(());
                }
                return Err(error);
            }
        };
    let message: Bound<'_, PyAny> = native_field(&result, "message")?;
    if message.is_truthy()? {
        print_object(&message, Some("stderr"), Some(""))?;
    }
    Ok(())
}

fn lifecycle_event<'py>(
    py: Python<'py>,
    event_type: &str,
    payload: Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host::entry(py, ExecutorHost::CreateLifecycleEvent)?,
        &[
            ("event_type", PyString::new(py, event_type).into_any()),
            ("payload", payload.into_any()),
        ],
    )
}

/// Install the discovered Recall replacement sinks, release the context row, and subscribe the session to every lifecycle fact and SQL artifact.
fn subscribe_recall(
    (scope, dispatcher): (&Bound<'_, CliObservabilityScope>, &Bound<'_, PyAny>),
    recall: &InvocationRecall,
) -> PyResult<()> {
    let Some(session) = recall.session.clone() else {
        return Ok(());
    };
    let py: Python<'_> = dispatcher.py();
    let (lifecycle, output) = scope.get().recall_sinks(py)?;
    let filter_active: bool = recall
        .replacements
        .configure(&session, (&lifecycle, &output))?;
    session.publish_context(filter_active);
    let sink = Bound::new(
        py,
        RecallLifecycleSink {
            session,
            replacements: Arc::clone(&recall.replacements),
            running_sql: Arc::default(),
            invocation_events: Arc::clone(&recall.invocation_events),
        },
    )?;
    dispatcher.call_method1("subscribe_history", (sink,))?;
    Ok(())
}

/// `target`, `adapter` and `selection_count` of the started invocation, when known; the count includes what selector files list.
fn add_invocation_context(
    invocation: &Invocation<'_>,
    payload: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if let Some(target) = &invocation.recall.target {
        payload.set_item("target", target)?;
    }
    if let Some(adapter) = &invocation.recall.adapter {
        payload.set_item("adapter", adapter)?;
    }
    let select: Bound<'_, PyAny> = invocation.args.getattr("select")?;
    if !select.is_none() {
        let (selectors, _) = effective_selection(&invocation.args)?;
        payload.set_item("selection_count", selectors.len())?;
    }
    Ok(())
}

/// The first line of `message`, at most [`ERROR_MESSAGE_CHARACTERS`] characters.
fn short_message(message: &str) -> String {
    let line: &str = message.lines().next().unwrap_or_default().trim();
    line.chars().take(ERROR_MESSAGE_CHARACTERS).collect()
}

/// A raised error's type name, code (empty for `SystemExit` or none) and short message; a part that cannot be read is left empty, so the original error and its terminal event survive.
fn raised_cause(py: Python<'_>, error: &PyErr) -> (String, String, String) {
    let error_type: String = error.get_type(py).name().map_or_else(
        |_unreadable| FALLBACK_ERROR_TYPE.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
    let readable = |part: &Bound<'_, PyAny>| -> String {
        part.str()
            .map(|text| text.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let (code, message): (String, String) = match error_parts(&value, "") {
        Ok((code, message, _)) => (readable(&code), short_message(&readable(&message))),
        Err(_unreadable) => (String::new(), String::new()),
    };
    if error.is_instance_of::<PySystemExit>(py) {
        return (error_type, String::new(), message);
    }
    (error_type, code, message)
}

/// Dispatch once while publishing canonical invocation lifecycle facts.
fn observed_dispatch<'py>(
    invocation: &Invocation<'py>,
    policy: InvocationPolicy,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    if !policy.observed {
        if let Some(session) = &invocation.recall.session {
            session.publish_context(false);
        }
        return dispatch_command(
            &invocation.args,
            invocation.handlers.clone(),
            &invocation.recall,
        );
    }
    let command: String = invocation.command_name();
    drop(take_failure_cause());
    drop(take_cost_status());
    let started: Instant = Instant::now();
    let started_at: SystemTime = SystemTime::now();
    let elapsed = |started: Instant| -> PyResult<Bound<'py, PyAny>> {
        u64::try_from(started.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .into_bound_py_any(py)
    };
    let observability: Bound<'py, CliObservabilityScope> = Bound::new(
        py,
        cli_observability_scope(
            invocation.args.clone().unbind(),
            invocation.project_dir()?.unbind(),
        ),
    )?;
    let started_payload = || -> PyResult<Bound<'py, PyDict>> {
        let payload: Bound<'py, PyDict> = PyDict::new(py);
        payload.set_item("command", &command)?;
        add_invocation_context(invocation, &payload)?;
        Ok(payload)
    };
    let failed_payload = |error: &PyErr| -> PyResult<Bound<'py, PyDict>> {
        let payload: Bound<'py, PyDict> = PyDict::new(py);
        payload.set_item("command", &command)?;
        payload.set_item("duration_ms", elapsed(started)?)?;
        let (error_type, code, message): (String, String, String) = raised_cause(py, error);
        payload.set_item("error_type", error_type)?;
        if !code.is_empty() {
            payload.set_item("error_code", code)?;
        }
        payload.set_item("error_message", message)?;
        if let Some(signal) = interrupting_signal(py, error) {
            payload.set_item("signal_number", signal)?;
            payload.set_item("exit_code", signal_exit_code(signal))?;
        }
        if let Some(status) = take_cost_status() {
            payload.set_item("cost_status", status)?;
        }
        Ok(payload)
    };
    let scope: Bound<'py, PyAny> = observability.clone().into_any();
    let observed: PyResult<Bound<'py, PyAny>> = within(&scope, |dispatcher| {
        let publish = |event: Bound<'py, PyAny>| -> PyResult<()> {
            dispatcher.call_method1("publish_lifecycle", (event,))?;
            Ok(())
        };
        subscribe_recall((&observability, dispatcher), &invocation.recall)?;
        publish(lifecycle_event(
            py,
            "invocation_started",
            started_payload()?,
        )?)?;
        let dispatched: PyResult<Bound<'py, PyAny>> = match dispatch_command(
            &invocation.args,
            invocation.handlers.clone(),
            &invocation.recall,
        ) {
            Ok(_) if terminated() => Err(CommandTerminated::new_err(TERMINATED_MESSAGE)),
            dispatched => dispatched,
        };
        match dispatched {
            Ok(exit_code) => {
                let payload: Bound<'py, PyDict> = PyDict::new(py);
                payload.set_item("command", &command)?;
                payload.set_item("duration_ms", elapsed(started)?)?;
                payload.set_item("exit_code", &exit_code)?;
                if let Some(status) = take_cost_status() {
                    payload.set_item("cost_status", status)?;
                }
                let terminal: &str = if exit_code.eq(0)? {
                    "invocation_completed"
                } else {
                    if let Some(cause) = take_failure_cause() {
                        payload.set_item("error_type", cause.error_type)?;
                        payload.set_item("error_code", cause.error_code)?;
                        payload.set_item("error_message", short_message(&cause.message))?;
                    }
                    "invocation_failed"
                };
                publish(lifecycle_event(py, terminal, payload)?)?;
                Ok(exit_code)
            }
            Err(error) => {
                publish(lifecycle_event(
                    py,
                    "invocation_failed",
                    failed_payload(&error)?,
                )?)?;
                Err(error)
            }
        }
    });
    if let (Err(error), Some(session)) = (&observed, &invocation.recall.session)
        && interrupting_signal(py, error).is_some()
        && !invocation.recall.invocation_events.ended()
    {
        let started: Option<PyResult<Bound<'py, PyDict>>> =
            (!invocation.recall.invocation_events.started()).then(started_payload);
        let recorded: PyResult<()> = started.transpose().and_then(|started| {
            record_interrupted_end(
                py,
                session,
                started.map(|payload| (started_at, payload)),
                failed_payload(error)?,
            )
        });
        drop(recorded);
    }
    observed
}

/// Record the end of an invocation interrupted before its own lifecycle events were all recorded, such as during project discovery, so Recall shows it cancelled instead of running. `started` holds the start event's time and payload when that event was not recorded either.
fn record_interrupted_end(
    py: Python<'_>,
    session: &sqlbuild_recall::writer::models::RecallSession,
    started: Option<(SystemTime, Bound<'_, PyDict>)>,
    failed: Bound<'_, PyDict>,
) -> PyResult<()> {
    if let Some((started_at, payload)) = started {
        let seconds: f64 = started_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64());
        let occurred_at: Bound<'_, PyDateTime> =
            PyDateTime::from_timestamp(py, seconds, Some(&PyTzInfo::utc(py)?.to_owned()))?;
        let started: Bound<'_, PyAny> = call_with(
            &host::entry(py, ExecutorHost::CreateLifecycleEvent)?,
            &[
                (
                    "event_type",
                    PyString::new(py, "invocation_started").into_any(),
                ),
                ("payload", payload.into_any()),
                ("occurred_at", occurred_at.into_any()),
            ],
        )?;
        publish_lifecycle_event(py, session, &started)?;
    }
    publish_lifecycle_event(
        py,
        session,
        &lifecycle_event(py, "invocation_failed", failed)?,
    )
}

/// Dispatch once and project every expected failure to its coded error and exit code.
fn handled_dispatch<'py>(invocation: &Invocation<'py>) -> PyResult<Bound<'py, PyAny>> {
    let _ = take_machine_output_written();
    let handled: PyResult<Bound<'py, PyAny>> = report_dispatch(invocation);
    let _ = take_machine_output_written();
    handled
}

/// Dispatch the command and report its failure, writing any machine-readable error document.
fn report_dispatch<'py>(invocation: &Invocation<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.py();
    let policy: InvocationPolicy = invocation_policy(&invocation.facts);
    report_skill_freshness(invocation, policy)?;
    let dispatched: PyResult<Bound<'py, PyAny>> = match &invocation.recall.configuration_error {
        Some(message) => Err(new_error(
            py,
            ErrorId::CliUser,
            message.clone(),
            RECALL_SETTINGS_CODE,
        )),
        None => observed_dispatch(invocation, policy),
    };
    shared_transient_line_coordinator(py)
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
        .call_method0("clear_active")?;
    let error: PyErr = match dispatched {
        Ok(exit_code) => return Ok(exit_code),
        Err(error) => error,
    };
    let value: Bound<'py, PyAny> = error.value(py).clone().into_any();
    if error.is_instance_of::<PySystemExit>(py) {
        let code: Bound<'py, PyAny> = native_field(&value, "code")?;
        return if code.is_instance_of::<PyInt>() {
            Ok(code)
        } else {
            exit(py, 1)
        };
    }
    match failure_kind(py, &value)? {
        Some(kind) => report_failure(invocation, value, kind),
        None => Err(error),
    }
}

/// `invocation_context_from_environment()`, or `{}` when the environment context is invalid.
fn external_context(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    match invocation_context(py, None) {
        Ok(context) => Ok(context),
        Err(error)
            if error.is_instance(py, &host::entry(py, ExecutorHost::OutputCaptureInputError)?) =>
        {
            Ok(PyDict::new(py).into_any())
        }
        Err(error) => Err(error),
    }
}

fn parsed_invocation<'py>(
    argv: &Bound<'py, PyAny>,
    handlers: Option<Bound<'py, PyAny>>,
    identity: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = argv.py();
    let use_color: bool = !argv.contains(NO_COLOR_OPTION)? && stdout_supports_color(py)?;
    let parsed: Bound<'py, PyAny> = parse_cli_invocation(py, argv.extract()?, use_color)?;
    let args: Bound<'py, PyAny> = native_field(&parsed, "args")?;
    if args.is_none() {
        let exit_code: Bound<'py, PyAny> = native_field(&parsed, "exit_code")?;
        return if exit_code.is_none() {
            exit(py, 1)
        } else {
            Ok(exit_code)
        };
    }
    let namespace = args.getattr("__dict__")?.cast_into::<PyDict>()?;
    let command: Option<String> = namespace
        .get_item("command")?
        .map(|value| value.extract())
        .transpose()?;
    let location_command: Option<String> = namespace
        .get_item("location_command")?
        .map(|value| value.extract())
        .transpose()?;
    if command.as_deref() == Some("location") && location_command.as_deref() == Some("runtime") {
        return within(&host::redirect_stdout_to_stderr(py)?, |_| {
            execute_parsed_invocation(args, handlers, identity, use_color)
        });
    }
    execute_parsed_invocation(args, handlers, identity, use_color)
}

/// Runtime stream routing must cover extension discovery, invocation scopes and their teardown.
fn execute_parsed_invocation<'py>(
    args: Bound<'py, PyAny>,
    handlers: Option<Bound<'py, PyAny>>,
    identity: &Bound<'py, PyAny>,
    use_color: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = args.py();
    let facts: CommandFacts = command_facts(&args)?;
    let policy: InvocationPolicy = invocation_policy(&facts);
    let recall: InvocationRecall = if policy.recalled {
        start_invocation_recall(py, &args, identity, &facts)?
    } else {
        InvocationRecall::default()
    };
    let recall_sink: Bound<'py, PyAny> = match recall.session.clone() {
        Some(session) => Bound::new(
            py,
            RecallLifecycleSink {
                session,
                replacements: Arc::clone(&recall.replacements),
                running_sql: Arc::default(),
                invocation_events: Arc::clone(&recall.invocation_events),
            },
        )?
        .into_any(),
        None => py.None().into_bound(py),
    };
    let closure_recall: InvocationRecall = recall.clone();
    let retained: (Py<PyAny>, Option<Py<PyAny>>) =
        (args.clone().unbind(), handlers.map(Bound::unbind));
    let operation: Bound<'py, PyCFunction> = PyCFunction::new_closure(
        py,
        None,
        None,
        move |call_args: &Bound<'_, PyTuple>,
              _: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py: Python<'_> = call_args.py();
            let invocation = Invocation {
                args: retained.0.bind(py).clone(),
                handlers: retained
                    .1
                    .as_ref()
                    .map(|handlers| handlers.bind(py).clone()),
                use_color,
                facts: facts.clone(),
                recall: closure_recall.clone(),
            };
            Ok(handled_dispatch(&invocation)?.unbind())
        },
    )?;
    let evidence_scope: Bound<'py, PyAny> = call_with(
        &host::entry(py, ExecutorHost::AuditEvidenceScope)?,
        &[(
            "mode",
            PyString::new(py, recall.audit_evidence.setting()).into_any(),
        )],
    )?;
    let dispatched: PyResult<Bound<'py, PyAny>> = within(&evidence_scope, |_| {
        if policy.creates_project {
            call_with(&operation.into_any(), &[])
        } else {
            call_with(
                &host::entry(py, FacadeHost::DispatchWithComputeLogs)?,
                &[
                    ("args", args),
                    ("identity", identity.clone()),
                    ("operation", operation.into_any()),
                    ("recall", recall_sink),
                    (
                        "capture",
                        PyBool::new(py, policy.captured).to_owned().into_any(),
                    ),
                ],
            )
        }
    });
    let exit_status: Option<i64> = match &dispatched {
        Ok(code) if code.is_instance_of::<PyInt>() => Some(code.extract().unwrap_or(1)),
        Ok(_) | Err(_) => None,
    };
    let finished: PyResult<()> = finish_invocation_recall(py, &recall, exit_status);
    let worker = recall
        .worker
        .lock()
        .map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err("worker completion lock is unavailable")
        })?
        .take();
    if let Some(worker) = worker {
        let code = match &finished {
            Ok(()) => exit_status.unwrap_or(1),
            Err(_) => 1,
        };
        let outcome = worker.finish(py, i32::try_from(code).unwrap_or(1))?;
        host::write_console(
            py,
            "stderr",
            &format!("Worker execution completed: {}\n", outcome.as_str()),
        )?;
        if !matches!(outcome, RunOutcome::Success | RunOutcome::Warn) {
            dispatched?;
            finished?;
            return exit(py, code.max(1));
        }
    }
    let exit_code: Bound<'py, PyAny> = dispatched?;
    finished?;
    Ok(exit_code)
}

/// `argv` without a leading launch record option, after offering the record it names.
fn offered_launch_record<'py>(argv: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = argv.py();
    let mut first: Option<String> = None;
    if argv.len()? > 0 {
        let item: Bound<'py, PyAny> = argv.get_item(0)?;
        if item.is_instance_of::<PyString>() {
            first = Some(item.extract()?);
        }
    }
    let Some(path) =
        first.and_then(|first| first.strip_prefix(LAUNCH_RECORD_OPTION).map(str::to_owned))
    else {
        offer_launch_record(py, None)?;
        return Ok(argv.clone());
    };
    offer_launch_record(py, Some(imported(py, "pathlib", "Path")?.call1((path,))?))?;
    argv.get_item(pyo3::types::PySlice::new(py, 1, isize::MAX, 1))
}

/// Run the CLI for `argv` (default `sys.argv[1:]`) and return its exit code.
#[pyfunction]
#[pyo3(signature = (argv=None, handlers=None))]
fn run_cli<'py>(
    py: Python<'py>,
    argv: Option<Bound<'py, PyAny>>,
    handlers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let argv: Bound<'py, PyAny> = match argv {
        Some(argv) => PyList::new(py, argv.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any(),
        None => py
            .import("sys")?
            .getattr("argv")?
            .get_item(pyo3::types::PySlice::new(py, 1, isize::MAX, 1))?,
    };
    let argv: Bound<'py, PyAny> = offered_launch_record(&argv)?;
    let mut termination: TerminationHandler = handle_sigterm(py)?;
    let ran: PyResult<Bound<'py, PyAny>> = invoke(py, &argv, handlers);
    if termination.settle(py) {
        drop(ran);
        return terminated_exit(py).into_bound_py_any(py);
    }
    ran
}

/// Run one parsed `argv` inside its invocation, external-context and diagnostics scopes.
fn invoke<'py>(
    py: Python<'py>,
    argv: &Bound<'py, PyAny>,
    handlers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let external: Bound<'py, PyAny> = external_context(py)?;
    let invocation_scope: Bound<'py, PyAny> =
        host::entry(py, ExecutorHost::InvocationScope)?.call0()?;
    within(&invocation_scope, |identity| {
        let external_scope: Bound<'py, PyAny> = call_with(
            &host::entry(py, ExecutorHost::InvocationExternalContextScope)?,
            &[("external_context", external.clone())],
        )?;
        within(&external_scope, |_| {
            let diagnostics: Bound<'py, PyAny> = call_with(
                &host::entry(py, ExecutorHost::DiagnosticsContext)?,
                &[(
                    "sqlbuild_invocation_id",
                    native_field(identity, "invocation_id")?,
                )],
            )?;
            within(&diagnostics, |_| {
                parsed_invocation(argv, handlers.clone(), identity)
            })
        })
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_cli, module)?)
}
