//! The compile's natively owned project, which producers record into and consumers read.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use pyo3::exceptions::{PyIndexError, PyRuntimeError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{FromPyObject, pyclass, pymethods};
use serde_json::Value;
use sqlbuild_analysis::compiled_project::main::project_models::project_models;
use sqlbuild_analysis::compiled_project::main::record_model_analysis::record_model_analysis;
use sqlbuild_analysis::compiled_project::main::record_skipped_analysis::record_skipped_analysis;
use sqlbuild_analysis::compiled_project::models::{
    CompiledModelFacts, CompiledProjectFacts, DeclarationFact, DynamicProofFact, ModelAnalysisFact,
    ModelConfigFacts, ModelReferenceFact, ModelSchemaFact,
};

use sqlbuild_project::models::CompiledProject;

use crate::bindings::_helpers::boundary::panics::NativeCompilerError;
use crate::bindings::_helpers::compiled_project::attachment_inputs::AttachedSqlTests;
use crate::bindings::_helpers::compiled_project::attachment_inputs::AuditFileRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::AuditSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::DiagnosticSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::FunctionRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::FunctionSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::ResourceDiagnostics;
use crate::bindings::_helpers::compiled_project::attachment_inputs::ScenarioRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::SourceRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::SourceSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::SqlTestSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::TestFileRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::resource_attachment;
use crate::bindings::_helpers::compiled_project::values::{object_value, plain_value};
use crate::bindings::_helpers::compiled_views::audit_views::{
    AttachedAudit, AuditRecord, NativeAuditInput, NativeCompiledAudit, audit_record,
};
use crate::bindings::_helpers::compiled_views::compiled_model_view::{
    CompiledViewClasses, NativeCompiledModel,
};
use crate::bindings::_helpers::compiled_views::function_views::{
    AttachedFunction, FunctionRecord, NativeCompiledFunction, NativeFunctionInput, function_record,
};
use crate::bindings::_helpers::compiled_views::inputs_view::NativeInputsView;
use crate::bindings::_helpers::compiled_views::model_input_view::{
    ModelInputRecord, NativeModelInput,
};
use crate::bindings::_helpers::compiled_views::project_host::PyProjectHost;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;
use crate::bindings::_helpers::compiled_views::seed_views::{
    NativeCompiledSeed, NativeSeedInput, SeedNamespace, SeedRecord, SeedViews, paired_seeds,
};
use crate::bindings::_helpers::compiled_views::source_views::{
    NativeCompiledSource, NativeSourceInput, SourceRecord,
};
use crate::bindings::_helpers::render::declaration_values::ConstantRendering;
use sqlbuild_project::types::InferredRow;
use sqlbuild_project::types::ObjectKeyRow;
use sqlbuild_project::types::SourceNamespace;

pub(crate) const CONTRACT_KEY: &str = "contract";
const INPUT_CLASSES_UNSET: &str = "no model loop recorded inputs into this project";
const ATTACHMENT_LOOP_UNSET: &str = "no model loop was kept to attach this project's resources";
pub(crate) const INCREMENTAL_MODE_KEY: &str = "incremental_mode";
const MATERIALIZED_KEY: &str = "materialized";
const TABLE_MATERIALIZATION: &str = "table";

/// `(name, type)`.
pub(crate) type NamedType = (String, Option<String>);
positional_record! {
    extract;
    /// `(output proven, bare dynamic pivot, (family key, inferred type), failure, (family, type))`.
    pub(crate) struct ProofPy {
        pub(crate) output_proven: bool,
        pub(crate) bare_dynamic_pivot: bool,
        pub(crate) families: Vec<NamedType>,
        pub(crate) failure_reason: Option<String>,
        pub(crate) named_families: Vec<NamedType>,
    }
}

positional_record! {
    extract;
    /// `(inferred, (name, type), nullable per column, has star, unchecked output columns)`.
    pub(crate) struct AnalysisPy {
        pub(crate) inferred: bool,
        pub(crate) inferred_columns: Vec<NamedType>,
        pub(crate) nullable_columns: Vec<bool>,
        pub(crate) fast_lineage_has_star: bool,
        pub(crate) unchecked: Vec<String>,
    }
}

