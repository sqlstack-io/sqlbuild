//! Public records of compiled project.

use crate::compiled_project::_helpers::attachment_inputs::AttachAudits;
use crate::compiled_project::_helpers::attachment_inputs::AttachFunctions;
use crate::compiled_project::_helpers::attachment_inputs::AttachSources;
use crate::compiled_project::_helpers::attachment_inputs::AttachSqlTests;
use crate::compiled_project::_helpers::attachment_inputs::ResourceDiagnosticsRun;
use crate::compiled_project::_helpers::attachment_inputs::resource_attachment;
use crate::compiled_project::_helpers::project::ATTACHMENT_LOOP_UNSET;
use crate::compiled_project::_helpers::project::INPUT_CLASSES_UNSET;
use crate::compiled_project::_helpers::project::ModelTextsPy;
use crate::compiled_project::_helpers::project::NamedType;
use crate::compiled_project::types::AttachedSqlTests;
use crate::compiled_project::types::FunctionSettingsTuple;
use crate::compiled_project::types::LoaderRow;
use crate::compiled_project::types::ResourceDiagnostics;
use crate::compiled_project::types::SourceRow;
use crate::compiled_project::types::SourceSettings;
use crate::compiled_project::types::TestBlockTuple;
use crate::compiled_project::types::TestCaseRow;
use crate::compiled_views::models::CompiledViewClasses;
use crate::compiled_views::models::NativeCompiledModel;
use crate::compiled_views::models::NativeInputsView;
use crate::compiled_views::models::NativeProjectView;
use crate::compiled_views::models::PyProjectHost;
use crate::compiled_views::types::ModelInputRecord;
use crate::resource_views::main::audit_views::audit_record::audit_record;
use crate::resource_views::main::function_views::function_record::function_record;
use crate::resource_views::main::seed_views::paired_seeds::paired_seeds;
use crate::resource_views::models::NativeAuditInput;
use crate::resource_views::models::NativeCompiledAudit;
use crate::resource_views::models::NativeCompiledFunction;
use crate::resource_views::models::NativeCompiledSeed;
use crate::resource_views::models::NativeCompiledSource;
use crate::resource_views::models::NativeFunctionInput;
use crate::resource_views::models::NativeSeedInput;
use crate::resource_views::models::NativeSourceInput;
use crate::resource_views::models::SeedNamespace;
use crate::resource_views::types::AttachedAudit;
use crate::resource_views::types::AttachedFunction;
use crate::resource_views::types::AuditRecord;
use crate::resource_views::types::FunctionRecord;
use crate::resource_views::types::SeedRecord;
use crate::resource_views::types::SeedViews;
use crate::resource_views::types::SourceRecord;
use pyo3::FromPyObject;
use pyo3::exceptions::PyIndexError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyString;
use sqlbuild_analysis::compiled_project::main::project_models::project_models;
use sqlbuild_analysis::compiled_project::main::record_model_analysis::record_model_analysis;
use sqlbuild_analysis::compiled_project::main::record_skipped_analysis::record_skipped_analysis;
use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;
use sqlbuild_analysis::compiled_project::models::CompiledProjectFacts;
use sqlbuild_analysis::compiled_project::models::DynamicProofFact;
use sqlbuild_analysis::compiled_project::models::ModelAnalysisFact;
use sqlbuild_analysis::compiled_project::models::ModelSchemaFact;
use sqlbuild_project::models::CompiledProject;
use sqlbuild_project::types::InferredRow;
use sqlbuild_project::types::ObjectKeyRow;
use sqlbuild_project::types::SourceNamespace;
use sqlbuild_python_base::boundary::main::native_compiler_error::native_compiler_error;
use sqlbuild_python_base::cache::models::SqlTestScanStore;
use sqlbuild_python_front::render::models::ConstantRendering;
use std::collections::HashMap;
use std::sync::Mutex;

