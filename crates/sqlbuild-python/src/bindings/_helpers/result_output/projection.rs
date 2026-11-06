//! Executor results matched to the canonical terminals of the installed event index.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::types::{PyFloat, PyInt};
use sqlbuild_core::nested_progress::main::audit_resource_id::audit_resource_id;
use sqlbuild_core::nested_progress::main::sql_test_resource_id::sql_test_resource_id;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_host::models::{
    AuditExecutionResult, FunctionExecutionResult, LoadExecutionResult, ModelExecutionResult,
    SeedExecutionResult, SqlTestExecutionResult,
};
use sqlbuild_host::types::ResultEnum;

use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::plan_output::values::text;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::result_output::resource_ids::{audit_identity, sql_test_identity};
use crate::bindings::_helpers::result_output::terminal_index::{TerminalEventIndex, current_index};

/// The executor result types that result output distinguishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResultKind {
    Model,
    Seed,
    Function,
    Load,
    PythonNode,
    PythonCheck,
    SqlTest,
    Audit,
    CloneItem,
    Other,
}

/// The kind of one executor result, by its Python class.
pub(crate) fn result_kind(result: &Bound<'_, PyAny>) -> PyResult<ResultKind> {
    let py: Python<'_> = result.py();
    let kinds: [(ExecutorHost, ResultKind); 9] = [
        (ExecutorHost::ModelExecutionResult, ResultKind::Model),
        (ExecutorHost::SeedExecutionResult, ResultKind::Seed),
        (ExecutorHost::FunctionExecutionResult, ResultKind::Function),
        (ExecutorHost::LoadExecutionResult, ResultKind::Load),
        (
            ExecutorHost::PythonNodeExecutionResult,
            ResultKind::PythonNode,
        ),
        (
            ExecutorHost::PythonCheckExecutionResult,
            ResultKind::PythonCheck,
        ),
        (ExecutorHost::SqlTestExecutionResult, ResultKind::SqlTest),
        (ExecutorHost::AuditExecutionResult, ResultKind::Audit),
        (ExecutorHost::CloneItemResult, ResultKind::CloneItem),
    ];
    for (class, kind) in kinds {
        if result.is_instance(&host::entry(py, class)?)? {
            return Ok(kind);
        }
    }
    Ok(ResultKind::Other)
}

/// `sql_test_resource_id` of one SQL test result.
pub(crate) fn sql_test_check_id(result: &Bound<'_, PyAny>) -> PyResult<String> {
    sql_test_identity(result, "test_name")
}

/// `audit_resource_id` of one audit result.
pub(crate) fn audit_check_id(result: &Bound<'_, PyAny>) -> PyResult<String> {
    audit_identity(result)
}

/// The canonical resource name and identifier of an executor result.
pub(crate) fn resource_identity(result: &Bound<'_, PyAny>) -> PyResult<Option<(String, String)>> {
    let name = |field: &str| -> PyResult<String> { text(&native_field(result, field)?) };
    Ok(Some(match result_kind(result)? {
        ResultKind::Model => {
            let resource: String = name("model_name")?;
            (resource.clone(), format!("model:{resource}"))
        }
        ResultKind::Seed => {
            let resource: String = name("seed_name")?;
            (resource.clone(), format!("seed:{resource}"))
        }
        ResultKind::Function => {
            let resource: String = name("function_name")?;
            (
                resource.clone(),
                format!("{}:{resource}", name("function_kind")?),
            )
        }
        ResultKind::Load => {
            let resource: String = name("source_name")?;
            (resource.clone(), format!("source:{resource}"))
        }
        ResultKind::PythonNode => {
            let resource: String = name("node_name")?;
            let kind: String = text(&native_field(&native_field(result, "kind")?, "value")?)?;
            (resource.clone(), format!("{kind}:{resource}"))
        }
        ResultKind::PythonCheck => {
            let resource: String = name("node_name")?;
            (resource.clone(), format!("check:{resource}"))
        }
        ResultKind::SqlTest => (name("test_name")?, sql_test_check_id(result)?),
        ResultKind::Audit => (name("audit_name")?, audit_check_id(result)?),
        ResultKind::CloneItem | ResultKind::Other => return Ok(None),
    }))
}

/// Resource identity shared by executor records and the permanent Python-node edge.
pub(crate) trait ResultIdentity {
    fn identity(&self) -> PyResult<Option<(String, String)>>;
}

impl<T: ResultIdentity> ResultIdentity for &T {
    fn identity(&self) -> PyResult<Option<(String, String)>> {
        (*self).identity()
    }
}

impl ResultIdentity for Bound<'_, PyAny> {
    fn identity(&self) -> PyResult<Option<(String, String)>> {
        resource_identity(self)
    }
}

macro_rules! named_identity {
    ($kind:ident $(<$value:ident>)?, $field:ident, $prefix:literal) => {
        impl $(<$value>)? ResultIdentity for $kind $(<$value>)? {
            fn identity(&self) -> PyResult<Option<(String, String)>> {
                Ok(Some((
                    self.$field.clone(),
                    format!("{}:{}", $prefix, self.$field),
                )))
            }
        }
    };
}

named_identity!(ModelExecutionResult<V>, model_name, "model");
named_identity!(SeedExecutionResult, seed_name, "seed");
named_identity!(LoadExecutionResult<V>, source_name, "source");

