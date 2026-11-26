//! A compiled project, read from the native compiled project that owns its models.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};

use crate::compiled_views::_helpers::project::loads::{LoadFact, load_facts, load_records};
use crate::graph::models::ResourceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const MODELS_FIELD: &str = "models";
const RUN_ID_FIELD: &str = "run_id";
const ENFORCE_EXPLICIT_FIELD: &str = "enforce_explicit_references";
const KEY_ATTRIBUTE: &str = "key";
const DEPS_ATTRIBUTE: &str = "deps";
pub(crate) const TAGS_ATTRIBUTE: &str = "tags";
pub(crate) const SCHEMA_ENTRY_ATTRIBUTE: &str = "schema_entry";
const RESOURCE_TYPE_ATTRIBUTE: &str = "resource_type";
const NAME_ATTRIBUTE: &str = "name";

/// Project inputs held as Python objects: user code, discovery or an integration owns them.
#[derive(Debug)]
pub(crate) struct BoundaryInputs {
    /// The resolved connection, adapter integration values.
    pub(crate) effective_connection: Py<PyAny>,
    /// Project variables, user values.
    pub(crate) effective_vars: Py<PyAny>,
    /// The native binding catalog.
    pub(crate) binding_catalog: Py<PyAny>,
    /// The adapter's lexical syntax.
    pub(crate) sql_lexical_syntax: Py<PyAny>,
    /// The compile cache directory path.
    pub(crate) compile_cache_dir: Py<PyAny>,
    /// Discovered loader functions, user callables.
    pub(crate) loader_functions: Py<PyAny>,
    /// Asset and check source locations, without retaining additional Python callables.
    pub(crate) python_execution_inputs: BTreeMap<String, (String, PathBuf)>,
    /// Discovered Python hooks, user callables.
    pub(crate) hook_functions: Py<PyAny>,
    /// Discovered SQL hook files.
    pub(crate) sql_hook_files: Py<PyAny>,
    /// Discovered custom materializations.
    pub(crate) materialization_files: Py<PyAny>,
    /// Public enums, user declaration values.
    pub(crate) public_enums: Py<PyAny>,
    /// Public constants, user declaration values.
    pub(crate) public_constants: Py<PyAny>,
    /// Loaded macro modules, user code.
    pub(crate) loaded_macros: Py<PyAny>,
    /// The external reference resolver integration, or `None`.
    pub(crate) external_sql_reference_resolver: Py<PyAny>,
    /// The native analysis session, or `None`.
    pub(crate) native_session: Py<PyAny>,
}

/// Compiler-derived project parts still held as Python objects until they are owner facts.
#[derive(Debug)]
pub(crate) struct CompilerHandles {
    /// Effective settings.
    pub(crate) settings: Py<PyAny>,
    /// Effective scenario config.
    pub(crate) scenario: Py<PyAny>,
    /// Compiled sources.
    pub(crate) sources: Py<PyAny>,
    /// Compiled seeds.
    pub(crate) seeds: Py<PyAny>,
    /// Compiled SQL functions.
    pub(crate) functions: Py<PyAny>,
    /// Compiled audits.
    pub(crate) audits: Py<PyAny>,
    /// Compiled SQL tests.
    pub(crate) sql_tests: Py<PyAny>,
    /// Compiled SQL scenarios.
    pub(crate) sql_scenarios: Py<PyAny>,
    /// Compiler diagnostics.
    pub(crate) diagnostics: Py<PyAny>,
    /// Literal relations no resource matched.
    pub(crate) unmatched_literal_sql_relations: Py<PyAny>,
    /// The declaration scope index.
    pub(crate) scope_index: Py<PyAny>,
}

/// The project-level values of one compiled project view.
#[derive(Debug)]
pub(crate) struct ProjectFields {
    pub(crate) run_id: String,
    pub(crate) effective_target_name: Option<String>,
    pub(crate) effective_target_database: Option<String>,
    pub(crate) effective_target_schema: Option<String>,
    pub(crate) sql_analysis_dialect: Option<String>,
    pub(crate) enforce_explicit_references: bool,
    pub(crate) models: Py<PyTuple>,
    /// The loads the project compiles.
    pub(crate) loads: Vec<LoadFact>,
    pub(crate) boundary: BoundaryInputs,
    pub(crate) compiler: CompilerHandles,
}