/// A function's paths, stem, discovered file, header values, body and SQL file contents.
#[derive(Debug)]
pub struct FunctionRow {
    pub file_path: String,
    pub relative_path: String,
    pub name: String,
    pub function_file: Py<PyAny>,
    pub header_values: Py<PyDict>,
    pub body: String,
    pub contents: Option<String>,
}

impl<'py> pyo3::FromPyObject<'py> for FunctionRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (file_path, relative_path, name, function_file, header_values, body, contents): (
            String,
            String,
            String,
            Py<PyAny>,
            Py<PyDict>,
            String,
            Option<String>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            file_path,
            relative_path,
            name,
            function_file,
            header_values,
            body,
            contents,
        })
    }
}

/// Adapter, validation, raw default and target namespaces, inheritance and resource kind.
#[derive(Debug)]
pub struct FunctionSettings {
    pub adapter_name: String,
    pub validate: bool,
    pub defaults: (Option<String>, Option<String>),
    pub target: Option<(Option<String>, Option<String>)>,
    pub inherit: bool,
    pub kind: Py<PyAny>,
}

impl<'py> pyo3::FromPyObject<'py> for FunctionSettings {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (adapter_name, validate, defaults, target, inherit, kind): FunctionSettingsTuple =
            pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            adapter_name,
            validate,
            defaults,
            target,
            inherit,
            kind,
        })
    }
}

/// A test block, its name, SQL, mode, declared parameter names, cases and authored SQL span.
#[derive(Debug)]
pub struct TestBlockRow {
    pub block: Py<PyAny>,
    pub name: Option<String>,
    pub sql_body: String,
    pub mode: String,
    pub parameters: Vec<String>,
    pub cases: Vec<TestCaseRow>,
    pub sql_body_span: Option<(usize, usize)>,
}

impl<'py> pyo3::FromPyObject<'py> for TestBlockRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (block, name, sql_body, mode, parameters, cases, sql_body_span): TestBlockTuple =
            pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            block,
            name,
            sql_body,
            mode,
            parameters,
            cases,
            sql_body_span,
        })
    }
}

/// A test file's paths, stem, discovered file, contents and blocks.
#[derive(Debug)]
pub struct TestFileRow {
    pub file_path: String,
    pub relative_path: String,
    pub stem: String,
    pub file: Py<PyAny>,
    pub contents: String,
    pub blocks: Vec<TestBlockRow>,
}

impl<'py> pyo3::FromPyObject<'py> for TestFileRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (file_path, relative_path, stem, file, contents, blocks): (
            String,
            String,
            String,
            Py<PyAny>,
            String,
            Vec<TestBlockRow>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            file_path,
            relative_path,
            stem,
            file,
            contents,
            blocks,
        })
    }
}

/// What every SQL test and scenario of one compile resolves and records with.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct SqlTestSettings {
    /// `CompileSqlTestInput`, its model and direct payloads, and `CompileSqlTestCte`.
    pub test_input: Py<PyAny>,
    pub model_payload: Py<PyAny>,
    pub direct_payload: Py<PyAny>,
    pub test_cte: Py<PyAny>,
    /// `CompileSqlScenarioInput` and `CompileSqlScenarioCte`.
    pub scenario_input: Py<PyAny>,
    pub scenario_cte: Py<PyAny>,
    /// `SqlTestMode` by value.
    pub modes: Py<PyDict>,
    /// The `ResourceKind`s of tests and scenarios, and the runtime `UsageKind`.
    pub test_kind: Py<PyAny>,
    pub scenario_kind: Py<PyAny>,
    pub runtime_usage: Py<PyAny>,
    /// The adapter's typed scalar renderer, returning `(text, None)` or `(None, error)`.
    pub render_parameter: Py<PyAny>,
    pub adapter_name: String,
    /// `SourceLocation`, for helper problems located natively.
    pub source_location: Py<PyAny>,
    /// The shared scan store with the scan algorithm and lexical rules keying it, if any.
    pub scan_store: Option<Py<SqlTestScanStore>>,
    pub scan_key: (String, String),
}