impl ResultIdentity for FunctionExecutionResult {
    fn identity(&self) -> PyResult<Option<(String, String)>> {
        Ok(Some((
            self.function_name.clone(),
            format!("{}:{}", self.function_kind, self.function_name),
        )))
    }
}

impl<V> ResultIdentity for SqlTestExecutionResult<V> {
    fn identity(&self) -> PyResult<Option<(String, String)>> {
        let path = self.source_path.as_ref().map(|path| {
            path.to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
        });
        let case = path
            .as_deref()
            .zip(self.block_index)
            .zip(self.case_name.as_deref())
            .map(|((path, block), case)| (path, block, case));
        Ok(Some((
            self.test_name.clone(),
            sql_test_resource_id(&self.test_name, case),
        )))
    }
}

impl<V> ResultIdentity for AuditExecutionResult<V> {
    fn identity(&self) -> PyResult<Option<(String, String)>> {
        let kind = self
            .attached_target_kind
            .map_or(self.attachment_kind.value(), ResultEnum::value);
        Ok(Some((
            self.audit_name.clone(),
            audit_resource_id(
                &self.audit_name,
                kind,
                (
                    self.attached_target_name.as_deref(),
                    self.attached_column_name.as_deref(),
                ),
            ),
        )))
    }
}

/// `round(value)`: the nearest integer, ties to even.
pub(crate) fn rounded(py: Python<'_>, value: f64) -> PyResult<Bound<'_, PyAny>> {
    py.import("builtins")?.getattr("round")?.call1((value,))
}

/// A number from Python: an `int` (including `bool`) or a `float`.
pub(crate) fn number(value: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    if value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>() {
        return Ok(Some(value.extract()?));
    }
    Ok(None)
}

/// Results matched against the installed terminal event index, if any.
pub(crate) struct Projection<'py> {
    pub(crate) py: Python<'py>,
    index: Option<PyRef<'py, TerminalEventIndex>>,
    /// Scenario whose resources are namespaced under `scenario:<name>:`.
    pub(crate) scenario: Option<String>,
}

impl<'py> Projection<'py> {
    pub(crate) fn installed(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            py,
            index: current_index(py)?,
            scenario: None,
        })
    }

    fn namespaced(&self, resource_id: String) -> String {
        match &self.scenario {
            Some(scenario) => format!("scenario:{scenario}:{resource_id}"),
            None => resource_id,
        }
    }

    /// The latest terminal of a resource, or `None` without one or without an index.
    fn terminal(
        &self,
        resource_name: &str,
        resource_id: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        match &self.index {
            Some(index) => index.latest_terminal(self.py, resource_name, Some(resource_id)),
            None => Ok(None),
        }
    }

    /// The strongest aggregate terminal of the invocation.
    pub(crate) fn lifecycle_terminal(&self) -> PyResult<Option<Bound<'py, PyAny>>> {
        match &self.index {
            Some(index) => index.aggregate_terminal(self.py),
            None => Ok(None),
        }
    }

    /// Whether the resource has a terminal; always true without an index.
    pub(crate) fn resource_terminal_recorded(
        &self,
        resource_name: &str,
        resource_id: &str,
    ) -> PyResult<bool> {
        if self.index.is_none() {
            return Ok(true);
        }
        Ok(self
            .terminal(resource_name, &self.namespaced(resource_id.to_owned()))?
            .is_some())
    }

    /// Whether an executor result has a terminal.
    pub(crate) fn terminal_recorded<T: ResultIdentity>(&self, result: &T) -> PyResult<bool> {
        match result.identity()? {
            Some((name, resource_id)) => self.resource_terminal_recorded(&name, &resource_id),
            None => Ok(self.index.is_none()),
        }
    }

    /// The results that have a terminal.
    pub(crate) fn terminal_results(
        &self,
        results: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
        for result in results.try_iter()? {
            let result: Bound<'py, PyAny> = result?;
            if self.terminal_recorded(&result)? {
                kept.push(result);
            }
        }
        Ok(kept)
    }

    /// Borrow typed results with recorded terminals; never recreate Python result records.
    pub(crate) fn retained<'a, T: ResultIdentity>(&self, results: &'a [T]) -> PyResult<Vec<&'a T>> {
        results
            .iter()
            .filter_map(|result| match self.terminal_recorded(result) {
                Ok(true) => Some(Ok(result)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    /// The terminal's `duration_ms`, or the fallback without an index.
    pub(crate) fn resource_duration(
        &self,
        (resource_name, resource_id): (&str, &str),
        fallback: Option<f64>,
    ) -> PyResult<Option<f64>> {
        if self.index.is_none() {
            return Ok(fallback);
        }
        let Some(terminal) =
            self.terminal(resource_name, &self.namespaced(resource_id.to_owned()))?
        else {
            return Ok(None);
        };
        number(&native_field(&terminal, "payload")?.call_method1("get", ("duration_ms",))?)
    }

    /// The rounded canonical duration of a result, else its own `duration_ms`.
    pub(crate) fn result_duration<T: ResultIdentity>(
        &self,
        result: &T,
        fallback: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let Some((name, resource_id)) = result.identity()? else {
            return Ok(fallback.clone());
        };
        match self.resource_duration((&name, &resource_id), number(fallback)?)? {
            Some(duration) => rounded(self.py, duration),
            None => Ok(self.py.None().into_bound(self.py)),
        }
    }
}
