//! One macro call evaluated natively: its nested calls and arguments, then the macro body in Python.

use std::collections::HashSet;

use pyo3::exceptions::{PyException, PyTypeError, PyUnicodeEncodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString, PyTuple, PyTupleMethods,
    PyTypeMethods,
};
use sqlbuild_render::macro_arguments::errors::ArgumentError;
use sqlbuild_render::macro_arguments::models::{ArgumentValue, MacroArguments};
use sqlbuild_render::macro_calls::main::macro_call_names::macro_call_names as scan_names;
use sqlbuild_render::macro_calls::main::scan_macro_call_sites::scan_macro_call_sites as scan_sites;
use sqlbuild_render::macro_calls::models::{MacroCallScan, MacroCallSite};

use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::render::macro_arguments::parsed_arguments;
use crate::bindings::_helpers::render::macro_expansion::{
    ExpansionClasses, NativeMacroExpansion, raised,
};
use crate::bindings::_helpers::render::macro_expansion_facts::ResourceRef;

const CONTEXT_PARAMETER: &str = "ctx";
const SURROGATE_HELP: &str = "Return valid Unicode text from the macro; lone surrogates usually \
                              come from bytes decoded with errors='surrogateescape'";
const UNARY_HELP: &str =
    "Return a number from the nested macro, or apply the sign inside the macro";
const NESTED_KEY_HELP: &str = "Return a string, number, True, False, None, or a tuple of them from a macro used as a dict key";

/// Text addressed by code point offsets, as the native scan reports them.
pub(crate) struct CallText<'a> {
    pub(crate) text: &'a str,
    /// The byte offset of each code point, then the text's length.
    bytes: Vec<usize>,
}

impl<'a> CallText<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let mut bytes: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
        bytes.push(text.len());
        Self { text, bytes }
    }

    /// The code points `start..end`.
    pub(crate) fn slice(&self, start: usize, end: usize) -> &'a str {
        &self.text[self.bytes[start]..self.bytes[end]]
    }
}

/// One expansion's state, its Python handle and the classes it builds objects from.
pub(crate) struct Evaluation<'a, 'py> {
    pub(crate) expansion: &'a NativeMacroExpansion,
    /// The expansion as the Python object macro contexts and the relation renderer call back into.
    pub(crate) handle: &'a Bound<'py, PyAny>,
    pub(crate) classes: &'a ExpansionClasses<'py>,
}

/// `evaluate_macro_call`: one scanned call's value; a top-level call must return SQL.
pub(crate) fn evaluate_call<'py>(
    run: &Evaluation<'_, 'py>,
    call: (&CallText<'_>, &MacroCallSite),
    top_level: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let (text, site) = call;
    let (expansion, classes) = (run.expansion, run.classes);
    let py: Python<'py> = run.handle.py();
    let name: &str = &site.name;
    let loaded_macro: Bound<'py, PyAny> = expansion.resolved_macro(classes, name, py)?;
    if let Some(value) = expansion.overrides.bind(py).get_item(name)?
        && !value.is_none()
    {
        let _ = final_sql(run, name, &value)?;
        return Ok(value);
    }
    let identity: Bound<'py, PyAny> = expansion.macro_identity(classes, name, py)?;
    expansion.record_macro_use(classes, name, &identity)?;
    let args_source: &str = text.slice(site.args_start, site.end - 1);
    expansion.facts.borrow_mut().open_call_site();
    let parsed: PyResult<(Bound<'py, PyTuple>, Bound<'py, PyDict>)> =
        call_arguments(run, name, args_source);
    let approved: HashSet<ResourceRef> = expansion.facts.borrow_mut().close_call_site();
    let arguments = match parsed {
        Ok(arguments) => arguments,
        Err(error) if error.is_instance(py, &classes.macro_argument_error) => {
            let shifted: Bound<'py, PyAny> = error
                .value(py)
                .call_method1("shifted", (site.args_start,))?;
            shifted.setattr("__suppress_context__", true)?;
            return Err(PyErr::from_value(shifted));
        }
        Err(error) => return Err(error),
    };
    let result: Bound<'py, PyAny> =
        called_macro(run, (name, &loaded_macro), arguments, approved)
            .map_err(|error| macro_failure(classes, error, name, &expansion.file_label))?;
    let Ok(sql) = result.downcast::<PyString>() else {
        if top_level {
            let message: String = format!(
                "Macro '@py.{name}' in '{}' must return a SQL string when used directly in SQL",
                expansion.file_label
            );
            return Err(raised(&classes.compile_error, &message));
        }
        return Ok(result);
    };
    let sql: String = final_sql(run, name, sql.as_any())?;
    expansion.report_generated(classes, name, &loaded_macro, &sql)?;
    Ok(result)
}