impl ProjectFields {
    /// The fields named in `fields`, each required.
    pub(crate) fn from_fields(fields: &Bound<'_, PyDict>) -> PyResult<Self> {
        Ok(Self {
            run_id: required(fields, RUN_ID_FIELD)?.extract()?,
            effective_target_name: required(fields, "effective_target_name")?.extract()?,
            effective_target_database: required(fields, "effective_target_database")?.extract()?,
            effective_target_schema: required(fields, "effective_target_schema")?.extract()?,
            sql_analysis_dialect: required(fields, "sql_analysis_dialect")?.extract()?,
            enforce_explicit_references: required(fields, ENFORCE_EXPLICIT_FIELD)?.extract()?,
            models: required(fields, MODELS_FIELD)?
                .downcast_into::<PyTuple>()?
                .unbind(),
            loads: load_facts(&required(fields, "loads")?)?,
            boundary: BoundaryInputs {
                effective_connection: required(fields, "effective_connection")?.unbind(),
                effective_vars: required(fields, "effective_vars")?.unbind(),
                binding_catalog: required(fields, "binding_catalog")?.unbind(),
                sql_lexical_syntax: required(fields, "sql_lexical_syntax")?.unbind(),
                compile_cache_dir: required(fields, "compile_cache_dir")?.unbind(),
                loader_functions: required(fields, "loader_functions")?.unbind(),
                python_execution_inputs: required(fields, "python_execution_inputs")?.extract()?,
                hook_functions: required(fields, "hook_functions")?.unbind(),
                sql_hook_files: required(fields, "sql_hook_files")?.unbind(),
                materialization_files: required(fields, "materialization_files")?.unbind(),
                public_enums: required(fields, "public_enums")?.unbind(),
                public_constants: required(fields, "public_constants")?.unbind(),
                loaded_macros: required(fields, "loaded_macros")?.unbind(),
                external_sql_reference_resolver: required(
                    fields,
                    "external_sql_reference_resolver",
                )?
                .unbind(),
                native_session: required(fields, "native_session")?.unbind(),
            },
            compiler: CompilerHandles {
                settings: required(fields, "settings")?.unbind(),
                scenario: required(fields, "scenario")?.unbind(),
                sources: required(fields, "sources")?.unbind(),
                seeds: required(fields, "seeds")?.unbind(),
                functions: required(fields, "functions")?.unbind(),
                audits: required(fields, "audits")?.unbind(),
                sql_tests: required(fields, "sql_tests")?.unbind(),
                sql_scenarios: required(fields, "sql_scenarios")?.unbind(),
                diagnostics: required(fields, "diagnostics")?.unbind(),
                unmatched_literal_sql_relations: required(
                    fields,
                    "unmatched_literal_sql_relations",
                )?
                .unbind(),
                scope_index: required(fields, "scope_index")?.unbind(),
            },
        })
    }

    /// These fields with `changes` applied.
    pub(crate) fn changed(&self, py: Python<'_>, changes: &Bound<'_, PyDict>) -> PyResult<Self> {
        let current: Bound<'_, PyDict> = self.as_fields(py)?;
        for (name, value) in changes.iter() {
            if !current.contains(&name)? {
                return Err(PyTypeError::new_err(format!(
                    "unknown project field {name}"
                )));
            }
            current.set_item(name, value)?;
        }
        Self::from_fields(&current)
    }