/// A scenario file's paths, discovered file, contents, SQL body and name.
#[derive(Debug)]
pub struct ScenarioRow {
    pub file_path: String,
    pub relative_path: String,
    pub scenario_file: Py<PyAny>,
    pub contents: String,
    pub sql_body: String,
    pub name: String,
}

impl<'py> pyo3::FromPyObject<'py> for ScenarioRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (file_path, relative_path, scenario_file, contents, sql_body, name): (
            String,
            String,
            Py<PyAny>,
            String,
            String,
            String,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            file_path,
            relative_path,
            scenario_file,
            contents,
            sql_body,
            name,
        })
    }
}

/// An audit block, its name, SQL body, evidence SQL, measurement flag and header values.
#[derive(Debug)]
pub struct AuditBlockRow {
    pub block: Py<PyAny>,
    pub name: Option<String>,
    pub sql_body: String,
    pub evidence_sql: Option<String>,
    pub measurement: bool,
    pub header: Py<PyDict>,
}

impl<'py> pyo3::FromPyObject<'py> for AuditBlockRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (block, name, sql_body, evidence_sql, measurement, header): (
            Py<PyAny>,
            Option<String>,
            String,
            Option<String>,
            bool,
            Py<PyDict>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            block,
            name,
            sql_body,
            evidence_sql,
            measurement,
            header,
        })
    }
}

impl<'py> pyo3::IntoPyObject<'py> for AuditBlockRow {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.block,
                self.name,
                self.sql_body,
                self.evidence_sql,
                self.measurement,
                self.header,
            ),
            py,
        )
    }
}

/// An audit file's paths, stem, discovered file, generic flag, contents and blocks.
#[derive(Debug)]
pub struct AuditFileRow {
    pub file_path: String,
    pub relative_path: String,
    pub stem: String,
    pub file: Py<PyAny>,
    pub generic: bool,
    pub contents: String,
    pub blocks: Vec<AuditBlockRow>,
}

impl<'py> pyo3::FromPyObject<'py> for AuditFileRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (file_path, relative_path, stem, file, generic, contents, blocks): (
            String,
            String,
            String,
            Py<PyAny>,
            bool,
            String,
            Vec<AuditBlockRow>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            file_path,
            relative_path,
            stem,
            file,
            generic,
            contents,
            blocks,
        })
    }
}

impl<'py> pyo3::IntoPyObject<'py> for AuditFileRow {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.file_path,
                self.relative_path,
                self.stem,
                self.file,
                self.generic,
                self.contents,
                self.blocks,
            ),
            py,
        )
    }
}

/// What every audit in one compile resolves against, and the Python values it builds with.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct AuditSettings {
    pub default_severity: Option<String>,
    pub default_run_scope: Option<String>,
    /// Built-in generic definitions as audit file rows, overridden by project definitions.
    pub builtins: Vec<AuditFileRow>,
    /// `DeclarationIdentity`, and the singular and generic audit `DeclarationKind`s.
    pub declaration_identity: Py<PyAny>,
    pub singular_kind: Py<PyAny>,
    pub audit_kind: Py<PyAny>,
    /// `ResourceKind` by value, for the resources singular audits read and attach to.
    pub resource_kinds: Py<PyDict>,
}

/// A Python resource's kind, name, description (a `str` that may hold surrogates), location, help.
#[derive(Debug)]
pub struct PythonResourceRow {
    pub kind: String,
    pub name: String,
    pub description: Option<Py<PyString>>,
    pub location: Py<PyAny>,
    pub help: String,
}

impl<'py> pyo3::FromPyObject<'py> for PythonResourceRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (kind, name, description, location, help): (
            String,
            String,
            Option<Py<PyString>>,
            Py<PyAny>,
            String,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            kind,
            name,
            description,
            location,
            help,
        })
    }
}

/// A scenario's name, relative path, contents and authored header description.
#[derive(Debug)]
pub struct ScenarioDescriptionRow {
    pub name: String,
    pub path: String,
    pub contents: String,
    pub raw: Py<PyAny>,
}

