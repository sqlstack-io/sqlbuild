//! The built-in rule catalogue returned as the Python `Rule` models.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_rules::models::RuleMetadata;
use sqlbuild_rules::rules::main::builtin_options::builtin_options;
use sqlbuild_rules::rules::main::catalogue::catalogue;
use sqlbuild_rules::rules::models::{BuiltinOption, BuiltinOptionKind};

use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::records::construct::{new_record, record_class};
use crate::bindings::_helpers::records::ids::RecordId;

const FAMILY_SUFFIX_LENGTH: usize = 3;

fn keywords<'py>(
    py: Python<'py>,
    fields: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    for (name, value) in fields {
        arguments.set_item(name, value)?;
    }
    Ok(arguments)
}

fn option<'py>(
    option_class: &Bound<'py, PyAny>,
    declared: BuiltinOption,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = option_class.py();
    let (factory, default): (&str, Bound<'py, PyAny>) = match declared.kind {
        BuiltinOptionKind::Boolean => ("boolean", PyBool::new(py, false).to_owned().into_any()),
        BuiltinOptionKind::StringList(defaults) => {
            ("string_list", PyTuple::new(py, defaults)?.into_any())
        }
        BuiltinOptionKind::TableList => {
            let arguments = keywords(
                py,
                vec![
                    ("name", PyString::new(py, declared.name).into_any()),
                    ("value_type", py.get_type::<PyTuple>().into_any()),
                    ("default", PyTuple::empty(py).into_any()),
                    (
                        "description",
                        PyString::new(py, declared.description).into_any(),
                    ),
                    ("item_type", py.get_type::<PyDict>().into_any()),
                ],
            )?;
            return new_record(py, RecordId::RuleOption, (), Some(&arguments));
        }
        BuiltinOptionKind::Integer { default, .. } => {
            ("integer", default.into_pyobject(py)?.into_any())
        }
    };
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("name", PyString::new(py, declared.name).into_any()),
        ("default", default),
        (
            "description",
            PyString::new(py, declared.description).into_any(),
        ),
    ];
    if let BuiltinOptionKind::Integer { minimum, .. } = declared.kind {
        fields.push(("minimum", minimum.into_pyobject(py)?.into_any()));
    }
    let arguments = keywords(py, fields)?;
    option_class.call_method(factory, (), Some(&arguments))
}

fn rule<'py>(
    py: Python<'py>,
    subjects: &Bound<'py, PyAny>,
    check: &Bound<'py, PyAny>,
    metadata: RuleMetadata,
) -> PyResult<Bound<'py, PyAny>> {
    let option_class: Bound<'py, PyAny> = record_class(py, RecordId::RuleOption)?;
    let options: Vec<Bound<'py, PyAny>> = builtin_options(&metadata.code)
        .into_iter()
        .map(|declared| option(&option_class, declared))
        .collect::<PyResult<_>>()?;
    let guidance: Bound<'py, PyAny> = match metadata.guidance {
        Some(guidance) => new_record(
            py,
            RecordId::RuleGuidance,
            (),
            Some(&keywords(
                py,
                vec![
                    (
                        "good_example",
                        PyString::new(py, &guidance.good_example).into_any(),
                    ),
                    (
                        "anti_tautology",
                        PyString::new(py, &guidance.anti_tautology).into_any(),
                    ),
                    (
                        "mutation_check",
                        PyString::new(py, &guidance.mutation_check).into_any(),
                    ),
                ],
            )?),
        )?,
        None => py.None().into_bound(py),
    };
    let family: &str = &metadata.code[..metadata.code.len().saturating_sub(FAMILY_SUFFIX_LENGTH)];
    let subject: Bound<'py, PyAny> = subjects.getattr(if metadata.project_wide {
        "PROJECT"
    } else {
        "MODEL"
    })?;
    let text = |value: &str| PyString::new(py, value).into_any();
    let arguments = keywords(
        py,
        vec![
            ("code", text(&metadata.code)),
            ("family", text(family)),
            ("slug", text(&metadata.slug)),
            ("message", text(&metadata.message)),
            ("remediation", text(&metadata.remediation)),
            ("check", check.clone()),
            ("options", PyTuple::new(py, options)?.into_any()),
            (
                "enabled_by_default",
                PyBool::new(py, metadata.enabled_by_default)
                    .to_owned()
                    .into_any(),
            ),
            ("subject", subject),
            ("guidance", guidance),
        ],
    )?;
    new_record(py, RecordId::Rule, (), Some(&arguments))
}

/// Every built-in rule as a `Rule` whose `check` is the given placeholder.
pub(crate) fn builtin_rules<'py>(
    py: Python<'py>,
    check: Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let subjects: Bound<'py, PyAny> = record_class(py, RecordId::RuleSubject)?;
    compiler_guard(|| Ok(catalogue()))?
        .into_iter()
        .map(|metadata| rule(py, &subjects, &check, metadata))
        .collect()
}