    /// Every field by name.
    pub(crate) fn as_fields<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item(RUN_ID_FIELD, &self.run_id)?;
        fields.set_item("effective_target_name", &self.effective_target_name)?;
        fields.set_item("effective_target_database", &self.effective_target_database)?;
        fields.set_item("effective_target_schema", &self.effective_target_schema)?;
        fields.set_item("sql_analysis_dialect", &self.sql_analysis_dialect)?;
        fields.set_item(ENFORCE_EXPLICIT_FIELD, self.enforce_explicit_references)?;
        fields.set_item(MODELS_FIELD, self.models.bind(py))?;
        fields.set_item(
            "effective_connection",
            self.boundary.effective_connection.bind(py),
        )?;
        fields.set_item("effective_vars", self.boundary.effective_vars.bind(py))?;
        fields.set_item("binding_catalog", self.boundary.binding_catalog.bind(py))?;
        fields.set_item(
            "sql_lexical_syntax",
            self.boundary.sql_lexical_syntax.bind(py),
        )?;
        fields.set_item(
            "compile_cache_dir",
            self.boundary.compile_cache_dir.bind(py),
        )?;
        fields.set_item("loads", load_records(py, &self.loads)?)?;
        fields.set_item("loader_functions", self.boundary.loader_functions.bind(py))?;
        fields.set_item(
            "python_execution_inputs",
            &self.boundary.python_execution_inputs,
        )?;
        fields.set_item("hook_functions", self.boundary.hook_functions.bind(py))?;
        fields.set_item("sql_hook_files", self.boundary.sql_hook_files.bind(py))?;
        fields.set_item(
            "materialization_files",
            self.boundary.materialization_files.bind(py),
        )?;
        fields.set_item("public_enums", self.boundary.public_enums.bind(py))?;
        fields.set_item("public_constants", self.boundary.public_constants.bind(py))?;
        fields.set_item("loaded_macros", self.boundary.loaded_macros.bind(py))?;
        fields.set_item(
            "external_sql_reference_resolver",
            self.boundary.external_sql_reference_resolver.bind(py),
        )?;
        fields.set_item("native_session", self.boundary.native_session.bind(py))?;
        fields.set_item("settings", self.compiler.settings.bind(py))?;
        fields.set_item("scenario", self.compiler.scenario.bind(py))?;
        fields.set_item("sources", self.compiler.sources.bind(py))?;
        fields.set_item("seeds", self.compiler.seeds.bind(py))?;
        fields.set_item("functions", self.compiler.functions.bind(py))?;
        fields.set_item("audits", self.compiler.audits.bind(py))?;
        fields.set_item("sql_tests", self.compiler.sql_tests.bind(py))?;
        fields.set_item("sql_scenarios", self.compiler.sql_scenarios.bind(py))?;
        fields.set_item("diagnostics", self.compiler.diagnostics.bind(py))?;
        fields.set_item(
            "unmatched_literal_sql_relations",
            self.compiler.unmatched_literal_sql_relations.bind(py),
        )?;
        fields.set_item("scope_index", self.compiler.scope_index.bind(py))?;
        Ok(fields)
    }
}

fn required<'py>(fields: &Bound<'py, PyDict>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    fields
        .get_item(name)?
        .ok_or_else(|| PyTypeError::new_err(format!("missing project field {name}")))
}

/// A source, seed or function's graph row, read from its compiled object.
pub(crate) fn resource_row(
    resource: &Bound<'_, PyAny>,
    tags: Option<&Bound<'_, PyAny>>,
) -> PyResult<ResourceRow> {
    let mut deps: Vec<(String, String)> = Vec::new();
    for key in native_field(resource, DEPS_ATTRIBUTE)?.try_iter()? {
        deps.push(key_row(&key?)?);
    }
    let mut tag_names: Vec<String> = Vec::new();
    if let Some(tags) = tags {
        for tag in tags.try_iter()? {
            tag_names.push(tag?.str()?.extract()?);
        }
    }
    Ok(ResourceRow {
        key: key_row(&native_field(resource, KEY_ATTRIBUTE)?)?,
        deps,
        tags: tag_names,
        folder: None,
    })
}

/// `(resource type, name)` of a `CompiledObjectKey`.
pub(crate) fn key_row(key: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    Ok((
        native_field(key, RESOURCE_TYPE_ATTRIBUTE)?
            .str()?
            .extract()?,
        native_field(key, NAME_ATTRIBUTE)?.extract()?,
    ))
}