impl<'py> pyo3::FromPyObject<'py> for ScenarioDescriptionRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (name, path, contents, raw): (String, String, String, Py<PyAny>) =
            pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            name,
            path,
            contents,
            raw,
        })
    }
}

/// A SQL hook file's name, relative path, contents and description.
#[derive(Debug)]
pub struct HookDescriptionRow {
    pub name: String,
    pub path: String,
    pub contents: String,
    pub description: Option<String>,
}

impl<'py> pyo3::FromPyObject<'py> for HookDescriptionRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (name, path, contents, description): (String, String, String, Option<String>) =
            pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            name,
            path,
            contents,
            description,
        })
    }
}

/// What the resource diagnostics read beyond the owner, and the Python values they build.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct DiagnosticSettings {
    /// `CompilerDiagnostic`, `SourceLocation`, `Path` and `CompileInputError`.
    pub compiler_diagnostic: Py<PyAny>,
    pub compile_error: Py<PyAny>,
    pub source_location: Py<PyAny>,
    pub path_class: Py<PyAny>,
    /// Every discovered model file, whose headers attach audit factories.
    pub model_files: Vec<Py<PyAny>>,
    /// Each discovered audit factory's name, relative path and line.
    pub audit_factories: Vec<(String, String, usize)>,
    /// Each scenario's name, relative path, contents and authored header description.
    pub scenarios: Vec<ScenarioDescriptionRow>,
    /// Each SQL hook file's name, relative path, contents and description.
    pub sql_hooks: Vec<HookDescriptionRow>,
    /// Each loader by name.
    pub loaders: HashMap<String, LoaderRow>,
    /// Python-defined resources: kind, name, description, location and help, read by Python.
    pub python_resources: Vec<PythonResourceRow>,
    /// The project's `max_description_lines` and `line_width`, which Python descriptions keep to.
    pub description_limit: (usize, i64),
}

/// The compile's runs behind the project's attachment methods.
#[derive(Clone, Copy, Debug)]
pub struct ResourceAttachment {
    pub sources: AttachSources,
    pub functions: AttachFunctions,
    pub diagnostics: ResourceDiagnosticsRun,
    pub sql_tests: AttachSqlTests,
    pub audits: AttachAudits,
}

/// `(output proven, bare dynamic pivot, (family key, inferred type), failure, (family, type))`.
#[derive(Debug)]
pub struct ProofPy {
    pub output_proven: bool,
    pub bare_dynamic_pivot: bool,
    pub families: Vec<NamedType>,
    pub failure_reason: Option<String>,
    pub named_families: Vec<NamedType>,
}

impl<'py> pyo3::FromPyObject<'py> for ProofPy {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (output_proven, bare_dynamic_pivot, families, failure_reason, named_families): (
            bool,
            bool,
            Vec<NamedType>,
            Option<String>,
            Vec<NamedType>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            output_proven,
            bare_dynamic_pivot,
            families,
            failure_reason,
            named_families,
        })
    }
}

/// `(inferred, (name, type), nullable per column, has star, unchecked output columns)`.
#[derive(Debug)]
pub struct AnalysisPy {
    pub inferred: bool,
    pub inferred_columns: Vec<NamedType>,
    pub nullable_columns: Vec<bool>,
    pub fast_lineage_has_star: bool,
    pub unchecked: Vec<String>,
}

impl<'py> pyo3::FromPyObject<'py> for AnalysisPy {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (inferred, inferred_columns, nullable_columns, fast_lineage_has_star, unchecked): (
            bool,
            Vec<NamedType>,
            Vec<bool>,
            bool,
            Vec<String>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            inferred,
            inferred_columns,
            nullable_columns,
            fast_lineage_has_star,
            unchecked,
        })
    }
}

