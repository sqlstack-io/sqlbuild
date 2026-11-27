//! A compile's attached inputs, read from the native compiled project that owns its models.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};

const MODEL_INPUTS_FIELD: &str = "model_inputs";
const RUN_ID_FIELD: &str = "run_id";
const NO_SQL_VALIDATION_FIELD: &str = "no_sql_validation";

/// Compile inputs held as Python objects: user code, discovery or an integration owns them.
pub(crate) struct BoundaryInputs {
    /// The discovered project config.
    pub(crate) project_config: Py<PyAny>,
    /// The discovered local config.
    pub(crate) local_config: Py<PyAny>,
    /// The discovered project inputs.
    pub(crate) discovered_inputs: Py<PyAny>,
    /// The adapter's lexical syntax.
    pub(crate) sql_lexical_syntax: Py<PyAny>,
    /// The selected target config, or `None`.
    pub(crate) effective_target: Py<PyAny>,
    /// The compile cache directory path.
    pub(crate) compile_cache_dir: Py<PyAny>,
    /// The resolved connection, adapter integration values.
    pub(crate) effective_connection: Py<PyAny>,
    /// Project variables, user values.
    pub(crate) effective_vars: Py<PyAny>,
    /// The macro context user macro bodies run with.
    pub(crate) macro_context: Py<PyAny>,
    /// Loaded macro modules, user code.
    pub(crate) loaded_macros: Py<PyAny>,
    /// Public enums, user declaration values.
    pub(crate) public_enums: Py<PyAny>,
    /// Public constants, user declaration values.
    pub(crate) public_constants: Py<PyAny>,
    /// The external reference resolver integration, or `None`.
    pub(crate) external_sql_reference_resolver: Py<PyAny>,
}

/// Compiler-derived inputs still held as Python objects until they are owner facts.
pub(crate) struct CompilerHandles {
    /// Effective settings.
    pub(crate) effective_settings: Py<PyAny>,
    /// Attached seed inputs.
    pub(crate) seed_inputs: Py<PyAny>,
    /// Attached source inputs.
    pub(crate) source_inputs: Py<PyAny>,
    /// Attached SQL function inputs.
    pub(crate) sql_function_inputs: Py<PyAny>,
    /// Attached SQL test inputs.
    pub(crate) test_inputs: Py<PyAny>,
    /// Attached SQL scenario inputs.
    pub(crate) scenario_inputs: Py<PyAny>,
    /// Attached audit inputs.
    pub(crate) audit_inputs: Py<PyAny>,
    /// Compiler diagnostics.
    pub(crate) diagnostics: Py<PyAny>,
    /// The declaration scope index.
    pub(crate) scope_index: Py<PyAny>,
    /// The declaration scope build.
    pub(crate) declaration_scope: Py<PyAny>,
}

/// The values of one compile inputs view.
pub(crate) struct InputsFields {
    pub(crate) run_id: String,
    pub(crate) effective_target_name: Option<String>,
    pub(crate) no_sql_validation: bool,
    pub(crate) model_inputs: Py<PyTuple>,
    pub(crate) boundary: BoundaryInputs,
    pub(crate) compiler: CompilerHandles,
}

impl std::fmt::Debug for InputsFields {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InputsFields")
            .finish_non_exhaustive()
    }
}

