//! Python function files discovered natively, with CPython as the authority on syntax and values.

use pyo3::exceptions::{PySyntaxError, PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods};
use sqlbuild_discovery::key_checks::main::unsupported_keys_help::unsupported_keys_help;
use sqlbuild_discovery::models::{DiscoveredFile, FileOutcome};
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_pysource::udf_functions::constants::UDF_KEYS;
use sqlbuild_pysource::udf_functions::main::udf_function::udf_function;
use sqlbuild_pysource::udf_functions::models::{UdfFunction, UdfShape};

use crate::discovery::_helpers::failures::raised::{failure_error, read_error};
use crate::discovery::_helpers::project::project_files::path_object;
use crate::discovery::_helpers::records::declaration_records::path_class;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const AST_MODULE: &str = "ast";
const DESCRIPTION_KEY: &str = "description";
const ENTRY_POINT_KEY: &str = "entry_point";
const UDF_STATEMENT: &str = "@udf(...)";

/// One file's paths and text, as its messages and record name them.
struct FunctionSource<'py> {
    file_path: Bound<'py, PyAny>,
    relative_path: Bound<'py, PyAny>,
    contents: Bound<'py, PyString>,
}

/// `ModelSqlParseError(f"Python function '{file_path}' {detail}")`, caused by `cause` if any.
fn parse_error(source: &FunctionSource<'_>, detail: &str, cause: Option<PyErr>) -> PyErr {
    let py: Python<'_> = source.file_path.py();
    let raised = (|| {
        let message: String = format!("Python function '{}' {detail}", source.file_path.str()?);
        error_class(py, ErrorId::ModelSqlParse)?.call1((message,))
    })();
    match raised {
        Ok(instance) => {
            let error: PyErr = PyErr::from_value(instance);
            error.set_cause(py, cause);
            error
        }
        Err(failure) => failure,
    }
}

/// `ast.parse(contents, filename=str(file_path))`, raising its syntax error as a parse error.
fn cpython_module<'py>(source: &FunctionSource<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.file_path.py();
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("filename", source.file_path.str()?)?;
    match py
        .import(AST_MODULE)?
        .getattr("parse")?
        .call((&source.contents,), Some(&keywords))
    {
        Ok(module) => Ok(module),
        Err(error) if error.is_instance_of::<PySyntaxError>(py) => {
            let detail: String = format!("could not be parsed: {}", error.value(py).str()?);
            Err(parse_error(source, &detail, Some(error)))
        }
        Err(error) => Err(error),
    }
}