/// One model as the compile-input stage produced it.
#[derive(Debug, FromPyObject)]
pub struct ModelInputPy<'py>(
    pub(crate) ModelTextsPy,
    pub(crate) (Bound<'py, PyDict>, Vec<String>, Option<String>),
    pub(crate) Vec<(String, String, Option<String>)>,
    pub(crate) Vec<String>,
    pub(crate) (Vec<DeclarationPy<'py>>, Vec<DeclarationPy<'py>>),
);

/// Name, relative path, members, value, value type and rendering of one declaration.
#[derive(Debug, FromPyObject)]
pub struct DeclarationPy<'py>(
    pub(crate) String,
    pub(crate) String,
    pub(crate) Vec<(String, Bound<'py, PyAny>)>,
    pub(crate) Bound<'py, PyAny>,
    pub(crate) Option<String>,
    pub(crate) Option<String>,
);

/// One compile's project, recorded by the stages that produce it, with its Python projections.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug, Default)]
pub struct NativeCompiledProject {
    pub(crate) project: CompiledProject<PyProjectHost>,
    /// The Python classes model input views project their attributes with.
    pub(crate) input_classes: Option<Py<PyAny>>,
    /// Shared immutable column projections, allocated only when Python consumers request them.
    pub(crate) inferred_column_projections: Mutex<HashMap<InferredRow, Py<PyAny>>>,
    /// The model loop and its constant rendering, which attach the project's other resources.
    pub(crate) attachment: Option<(Py<PyAny>, Py<ConstantRendering>)>,
}

impl NativeCompiledProject {
    /// The facts recorded so far, for native consumers such as the rules request.
    pub fn facts(&self) -> &CompiledProjectFacts {
        &self.project.facts
    }

    /// A project holding only `facts`, which analyses can be recorded into apart from the compile's.
    pub fn with_facts(facts: CompiledProjectFacts) -> Self {
        Self {
            project: CompiledProject::with_facts(facts),
            ..Self::default()
        }
    }

    /// Retain facts a producer finished natively.
    pub fn retain(&mut self, facts: CompiledModelFacts) {
        self.project.retain(facts);
    }

    /// Retain a finished model's input record; return its index for the model's view.
    pub fn retain_input(&mut self, record: ModelInputRecord) -> usize {
        self.project.retain_input(record)
    }

    /// Keep the classes model input views project their attributes with.
    pub fn set_input_classes(&mut self, classes: Py<PyAny>) {
        self.input_classes = Some(classes);
    }

    /// Count one lineage graph a project view built.
    pub fn record_graph_build(&mut self) {
        self.project.graph_builds += 1;
    }

    /// The kept model loop and its constant rendering.
    pub fn attachment_loop(&self, py: Python<'_>) -> PyResult<(Py<PyAny>, Py<ConstantRendering>)> {
        match &self.attachment {
            Some((model_loop, host)) => Ok((model_loop.clone_ref(py), host.clone_ref(py))),
            None => Err(PyRuntimeError::new_err(ATTACHMENT_LOOP_UNSET)),
        }
    }

    /// Keep the model loop and its constant rendering for attaching the project's other resources.
    pub fn keep_attachment_loop(&mut self, model_loop: Py<PyAny>, host: Py<ConstantRendering>) {
        self.attachment = Some((model_loop, host));
    }

    /// Retain attached functions; return the index of the first.
    pub fn extend_functions(&mut self, attached: Vec<AttachedFunction>) -> usize {
        let first: usize = self.project.functions.len();
        self.project
            .functions
            .extend(attached.into_iter().map(function_record));
        first
    }

    /// Retain attached audits; return the index of the first.
    pub fn extend_audits(&mut self, attached: Vec<AttachedAudit>) -> usize {
        let first: usize = self.project.audits.len();
        self.project
            .audits
            .extend(attached.into_iter().map(audit_record));
        first
    }

    /// Every attached audit, in attachment order.
    pub fn audits(&self) -> &[AuditRecord] {
        self.project.audits()
    }

    /// Every attached function, in attachment order.
    pub fn functions(&self) -> &[FunctionRecord] {
        self.project.functions()
    }