impl InputsFields {
    /// The fields named in `fields`, each required.
    pub(crate) fn from_fields(fields: &Bound<'_, PyDict>) -> PyResult<Self> {
        Ok(Self {
            run_id: required(fields, RUN_ID_FIELD)?.extract()?,
            effective_target_name: required(fields, "effective_target_name")?.extract()?,
            no_sql_validation: required(fields, NO_SQL_VALIDATION_FIELD)?.extract()?,
            model_inputs: required(fields, MODEL_INPUTS_FIELD)?
                .cast_into::<PyTuple>()?
                .unbind(),
            boundary: BoundaryInputs {
                project_config: required(fields, "project_config")?.unbind(),
                local_config: required(fields, "local_config")?.unbind(),
                discovered_inputs: required(fields, "discovered_inputs")?.unbind(),
                sql_lexical_syntax: required(fields, "sql_lexical_syntax")?.unbind(),
                effective_target: required(fields, "effective_target")?.unbind(),
                compile_cache_dir: required(fields, "compile_cache_dir")?.unbind(),
                effective_connection: required(fields, "effective_connection")?.unbind(),
                effective_vars: required(fields, "effective_vars")?.unbind(),
                macro_context: required(fields, "macro_context")?.unbind(),
                loaded_macros: required(fields, "loaded_macros")?.unbind(),
                public_enums: required(fields, "public_enums")?.unbind(),
                public_constants: required(fields, "public_constants")?.unbind(),
                external_sql_reference_resolver: required(
                    fields,
                    "external_sql_reference_resolver",
                )?
                .unbind(),
            },
            compiler: CompilerHandles {
                effective_settings: required(fields, "effective_settings")?.unbind(),
                seed_inputs: required(fields, "seed_inputs")?.unbind(),
                source_inputs: required(fields, "source_inputs")?.unbind(),
                sql_function_inputs: required(fields, "sql_function_inputs")?.unbind(),
                test_inputs: required(fields, "test_inputs")?.unbind(),
                scenario_inputs: required(fields, "scenario_inputs")?.unbind(),
                audit_inputs: required(fields, "audit_inputs")?.unbind(),
                diagnostics: required(fields, "diagnostics")?.unbind(),
                scope_index: required(fields, "scope_index")?.unbind(),
                declaration_scope: required(fields, "declaration_scope")?.unbind(),
            },
        })
    }

    /// Every field by name.
    pub(crate) fn as_fields<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item(RUN_ID_FIELD, &self.run_id)?;
        fields.set_item("effective_target_name", &self.effective_target_name)?;
        fields.set_item(NO_SQL_VALIDATION_FIELD, self.no_sql_validation)?;
        fields.set_item(MODEL_INPUTS_FIELD, self.model_inputs.bind(py))?;
        fields.set_item("project_config", self.boundary.project_config.bind(py))?;
        fields.set_item("local_config", self.boundary.local_config.bind(py))?;
        fields.set_item(
            "discovered_inputs",
            self.boundary.discovered_inputs.bind(py),
        )?;
        fields.set_item(
            "sql_lexical_syntax",
            self.boundary.sql_lexical_syntax.bind(py),
        )?;
        fields.set_item("effective_target", self.boundary.effective_target.bind(py))?;
        fields.set_item(
            "compile_cache_dir",
            self.boundary.compile_cache_dir.bind(py),
        )?;
        fields.set_item(
            "effective_connection",
            self.boundary.effective_connection.bind(py),
        )?;
        fields.set_item("effective_vars", self.boundary.effective_vars.bind(py))?;
        fields.set_item("macro_context", self.boundary.macro_context.bind(py))?;
        fields.set_item("loaded_macros", self.boundary.loaded_macros.bind(py))?;
        fields.set_item("public_enums", self.boundary.public_enums.bind(py))?;
        fields.set_item("public_constants", self.boundary.public_constants.bind(py))?;
        fields.set_item(
            "external_sql_reference_resolver",
            self.boundary.external_sql_reference_resolver.bind(py),
        )?;
        fields.set_item(
            "effective_settings",
            self.compiler.effective_settings.bind(py),
        )?;
        fields.set_item("seed_inputs", self.compiler.seed_inputs.bind(py))?;
        fields.set_item("source_inputs", self.compiler.source_inputs.bind(py))?;
        fields.set_item(
            "sql_function_inputs",
            self.compiler.sql_function_inputs.bind(py),
        )?;
        fields.set_item("test_inputs", self.compiler.test_inputs.bind(py))?;
        fields.set_item("scenario_inputs", self.compiler.scenario_inputs.bind(py))?;
        fields.set_item("audit_inputs", self.compiler.audit_inputs.bind(py))?;
        fields.set_item("diagnostics", self.compiler.diagnostics.bind(py))?;
        fields.set_item("scope_index", self.compiler.scope_index.bind(py))?;
        fields.set_item(
            "declaration_scope",
            self.compiler.declaration_scope.bind(py),
        )?;
        Ok(fields)
    }
}

fn required<'py>(fields: &Bound<'py, PyDict>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    fields
        .get_item(name)?
        .ok_or_else(|| PyTypeError::new_err(format!("missing compile input field {name}")))
}
