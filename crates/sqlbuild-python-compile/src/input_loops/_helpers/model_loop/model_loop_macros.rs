//! A model query's macro calls, expanded natively with the model's visible declarations.

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::compiled_project::main::authored_model_offset::authored_model_offset;
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::compiled_project::models::{SplicedSpan, TextPosition};
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::ReferenceExtraction;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::input_loops::_helpers::model::model_loop::LoopContext;
use crate::input_loops::_helpers::model::model_loop::VisibleDeclarations;
use crate::input_loops::_helpers::model::model_loop::compile_error;
use crate::input_loops::_helpers::model::model_loop_inputs::reference_row;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::render::main::expansion::expanded::expanded;
use sqlbuild_python_front::render::models::{ExpansionOutput, NativeMacroExpansion, StateInput};
use sqlbuild_python_front::sqltext::models::LexicalSyntaxInput;

/// Fields of `_VisibleModelDeclarations` a `DeclarationResolutionContext` is built from.
const CONTEXT_FIELDS: [&str; 9] = [
    "enums",
    "constants",
    "inaccessible_enums",
    "inaccessible_constants",
    "enum_visibility",
    "constant_visibility",
    "macros",
    "macro_records",
    "inaccessible_macros",
];
const ENFORCE_ATTRIBUTE: &str = "_enforce_explicit_references";

/// What every model's macro expansion reads: the project's macros, context, bridge and classes.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct MacroClasses<'py> {
    pub(crate) loaded_macros: Py<PyDict>,
    macro_context: Bound<'py, PyAny>,
    /// The active `MacroBridge`, or `None`.
    bridge: Bound<'py, PyAny>,
    /// Whether a declaration scope resolver indexes the project, so declarations are scoped.
    pub(crate) scoped: bool,
    pub(crate) declaration_context: Bound<'py, PyAny>,
    macro_argument_error: Bound<'py, PyAny>,
    expansion: Py<PyDict>,
    pub(crate) generic_syntax: LexicalSyntaxInput,
}

/// The model whose query is expanded: its paths, authored text and declaration passes.
pub(crate) struct MacroModel<'a> {
    pub(crate) file_path: &'a str,
    pub(crate) relative_path: &'a str,
    pub(crate) contents: &'a str,
    pub(crate) authored_query: &'a str,
    /// Whether interpolation left the authored query unchanged, so offsets map to the file.
    pub(crate) authored: bool,
    pub(crate) declaration_spans: &'a [SplicedSpan],
}

/// The expanded query, its call-site references, and the expansion facts the host keeps.
pub(crate) struct ExpandedMacros {
    pub(crate) references: Vec<ReferenceRow>,
    pub(crate) output: ExpansionOutput,
}

/// `_macro_expansion`: expand the query, locating argument errors in the authored file.
pub(crate) fn expanded_macros<'py>(
    context: &LoopContext<'_, 'py>,
    visible: &VisibleDeclarations<'py>,
    sql: &str,
    model: &MacroModel<'_>,
) -> PyResult<ExpandedMacros> {
    let classes: &MacroClasses<'py> = &context.classes.macros;
    let py: Python<'py> = visible.consumer.py();
    let declarations: Bound<'py, PyAny> = if classes.scoped {
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        for field in CONTEXT_FIELDS {
            fields.set_item(field, native_field(&visible.declarations, field)?)?;
        }
        fields.set_item("consumer", &visible.consumer)?;
        classes.declaration_context.call((), Some(&fields))?
    } else {
        py.None().into_bound(py)
    };
    let output: ExpansionOutput = match expanded_with(
        context,
        (&declarations, &visible.consumer),
        model.file_path,
        sql,
    ) {
        Ok(output) => output,
        Err(error) => return Err(located(context, error, (sql, model))),
    };
    let references: Vec<ReferenceRow> = argument_references(context, &output)?;
    Ok(ExpandedMacros { references, output })
}

