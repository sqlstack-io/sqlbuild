//! Derived views and constructors of the natively hosted custom-rule records.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Keyword fields every `RuleOption` constructor requires.
const REQUIRED_OPTION_FIELDS: [&str; 3] = ["name", "default", "description"];

/// `pathlib.Path(value)` of a `ProjectPath`.
fn project_path<'py>(instance: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    imported(instance.py(), "pathlib", "Path")?.call1((native_field(instance, "value")?,))
}

/// `ProjectPath.parts`.
pub(crate) fn project_path_parts(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(&project_path(instance)?, "parts")?.unbind())
}

/// `ProjectPath.name`.
pub(crate) fn project_path_name(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(&project_path(instance)?, "name")?.unbind())
}

/// `ProjectPath.depth`: the number of path parts.
pub(crate) fn project_path_depth(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    native_field(&project_path(instance)?, "parts")?
        .len()?
        .into_py_any(instance.py())
}

/// `Rule.project_wide`: whether the rule runs once for the compiled project.
pub(crate) fn rule_project_wide(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let project: Bound<'_, PyAny> = enum_member(instance.py(), RecordId::RuleSubject, "PROJECT")?;
    native_field(instance, "subject")?
        .is(&project)
        .into_py_any(instance.py())
}

/// `RuleResult.finding_count`.
pub(crate) fn rule_result_finding_count(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    native_field(instance, "findings")?
        .len()?
        .into_py_any(instance.py())
}

/// `RuleOption[T]`: the generic alias annotations use.
pub(crate) fn rule_option_class_getitem(
    receiver: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    _kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = receiver.py();
    let class: Bound<'_, PyAny> = record_class(py, RecordId::RuleOption)?;
    Ok(imported(py, "types", "GenericAlias")?
        .call1((class, args.get_item(0)?))?
        .unbind())
}

/// A `RuleOption` of `value_type` from the keyword fields named in `names`, plus `extra`.
fn rule_option(
    (args, kwargs): (&Bound<'_, PyTuple>, Option<&Bound<'_, PyDict>>),
    (value_type, names): (Bound<'_, PyAny>, &[&str]),
    extra: &[(&str, Bound<'_, PyAny>)],
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = value_type.py();
    if !args.is_empty() {
        return Err(PyTypeError::new_err(
            "RuleOption constructors take keyword arguments only",
        ));
    }
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("value_type", &value_type)?;
    if let Some(kwargs) = kwargs {
        for (name, value) in kwargs.iter() {
            let name: String = name.extract()?;
            if !names.contains(&name.as_str()) {
                return Err(PyTypeError::new_err(format!(
                    "RuleOption constructor got an unexpected keyword argument '{name}'"
                )));
            }
            fields.set_item(name, value)?;
        }
    }
    for name in REQUIRED_OPTION_FIELDS {
        if !fields.contains(name)? {
            return Err(PyTypeError::new_err(format!(
                "RuleOption constructor missing required keyword-only argument: '{name}'"
            )));
        }
    }
    for (name, value) in extra {
        fields.set_item(*name, value)?;
    }
    Ok(new_record(py, RecordId::RuleOption, (), Some(&fields))?.unbind())
}

/// The builtin type `name`.
fn builtin<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    imported(py, "builtins", name)
}

/// `RuleOption.boolean(*, name, default, description)`.
pub(crate) fn rule_option_boolean(
    receiver: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let value_type: Bound<'_, PyAny> = builtin(receiver.py(), "bool")?;
    rule_option(
        (args, kwargs),
        (value_type, &["name", "default", "description"]),
        &[],
    )
}

/// `RuleOption.integer(*, name, default, description, minimum=None, maximum=None)`.
pub(crate) fn rule_option_integer(
    receiver: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let value_type: Bound<'_, PyAny> = builtin(receiver.py(), "int")?;
    rule_option(
        (args, kwargs),
        (
            value_type,
            &["name", "default", "description", "minimum", "maximum"],
        ),
        &[],
    )
}

/// `RuleOption.string(*, name, default, description, choices=())`.
pub(crate) fn rule_option_string(
    receiver: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let value_type: Bound<'_, PyAny> = builtin(receiver.py(), "str")?;
    rule_option(
        (args, kwargs),
        (value_type, &["name", "default", "description", "choices"]),
        &[],
    )
}

/// `RuleOption.string_list(*, name, default, description, minimum_items=None)`.
pub(crate) fn rule_option_string_list(
    receiver: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = receiver.py();
    rule_option(
        (args, kwargs),
        (
            builtin(py, "tuple")?,
            &["name", "default", "description", "minimum_items"],
        ),
        &[("item_type", builtin(py, "str")?)],
    )
}

/// `RuleOption.integer_list(*, name, default, description, minimum_items=None)`.
pub(crate) fn rule_option_integer_list(
    receiver: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = receiver.py();
    rule_option(
        (args, kwargs),
        (
            builtin(py, "tuple")?,
            &["name", "default", "description", "minimum_items"],
        ),
        &[("item_type", builtin(py, "int")?)],
    )
}