/// The macro's result, called with its context, arguments and call-site references approved.
fn called_macro<'py>(
    run: &Evaluation<'_, 'py>,
    called: (&str, &Bound<'py, PyAny>),
    arguments: (Bound<'py, PyTuple>, Bound<'py, PyDict>),
    approved: HashSet<ResourceRef>,
) -> PyResult<Bound<'py, PyAny>> {
    let (name, loaded_macro) = called;
    let (args, kwargs) = arguments;
    let (expansion, classes) = (run.expansion, run.classes);
    let py: Python<'py> = run.handle.py();
    let context_arguments: Bound<'py, PyDict> = PyDict::new(py);
    context_arguments.set_item("expansion", run.handle)?;
    context_arguments.set_item("declarations", expansion.declarations.bind(py))?;
    context_arguments.set_item("file_path", expansion.file_path.bind(py))?;
    context_arguments.set_item("macro_context", expansion.macro_context.bind(py))?;
    let context: Bound<'py, PyAny> = classes
        .invocation_context
        .call((), Some(&context_arguments))?;
    expansion.facts.borrow_mut().begin_rendering(approved);
    let token: PyResult<Bound<'py, PyAny>> = native_field(run.handle, "render_relation")
        .and_then(|renderer| classes.relation_renderer.call_method1("set", (renderer,)));
    let result: PyResult<Bound<'py, PyAny>> = token
        .as_ref()
        .map_err(|error| error.clone_ref(py))
        .and_then(|_| invoke(classes, (name, loaded_macro), &context, (&args, &kwargs)));
    let reset: PyResult<()> = match &token {
        Ok(token) => classes
            .relation_renderer
            .call_method1("reset", (token,))
            .map(|_| ()),
        Err(_) => Ok(()),
    };
    expansion.facts.borrow_mut().end_rendering();
    let result: Bound<'py, PyAny> = result?;
    reset?;
    Ok(result)
}

/// `_call_loaded_macro`: the macro called with its injected context when it takes one.
fn invoke<'py>(
    classes: &ExpansionClasses<'py>,
    called: (&str, &Bound<'py, PyAny>),
    context: &Bound<'py, PyAny>,
    arguments: (&Bound<'py, PyTuple>, &Bound<'py, PyDict>),
) -> PyResult<Bound<'py, PyAny>> {
    let (name, loaded_macro) = called;
    let (args, kwargs) = arguments;
    let function: Bound<'py, PyAny> = native_field(loaded_macro, "function")?;
    if !native_field(loaded_macro, "injects_context")?.is_truthy()? {
        return function.call(args, Some(kwargs));
    }
    if kwargs.contains(CONTEXT_PARAMETER)? {
        let message: String = format!(
            "Macro '@py.{name}' must not be called with keyword argument 'ctx'; 'ctx' is reserved \
             for injected macro context"
        );
        return Err(raised(&classes.compile_error, &message));
    }
    let mut positional: Vec<Bound<'py, PyAny>> = vec![context.clone()];
    positional.extend(args.iter());
    function.call(PyTuple::new(args.py(), positional)?, Some(kwargs))
}