    /// Every attached source, in attachment order.
    pub fn sources(&self) -> &[SourceRecord] {
        self.project.sources()
    }

    /// Keep one attached source.
    pub fn retain_source(&mut self, source: SourceRecord) {
        self.project.sources.push(source);
    }

    /// Every attached seed, in attachment order.
    pub fn seeds(&self) -> &[SeedRecord] {
        self.project.seeds()
    }

    /// The schema fact recorded for an input record's model.
    pub fn input_schema_fact(&self, record: &ModelInputRecord) -> Option<&ModelSchemaFact> {
        self.project.input_schema_fact(record)
    }

    /// Every model's input record, in loop order.
    pub fn inputs(&self) -> &[ModelInputRecord] {
        &self.project.inputs
    }

    /// The input record at `index`.
    pub fn input(&self, index: usize) -> PyResult<&ModelInputRecord> {
        self.project
            .input(index)
            .ok_or_else(|| PyIndexError::new_err(index))
    }

    /// The input record at `index`, for recording what later stages finish.
    pub fn input_mut(&mut self, index: usize) -> PyResult<&mut ModelInputRecord> {
        self.project
            .inputs
            .get_mut(index)
            .ok_or_else(|| PyIndexError::new_err(index))
    }

    /// Record that the native parser skipped the model at `index`: it is not analysed.
    pub fn mark_analysis_skipped(&mut self, index: usize) -> PyResult<()> {
        let record: &mut ModelInputRecord = self.input_mut(index)?;
        record.sql_validation_enabled = false;
        let name: String = record.name.clone();
        record_skipped_analysis(&mut self.project.facts, &name);
        Ok(())
    }

    /// The classes model input views project their attributes with.
    pub fn input_classes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.input_classes
            .as_ref()
            .map(|classes| classes.bind(py).clone())
            .ok_or_else(|| PyIndexError::new_err(INPUT_CLASSES_UNSET))
    }

    pub fn inferred_column<'py>(
        &self,
        py: Python<'py>,
        classes: &CompiledViewClasses<'py>,
        row: &InferredRow,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (name, column_type, nullability) = row;
        if let Some(column) = self
            .inferred_column_projections
            .lock()
            .map_err(|error| native_compiler_error(error.to_string()))?
            .get(row)
        {
            return Ok(column.bind(py).clone());
        }
        let fields = PyDict::new(py);
        fields.set_item("name", name)?;
        fields.set_item("type", column_type.as_deref())?;
        fields.set_item("nullability", classes.nullability.call1((nullability,))?)?;
        let column = classes.inferred_column.call((), Some(&fields))?;
        self.inferred_column_projections
            .lock()
            .map_err(|error| native_compiler_error(error.to_string()))?
            .insert(row.clone(), column.clone().unbind());
        Ok(column)
    }
}

#[pymethods]
impl NativeCompiledProject {
    #[new]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Attach a recorded model's typed analysis; `False` when that model was never recorded.
    pub fn record_model_analysis(
        &mut self,
        name: &str,
        columns: AnalysisPy,
        dynamic_proof: Option<ProofPy>,
    ) -> bool {
        let AnalysisPy {
            inferred,
            inferred_columns,
            nullable_columns,
            fast_lineage_has_star,
            unchecked,
        } = columns;
        let analysis: ModelAnalysisFact = ModelAnalysisFact {
            inferred_columns,
            dynamic_proof: dynamic_proof.map(
                |ProofPy {
                     output_proven,
                     bare_dynamic_pivot,
                     families,
                     failure_reason,
                     named_families,
                 }| DynamicProofFact {
                    output_proven,
                    bare_dynamic_pivot,
                    families,
                    failure_reason,
                    named_families,
                },
            ),
            inferred,
            nullable_columns,
            fast_lineage_has_star,
            unchecked_output_columns: unchecked,
        };
        record_model_analysis(&mut self.project.facts, name, analysis)
    }

