//! A compiled project, read from the native compiled project that owns its models.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use pyo3::{pyclass, pymethods};

use crate::bindings::_helpers::compile_driver::stage_work::{LINEAGE_GRAPH_BUILDS, count_stage};
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::compiled_model_view::{
    NativeCompiledModel, model_graph_row,
};
use crate::bindings::_helpers::compiled_views::function_views::NativeCompiledFunction;
use crate::bindings::_helpers::compiled_views::model_input_view::{
    InputViewClasses, authored_expansion, view_classes,
};
use crate::bindings::_helpers::compiled_views::seed_views::NativeCompiledSeed;
use crate::bindings::_helpers::compiled_views::source_views::NativeCompiledSource;
use crate::bindings::_helpers::graph::project_graph::{
    NativeProjectGraph, ResourceRow, project_graph,
};
use crate::bindings::_helpers::records::fields::field as native_field;

const MODELS_FIELD: &str = "models";
const RUN_ID_FIELD: &str = "run_id";
const ENFORCE_EXPLICIT_FIELD: &str = "enforce_explicit_references";
const KEY_ATTRIBUTE: &str = "key";
const DEPS_ATTRIBUTE: &str = "deps";
const TAGS_ATTRIBUTE: &str = "tags";
const SCHEMA_ENTRY_ATTRIBUTE: &str = "schema_entry";
const RESOURCE_TYPE_ATTRIBUTE: &str = "resource_type";
const NAME_ATTRIBUTE: &str = "name";

/// Project inputs held as Python objects: user code, discovery or an integration owns them.
struct BoundaryInputs {
    /// The resolved connection, adapter integration values.
    effective_connection: Py<PyAny>,
    /// Project variables, user values.
    effective_vars: Py<PyAny>,
    /// The native binding catalog.
    binding_catalog: Py<PyAny>,
    /// The adapter's lexical syntax.
    sql_lexical_syntax: Py<PyAny>,
    /// The compile cache directory path.
    compile_cache_dir: Py<PyAny>,
    /// Discovered loader functions, user callables.
    loader_functions: Py<PyAny>,
    /// Task/asset/check source locations, without retaining additional Python callables.
    python_execution_inputs: BTreeMap<String, (String, PathBuf)>,
    /// Discovered Python hooks, user callables.
    hook_functions: Py<PyAny>,
    /// Discovered SQL hook files.
    sql_hook_files: Py<PyAny>,
    /// Discovered custom materializations.
    materialization_files: Py<PyAny>,
    /// Public enums, user declaration values.
    public_enums: Py<PyAny>,
    /// Public constants, user declaration values.
    public_constants: Py<PyAny>,
    /// Loaded macro modules, user code.
    loaded_macros: Py<PyAny>,
    /// The external reference resolver integration, or `None`.
    external_sql_reference_resolver: Py<PyAny>,
    /// The native analysis session, or `None`.
    native_session: Py<PyAny>,
}

/// Compiler-derived project parts still held as Python objects until they are owner facts.
struct CompilerHandles {
    /// Effective settings.
    settings: Py<PyAny>,
    /// Effective scenario config.
    scenario: Py<PyAny>,
    /// Compiled sources.
    sources: Py<PyAny>,
    /// Compiled seeds.
    seeds: Py<PyAny>,
    /// Compiled SQL functions.
    functions: Py<PyAny>,
    /// Compiled audits.
    audits: Py<PyAny>,
    /// Compiled SQL tests.
    sql_tests: Py<PyAny>,
    /// Compiled SQL scenarios.
    sql_scenarios: Py<PyAny>,
    /// Compiler diagnostics.
    diagnostics: Py<PyAny>,
    /// Literal relations no resource matched.
    unmatched_literal_sql_relations: Py<PyAny>,
    /// The declaration scope index.
    scope_index: Py<PyAny>,
}

/// The project-level values of one compiled project view.
struct ProjectFields {
    run_id: String,
    effective_target_name: Option<String>,
    effective_target_database: Option<String>,
    effective_target_schema: Option<String>,
    sql_analysis_dialect: Option<String>,
    enforce_explicit_references: bool,
    models: Py<PyTuple>,
    boundary: BoundaryInputs,
    compiler: CompilerHandles,
}