positional_record! {
    extract;
    /// A model's name, relative path, compiled and authored queries, and authored SQL file.
    pub(crate) struct ModelTextsPy {
        name: String,
        relative_path: String,
        query_sql: String,
        authored_query_sql: String,
        authored_sql: String,
    }
}

/// Encoded config values and the encoder's error, if any.
pub(crate) type ConfigValues = (BTreeMap<String, Value>, Option<String>);

/// One model as the compile-input stage produced it.
#[derive(FromPyObject)]
pub(crate) struct ModelInputPy<'py>(
    ModelTextsPy,
    (Bound<'py, PyDict>, Vec<String>, Option<String>),
    Vec<(String, String, Option<String>)>,
    Vec<String>,
    (Vec<DeclarationPy<'py>>, Vec<DeclarationPy<'py>>),
);

/// Name, relative path, members, value, value type and rendering of one declaration.
#[derive(FromPyObject)]
pub(crate) struct DeclarationPy<'py>(
    String,
    String,
    Vec<(String, Bound<'py, PyAny>)>,
    Bound<'py, PyAny>,
    Option<String>,
    Option<String>,
);

/// One compile's project, recorded by the stages that produce it, with its Python projections.
#[pyclass(module = "sqlbuild._native")]
#[derive(Default)]
pub(crate) struct NativeCompiledProject {
    project: CompiledProject<PyProjectHost>,
    /// The Python classes model input views project their attributes with.
    input_classes: Option<Py<PyAny>>,
    /// Shared immutable column projections, allocated only when Python consumers request them.
    inferred_column_projections: Mutex<HashMap<InferredRow, Py<PyAny>>>,
    /// The model loop and its constant rendering, which attach the project's other resources.
    attachment: Option<(Py<PyAny>, Py<ConstantRendering>)>,
}

impl NativeCompiledProject {
    /// The facts recorded so far, for native consumers such as the rules request.
    pub(crate) fn facts(&self) -> &CompiledProjectFacts {
        &self.project.facts
    }

    /// A project holding only `facts`, which analyses can be recorded into apart from the compile's.
    pub(crate) fn with_facts(facts: CompiledProjectFacts) -> Self {
        Self {
            project: CompiledProject::with_facts(facts),
            ..Self::default()
        }
    }

    /// Retain facts a producer finished natively.
    pub(crate) fn retain(&mut self, facts: CompiledModelFacts) {
        self.project.retain(facts);
    }

    /// Retain a finished model's input record; return its index for the model's view.
    pub(crate) fn retain_input(&mut self, record: ModelInputRecord) -> usize {
        self.project.retain_input(record)
    }

    /// Keep the classes model input views project their attributes with.
    pub(crate) fn set_input_classes(&mut self, classes: Py<PyAny>) {
        self.input_classes = Some(classes);
    }

    /// Count one lineage graph a project view built.
    pub(crate) fn record_graph_build(&mut self) {
        self.project.graph_builds += 1;
    }

    /// The kept model loop and its constant rendering.
    pub(crate) fn attachment_loop(
        &self,
        py: Python<'_>,
    ) -> PyResult<(Py<PyAny>, Py<ConstantRendering>)> {
        match &self.attachment {
            Some((model_loop, host)) => Ok((model_loop.clone_ref(py), host.clone_ref(py))),
            None => Err(PyRuntimeError::new_err(ATTACHMENT_LOOP_UNSET)),
        }
    }

    /// Keep the model loop and its constant rendering for attaching the project's other resources.
    pub(crate) fn keep_attachment_loop(
        &mut self,
        model_loop: Py<PyAny>,
        host: Py<ConstantRendering>,
    ) {
        self.attachment = Some((model_loop, host));
    }

