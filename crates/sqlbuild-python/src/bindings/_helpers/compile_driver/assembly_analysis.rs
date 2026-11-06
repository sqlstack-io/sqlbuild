//! Project assembly's inference profile, binding catalog, source shapes and model analysis.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple, PyTupleMethods};

use sqlbuild_analysis::assembly::analysis_session::main::native_dialect::native_dialect;
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;
use sqlbuild_host::compile_context::main::record_compile_cpu_timing::record_compile_cpu_timing;
use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::analysis::binding_catalog::{
    BindingCatalog, CatalogKey, ignores_quoted_case,
};
use crate::bindings::_helpers::analysis_session::model_analyses::{
    ModelAnalyses, ModelAnalysisRequest, analyze_models,
};
use crate::bindings::_helpers::analysis_session::profile_inputs::text_pairs;
use crate::bindings::_helpers::analysis_session::stage_functions::expression_source_shapes;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compile_context::compile_timings::PythonProcessClock;
use crate::bindings::_helpers::compile_driver::analysis_cache::analysis_cache_context;
use crate::bindings::_helpers::compile_driver::assembly::{AssemblyRequest, AssemblyStage};
use crate::bindings::_helpers::compile_driver::stage_work::{
    EXPRESSION_SHAPES, MODEL_ANALYSIS, count_stage,
};
use crate::bindings::_helpers::compiled_views::model_input_view::attached_schema;
use crate::bindings::_helpers::compiled_views::source_views::read_source_view;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::project_assembly::declared_relations::{
    DeclaredRelations, DeclaredShapes, declared_relations,
};
use crate::bindings::_helpers::records::construct::{enum_member, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::fields::replaced;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::semantic_shapes::declarations::{declared_types, function_names};
use crate::bindings::_helpers::sqltext::lexical_syntax::optional_record_syntax;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use std::collections::HashMap;

const GENERIC_DIALECT: &str = "generic";
const UDF_PREFIX: &str = "__sqlbuild_udf_";

/// The analysis state every assembled model and the project share.
pub(crate) struct AnalysisContext<'py> {
    pub(crate) profile: Bound<'py, PyAny>,
    /// Python's `settings.sql_analysis and not skip_column_inference`, as that value.
    pub(crate) sql_analysis_enabled: Bound<'py, PyAny>,
    pub(crate) column_types: Bound<'py, PyAny>,
    pub(crate) nullability: Bound<'py, PyAny>,
    pub(crate) complete_schemas: Bound<'py, PyAny>,
    pub(crate) dynamic_families: Vec<(String, Vec<DynamicFamily>)>,
    /// The model analyses, or `None` when SQL analysis does not run.
    pub(crate) analyses: Option<ModelAnalyses<'py>>,
    /// The native rows of `column_types`, `nullability` and `complete_schemas`.
    pub(crate) declared: DeclaredShapes,
    /// The lexical rules the project's SQL is authored in.
    pub(crate) syntax: Option<LexicalSyntax>,
}

/// Whether `model_input` is selected for analysis by `names` (`None` selects every model).
pub(crate) fn selected_for_analysis(
    model_input: &Bound<'_, PyAny>,
    names: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    Ok(names.is_none() || names.contains(model_stem(model_input)?)?)
}

/// The model's name: its file's stem.
pub(crate) fn model_stem<'py>(model_input: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    native_field(model_input, "model_file")?
        .getattr("file_path")?
        .getattr("stem")
}

/// Python's `left and not right`.
pub(crate) fn and_not<'py>(
    left: &Bound<'py, PyAny>,
    right: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !left.is_truthy()? {
        return Ok(left.clone());
    }
    Ok(PyBool::new(left.py(), !right.is_truthy()?)
        .to_owned()
        .into_any())
}