    /// Record each assembled native model view's final typed analysis; return how many.
    pub fn record_view_analyses(
        slf: &Bound<'_, Self>,
        views: Vec<PyRef<'_, NativeCompiledModel>>,
    ) -> PyResult<usize> {
        let py: Python<'_> = slf.py();
        let mut facts: Vec<(String, ModelAnalysisFact)> = Vec::with_capacity(views.len());
        for view in &views {
            facts.push(view.analysis_fact(py)?);
        }
        let mut owner = slf.borrow_mut();
        let mut recorded: usize = 0;
        for (name, analysis) in facts {
            recorded += usize::from(record_model_analysis(
                &mut owner.project.facts,
                &name,
                analysis,
            ));
        }
        Ok(recorded)
    }

    /// The compile inputs view of this owner's model inputs with these input fields.
    pub fn inputs_view(
        slf: Bound<'_, Self>,
        fields: Bound<'_, PyDict>,
    ) -> PyResult<NativeInputsView> {
        NativeInputsView::new(slf.unbind(), &fields)
    }

    /// The compiled project view of this owner's models with these project fields.
    pub fn project_view(
        slf: Bound<'_, Self>,
        fields: Bound<'_, PyDict>,
    ) -> PyResult<NativeProjectView> {
        NativeProjectView::new(slf.unbind(), &fields)
    }