    /// Retain attached functions; return the index of the first.
    pub(crate) fn extend_functions(&mut self, attached: Vec<AttachedFunction>) -> usize {
        let first: usize = self.project.functions.len();
        self.project
            .functions
            .extend(attached.into_iter().map(function_record));
        first
    }

    /// Retain attached audits; return the index of the first.
    pub(crate) fn extend_audits(&mut self, attached: Vec<AttachedAudit>) -> usize {
        let first: usize = self.project.audits.len();
        self.project
            .audits
            .extend(attached.into_iter().map(audit_record));
        first
    }

    /// Every attached audit, in attachment order.
    pub(crate) fn audits(&self) -> &[AuditRecord] {
        self.project.audits()
    }

    /// Every attached function, in attachment order.
    pub(crate) fn functions(&self) -> &[FunctionRecord] {
        self.project.functions()
    }

    /// Every attached source, in attachment order.
    pub(crate) fn sources(&self) -> &[SourceRecord] {
        self.project.sources()
    }

    /// Keep one attached source.
    pub(crate) fn retain_source(&mut self, source: SourceRecord) {
        self.project.sources.push(source);
    }

    /// Every attached seed, in attachment order.
    pub(crate) fn seeds(&self) -> &[SeedRecord] {
        self.project.seeds()
    }

    /// The schema fact recorded for an input record's model.
    pub(crate) fn input_schema_fact(&self, record: &ModelInputRecord) -> Option<&ModelSchemaFact> {
        self.project.input_schema_fact(record)
    }

    /// Every model's input record, in loop order.
    pub(crate) fn inputs(&self) -> &[ModelInputRecord] {
        &self.project.inputs
    }

    /// The input record at `index`.
    pub(crate) fn input(&self, index: usize) -> PyResult<&ModelInputRecord> {
        self.project
            .input(index)
            .ok_or_else(|| PyIndexError::new_err(index))
    }

    /// The input record at `index`, for recording what later stages finish.
    pub(crate) fn input_mut(&mut self, index: usize) -> PyResult<&mut ModelInputRecord> {
        self.project
            .inputs
            .get_mut(index)
            .ok_or_else(|| PyIndexError::new_err(index))
    }

    /// Record that the native parser skipped the model at `index`: it is not analysed.
    pub(crate) fn mark_analysis_skipped(&mut self, index: usize) -> PyResult<()> {
        let record: &mut ModelInputRecord = self.input_mut(index)?;
        record.sql_validation_enabled = false;
        let name: String = record.name.clone();
        record_skipped_analysis(&mut self.project.facts, &name);
        Ok(())
    }

    /// The classes model input views project their attributes with.
    pub(crate) fn input_classes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.input_classes
            .as_ref()
            .map(|classes| classes.bind(py).clone())
            .ok_or_else(|| PyIndexError::new_err(INPUT_CLASSES_UNSET))
    }

    pub(crate) fn inferred_column<'py>(
        &self,
        py: Python<'py>,
        classes: &CompiledViewClasses<'py>,
        row: &InferredRow,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (name, column_type, nullability) = row;
        if let Some(column) = self
            .inferred_column_projections
            .lock()
            .map_err(|error| NativeCompilerError::new_err(error.to_string()))?
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
            .map_err(|error| NativeCompilerError::new_err(error.to_string()))?
            .insert(row.clone(), column.clone().unbind());
        Ok(column)
    }
}