/// The profile, catalog, declared relations and model analyses of `request`'s project.
pub(crate) fn analysis_context<'py>(
    py: Python<'py>,
    request: &AssemblyRequest,
) -> PyResult<AnalysisContext<'py>> {
    let inputs: &Bound<'py, PyAny> = request.inputs.bind(py);
    let sql_analysis_enabled: Bound<'py, PyAny> = and_not(
        &native_field(inputs, "effective_settings")?.getattr("sql_analysis")?,
        request.skip_column_inference.bind(py),
    )?;
    let DeclaredRelations {
        column_types,
        nullability,
        complete_schemas,
        declared,
    }: DeclaredRelations<'py> = declared_relations(inputs.clone())?;
    let (column_types, nullability, complete_schemas): (
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
    ) = (
        column_types.into_any(),
        nullability.into_any(),
        complete_schemas.into_any(),
    );
    let dynamic_families = dynamic_families_by_table(inputs)?;
    let profile: Bound<'py, PyAny> =
        profile_with_catalog(inputs, request.inference_profile.bind(py), &column_types)?;
    let mut context: AnalysisContext<'py> = AnalysisContext {
        profile,
        sql_analysis_enabled,
        column_types,
        nullability,
        complete_schemas,
        dynamic_families,
        analyses: None,
        declared,
        syntax: optional_record_syntax(&native_field(inputs, "sql_lexical_syntax")?)?,
    };
    if context.sql_analysis_enabled.is_truthy()? {
        let declared: DeclaredShapes = std::mem::take(&mut context.declared);
        context.declared = record_expression_shapes(request, &context, declared)?;
    }
    let catalog: Bound<'py, PyAny> = native_field(&context.profile, "binding_catalog")?;
    native_field(&catalog, "native")?
        .call_method1("update_relations", (&context.complete_schemas,))?;
    native_field(&catalog, "schemas")?.call_method1("update", (&context.complete_schemas,))?;
    context.analyses = model_analyses(request, &context)?;
    Ok(context)
}

/// `_build_dynamic_families_by_table`: enforced contract models' dynamic column families.
fn dynamic_families_by_table<'py>(
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Vec<(String, Vec<DynamicFamily>)>> {
    let py: Python<'py> = inputs.py();
    let enforced: Bound<'py, PyAny> = enum_member(py, RecordId::ContractPolicy, "ENFORCED")?;
    let mut families: Vec<(String, Vec<DynamicFamily>)> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for model_input in native_field(inputs, "model_inputs")?.try_iter()? {
        let model_input: Bound<'py, PyAny> = model_input?;
        let contract: Bound<'py, PyAny> = native_field(&model_input, "config")?
            .getattr("values")?
            .call_method1("get", ("contract",))?;
        if !contract.eq(&enforced)? {
            continue;
        }
        let Some(schema) = attached_schema(&model_input)? else {
            continue;
        };
        if !schema.dynamic_columns.is_empty() {
            if let Some(&index) = positions.get(&schema.name) {
                families[index].1 = schema.dynamic_columns;
            } else {
                positions.insert(schema.name.clone(), families.len());
                families.push((schema.name, schema.dynamic_columns));
            }
        }
    }
    Ok(families)
}

/// Sorted function names and declared types registered with native validation.
pub(crate) fn declared_names<'py>(
    inputs: &Bound<'py, PyAny>,
    column_types: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = inputs.py();
    let functions: Bound<'py, PyAny> = native_field(inputs, "sql_function_inputs")?;
    let names: Bound<'py, PyAny> = PyTuple::new(py, function_names(&functions)?)?.into_any();
    let types: Bound<'py, PyAny> =
        PyTuple::new(py, declared_types(&functions, column_types)?)?.into_any();
    Ok((names, types))
}

/// The adapter's profile with project declarations, the native dialect and a binding catalog.
fn profile_with_catalog<'py>(
    inputs: &Bound<'py, PyAny>,
    inference_profile: &Bound<'py, PyAny>,
    column_types: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.py();
    let profile: Bound<'py, PyAny> = if inference_profile.is_truthy()? {
        inference_profile.clone()
    } else {
        host::entry(py, AdapterHost::ExpressionInferenceProfile)?.call0()?
    };
    let (known_functions, known_types): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        declared_names(inputs, column_types)?;
    let dialect: Bound<'py, PyAny> = native_field(&profile, "sql_analysis_dialect")?;
    let ignore_case: Bound<'py, PyAny> = PyBool::new(
        py,
        ignores_quoted_case(&native_field(inputs, "effective_connection")?, &dialect)?,
    )
    .to_owned()
    .into_any();
    let name: Option<String> = if dialect.is_none() {
        None
    } else {
        Some(dialect.extract()?)
    };
    let native: Bound<'py, PyAny> = match native_dialect(name.as_deref()) {
        Some(native) if Some(native) != name.as_deref() => PyString::new(py, native).into_any(),
        _ => dialect,
    };
    let profile: Bound<'py, PyAny> = replaced(
        &profile.clone(),
        &[
            ("semantic_known_functions", known_functions),
            ("semantic_known_types", known_types),
            ("quoted_identifiers_ignore_case", ignore_case),
            ("sql_analysis_dialect", native),
            (
                "function_return_types",
                function_return_types(inputs, &profile)?.into_any(),
            ),
        ],
    )?;
    let dialect: Bound<'py, PyAny> = native_field(&profile, "sql_analysis_dialect")?;
    let dialect: String = if dialect.is_truthy()? {
        dialect.extract()?
    } else {
        GENERIC_DIALECT.to_owned()
    };
    let catalog: Bound<'py, PyAny> = Bound::new(
        py,
        BindingCatalog::create(
            py,
            CatalogKey {
                dialect,
                quoted_ignore_case: native_field(&profile, "quoted_identifiers_ignore_case")?
                    .extract()?,
                known_functions: native_field(&profile, "semantic_known_functions")?.extract()?,
                known_types: native_field(&profile, "semantic_known_types")?.extract()?,
            },
            PyDict::new(py).as_any(),
        )?,
    )?
    .into_any();
    replaced(&profile, &[("binding_catalog", catalog)])
}