    /// Pair seed declarations with their CSV files and keep them; return each seed's view.
    pub fn attach_seeds(
        slf: &Bound<'_, Self>,
        declarations: Vec<(Py<PyAny>, Py<PyAny>)>,
        seed_files: Vec<Py<PyAny>>,
        compile_error: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<NativeSeedInput>> {
        let seeds: Vec<SeedRecord> = paired_seeds(declarations, &seed_files, compile_error)?;
        let count: usize = seeds.len();
        slf.borrow_mut().project.seeds = seeds;
        Ok((0..count)
            .map(|index| NativeSeedInput::new(slf.clone().unbind(), index))
            .collect())
    }

    /// Keep the destinations native project assembly resolved, in seed order.
    pub(crate) fn record_seed_namespaces(
        &mut self,
        namespaces: Vec<SeedNamespace>,
    ) -> PyResult<()> {
        for (index, namespace) in namespaces.into_iter().enumerate() {
            self.project
                .seeds
                .get_mut(index)
                .ok_or_else(|| PyIndexError::new_err(index))?
                .set_namespace(namespace);
        }
        Ok(())
    }

    /// Attach every discovered source in order with the kept loop; return each source's view.
    pub fn attach_sources(
        slf: &Bound<'_, Self>,
        rows: Vec<SourceRow>,
        settings: SourceSettings,
    ) -> PyResult<Vec<NativeSourceInput>> {
        (resource_attachment()?.sources)(slf, rows, settings)
    }

    /// Attach every SQL, then Python, function with the kept loop; return each one's view.
    pub fn attach_functions(
        slf: &Bound<'_, Self>,
        rows: (Vec<FunctionRow>, Vec<FunctionRow>),
        settings: FunctionSettings,
    ) -> PyResult<Vec<NativeFunctionInput>> {
        (resource_attachment()?.functions)(slf, rows, settings)
    }

    /// The orphaned audit factory warnings, missing descriptions and cursor-input errors.
    pub(crate) fn resource_diagnostics<'py>(
        &self,
        py: Python<'py>,
        settings: DiagnosticSettings,
    ) -> PyResult<ResourceDiagnostics<'py>> {
        (resource_attachment()?.diagnostics)(self, py, &settings)
    }

    /// Attach every SQL test case, then every scenario, with the kept loop.
    pub fn attach_sql_tests(
        slf: &Bound<'_, Self>,
        files: Vec<TestFileRow>,
        scenarios: Vec<ScenarioRow>,
        settings: SqlTestSettings,
    ) -> PyResult<AttachedSqlTests> {
        (resource_attachment()?.sql_tests)(slf, files, scenarios, settings)
    }

    /// Attach singular, then resource-attached, audits; return views and override warnings.
    pub fn attach_audits<'py>(
        slf: &Bound<'py, Self>,
        files: Vec<AuditFileRow>,
        settings: AuditSettings,
    ) -> PyResult<(Vec<NativeAuditInput>, Vec<Bound<'py, PyAny>>)> {
        (resource_attachment()?.audits)(slf, files, settings)
    }
    /// Keep the scope dependency keys native project assembly resolved, in audit order.
    pub(crate) fn record_audit_scope_deps(&mut self, deps: Vec<Vec<ObjectKeyRow>>) -> PyResult<()> {
        for (index, keys) in deps.into_iter().enumerate() {
            self.project
                .audits
                .get_mut(index)
                .ok_or_else(|| PyIndexError::new_err(index))?
                .set_scope_deps(keys);
        }
        Ok(())
    }

    /// The assembled audit at `index`.
    pub(crate) fn compiled_audit(
        slf: &Bound<'_, Self>,
        index: usize,
    ) -> PyResult<NativeCompiledAudit> {
        if index >= slf.borrow().project.audits.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledAudit::new(slf.clone().unbind(), index))
    }

    /// Keep the dependency keys native project assembly resolved, in function order.
    pub(crate) fn record_function_deps(&mut self, deps: Vec<Vec<ObjectKeyRow>>) -> PyResult<()> {
        for (index, keys) in deps.into_iter().enumerate() {
            self.project
                .functions
                .get_mut(index)
                .ok_or_else(|| PyIndexError::new_err(index))?
                .set_deps(keys);
        }
        Ok(())
    }

    /// The assembled function at `index`.
    pub(crate) fn compiled_function(
        slf: &Bound<'_, Self>,
        index: usize,
    ) -> PyResult<NativeCompiledFunction> {
        if index >= slf.borrow().project.functions.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledFunction::new(slf.clone().unbind(), index))
    }

    /// Keep the namespaces native project assembly resolved, in source order.
    pub(crate) fn record_source_namespaces(
        &mut self,
        namespaces: Vec<SourceNamespace>,
    ) -> PyResult<()> {
        for (index, namespace) in namespaces.into_iter().enumerate() {
            self.project
                .sources
                .get_mut(index)
                .ok_or_else(|| PyIndexError::new_err(index))?
                .set_namespace(namespace);
        }
        Ok(())
    }

    /// The assembled source at `index`.
    pub(crate) fn compiled_source(
        slf: &Bound<'_, Self>,
        index: usize,
    ) -> PyResult<NativeCompiledSource> {
        if index >= slf.borrow().project.sources.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledSource::new(slf.clone().unbind(), index))
    }

    /// The assembled seed at `index`.
    pub(crate) fn compiled_seed(
        slf: &Bound<'_, Self>,
        index: usize,
    ) -> PyResult<NativeCompiledSeed> {
        if index >= slf.borrow().project.seeds.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledSeed::new(slf.clone().unbind(), index))
    }

    /// How many lineage graphs this owner's project views built.
    pub(crate) fn graph_builds(&self) -> usize {
        self.project.graph_builds
    }

    /// Whether both projects retained the same facts.
    pub(crate) fn __eq__(&self, other: &Self) -> bool {
        self.project.facts == other.project.facts
    }

    /// The recorded models' names, in production order.
    pub(crate) fn model_names(&self) -> Vec<String> {
        project_models(&self.project.facts)
            .iter()
            .map(|model| model.name.clone())
            .collect()
    }
}

/// A reference as the tuple Python reads: kind, name, package and call argument count.
#[derive(Debug)]
pub struct ReferenceTuple {
    pub kind: String,
    pub name: String,
    pub package: Option<String>,
    pub call_argument_count: Option<usize>,
}

impl<'py> pyo3::IntoPyObject<'py> for ReferenceTuple {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (self.kind, self.name, self.package, self.call_argument_count),
            py,
        )
    }
}
