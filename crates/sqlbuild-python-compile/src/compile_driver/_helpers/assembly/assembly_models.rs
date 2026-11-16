//! Each compiled model of an assembled project, with its syntax checks and analysis fallback.

use pyo3::IntoPyObject;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSetMethods, PyList, PyString, PyTuple};
use sqlbuild_analysis::analysis_route::main::analysis_route::analysis_route;
use sqlbuild_analysis::analysis_route::models::AnalysisRoute;

use crate::analysis_session::models::ModelAnalyses;
use crate::compile_driver::_helpers::assembly::assembly_analysis::AnalysisContext;
use crate::compile_driver::_helpers::assembly::assembly_analysis::model_stem;
use crate::compile_driver::_helpers::assembly::assembly_analysis::selected_for_analysis;
use crate::hand_built::main::compiled_model::compiled_model as hand_built_model;
use crate::project_assembly::main::model_placeholders::model_placeholders;
use crate::project_assembly::main::validate_hook_sql_syntax::validate_hook_sql_syntax;
use crate::project_assembly::main::validate_sql_syntax::validate_sql_syntax;
use crate::project_assembly::types::ParseOptions;
use sqlbuild_python_base::boundary::main::native_compiler_error::native_compiler_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::attachments::main::analysis_cursor_sql::analysis_cursor_sql;
use sqlbuild_python_front::render::main::calls::top_level_macro_call_names::top_level_macro_call_names;
use sqlbuild_python_project::analysis_session::main::profile_dialect::profile_dialect;
use sqlbuild_python_project::analysis_session::main::query_column_rows::query_column_rows;
use sqlbuild_python_project::analysis_session::types::ColumnsRow;
use sqlbuild_python_project::compiled_views::main::model_inputs::input_analysis_sql::input_analysis_sql;
use sqlbuild_python_project::compiled_views::models::NativeModelInput;

const NATIVE_FAILURE_PREFIX: &str = "NativeCompilerError: ";
const HOOK_NAMES: [&str; 2] = ["pre_hooks", "post_hooks"];

/// One model's assembly inputs: the attached model, its native facts and its two gates.
struct ModelRequest<'a, 'py> {
    model_input: &'a Bound<'py, PyAny>,
    native_model: Bound<'py, PyAny>,
    sql_analysis_enabled: bool,
    sql_validation_enabled: bool,
}

/// Every model of the project, compiled in input order.
pub(crate) fn compiled_models<'py>(
    inputs: &Bound<'py, PyAny>,
    context: &AnalysisContext<'py>,
    (native_models, names): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = inputs.py();
    let analysis_enabled: bool = context.sql_analysis_enabled.is_truthy()?;
    let mut models: Vec<Bound<'py, PyAny>> = Vec::new();
    for (index, model_input) in native_field(inputs, "model_inputs")?
        .try_iter()?
        .enumerate()
    {
        let model_input: Bound<'py, PyAny> = model_input?;
        let sql_analysis_enabled: bool = analysis_enabled
            && native_field(&model_input, "sql_validation_enabled")?.is_truthy()?
            && selected_for_analysis(&model_input, names)?;
        let request: ModelRequest<'_, 'py> = ModelRequest {
            model_input: &model_input,
            sql_analysis_enabled,
            sql_validation_enabled: selected_for_analysis(&model_input, names)?,
            native_model: native_models.get_item(index)?,
        };
        models.push(compiled_model(context, &request)?);
    }
    PyTuple::new(py, models)
}