#[pymethods]
impl NativeCompiledProject {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    /// Attach a recorded model's typed analysis; `False` when that model was never recorded.
    pub(crate) fn record_model_analysis(
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
    pub(crate) fn record_view_analyses(
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
    pub(crate) fn inputs_view(
        slf: Bound<'_, Self>,
        fields: Bound<'_, PyDict>,
    ) -> PyResult<NativeInputsView> {
        NativeInputsView::new(slf.unbind(), &fields)
    }

    /// The compiled project view of this owner's models with these project fields.
    pub(crate) fn project_view(
        slf: Bound<'_, Self>,
        fields: Bound<'_, PyDict>,
    ) -> PyResult<NativeProjectView> {
        NativeProjectView::new(slf.unbind(), &fields)
    }

    /// Pair seed declarations with their CSV files and keep them; return each seed's view.
    pub(crate) fn attach_seeds(
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
    fn record_seed_namespaces(&mut self, namespaces: Vec<SeedNamespace>) -> PyResult<()> {
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
    pub(crate) fn attach_sources(
        slf: &Bound<'_, Self>,
        rows: Vec<SourceRow>,
        settings: SourceSettings,
    ) -> PyResult<Vec<NativeSourceInput>> {
        (resource_attachment()?.sources)(slf, rows, settings)
    }

    /// Attach every SQL, then Python, function with the kept loop; return each one's view.
    pub(crate) fn attach_functions(
        slf: &Bound<'_, Self>,
        rows: (Vec<FunctionRow>, Vec<FunctionRow>),
        settings: FunctionSettings,
    ) -> PyResult<Vec<NativeFunctionInput>> {
        (resource_attachment()?.functions)(slf, rows, settings)
    }

    /// The orphaned audit factory warnings, missing descriptions and cursor-input errors.
    fn resource_diagnostics<'py>(
        &self,
        py: Python<'py>,
        settings: DiagnosticSettings,
    ) -> PyResult<ResourceDiagnostics<'py>> {
        (resource_attachment()?.diagnostics)(self, py, &settings)
    }

    /// Attach every SQL test case, then every scenario, with the kept loop.
    pub(crate) fn attach_sql_tests(
        slf: &Bound<'_, Self>,
        files: Vec<TestFileRow>,
        scenarios: Vec<ScenarioRow>,
        settings: SqlTestSettings,
    ) -> PyResult<AttachedSqlTests> {
        (resource_attachment()?.sql_tests)(slf, files, scenarios, settings)
    }

    /// Attach singular, then resource-attached, audits; return views and override warnings.
    pub(crate) fn attach_audits<'py>(
        slf: &Bound<'py, Self>,
        files: Vec<AuditFileRow>,
        settings: AuditSettings,
    ) -> PyResult<(Vec<NativeAuditInput>, Vec<Bound<'py, PyAny>>)> {
        (resource_attachment()?.audits)(slf, files, settings)
    }
    /// Keep the scope dependency keys native project assembly resolved, in audit order.
    fn record_audit_scope_deps(&mut self, deps: Vec<Vec<ObjectKeyRow>>) -> PyResult<()> {
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
    fn compiled_audit(slf: &Bound<'_, Self>, index: usize) -> PyResult<NativeCompiledAudit> {
        if index >= slf.borrow().project.audits.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledAudit::new(slf.clone().unbind(), index))
    }

    /// Keep the dependency keys native project assembly resolved, in function order.
    fn record_function_deps(&mut self, deps: Vec<Vec<ObjectKeyRow>>) -> PyResult<()> {
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
    fn compiled_function(slf: &Bound<'_, Self>, index: usize) -> PyResult<NativeCompiledFunction> {
        if index >= slf.borrow().project.functions.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledFunction::new(slf.clone().unbind(), index))
    }

    /// Keep the namespaces native project assembly resolved, in source order.
    fn record_source_namespaces(&mut self, namespaces: Vec<SourceNamespace>) -> PyResult<()> {
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
    fn compiled_source(slf: &Bound<'_, Self>, index: usize) -> PyResult<NativeCompiledSource> {
        if index >= slf.borrow().project.sources.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledSource::new(slf.clone().unbind(), index))
    }

    /// The assembled seed at `index`.
    fn compiled_seed(slf: &Bound<'_, Self>, index: usize) -> PyResult<NativeCompiledSeed> {
        if index >= slf.borrow().project.seeds.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledSeed::new(slf.clone().unbind(), index))
    }

    /// How many lineage graphs this owner's project views built.
    fn graph_builds(&self) -> usize {
        self.project.graph_builds
    }

    /// Whether both projects retained the same facts.
    fn __eq__(&self, other: &Self) -> bool {
        self.project.facts == other.project.facts
    }