/// The literal `@udf(...)` keyword values, raising the first expansion or non-literal value.
fn keyword_values<'py>(
    source: &FunctionSource<'py>,
    function: &UdfFunction,
    call: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = call.py();
    let literal_eval: Bound<'py, PyAny> = py.import(AST_MODULE)?.getattr("literal_eval")?;
    let values: Bound<'py, PyDict> = PyDict::new(py);
    let nodes: Bound<'py, PyAny> = native_field(call, "keywords")?;
    for (keyword, node) in function.keywords.iter().zip(nodes.try_iter()?) {
        let Some(name) = &keyword.name else {
            let detail: String = format!("{UDF_STATEMENT} must not use **kwargs expansion");
            return Err(parse_error(source, &detail, None));
        };
        match literal_eval.call1((native_field(&node?, "value")?,)) {
            Ok(value) => values.set_item(name, value)?,
            Err(error)
                if error.is_instance_of::<PyValueError>(py)
                    || error.is_instance_of::<PyTypeError>(py) =>
            {
                let detail: String = format!("{UDF_STATEMENT} value for '{name}' must be literal");
                return Err(parse_error(source, &detail, Some(error)));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(values)
}

/// The unsupported-keys error of the first keywords `@udf(...)` does not accept, if any.
fn unsupported_keys(source: &FunctionSource<'_>, function: &UdfFunction) -> PyResult<()> {
    let unsupported: Vec<(&str, usize)> = function
        .keywords
        .iter()
        .filter_map(|keyword| Some((keyword.name.as_deref()?, keyword.line)))
        .filter(|(name, _)| !UDF_KEYS.contains(name))
        .collect();
    let Some((_, line)) = unsupported.first() else {
        return Ok(());
    };
    let py: Python<'_> = source.file_path.py();
    let keys: Vec<&str> = unsupported.iter().map(|(name, _)| *name).collect();
    let supported: Vec<String> = UDF_KEYS.iter().map(|key| (*key).to_owned()).collect();
    let message: String = format!(
        "{UDF_STATEMENT} in '{}:{line}' has unsupported keys: {}",
        source.file_path.str()?,
        keys.join(", ")
    );
    let help: String = unsupported_keys_help(&keys, &supported);
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item("help", help)?;
    let error: Bound<'_, PyAny> =
        error_class(py, ErrorId::ModelSqlParse)?.call((message,), Some(&keywords))?;
    Err(PyErr::from_value(error))
}

/// `ast.unparse` of the module without its `udf` imports and the function's `@udf` decorator.
fn warehouse_body<'py>(
    module: &Bound<'py, PyAny>,
    function: &UdfFunction,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = module.py();
    let ast: Bound<'py, PyModule> = py.import(AST_MODULE)?;
    let statements: Bound<'py, PyList> = native_field(module, "body")?.downcast_into::<PyList>()?;
    let body: Bound<'py, PyList> = PyList::empty(py);
    for (index, statement) in statements.iter().enumerate() {
        if function.removed_statements.contains(&index) {
            continue;
        }
        if index == function.statement_index {
            let decorators: Bound<'py, PyList> = PyList::empty(py);
            for (position, decorator) in native_field(&statement, "decorator_list")?
                .try_iter()?
                .enumerate()
            {
                if !function.removed_decorators.contains(&position) {
                    decorators.append(decorator?)?;
                }
            }
            statement.setattr("decorator_list", decorators)?;
        }
        body.append(statement)?;
    }
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("body", body)?;
    keywords.set_item("type_ignores", PyList::empty(py))?;
    let cleaned: Bound<'py, PyAny> = native_field(&ast, "Module")?.call((), Some(&keywords))?;
    native_field(&ast, "unparse")?
        .call1((cleaned,))?
        .call_method0("strip")
}

/// `parse_python_function`: `(header_values, entry_point, body_python)` of one function file.
fn parsed_function<'py>(
    source: &FunctionSource<'py>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = source.file_path.py();
    let module: Bound<'py, PyAny> = cpython_module(source)?;
    let version = py.version_info();
    let shape: UdfShape = udf_function(source.contents.to_str()?, (version.major, version.minor))
        .ok_or_else(|| {
        compiler_error("a Python function file CPython parses did not parse natively")
    })?;
    let function: UdfFunction = match shape {
        UdfShape::Missing => {
            let detail = "must define exactly one function decorated with @udf(...)";
            return Err(parse_error(source, detail, None));
        }
        UdfShape::Multiple => {
            let detail = "must define only one function decorated with @udf(...)";
            return Err(parse_error(source, detail, None));
        }
        UdfShape::Single(function) => function,
    };
    let statement: Bound<'py, PyAny> =
        native_field(&module, "body")?.get_item(function.statement_index)?;
    let decorator: usize = function
        .removed_decorators
        .first()
        .copied()
        .ok_or_else(|| compiler_error("a @udf(...) function without its decorator"))?;
    let call: Bound<'py, PyAny> =
        native_field(&statement, "decorator_list")?.get_item(decorator)?;
    let values: Bound<'py, PyDict> = keyword_values(source, &function, &call)?;
    if function.has_positional_arguments {
        let detail: String = format!("{UDF_STATEMENT} must use keyword arguments only");
        return Err(parse_error(source, &detail, None));
    }
    unsupported_keys(source, &function)?;
    if !values.contains(DESCRIPTION_KEY)?
        && let Some(docstring) = &function.docstring
    {
        values.set_item(DESCRIPTION_KEY, docstring)?;
    }
    let entry_point: Bound<'py, PyAny> = match values.get_item(ENTRY_POINT_KEY)? {
        Some(value) if !value.is_none() => value,
        _ => {
            values.set_item(ENTRY_POINT_KEY, &function.name)?;
            PyString::new(py, &function.name).into_any()
        }
    };
    let stripped: Option<Bound<'py, PyAny>> = match entry_point.downcast::<PyString>() {
        Ok(text) => {
            Some(text.call_method0("strip")?).filter(|text| text.is_truthy().unwrap_or(false))
        }
        Err(_) => None,
    };
    let Some(entry_point) = stripped else {
        let detail: String = format!("{UDF_STATEMENT} entry_point must be a non-empty string");
        return Err(parse_error(source, &detail, None));
    };
    Ok((values, entry_point, warehouse_body(&module, &function)?))
}

pub(crate) fn function_record<'py>(
    project_dir: &Bound<'py, PyAny>,
    tree: &ProjectTree,
    file: DiscoveredFile<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let relative_path: Bound<'py, PyAny> =
        path_class(py)?.call1((path_object(py, tree, file.relative_path)?,))?;
    let file_path: Bound<'py, PyAny> = project_dir.div(&relative_path)?;
    let contents: String = match file.outcome {
        FileOutcome::Parsed(contents) => contents,
        FileOutcome::Failed(failure) => return Err(failure_error(py, failure)),
        FileOutcome::Unreadable(failure) => return Err(read_error(failure, &file_path)),
    };
    let source = FunctionSource {
        file_path,
        relative_path,
        contents: PyString::new(py, &contents),
    };
    let (header_values, entry_point, body_python) = parsed_function(&source)?;
    new_record_with(
        py,
        RecordId::DiscoveredPythonFunctionFile,
        &[
            ("file_path", source.file_path.clone()),
            ("relative_path", source.relative_path.clone()),
            ("contents", source.contents.clone().into_any()),
            ("header_values", header_values.into_any()),
            ("entry_point", entry_point),
            ("body_python", body_python),
        ],
    )
}