/// `_assemble_compiled_model`.
fn compiled_model<'py>(
    context: &AnalysisContext<'py>,
    request: &ModelRequest<'_, 'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let model_input: &Bound<'py, PyAny> = request.model_input;
    let py: Python<'py> = model_input.py();
    let model_name: Bound<'py, PyAny> = model_stem(model_input)?;
    let syntax_validated: bool =
        native_field(&request.native_model, "syntax_valid")?.is_truthy()?;
    let config_values: Bound<'py, PyAny> =
        native_field(model_input, "config")?.getattr("values")?;
    let analysis_query_sql: Bound<'py, PyAny> = PyString::new(
        py,
        &analysis_cursor_sql(
            py,
            &input_analysis_sql(model_input, &profile_dialect(&context.profile)?)?,
            &mapping_get(&config_values, "cursor_type")?,
        )?,
    )
    .into_any();
    let placeholder_values: Option<Bound<'py, PyDict>> = model_placeholders(model_input)?;
    let placeholders: Bound<'py, PyAny> = match &placeholder_values {
        Some(values) => values.clone().into_any(),
        None => py.None().into_bound(py),
    };
    let input_validated: bool = native_field(model_input, "sql_validation_enabled")?.is_truthy()?;
    let location = || -> PyResult<(String, Bound<'py, PyAny>)> {
        Ok((
            model_name.extract()?,
            native_field(model_input, "model_file")?.getattr("file_path")?,
        ))
    };
    let syntax = || -> PyResult<ParseOptions> {
        Ok((
            placeholder_values
                .as_ref()
                .map(|values| values.extract())
                .transpose()?,
            native_field(&context.profile, "sql_analysis_dialect")?.extract()?,
        ))
    };
    let validate_sql = || -> PyResult<()> {
        validate_sql_syntax(
            py,
            &analysis_query_sql.extract::<String>()?,
            location()?,
            syntax()?,
        )
    };
    let analysis_dialect: Option<String> =
        native_field(&context.profile, "sql_analysis_dialect")?.extract()?;
    let legacy_route: bool = matches!(
        analysis_route(analysis_dialect.as_deref()),
        AnalysisRoute::Legacy
    );
    let validates: bool = input_validated && !syntax_validated && legacy_route;
    if request.sql_validation_enabled && validates {
        for hook_name in HOOK_NAMES {
            validate_hook_sql_syntax(
                mapping_get(&config_values, hook_name)?,
                hook_name,
                location()?,
                syntax()?,
            )?;
        }
    }
    let mut session_analysis: Bound<'py, PyAny> = py.None().into_bound(py);
    let mut fallback: Bound<'py, PyAny> = py.None().into_bound(py);
    if request.sql_analysis_enabled {
        let analysed = match &context.analyses {
            Some(analyses) => analyses
                .positions
                .get_item(&model_name)?
                .map(|position| (analyses, position)),
            None => None,
        };
        let Some((analyses, position)) = analysed else {
            return Err(native_compiler_error(format!(
                "{NATIVE_FAILURE_PREFIX}native model analysis: the session returned no \
                 analysis for model '{}'",
                model_name.str()?
            )));
        };
        let session: Bound<'py, PyAny> = analyses.session.as_ref().map_or_else(
            || py.None().into_bound(py),
            |session| session.clone().into_any(),
        );
        session_analysis = PyTuple::new(py, [session, position])?.into_any();
        if !analyses.succeeded.contains(&model_name)? {
            if validates {
                validate_sql()?;
            }
            fallback = fallback_columns(context, &analysis_query_sql, &placeholders)?;
        }
    } else if request.sql_validation_enabled && validates {
        validate_sql()?;
    }
    let declared_macro_deps: Bound<'py, PyAny> = native_field(model_input, "macro_deps")?;
    let macro_deps: Bound<'py, PyAny> = if declared_macro_deps.is_truthy()? {
        declared_macro_deps
    } else {
        top_level_macro_call_names(
            py,
            &native_field(model_input, "macro_source_sql")?.extract::<String>()?,
            context.syntax.as_ref(),
        )?
        .into_any()
    };
    let native_model: &Bound<'py, PyAny> = &request.native_model;
    if model_input.is_instance_of::<NativeModelInput>() {
        let macro_list: Bound<'py, PyList> =
            PyList::new(py, macro_deps.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        return model_input.call_method1(
            "compiled_model",
            ((
                native_field(native_model, "deps")?,
                session_analysis,
                fallback,
                native_field(native_model, "dynamic_contract")?,
                macro_list,
            ),),
        );
    }
    let analysis: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::HandBuiltAnalysis,
        &[
            ("session", session_analysis),
            ("fallback", fallback),
            (
                "standalone_proof",
                native_field(native_model, "dynamic_contract")?,
            ),
        ],
    )?;
    hand_built_model(
        model_input,
        &native_field(native_model, "deps")?,
        &analysis,
        &macro_deps,
    )
}

/// The columns a parse-only native query inference reads from a model whose analysis failed.
fn fallback_columns<'py>(
    context: &AnalysisContext<'py>,
    analysis_query_sql: &Bound<'py, PyAny>,
    placeholders: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = analysis_query_sql.py();
    let query: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::NativeColumnQuery,
        &[
            ("sql", analysis_query_sql.clone()),
            ("mode", PyString::new(py, "parse").into_any()),
            ("placeholders", placeholders.clone()),
        ],
    )?;
    let empty: Bound<'py, PyAny> = PyDict::new(py).into_any();
    let nullability: Bound<'py, PyAny> = if context.nullability.is_truthy()? {
        context.nullability.clone()
    } else {
        empty.clone()
    };
    let rows: Vec<ColumnsRow> = query_column_rows(
        &PyTuple::new(py, [query])?.into_any(),
        &context.profile,
        (&empty, &nullability),
    )?;
    rows.into_iter()
        .next()
        .map(|(_, columns, _)| columns)
        .into_bound_py_any(py)
}

/// `_project_binding_diagnostics`: every analysed model's located binding diagnostics.
pub(crate) fn project_binding_diagnostics<'py>(
    models: &Bound<'py, PyTuple>,
    analyses: Option<&ModelAnalyses<'py>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::new();
    let Some(analyses) = analyses else {
        return Ok(diagnostics);
    };
    for model in models.as_any().try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        if analyses.positions.contains(native_field(&model, "name")?)? {
            for diagnostic in native_field(&model, "binding_diagnostics")?.try_iter()? {
                diagnostics.push(diagnostic?);
            }
        }
    }
    Ok(diagnostics)
}

/// `_binding_rows`: each analysed model's raw binding rows by model name.
pub(crate) fn binding_rows<'py>(
    py: Python<'py>,
    analyses: Option<&ModelAnalyses<'py>>,
) -> PyResult<Bound<'py, PyDict>> {
    let rows_by_name: Bound<'py, PyDict> = PyDict::new(py);
    let Some(session) = analyses.and_then(|analyses| analyses.session.as_ref()) else {
        return Ok(rows_by_name);
    };
    let rows = session.borrow().binding_rows();
    for (name, position) in analyses
        .map(|analyses| analyses.positions.iter())
        .into_iter()
        .flatten()
    {
        let position: usize = position.extract()?;
        rows_by_name.set_item(name, rows.get(position).cloned().into_pyobject(py)?)?;
    }
    Ok(rows_by_name)
}