/// Python's `except` clauses around a macro call: compile errors pass, other exceptions wrap.
fn macro_failure(
    classes: &ExpansionClasses<'_>,
    error: PyErr,
    name: &str,
    file_label: &str,
) -> PyErr {
    let py: Python<'_> = classes.compile_error.py();
    if error.is_instance(py, &classes.compile_error) {
        return error;
    }
    let reason: &str = if error.is_instance_of::<PyTypeError>(py) {
        "could not be called"
    } else if error.is_instance_of::<PyException>(py) {
        "failed"
    } else {
        return error;
    };
    let detail: String = match error.value(py).str() {
        Ok(text) => text.to_string(),
        Err(failure) => return failure,
    };
    let wrapped: PyErr = raised(
        &classes.compile_error,
        &format!("Macro '@py.{name}' in '{file_label}' {reason}: {detail}"),
    );
    wrapped.set_cause(py, Some(error));
    wrapped
}

/// `_validate_final_macro_sql`: valid Unicode without SQLBuild calls, as Rust text.
fn final_sql(run: &Evaluation<'_, '_>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<String> {
    let (expansion, classes) = (run.expansion, run.classes);
    let py: Python<'_> = value.py();
    if let Err(error) = value.call_method1("encode", ("utf-8",)) {
        if !error.is_instance_of::<PyUnicodeEncodeError>(py) {
            return Err(error);
        }
        let start: Bound<'_, PyAny> = native_field(error.value(py), "start")?;
        let surrogate: String = value.get_item(start)?.repr()?.to_string();
        let message: String = format!(
            "Macro '@py.{name}' in '{}' returned text with the lone surrogate {surrogate}, which is \
             not valid Unicode",
            expansion.file_label
        );
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("help", SURROGATE_HELP)?;
        let raised_error: Bound<'_, PyAny> =
            classes.compile_error.call((message,), Some(&keywords))?;
        raised_error.setattr("__suppress_context__", true)?;
        return Err(PyErr::from_value(raised_error));
    }
    let sql: String = value.extract()?;
    let names: Vec<String> = scan_names(&sql, false, expansion.syntax.as_ref())
        .map_err(|error| raised(&classes.compile_error, error.message()))?;
    if names.is_empty() {
        return Ok(sql);
    }
    let calls: String = names
        .iter()
        .map(|call| format!("@py.{call}()"))
        .collect::<Vec<String>>()
        .join(", ");
    let message: String = format!(
        "Macro '@py.{name}' in '{}' returned SQLBuild call(s) {calls}. Macro output must be final \
         SQL. Compose macros with ordinary Python imports and function calls instead; nested \
         @py.macro() calls are supported only when explicitly authored in SQL",
        expansion.file_label
    );
    Err(raised(&classes.compile_error, &message))
}

/// `_parse_macro_arguments`: nested calls evaluated in order, then the arguments built.
fn call_arguments<'py>(
    run: &Evaluation<'_, 'py>,
    name: &str,
    args_source: &str,
) -> PyResult<(Bound<'py, PyTuple>, Bound<'py, PyDict>)> {
    let py: Python<'py> = run.handle.py();
    if str::trim(args_source).is_empty() {
        return Ok((PyTuple::empty(py), PyDict::new(py)));
    }
    let text: CallText<'_> = CallText::new(args_source);
    let scan: MacroCallScan = scan_sites(args_source, run.expansion.syntax.as_ref());
    let mut values: Vec<Bound<'py, PyAny>> = Vec::with_capacity(scan.sites.len());
    for site in &scan.sites {
        values.push(evaluate_call(run, (&text, site), false)?);
    }
    if let Some(failure) = &scan.failure {
        return Err(run
            .expansion
            .failed_scan(run.classes, (&text, failure), Some(name), py));
    }
    let nested: Vec<(usize, usize)> = scan
        .sites
        .iter()
        .map(|site| (site.start, site.end))
        .collect();
    let source: ArgumentSource<'_, 'py> = ArgumentSource {
        run,
        name,
        text: &text,
        nested: &nested,
        values: &values,
    };
    let arguments: MacroArguments = match parsed_arguments(py, args_source, &nested)? {
        Ok(arguments) => arguments,
        Err(error) => return Err(source.parse_error(&error)),
    };
    let mut positional: Vec<Bound<'py, PyAny>> = Vec::with_capacity(arguments.positional.len());
    for value in &arguments.positional {
        positional.push(source.built(value)?);
    }
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    for (keyword, value) in &arguments.keywords {
        keywords.set_item(keyword, source.built(value)?)?;
    }
    let mut references: Vec<ResourceRef> = Vec::with_capacity(arguments.typed_references.len());
    for (function, reference_name) in &arguments.typed_references {
        let kind: String = native_field(
            &run.classes
                .ref_kinds
                .get_item(function)?
                .ok_or_else(|| PyValueError::new_err(function.clone()))?,
            "value",
        )?
        .extract()?;
        references.push((kind, reference_name.clone()));
    }
    run.expansion
        .facts
        .borrow_mut()
        .record_call_site_refs(&references);
    Ok((PyTuple::new(py, positional)?, keywords))
}