impl ProjectFields {
    /// The fields named in `fields`, each required.
    fn from_fields(fields: &Bound<'_, PyDict>) -> PyResult<Self> {
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
    fn changed(&self, py: Python<'_>, changes: &Bound<'_, PyDict>) -> PyResult<Self> {
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
    fn as_fields<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
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

/// One compiled project, its models projected from the owner.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeProjectView {
    owner: Py<NativeCompiledProject>,
    fields: ProjectFields,
    lineage_graph: PyOnceLock<Py<NativeProjectGraph>>,
    sql_expansions: PyOnceLock<Py<PyDict>>,
}

impl NativeProjectView {
    /// The view over `owner` with these project fields.
    pub(crate) fn new(
        owner: Py<NativeCompiledProject>,
        fields: &Bound<'_, PyDict>,
    ) -> PyResult<Self> {
        Ok(Self::with_fields(
            owner,
            ProjectFields::from_fields(fields)?,
        ))
    }

    /// The project field custom-rule facts and rules runs read by `name`, or `None` for any other.
    pub(crate) fn fact_field(&self, py: Python<'_>, name: &str) -> Option<Py<PyAny>> {
        let field: &Py<PyAny> = match name {
            "settings" => &self.fields.compiler.settings,
            "sql_scenarios" => &self.fields.compiler.sql_scenarios,
            "scope_index" => &self.fields.compiler.scope_index,
            "diagnostics" => &self.fields.compiler.diagnostics,
            "sources" => &self.fields.compiler.sources,
            "seeds" => &self.fields.compiler.seeds,
            "functions" => &self.fields.compiler.functions,
            "audits" => &self.fields.compiler.audits,
            "sql_tests" => &self.fields.compiler.sql_tests,
            "public_enums" => &self.fields.boundary.public_enums,
            "public_constants" => &self.fields.boundary.public_constants,
            "models" => return Some(self.fields.models.clone_ref(py).into_any()),
            _ => return None,
        };
        Some(field.clone_ref(py))
    }

    /// The compiled project this view projects.
    pub(crate) fn owner(&self) -> &Py<NativeCompiledProject> {
        &self.owner
    }

    fn with_fields(owner: Py<NativeCompiledProject>, fields: ProjectFields) -> Self {
        Self {
            owner,
            fields,
            lineage_graph: PyOnceLock::new(),
            sql_expansions: PyOnceLock::new(),
        }
    }
}

#[pymethods]
impl NativeProjectView {
    /// This project with `changes` applied; the owner's models are shared.
    #[pyo3(signature = (**changes))]
    fn with_changes(&self, py: Python<'_>, changes: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let fields: ProjectFields = match changes {
            Some(changes) => self.fields.changed(py, &changes)?,
            None => self.fields.changed(py, &PyDict::new(py))?,
        };
        Ok(Self::with_fields(self.owner.clone_ref(py), fields))
    }

    /// Every field by name, for an explicit `CompiledProject` projection.
    #[pyo3(signature = (include_sql_expansions = true))]
    pub(crate) fn fields<'py>(
        &self,
        py: Python<'py>,
        include_sql_expansions: bool,
    ) -> PyResult<Bound<'py, PyDict>> {
        let fields: Bound<'py, PyDict> = self.fields.as_fields(py)?;
        fields.set_item(
            "sql_expansions",
            if include_sql_expansions {
                self.sql_expansions(py)?
            } else {
                PyDict::new(py).unbind()
            },
        )?;
        fields.set_item("native_project", self.owner.bind(py))?;
        Ok(fields)
    }

    #[getter]
    pub(crate) fn run_id(&self) -> String {
        self.fields.run_id.clone()
    }

    #[getter]
    pub(crate) fn effective_target_name(&self) -> Option<String> {
        self.fields.effective_target_name.clone()
    }

    #[getter]
    pub(crate) fn effective_target_database(&self) -> Option<String> {
        self.fields.effective_target_database.clone()
    }

    #[getter]
    pub(crate) fn effective_target_schema(&self) -> Option<String> {
        self.fields.effective_target_schema.clone()
    }

    #[getter]
    pub(crate) fn sql_analysis_dialect(&self) -> Option<String> {
        self.fields.sql_analysis_dialect.clone()
    }

    #[getter(enforce_explicit_references)]
    pub(crate) fn is_explicit_reference_enforced(&self) -> bool {
        self.fields.enforce_explicit_references
    }

    #[getter]
    pub(crate) fn models(&self, py: Python<'_>) -> Py<PyTuple> {
        self.fields.models.clone_ref(py)
    }

    #[getter]
    pub(crate) fn native_project(&self, py: Python<'_>) -> Py<NativeCompiledProject> {
        self.owner.clone_ref(py)
    }

    #[getter]
    pub(crate) fn effective_connection(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.effective_connection.clone_ref(py)
    }

    #[getter]
    pub(crate) fn effective_vars(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.effective_vars.clone_ref(py)
    }

    #[getter]
    pub(crate) fn binding_catalog(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.binding_catalog.clone_ref(py)
    }

    #[getter]
    pub(crate) fn sql_lexical_syntax(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.sql_lexical_syntax.clone_ref(py)
    }

    #[getter]
    pub(crate) fn compile_cache_dir(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.compile_cache_dir.clone_ref(py)
    }

    #[getter]
    pub(crate) fn loader_functions(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.loader_functions.clone_ref(py)
    }

    #[getter]
    pub(crate) fn python_execution_inputs(&self) -> BTreeMap<String, (String, PathBuf)> {
        self.fields.boundary.python_execution_inputs.clone()
    }

    #[getter]
    pub(crate) fn hook_functions(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.hook_functions.clone_ref(py)
    }

    #[getter]
    pub(crate) fn sql_hook_files(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.sql_hook_files.clone_ref(py)
    }

    #[getter]
    pub(crate) fn materialization_files(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.materialization_files.clone_ref(py)
    }

    #[getter]
    pub(crate) fn public_enums(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.public_enums.clone_ref(py)
    }

    #[getter]
    pub(crate) fn public_constants(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.public_constants.clone_ref(py)
    }

    #[getter]
    pub(crate) fn loaded_macros(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.loaded_macros.clone_ref(py)
    }

    #[getter]
    pub(crate) fn external_sql_reference_resolver(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields
            .boundary
            .external_sql_reference_resolver
            .clone_ref(py)
    }

    #[getter]
    pub(crate) fn native_session(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.boundary.native_session.clone_ref(py)
    }

    #[getter]
    pub(crate) fn settings(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.settings.clone_ref(py)
    }

    #[getter]
    pub(crate) fn scenario(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.scenario.clone_ref(py)
    }

    #[getter]
    pub(crate) fn sources(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.sources.clone_ref(py)
    }

    #[getter]
    pub(crate) fn seeds(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.seeds.clone_ref(py)
    }

    #[getter]
    pub(crate) fn functions(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.functions.clone_ref(py)
    }

    #[getter]
    pub(crate) fn audits(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.audits.clone_ref(py)
    }

    #[getter]
    pub(crate) fn sql_tests(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.sql_tests.clone_ref(py)
    }

    #[getter]
    pub(crate) fn sql_scenarios(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.sql_scenarios.clone_ref(py)
    }

    #[getter]
    pub(crate) fn diagnostics(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.diagnostics.clone_ref(py)
    }

    #[getter]
    pub(crate) fn unmatched_literal_sql_relations(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields
            .compiler
            .unmatched_literal_sql_relations
            .clone_ref(py)
    }

    #[getter]
    pub(crate) fn scope_index(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.compiler.scope_index.clone_ref(py)
    }

    /// Each model's authored-to-expanded passes by file path, where interpolation kept the query.
    #[getter]
    pub(crate) fn sql_expansions(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.sql_expansions
            .get_or_try_init(py, || {
                let classes: InputViewClasses<'_> = view_classes(&self.owner, py)?;
                let owner = self.owner.bind(py).borrow();
                let expansions: Bound<'_, PyDict> = PyDict::new(py);
                for record in owner.inputs() {
                    let expansion: Py<PyAny> = authored_expansion(&classes, record)?;
                    if !expansion.is_none(py) {
                        let path: Bound<'_, PyAny> =
                            record.host.model_file.get().path_objects(py)?.0;
                        expansions.set_item(path, expansion)?;
                    }
                }
                Ok::<_, pyo3::PyErr>(expansions.unbind())
            })
            .map(|expansions| expansions.clone_ref(py))
    }

    /// The lineage edges, tags, model folders and names, indexed natively once per project.
    #[getter]
    pub(crate) fn lineage_graph(&self, py: Python<'_>) -> PyResult<Py<NativeProjectGraph>> {
        self.lineage_graph
            .get_or_try_init(py, || {
                let mut rows: Vec<ResourceRow> = Vec::new();
                for model in self.fields.models.bind(py).iter() {
                    let model: Bound<'_, NativeCompiledModel> = model.downcast_into()?;
                    rows.push(model_graph_row(model.get(), py)?);
                }
                let compiler: &CompilerHandles = &self.fields.compiler;
                for source in compiler.sources.bind(py).try_iter()? {
                    let source: Bound<'_, PyAny> = source?;
                    rows.push(match source.cast::<NativeCompiledSource>() {
                        Ok(native) => native.get().graph_row(py)?,
                        Err(_) => resource_row(&source, None)?,
                    });
                }
                for seed in compiler.seeds.bind(py).try_iter()? {
                    let seed: Bound<'_, PyAny> = seed?;
                    rows.push(match seed.cast::<NativeCompiledSeed>() {
                        Ok(native) => native.get().graph_row(py)?,
                        Err(_) => {
                            let tags: Bound<'_, PyAny> = native_field(
                                &native_field(&seed, SCHEMA_ENTRY_ATTRIBUTE)?,
                                TAGS_ATTRIBUTE,
                            )?;
                            resource_row(&seed, Some(&tags))?
                        }
                    });
                }
                for function in compiler.functions.bind(py).try_iter()? {
                    let function: Bound<'_, PyAny> = function?;
                    rows.push(match function.cast::<NativeCompiledFunction>() {
                        Ok(native) => native.get().graph_row(py)?,
                        Err(_) => {
                            let tags: Bound<'_, PyAny> = native_field(&function, TAGS_ATTRIBUTE)?;
                            resource_row(&function, Some(&tags))?
                        }
                    });
                }
                self.owner.bind(py).borrow_mut().record_graph_build();
                count_stage(py, LINEAGE_GRAPH_BUILDS, 1)?;
                Py::new(py, project_graph(rows)?)
            })
            .map(|graph| graph.clone_ref(py))
    }
}

/// A source, seed or function's graph row, read from its compiled object.
fn resource_row(
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