/// `expand_sql_macros_result`: expand `sql` with the declarations (or `None`) and consumer.
pub(crate) fn expanded_with<'py>(
    context: &LoopContext<'_, 'py>,
    reader: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    file_path: &str,
    sql: &str,
) -> PyResult<ExpansionOutput> {
    let (declarations, consumer) = reader;
    let classes: &MacroClasses<'py> = &context.classes.macros;
    let py: Python<'py> = consumer.py();
    let expansion: NativeMacroExpansion = NativeMacroExpansion::new(
        &context.classes.path_class.call1((file_path,))?,
        StateInput {
            loaded_macros: classes.loaded_macros.clone_ref(py),
            overrides: PyDict::new(py).unbind(),
            macro_context: classes.macro_context.clone().unbind(),
            declarations: declarations.clone().unbind(),
            consumer: consumer.clone().unbind(),
        },
        native_field(&classes.macro_context, ENFORCE_ATTRIBUTE)?.is_truthy()?,
        classes.expansion.clone_ref(py),
    )?;
    let expansion: Bound<'py, NativeMacroExpansion> = Bound::new(py, expansion)?;
    expanded(&expansion, sql, &classes.bridge)
}

/// `call_site_sql_references`: typed macro arguments as the generic SQL reference scan reads them.
pub(crate) fn argument_references(
    context: &LoopContext<'_, '_>,
    output: &ExpansionOutput,
) -> PyResult<Vec<ReferenceRow>> {
    if output.argument_references.is_empty() {
        return Ok(Vec::new());
    }
    let functions: &Bound<'_, PyDict> = &context.classes.macros.ref_functions()?;
    let mut calls: Vec<String> = Vec::with_capacity(output.argument_references.len());
    for (kind, name) in &output.argument_references {
        let function: String = functions
            .get_item(kind)?
            .map_or_else(|| Ok(String::new()), |found| found.extract())?;
        calls.push(format!("{function}(\"{name}\")"));
    }
    let syntax: LexicalSyntax = context.classes.macros.generic_syntax.clone().into();
    match extract_sql_references(&calls.join(" "), &syntax) {
        ReferenceExtraction::Extracted(scanned) => {
            Ok(scanned.references.iter().map(reference_row).collect())
        }
        ReferenceExtraction::Failed(failure) => {
            Err(compile_error(context.classes, &failure.message, None))
        }
    }
}

impl<'py> MacroClasses<'py> {
    /// The names of the project's loaded macros.
    pub(crate) fn macro_names(&self) -> PyResult<std::collections::HashSet<String>> {
        let py: Python<'py> = self.macro_context.py();
        self.loaded_macros
            .bind(py)
            .iter()
            .map(|(name, _)| name.extract())
            .collect()
    }

    /// Typed reference function by `SqlResourceRefKind` value, from the expansion classes.
    fn ref_functions(&self) -> PyResult<Bound<'py, PyDict>> {
        let py: Python<'py> = self.macro_context.py();
        Ok(self
            .expansion
            .bind(py)
            .get_item("ref_functions")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("ref_functions"))?
            .downcast_into::<PyDict>()?)
    }
}

/// A `MacroArgumentError` at its authored file location, when its offset is authored text.
fn located(
    context: &LoopContext<'_, '_>,
    error: PyErr,
    expansion: (&str, &MacroModel<'_>),
) -> PyErr {
    let (sql, model) = expansion;
    let classes: &MacroClasses<'_> = &context.classes.macros;
    let py: Python<'_> = classes.macro_context.py();
    if !model.authored || !error.is_instance(py, &classes.macro_argument_error) {
        return error;
    }
    let relocated = (|| -> PyResult<Option<PyErr>> {
        let value: Bound<'_, PyAny> = error.value(py).clone().into_any();
        let offset: usize = native_field(&value, "offset")?.extract()?;
        let passes: Vec<Vec<SplicedSpan>> = vec![model.declaration_spans.to_vec()];
        let Some(authored) =
            authored_model_offset(model.contents, model.authored_query, (sql, offset), &passes)
        else {
            return Ok(None);
        };
        let position: TextPosition = text_position(model.contents, authored);
        let fields: Bound<'_, PyDict> = PyDict::new(py);
        fields.set_item(
            "path",
            context.classes.path_class.call1((model.relative_path,))?,
        )?;
        fields.set_item("line", position.line)?;
        fields.set_item("column", position.column)?;
        let location: Bound<'_, PyAny> =
            context.contracts.source_location.call((), Some(&fields))?;
        let moved: Bound<'_, PyAny> = value.call_method1("located", (location,))?;
        moved.setattr("__suppress_context__", true)?;
        Ok(Some(PyErr::from_value(moved)))
    })();
    match relocated {
        Ok(Some(moved)) => moved,
        Ok(None) => error,
        Err(failure) => failure,
    }
}