/// One call's argument text, its nested call spans and their values.
struct ArgumentSource<'a, 'py> {
    run: &'a Evaluation<'a, 'py>,
    name: &'a str,
    text: &'a CallText<'a>,
    nested: &'a [(usize, usize)],
    values: &'a [Bound<'py, PyAny>],
}

impl<'py> ArgumentSource<'_, 'py> {
    /// `_built`: the Python object one planned value stands for.
    fn built(&self, value: &ArgumentValue) -> PyResult<Bound<'py, PyAny>> {
        let classes: &ExpansionClasses<'py> = self.run.classes;
        let py: Python<'py> = self.run.handle.py();
        Ok(match value {
            ArgumentValue::Str(text) => PyString::new(py, text).into_any(),
            ArgumentValue::Bool(flag) => PyBool::new(py, *flag).to_owned().into_any(),
            ArgumentValue::None => py.None().into_bound(py),
            ArgumentValue::Int { radix, digits } => classes.int_type.call1((digits, radix))?,
            ArgumentValue::Float(text) => classes.float_type.call1((text,))?,
            ArgumentValue::NestedCall(index) => self.values[*index].clone(),
            ArgumentValue::TypedReference { function, name } => {
                let kind: Bound<'py, PyAny> = classes
                    .ref_kinds
                    .get_item(function)?
                    .ok_or_else(|| PyValueError::new_err(function.clone()))?;
                let fields: Bound<'py, PyDict> = PyDict::new(py);
                fields.set_item("kind", kind)?;
                fields.set_item("name", name)?;
                classes.resource_ref.call((), Some(&fields))?
            }
            ArgumentValue::List(items) => PyList::new(py, self.items(items)?)?.into_any(),
            ArgumentValue::Tuple(items) => PyTuple::new(py, self.items(items)?)?.into_any(),
            ArgumentValue::Dict(pairs) => {
                let mapping: Bound<'py, PyDict> = PyDict::new(py);
                for (key, item) in pairs {
                    let built_key: Bound<'py, PyAny> = self.built(key)?;
                    if let Err(error) = built_key.hash() {
                        return Err(self.unhashable_key(key, error));
                    }
                    mapping.set_item(built_key, self.built(item)?)?;
                }
                mapping.into_any()
            }
            ArgumentValue::Negative(inner) => self.number(inner, "-")?.neg()?,
            ArgumentValue::Positive(inner) => self.number(inner, "+")?.pos()?,
        })
    }

    fn items(&self, items: &[ArgumentValue]) -> PyResult<Vec<Bound<'py, PyAny>>> {
        items.iter().map(|item| self.built(item)).collect()
    }

    /// `_number`: the operand of a unary sign, which a nested call must return as a number.
    fn number(&self, operand: &ArgumentValue, sign: &str) -> PyResult<Bound<'py, PyAny>> {
        let number: Bound<'py, PyAny> = self.built(operand)?;
        if number.is_instance_of::<PyInt>() || number.is_instance_of::<PyFloat>() {
            return Ok(number);
        }
        let mut row: &ArgumentValue = operand;
        let call: usize = loop {
            match row {
                ArgumentValue::NestedCall(index) => break *index,
                ArgumentValue::Negative(inner) | ArgumentValue::Positive(inner) => row = inner,
                _ => {
                    return Err(PyValueError::new_err(
                        "unary sign over a non-number literal",
                    ));
                }
            }
        };
        let type_name: String = self.values[call].get_type().name()?.to_string();
        let detail: String = format!(
            "apply unary {sign} to the value of a nested macro call, which is a {type_name} and \
             not a number"
        );
        Err(self.argument_error(self.nested[call].0, &detail, UNARY_HELP))
    }

    /// The error for a dict key a nested macro call made unhashable, at that call.
    fn unhashable_key(&self, key: &ArgumentValue, error: PyErr) -> PyErr {
        let Some(call) = nested_calls(key)
            .into_iter()
            .find(|call| self.values[*call].hash().is_err())
        else {
            return error;
        };
        let type_name: String = match self.values[call].get_type().name() {
            Ok(name) => name.to_string(),
            Err(failure) => return failure,
        };
        let detail: String =
            format!("use the value of a nested macro call, which is a {type_name}, as a dict key");
        self.argument_error(self.nested[call].0, &detail, NESTED_KEY_HELP)
    }

    /// The `MacroArgumentError` for a native parse error at its line and column.
    fn parse_error(&self, error: &ArgumentError) -> PyErr {
        let mut offset: usize = 0;
        for line in self
            .text
            .text
            .split('\n')
            .take(error.line.saturating_sub(1))
        {
            offset += line.chars().count() + 1;
        }
        offset += error.column.saturating_sub(1);
        self.argument_error(offset, &error.detail, &error.help)
    }

    /// `MacroArgumentError` at a code point offset of the argument text.
    fn argument_error(&self, offset: usize, detail: &str, help: &str) -> PyErr {
        macro_argument_error(
            (self.run.expansion, self.run.classes),
            self.text,
            (self.name, offset),
            (detail, help),
        )
    }
}