/// The profile's function return types with each scalar project function's analysis name.
fn function_return_types<'py>(
    inputs: &Bound<'py, PyAny>,
    profile: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = inputs.py();
    let types: Bound<'py, PyDict> = PyDict::new(py);
    types.call_method1("update", (native_field(profile, "function_return_types")?,))?;
    let scalar: Bound<'py, PyDict> = PyDict::new(py);
    for function in native_field(inputs, "sql_function_inputs")?.try_iter()? {
        let function: Bound<'py, PyAny> = function?;
        if native_field(&function, "return_columns")?.is_truthy()? {
            continue;
        }
        let name: Bound<'py, PyAny> = native_field(&function, "name")?.str()?.into_any();
        let key: Bound<'py, PyAny> = PyString::new(py, UDF_PREFIX)
            .into_any()
            .add(name)?
            .call_method0("upper")?;
        scalar.set_item(key, native_field(&function, "returns")?)?;
    }
    types.update(scalar.as_mapping())?;
    Ok(types)
}

/// Record each expression source's analysed shape as its declared relation.
fn record_expression_shapes<'py>(
    request: &AssemblyRequest,
    context: &AnalysisContext<'py>,
    mut declared: DeclaredShapes,
) -> PyResult<DeclaredShapes> {
    let py: Python<'py> = context.profile.py();
    let inputs: &Bound<'py, PyAny> = request.inputs.bind(py);
    let mut sources: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut expressions: Vec<Bound<'py, PyAny>> = Vec::new();
    for source_input in native_field(inputs, "source_inputs")?.try_iter()? {
        let source_input: Bound<'py, PyAny> = source_input?;
        let expression: Bound<'py, PyAny> = source_expression(&source_input)?;
        if expression.is_truthy()? {
            sources.push(source_input);
            expressions.push(expression);
        }
    }
    let expressions: Bound<'py, PyAny> = PyTuple::new(py, expressions)?.into_any();
    request.before_stage(AssemblyStage::ExpressionShapes, || {
        keywords(
            py,
            &[
                ("expressions", expressions.clone()),
                ("profile", context.profile.clone()),
            ],
        )
    })?;
    count_stage(py, EXPRESSION_SHAPES, 1)?;
    let shapes: Vec<Bound<'py, PyAny>> = expression_source_shapes(&expressions, &context.profile)?
        .iter()
        .collect();
    if shapes.len() != sources.len() {
        let relation: &str = if shapes.len() < sources.len() {
            "shorter"
        } else {
            "longer"
        };
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "zip() argument 2 is {relation} than argument 1"
        )));
    }
    let unknown: Bound<'py, PyAny> = enum_member(py, RecordId::InferredNullability, "UNKNOWN")?;
    for (source_input, shape) in sources.iter().zip(shapes) {
        if shape.is_none() {
            continue;
        }
        let name: Bound<'py, PyAny> = source_name(source_input)?;
        let declared_types: Bound<'py, PyAny> =
            mapping_get_or(&context.column_types, &name, PyDict::new(py))?;
        let declared_shape: Bound<'py, PyDict> = PyDict::new(py);
        for item in mapping_items(&shape)?.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            let column: Bound<'py, PyAny> = item.get_item(0)?;
            declared_shape.set_item(
                &column,
                mapping_get_or(&declared_types, &column, item.get_item(1)?)?,
            )?;
        }
        context.complete_schemas.set_item(&name, &declared_shape)?;
        context.column_types.set_item(&name, &declared_shape)?;
        let nullability: Bound<'py, PyDict> = PyDict::new(py);
        for column in declared_shape.keys() {
            let known: Bound<'py, PyAny> =
                mapping_get_or(&context.nullability, &name, PyDict::new(py))?;
            nullability.set_item(&column, mapping_get_or(&known, &column, &unknown)?)?;
        }
        context.nullability.set_item(&name, &nullability)?;
        let key: String = name.extract()?;
        let shape_pairs: Pairs = text_pairs(declared_shape.as_any())?;
        declared.complete.set(key.clone(), shape_pairs.clone());
        declared.types.set(key.clone(), shape_pairs);
        declared
            .nullability
            .set(key, text_pairs(nullability.as_any())?);
    }
    Ok(declared)
}