    /// The recorded models' names, in production order.
    fn model_names(&self) -> Vec<String> {
        project_models(&self.project.facts)
            .iter()
            .map(|model| model.name.clone())
            .collect()
    }
}

pub(crate) fn model_facts(model: ModelInputPy<'_>) -> PyResult<CompiledModelFacts> {
    let ModelInputPy(
        ModelTextsPy {
            name,
            relative_path,
            query_sql,
            authored_query_sql,
            authored_sql,
        },
        (values, header_keys, layer_schema),
        references,
        enum_columns,
        (enum_declarations, constant_declarations),
    ) = model;
    let config_values: ConfigValues = config_values(&values);
    Ok(CompiledModelFacts {
        name,
        relative_path,
        query_sql: query_sql.into(),
        authored_query_sql: authored_query_sql.into(),
        authored_sql: authored_sql.into(),
        config: ModelConfigFacts {
            values: config_values.0,
            encoding_error: config_values.1,
            header_keys,
            layer_schema,
            contract: text_value(&values, CONTRACT_KEY)?,
            materialized: materialization(&values)?,
            incremental_mode: text_value(&values, INCREMENTAL_MODE_KEY)?,
        },
        references: references
            .into_iter()
            .map(|(kind, name, package)| ModelReferenceFact {
                kind,
                name,
                package,
            })
            .collect(),
        schema: None,
        enum_columns,
        enum_declarations: declarations(enum_declarations)?,
        constant_declarations: declarations(constant_declarations)?,
        analysis: None,
        typed_calls: Vec::new(),
        expansion_passes: Vec::new(),
        sql_analysis_skipped: false,
    })
}

/// The config values, or the encoder's error, kept for the consumer that reads them.
pub(crate) fn config_values(values: &Bound<'_, PyDict>) -> ConfigValues {
    match object_value(values) {
        Ok(Value::Object(entries)) => (entries.into_iter().collect(), None),
        Ok(_) => (BTreeMap::new(), None),
        Err(error) => (BTreeMap::new(), Some(error.to_string())),
    }
}

/// The `materialized` config value when it is text, `table` when it is absent.
pub(crate) fn materialization(values: &Bound<'_, PyDict>) -> PyResult<Option<String>> {
    if values.get_item(MATERIALIZED_KEY)?.is_none() {
        return Ok(Some(TABLE_MATERIALIZATION.to_owned()));
    }
    text_value(values, MATERIALIZED_KEY)
}

/// A config value when it is text.
pub(crate) fn text_value(values: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match values.get_item(key)? {
        Some(value) if value.is_instance_of::<PyString>() => Ok(Some(value.extract()?)),
        _ => Ok(None),
    }
}

pub(crate) fn declarations(rows: Vec<DeclarationPy<'_>>) -> PyResult<Vec<DeclarationFact>> {
    rows.into_iter().map(declaration).collect()
}

fn declaration(row: DeclarationPy<'_>) -> PyResult<DeclarationFact> {
    let DeclarationPy(name, relative_path, members, value, value_type, render_as) = row;
    Ok(DeclarationFact {
        name,
        relative_path,
        members: members
            .into_iter()
            .map(|(name, value)| Ok((name, plain_value(&value)?)))
            .collect::<PyResult<_>>()?,
        value: Some(plain_value(&value)?).filter(|value| !value.is_null()),
        value_type,
        render_as,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelInput>()?;
    module.add_class::<NativeCompiledModel>()?;
    module.add_class::<NativeProjectView>()?;
    module.add_class::<NativeInputsView>()?;
    module.add_class::<NativeSeedInput>()?;
    module.add_class::<NativeCompiledSeed>()?;
    module.add_class::<NativeSourceInput>()?;
    module.add_class::<NativeFunctionInput>()?;
    module.add_class::<NativeAuditInput>()?;
    module.add_class::<NativeCompiledAudit>()?;
    module.add_class::<NativeCompiledFunction>()?;
    module.add_class::<NativeCompiledSource>()?;
    module.add_class::<NativeCompiledProject>()
}