/// The nested macro calls inside `value`, in authored order.
fn nested_calls(value: &ArgumentValue) -> Vec<usize> {
    match value {
        ArgumentValue::NestedCall(index) => vec![*index],
        ArgumentValue::List(items) | ArgumentValue::Tuple(items) => {
            items.iter().flat_map(nested_calls).collect()
        }
        ArgumentValue::Dict(pairs) => pairs
            .iter()
            .flat_map(|(key, item)| nested_calls(key).into_iter().chain(nested_calls(item)))
            .collect(),
        ArgumentValue::Negative(inner) | ArgumentValue::Positive(inner) => nested_calls(inner),
        _ => Vec::new(),
    }
}

/// `MacroArgumentError` of the call `name` at code point `offset` of `text`.
pub(crate) fn macro_argument_error(
    (expansion, classes): (&NativeMacroExpansion, &ExpansionClasses<'_>),
    text: &CallText<'_>,
    (name, offset): (&str, usize),
    (detail, help): (&str, &str),
) -> PyErr {
    let before: &str = text.slice(0, offset.min(text.bytes.len() - 1));
    let line: usize = before.matches('\n').count() + 1;
    let column: usize = before
        .rsplit_once('\n')
        .map_or(before, |(_, last)| last)
        .chars()
        .count()
        + 1;
    let created = (|| {
        let py: Python<'_> = classes.macro_argument_error.py();
        let fields: Bound<'_, PyDict> = PyDict::new(py);
        fields.set_item("detail", detail)?;
        fields.set_item("help", help)?;
        fields.set_item("macro_name", name)?;
        fields.set_item("file_path", expansion.file_path.bind(py))?;
        fields.set_item("offset", offset)?;
        fields.set_item("relative_position", (line, column))?;
        classes.macro_argument_error.call((), Some(&fields))
    })();
    match created {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}
