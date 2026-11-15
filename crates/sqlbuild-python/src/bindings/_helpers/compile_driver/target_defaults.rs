//! Adapter default schema and database applied to compiled relation locations.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;

use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::compiled_views::function_views::NativeCompiledFunction;
use crate::bindings::_helpers::compiled_views::records::project_with;
use crate::bindings::_helpers::compiled_views::seed_views::NativeCompiledSeed;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;

/// The adapter defaults every compiled location falls back to.
pub(crate) struct TargetDefaults<'py> {
    pub(crate) default_schema: Bound<'py, PyAny>,
    pub(crate) default_database: Bound<'py, PyAny>,
    pub(crate) render_qualified_name: Bound<'py, PyAny>,
    pub(crate) python_functions_inherit_default_namespace: Bound<'py, PyAny>,
}

impl<'py> TargetDefaults<'py> {
    /// `(default_schema, default_database)`.
    fn pair(&self) -> PyResult<(Option<String>, Option<String>)> {
        Ok((
            self.default_schema.extract()?,
            self.default_database.extract()?,
        ))
    }
}

/// `left or right`.
fn or_else<'py>(left: Bound<'py, PyAny>, right: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(if left.is_truthy()? {
        left
    } else {
        right.clone()
    })
}

/// Apply adapter default schema/database to compiled project targets.
pub(crate) fn apply_target_defaults<'py>(
    project: &Bound<'py, PyAny>,
    defaults: &TargetDefaults<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let mut models: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        models.push(if let Ok(native) = model.cast::<NativeCompiledModel>() {
            Bound::new(
                py,
                native.get().with_target_defaults(
                    py,
                    defaults.pair()?,
                    defaults.render_qualified_name.clone(),
                )?,
            )?
            .into_any()
        } else {
            replaced(
                &model,
                &[(
                    "destination",
                    resolved_target(&native_field(&model, "destination")?, defaults)?,
                )],
            )?
        });
    }
    let mut seeds: Vec<Bound<'py, PyAny>> = Vec::new();
    for seed in native_field(project, "seeds")?.try_iter()? {
        let seed: Bound<'py, PyAny> = seed?;
        seeds.push(if let Ok(native) = seed.cast::<NativeCompiledSeed>() {
            Bound::new(
                py,
                native.get().with_target_defaults(
                    py,
                    defaults.pair()?,
                    defaults.render_qualified_name.clone(),
                )?,
            )?
            .into_any()
        } else {
            replaced(
                &seed,
                &[(
                    "destination",
                    resolved_target(&native_field(&seed, "destination")?, defaults)?,
                )],
            )?
        });
    }
    let mut functions: Vec<Bound<'py, PyAny>> = Vec::new();
    for function in native_field(project, "functions")?.try_iter()? {
        let function: Bound<'py, PyAny> = function?;
        let native: &Bound<'py, NativeCompiledFunction> = function.cast()?;
        functions.push(
            Bound::new(
                py,
                native.get().with_target_defaults(
                    py,
                    defaults.pair()?,
                    defaults.render_qualified_name.clone(),
                    defaults
                        .python_functions_inherit_default_namespace
                        .extract()?,
                )?,
            )?
            .into_any(),
        );
    }
    project_with(
        &project.clone(),
        Some(keywords(
            py,
            &[
                (
                    "effective_target_database",
                    or_else(
                        native_field(project, "effective_target_database")?,
                        &defaults.default_database,
                    )?,
                ),
                (
                    "effective_target_schema",
                    or_else(
                        native_field(project, "effective_target_schema")?,
                        &defaults.default_schema,
                    )?,
                ),
                ("models", PyTuple::new(py, models)?.into_any()),
                ("seeds", PyTuple::new(py, seeds)?.into_any()),
                ("functions", PyTuple::new(py, functions)?.into_any()),
            ],
        )?),
    )
}

/// `_resolve_target`: `target` with adapter defaults for a missing schema or database.
fn resolved_target<'py>(
    target: &Bound<'py, PyAny>,
    defaults: &TargetDefaults<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target.py();
    let target_schema: Bound<'py, PyAny> = native_field(target, "schema")?;
    let target_database: Bound<'py, PyAny> = native_field(target, "database")?;
    let schema: Bound<'py, PyAny> = if target_schema.is_none() {
        defaults.default_schema.clone()
    } else {
        target_schema.clone()
    };
    let database: Bound<'py, PyAny> = if target_database.is_none() {
        defaults.default_database.clone()
    } else {
        target_database.clone()
    };
    let name: Bound<'py, PyAny> = native_field(target, "name")?;
    let qualified_name: Bound<'py, PyAny> = call_with(
        &defaults.render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", name.clone()),
        ],
    )?;
    if schema.eq(&target_schema)?
        && database.eq(&target_database)?
        && qualified_name.eq(native_field(target, "qualified_name")?)?
    {
        return Ok(target.clone());
    }
    new_record_with(
        py,
        RecordId::CompiledRelationLocation,
        &[
            ("database", database),
            ("schema", schema),
            ("name", name),
            ("qualified_name", qualified_name),
            ("logical_schema", native_field(target, "logical_schema")?),
            (
                "logical_database",
                native_field(target, "logical_database")?,
            ),
        ],
    )
}