/// `source_input.source_entry.expression`; a native source answers from its attached record.
fn source_expression<'py>(source_input: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    match read_source_view(source_input, |source, py| {
        source
            .attached()
            .expression
            .as_deref()
            .into_bound_py_any(py)
    }) {
        Some(expression) => expression,
        None => native_field(source_input, "source_entry")?.getattr("expression"),
    }
}

/// `source_input.source_entry.name`; a native source answers from its attached record.
fn source_name<'py>(source_input: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    match read_source_view(source_input, |source, py| {
        Ok(PyString::new(py, &source.attached().name).into_any())
    }) {
        Some(name) => name,
        None => native_field(source_input, "source_entry")?.getattr("name"),
    }
}

/// Each selected model's native analysis, timed as `model_analysis_ms`, when SQL analysis runs.
fn model_analyses<'py>(
    request: &AssemblyRequest,
    context: &AnalysisContext<'py>,
) -> PyResult<Option<ModelAnalyses<'py>>> {
    let py: Python<'py> = context.profile.py();
    if !context.sql_analysis_enabled.is_truthy()? {
        return Ok(None);
    }
    let inputs: &Bound<'py, PyAny> = request.inputs.bind(py);
    let names: &Bound<'py, PyAny> = request.analysis_model_names.bind(py);
    let mode: &Bound<'py, PyAny> = request.column_lineage_mode.bind(py);
    let lineage_mode: Bound<'py, PyAny> = record_class(py, RecordId::ColumnLineageMode)?;
    let rich: bool = mode.eq(native_field(&lineage_mode, "RICH")?)?;
    let allow_compact: bool = mode.eq(native_field(&lineage_mode, "FAST")?)? || rich;
    let flags: (Bound<'py, PyAny>, Bound<'py, PyAny>) = (
        PyBool::new(py, allow_compact).to_owned().into_any(),
        PyBool::new(py, rich).to_owned().into_any(),
    );
    let cache: Bound<'py, PyAny> = if names.ne(PyFrozenSet::empty(py)?)? {
        analysis_cache(request, context, &flags)?
    } else {
        py.None().into_bound(py)
    };
    record_compile_timing("model_analysis_ms", || {
        record_compile_cpu_timing(&PythonProcessClock(py), "model_analysis_cpu_ms", || {
            let mut model_inputs: Vec<Bound<'py, PyAny>> = Vec::new();
            for model_input in native_field(inputs, "model_inputs")?.try_iter()? {
                let model_input: Bound<'py, PyAny> = model_input?;
                if native_field(&model_input, "sql_validation_enabled")?.is_truthy()?
                    && selected_for_analysis(&model_input, names)?
                {
                    model_inputs.push(model_input);
                }
            }
            count_stage(py, MODEL_ANALYSIS, 1)?;
            analyze_models(&ModelAnalysisRequest {
                model_inputs,
                profile: context.profile.clone(),
                column_types: context.column_types.clone(),
                nullability: context.nullability.clone(),
                complete_schemas: context.complete_schemas.clone(),
                dynamic_families: &context.dynamic_families,
                rich_type_inference: rich,
                cache,
                declared: Some(&context.declared),
            })
            .map(Some)
        })
    })
}

/// The analysis cache context keyed by the project's declarations, target and vars.
fn analysis_cache<'py>(
    request: &AssemblyRequest,
    context: &AnalysisContext<'py>,
    (allow_compact, rich): &(Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = context.profile.py();
    let inputs: &Bound<'py, PyAny> = request.inputs.bind(py);
    let (known_functions, known_types): (Bound<'py, PyAny>, Bound<'py, PyAny>) =
        declared_names(inputs, &context.column_types)?;
    let namespace: Bound<'py, PyDict> = keywords(
        py,
        &[
            ("known_functions", known_functions),
            ("known_types", known_types),
            ("target", native_field(inputs, "effective_target_name")?),
            ("vars", native_field(inputs, "effective_vars")?),
        ],
    )?;
    Ok(analysis_cache_context(
        request.analysis_cache_dir.bind(py),
        &context.profile,
        (allow_compact, rich),
        namespace.as_any(),
    )?
    .unwrap_or_else(|| py.None().into_bound(py)))
}
