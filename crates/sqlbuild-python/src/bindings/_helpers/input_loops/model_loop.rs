//! The compile-input model loop: each model's steps run in the attachment's order, natively.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use pyo3::{FromPyObject, pyclass, pymethods};
use sqlbuild_analysis::compiled_project::main::analysis_gate::analysis_gate;
use sqlbuild_analysis::compiled_project::main::calls_macros::calls_macros;
use sqlbuild_analysis::compiled_project::main::located_scan_error::located_scan_error;
use sqlbuild_analysis::compiled_project::main::merge_call_site_references::merge_call_site_references;
use sqlbuild_analysis::compiled_project::models::{
    AnalysisGate, ModelLoopSettings, ScanFaultOrigin, SplicedSpan,
};
use sqlbuild_analysis::sql_macros::main::moved_typed_calls::moved_typed_calls;
use sqlbuild_analysis::sql_macros::main::spliced_typed_calls::spliced_typed_calls;
use sqlbuild_analysis::sql_macros::models::{TextSplice, TypedCall};
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics_with_spans::replace_intrinsics_with_spans;
use sqlbuild_attachments::cursor_intrinsics::main::validated_model_intrinsics::validated_model_intrinsics;
use sqlbuild_attachments::cursor_intrinsics::models::IntrinsicModel;
use sqlbuild_attachments::functions::models::KnownResources;
use sqlbuild_attachments::test_targets::models::TargetCatalog;
use sqlbuild_discovery::model_files::models::HeaderFacts;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::ReferenceExtraction;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::compiled_project::attachment_inputs::AttachedSqlTests;
use crate::bindings::_helpers::compiled_project::attachment_inputs::AuditFileRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::AuditSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::FunctionRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::FunctionSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::ScenarioRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::SourceRow;
use crate::bindings::_helpers::compiled_project::attachment_inputs::SourceSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::SqlTestSettings;
use crate::bindings::_helpers::compiled_project::attachment_inputs::TestFileRow;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::audit_views::AttachedAudit;
use crate::bindings::_helpers::compiled_views::function_views::AttachedFunction;
use crate::bindings::_helpers::compiled_views::model_input_view::NativeModelInput;
use crate::bindings::_helpers::compiled_views::source_views::{
    AttachedSource, NativeSourceInput, retain_sources,
};
use crate::bindings::_helpers::discovery::model_file_record::{ModelFileTexts, NativeModelFile};
use crate::bindings::_helpers::input_loops::audit_loop::{AttachmentOwner, attached_audits};
use crate::bindings::_helpers::input_loops::function_loop::{
    attached_functions, checked_table_function_arities,
};
use crate::bindings::_helpers::input_loops::model_loop_calls::{CallOrigin, report_invalid_calls};
use crate::bindings::_helpers::input_loops::model_loop_config::ModelConfigs;
use crate::bindings::_helpers::input_loops::model_loop_external::external_errors;
use crate::bindings::_helpers::input_loops::model_loop_header::{
    AttachedHeader, HeaderSource, header_schema,
};
use crate::bindings::_helpers::input_loops::model_loop_hooks::{
    ExpandedHooks, HookModel, SqlHookClasses, expanded_hooks,
};
use crate::bindings::_helpers::input_loops::model_loop_host::{
    DeclarationScan, FinishedModel, InterpolationRow, ModelLoopRow, RecordClasses, RecordInputs,
    finished_record,
};
use crate::bindings::_helpers::input_loops::model_loop_inputs::{
    analysis_override, hook_presence, reference_row,
};
use crate::bindings::_helpers::input_loops::model_loop_macros::{
    ExpandedMacros, MacroClasses, MacroModel, expanded_macros,
};
use crate::bindings::_helpers::input_loops::model_loop_named::{SCHEMA_DECLARATION, named_usages};
use crate::bindings::_helpers::input_loops::model_loop_opt_out::{
    OptOut, OptOutModel, reject_unparseable_query, rejected_opt_out,
};
use crate::bindings::_helpers::input_loops::model_loop_prepass::{
    NativeHeader, all_header_facts, native_header,
};
use crate::bindings::_helpers::input_loops::model_loop_python_hooks::{
    HookClasses, HookSignatures,
};
use crate::bindings::_helpers::input_loops::model_loop_schema::{
    ModelSchema, model_schema, schema_column_names,
};
use crate::bindings::_helpers::input_loops::model_loop_visibility::{
    ModelVisibility, VisibilityClasses, VisibilityModel,
};
use crate::bindings::_helpers::input_loops::source_loop::attached_sources;
use crate::bindings::_helpers::input_loops::sql_test_loop::TestScope;
use crate::bindings::_helpers::model_config::config_classes::ConfigClasses;
use crate::bindings::_helpers::model_config::config_errors::raised_config_exception;
use crate::bindings::_helpers::model_config::header_metadata::ContractClasses;
use crate::bindings::_helpers::model_config::model_validator::NativeModelValidator;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::render::declaration_expansion::{
    DeclarationExpansion, ExpandedDeclarations, ExpansionClasses, ExpansionConsumer,
    expand_declarations,
};
use crate::bindings::_helpers::render::declaration_values::ConstantRendering;
use crate::bindings::_helpers::render::macro_call_rows::SpanRow;
use crate::bindings::_helpers::render::macro_call_rows::span_row;
use crate::bindings::_helpers::render::sql_macro_host::{SqlCallOrigin, SqlMacroSource};
use crate::bindings::_helpers::sqltext::declaration_references::declaration_scan;
use crate::bindings::_helpers::sqltext::lexical_syntax::LexicalSyntaxInput;
use crate::bindings::_helpers::sqltext::static_sql::{
    InterpolationRow as SqlInterpolationRow, InterpolationSources, interpolation_batch,
};
use crate::bindings::types::CompilerDetach;
use sqlbuild_project::models::ReferenceRow;

const MATERIALIZED_KEY: &str = "materialized";
const CURSOR_KEY: &str = "cursor";
const INCREMENTAL_MATERIALIZATION: &str = "incremental";
const VALUES_ATTRIBUTE: &str = "values";
const RETENTION_ATTRIBUTE: &str = "time_travel_retention";
const UNMANAGED_ATTRIBUTE: &str = "unmanaged";
const TABLE_TYPE_ATTRIBUTE: &str = "table_type";
const DECLARED_ATTRIBUTE: &str = "declared";
const VALIDATE_METHOD: &str = "validate";
const HELP_ARGUMENT: &str = "help";
const CODE_ARGUMENT: &str = "code";
const ANALYSIS_CONFLICT: &str = "MODEL sql_analysis conflicts with legacy sql_validation";
const ANALYSIS_CONFLICT_CODE: &str = "P003";
const SQL_HOOKS_KEY: &str = "sql_hooks";
const HOOK_FILES_KEY: &str = "hook_files";
const DEFINITIONS_KEY: &str = "definitions";
const COMPILE_ERROR_KEY: &str = "compile_error";

/// The native validator, header contract classes, and the loop's Python classes.
type LoopChecks = (Py<PyAny>, Py<PyDict>, Py<PyDict>);
positional_record! {
    extract;
    /// Whether SQL analysis runs, is skipped, is deferred, and is required.
    pub(crate) struct LoopSwitches {
        pub(crate) sql_analysis: bool,
        pub(crate) no_sql_validation: bool,
        pub(crate) defer_sql_validation: bool,
        pub(crate) require_sql_analysis: bool,
    }
}

/// `(analysis switches, project config path)`.
type LoopSettingsInput = (LoopSwitches, String);

/// The Python classes and error factory the loop builds usages, hooks and errors with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct LoopClasses<'py> {
    pub(crate) usage_record: Bound<'py, PyAny>,
    pub(crate) compile_error: Bound<'py, PyAny>,
    pub(crate) sql_hook_entry: Bound<'py, PyAny>,
    /// Discovered audit factories by name.
    pub(crate) audit_factories: Bound<'py, PyDict>,
    /// `UsageKind.GENERATED`, for usages an enum-typed column generates.
    pub(crate) generated_usage: Bound<'py, PyAny>,
    /// `ContractPolicy.ENFORCED`.
    pub(crate) contract_enforced: Bound<'py, PyAny>,
    /// Public reusable model schemas by name.
    pub(crate) model_schemas: Bound<'py, PyDict>,
    /// Each indexed schema's and Python hook's declaration record, by `(kind, name)`.
    pub(crate) named_records: Bound<'py, PyDict>,
    /// Project enums by name, as a schema's column types resolve them.
    pub(crate) schema_enums: Bound<'py, PyDict>,
    /// `CompilerDiagnostic` and `report_compile_diagnostic`, for S006.
    pub(crate) compiler_diagnostic: Bound<'py, PyAny>,
    pub(crate) report_diagnostic: Bound<'py, PyAny>,
    /// `pathlib.Path`.
    pub(crate) path_class: Bound<'py, PyAny>,
    /// The dbt manifest's `ExternalSqlReferenceResolver`, or `None`.
    pub(crate) external_resolver: Bound<'py, PyAny>,
    /// `SqlReferenceKind`.
    pub(crate) reference_kind: Bound<'py, PyAny>,
    /// What a model's effective config is built from.
    pub(crate) config: ConfigClasses<'py>,
    /// What a model's visible declarations are resolved from.
    pub(crate) visibility: VisibilityClasses<'py>,
    /// What a model's Python hooks are checked against.
    pub(crate) hooks: HookClasses<'py>,
    /// What each model's SQL hooks are expanded with.
    pub(crate) sql_hooks: SqlHookClasses<'py>,
    /// What each model's macro calls are expanded with.
    pub(crate) macros: MacroClasses<'py>,
    /// `sql_analysis_opt_out_help(model_owned=True)`, the help of a model's syntax error.
    pub(crate) syntax_help: String,
    /// The same help for SQL a project's other resources own.
    pub(crate) source_syntax_help: String,
    /// `InvalidScopePathError`.
    pub(crate) scope_path_error: Bound<'py, PyAny>,
    /// What each finished model's input record is built with.
    pub(crate) record: RecordClasses<'py>,
    /// What model input views project their attributes with.
    pub(crate) input_view: Bound<'py, PyAny>,
}

/// The ordered model loop of one compile-input stage.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeModelLoop {
    pub(crate) validator: Py<PyAny>,
    pub(crate) syntax: LexicalSyntax,
    pub(crate) reserved_markers: Vec<String>,
    pub(crate) settings: ModelLoopSettings,
    pub(crate) project_config_path: String,
    contract_classes: Py<PyDict>,
    loop_classes: Py<PyDict>,
}

/// What one run reads beside each model: the host, Python classes and header contract classes.
pub(crate) struct LoopContext<'a, 'py> {
    pub(crate) host: &'a Bound<'py, ConstantRendering>,
    pub(crate) classes: &'a LoopClasses<'py>,
    pub(crate) contracts: &'a ContractClasses<'py>,
    /// The lexical rules the project's SQL is authored in.
    pub(crate) syntax: &'a LexicalSyntax,
}

/// The declarations a model sees and the identity its usages are recorded for.
pub(crate) struct VisibleDeclarations<'py> {
    pub(crate) declarations: Bound<'py, PyAny>,
    pub(crate) consumer: Bound<'py, PyAny>,
}

/// What the model loop knows about a model before finishing it.
struct LoopModel<'a, 'py> {
    row: &'a ModelLoopRow,
    config: Bound<'py, PyAny>,
    visible: &'a VisibleDeclarations<'py>,
    signatures: &'a HookSignatures,
    visibility: &'a ModelVisibility<'py>,
}

#[pymethods]
impl NativeModelLoop {
    #[new]
    pub(crate) fn new(
        checks: LoopChecks,
        syntax: LexicalSyntaxInput,
        reserved_markers: Vec<String>,
        settings: LoopSettingsInput,
    ) -> PyResult<Self> {
        let (validator, contract_classes, loop_classes) = checks;
        let (
            LoopSwitches {
                sql_analysis,
                no_sql_validation,
                defer_sql_validation,
                require_sql_analysis,
            },
            project_config_path,
        ) = settings;
        Python::attach(|py| index_sql_hook_definitions(loop_classes.bind(py)))?;
        Ok(Self {
            validator,
            syntax: syntax.into(),
            reserved_markers,
            settings: ModelLoopSettings {
                sql_analysis,
                no_sql_validation,
                defer_sql_validation,
                require_sql_analysis,
            },
            project_config_path,
            contract_classes,
            loop_classes,
        })
    }
}

/// Replace the SQL hook classes' discovered `hook_files` with their `definitions` by name.
fn index_sql_hook_definitions(loop_classes: &Bound<'_, PyDict>) -> PyResult<()> {
    let py: Python<'_> = loop_classes.py();
    let hooks: Bound<'_, PyDict> = loop_classes
        .as_any()
        .get_item(SQL_HOOKS_KEY)?
        .downcast_into()?;
    let definitions: Bound<'_, PyDict> = PyDict::new(py);
    for hook_file in hooks.as_any().get_item(HOOK_FILES_KEY)?.try_iter()? {
        let hook_file: Bound<'_, PyAny> = hook_file?;
        let name: Bound<'_, PyAny> = native_field(&hook_file, "name")?;
        if let Some(existing) = definitions.get_item(&name)? {
            let message: String = format!(
                "Duplicate SQL hook name '{}' found in {} and {}",
                name.str()?,
                native_field(&existing, "relative_path")?.str()?,
                native_field(&hook_file, "relative_path")?.str()?
            );
            let error: Bound<'_, PyAny> = loop_classes
                .as_any()
                .get_item(COMPILE_ERROR_KEY)?
                .call1((message,))?;
            return Err(PyErr::from_value(error));
        }
        definitions.set_item(name, hook_file)?;
    }
    hooks.set_item(DEFINITIONS_KEY, definitions)
}

/// Each model's `@@`-interpolated query and declaration scan, all interpolated before any runs.
fn prepared_rows<'py>(
    py: Python<'py>,
    (classes, syntax): (&LoopClasses<'py>, &LexicalSyntax),
    model_files: Vec<Py<NativeModelFile>>,
) -> PyResult<Vec<ModelLoopRow>> {
    let hooks = &classes.sql_hooks;
    let mut sqls: Vec<(String, String)> = Vec::with_capacity(model_files.len());
    let mut paths: Vec<ModelFileTexts> = Vec::with_capacity(model_files.len());
    for model_file in &model_files {
        let record: &NativeModelFile = model_file.get();
        let texts: ModelFileTexts = record.path_texts(py)?;
        sqls.push((record.query_text().to_string(), texts.file_path.clone()));
        paths.push(texts);
    }
    let interpolated: Vec<SqlInterpolationRow> = interpolation_batch(
        InterpolationSources(
            hooks.effective_vars.clone(),
            hooks.environment.clone(),
            None,
            hooks.render_project_var.clone(),
            Some(syntax.clone()),
        ),
        &sqls,
    )?;
    let mut rows: Vec<ModelLoopRow> = Vec::with_capacity(model_files.len());
    for (((model_file, paths), (authored, _)), interpolated) in model_files
        .into_iter()
        .zip(paths)
        .zip(sqls)
        .zip(interpolated)
    {
        let SqlInterpolationRow {
            sql: rendered,
            spans,
            reads,
            error,
        } = interpolated;
        let sql: String = rendered.unwrap_or(authored);
        let scan: Option<DeclarationScan> = error
            .is_none()
            .then(|| declaration_scan(&sql, Some(syntax)));
        let reads: Vec<(String, String)> = reads
            .into_iter()
            .map(|(kind, name)| (kind.to_owned(), name))
            .collect();
        let interpolation: InterpolationRow = InterpolationRow {
            sql,
            reads,
            error,
            spans,
        };
        rows.push(ModelLoopRow::new((model_file, paths), scan, interpolation));
    }
    Ok(rows)
}

impl NativeModelLoop {
    fn run_models<'py>(
        &self,
        py: Python<'py>,
        project: Bound<'py, NativeCompiledProject>,
        host: Bound<'py, ConstantRendering>,
        models: Vec<ModelLoopRow>,
    ) -> PyResult<Vec<Bound<'py, NativeModelInput>>> {
        let contracts: ContractClasses<'py> = self.contract_classes.bind(py).extract()?;
        let classes: LoopClasses<'py> = self.loop_classes.bind(py).extract()?;
        let context: LoopContext<'_, 'py> = LoopContext {
            host: &host,
            classes: &classes,
            contracts: &contracts,
            syntax: &self.syntax,
        };
        project
            .borrow_mut()
            .set_input_classes(classes.input_view.clone().unbind());
        let mut inputs: Vec<Bound<'py, NativeModelInput>> = Vec::with_capacity(models.len());
        let configs: ModelConfigs<'py> = ModelConfigs::default();
        let visibility: ModelVisibility<'py> = ModelVisibility::default();
        let signatures: HookSignatures = HookSignatures::default();
        for row in models {
            let ModelFileTexts {
                name,
                relative_path,
                file_path,
                ..
            } = &row.paths;
            let record: &NativeModelFile = row.model_file.get();
            let (enums, constants) = record.declaration_tuples(py);
            let visible: VisibleDeclarations<'py> = visibility.for_model(
                &context,
                &VisibilityModel {
                    name,
                    file_path,
                    local: (enums.as_any(), constants.as_any()),
                },
            )?;
            let config: Bound<'py, PyAny> =
                configs.effective(&context, record, (name, relative_path), row.header.as_ref())?;
            let model: LoopModel<'_, 'py> = LoopModel {
                row: &row,
                config,
                visible: &visible,
                signatures: &signatures,
                visibility: &visibility,
            };
            let finished: FinishedModel<'py> = self.finish(py, &context, model)?;
            let record_inputs: RecordInputs<'_, 'py> = RecordInputs {
                classes: &classes.record,
                visible: &visible.declarations,
            };
            let (record, facts) = finished_record(&record_inputs, row, finished)?;
            let mut owner = project.borrow_mut();
            owner.retain(facts);
            let index: usize = owner.retain_input(record);
            drop(owner);
            inputs.push(Bound::new(
                py,
                NativeModelInput::new(project.clone().unbind(), index),
            )?);
        }
        Ok(inputs)
    }
}

impl NativeModelLoop {
    /// Build every model in order into `project`; return each model's input view.
    pub(crate) fn run<'py>(
        slf: &Bound<'py, Self>,
        project: Bound<'py, NativeCompiledProject>,
        host: Bound<'py, ConstantRendering>,
        model_files: Vec<Py<NativeModelFile>>,
    ) -> PyResult<Vec<Bound<'py, NativeModelInput>>> {
        let py: Python<'py> = slf.py();
        let classes: LoopClasses<'py> = slf.get().loop_classes.bind(py).extract()?;
        let mut models: Vec<ModelLoopRow> =
            prepared_rows(py, (&classes, &slf.get().syntax), model_files)?;
        let headers: Vec<Option<NativeHeader>> = models.iter().map(native_header).collect();
        let facts: Vec<Option<HeaderFacts>> = py
            .compiler_detach(|| Ok(all_header_facts(&headers)))
            .map_err(compiler_error)?;
        for (row, facts) in models.iter_mut().zip(facts) {
            row.header = facts;
        }
        project
            .borrow_mut()
            .keep_attachment_loop(slf.clone().unbind().into_any(), host.clone().unbind());
        slf.get().run_models(py, project, host, models)
    }

    /// Attach every discovered source in order into `project`; return each source's view.
    pub(crate) fn run_sources<'py>(
        &self,
        project: &Bound<'py, NativeCompiledProject>,
        host: &Bound<'py, ConstantRendering>,
        rows: Vec<SourceRow>,
        settings: SourceSettings,
    ) -> PyResult<Vec<NativeSourceInput>> {
        let py: Python<'py> = host.py();
        let contracts: ContractClasses<'py> = self.contract_classes.bind(py).extract()?;
        let classes: LoopClasses<'py> = self.loop_classes.bind(py).extract()?;
        let context: LoopContext<'_, 'py> = LoopContext {
            host,
            classes: &classes,
            contracts: &contracts,
            syntax: &self.syntax,
        };
        let attached: Vec<AttachedSource> = attached_sources(self, &context, rows, settings)?;
        Ok(retain_sources(project, attached))
    }

    /// Attach every singular audit, then each owner's attached audits.
    pub(crate) fn run_audits<'py>(
        &self,
        host: &Bound<'py, ConstantRendering>,
        files: &[AuditFileRow],
        owners: Vec<AttachmentOwner<'py>>,
        settings: &AuditSettings,
    ) -> PyResult<(Vec<AttachedAudit>, Vec<Bound<'py, PyAny>>)> {
        let py: Python<'py> = host.py();
        let contracts: ContractClasses<'py> = self.contract_classes.bind(py).extract()?;
        let classes: LoopClasses<'py> = self.loop_classes.bind(py).extract()?;
        let context: LoopContext<'_, 'py> = LoopContext {
            host,
            classes: &classes,
            contracts: &contracts,
            syntax: &self.syntax,
        };
        attached_audits(self, &context, (files, owners), settings)
    }

    /// Attach every SQL test case, then every scenario, against the project's resources.
    pub(crate) fn run_sql_tests<'py>(
        &self,
        host: &Bound<'py, ConstantRendering>,
        rows: (&[TestFileRow], &[ScenarioRow]),
        settings: &SqlTestSettings,
        table_function_arities: HashMap<String, usize>,
    ) -> PyResult<AttachedSqlTests> {
        let py: Python<'py> = host.py();
        let contracts: ContractClasses<'py> = self.contract_classes.bind(py).extract()?;
        let classes: LoopClasses<'py> = self.loop_classes.bind(py).extract()?;
        let context: LoopContext<'_, 'py> = LoopContext {
            host,
            classes: &classes,
            contracts: &contracts,
            syntax: &self.syntax,
        };
        let validator: Bound<'py, NativeModelValidator> =
            self.validator.bind(py).downcast()?.clone();
        let known: KnownResources<'_> = validator.get().known_resources();
        let resolver: &Bound<'py, PyAny> = &classes.external_resolver;
        let extended =
            |names: &HashSet<String>, method: &str, keyword: &str| -> PyResult<HashSet<String>> {
                let mut names: HashSet<String> = names.clone();
                if !resolver.is_none() {
                    let arguments: Bound<'py, PyDict> = PyDict::new(py);
                    arguments.set_item(keyword, &names)?;
                    let added: HashSet<String> = resolver
                        .call_method(method, (), Some(&arguments))?
                        .extract()?;
                    names.extend(added);
                }
                Ok(names)
            };
        let models: HashSet<String> = extended(
            known.models,
            "extend_sql_test_model_names",
            "known_model_names",
        )?;
        let seeds: HashSet<String> = extended(
            known.seeds,
            "extend_sql_test_seed_names",
            "known_seed_names",
        )?;
        let sources: HashSet<String> = extended(
            known.sources,
            "extend_sql_test_source_names",
            "known_source_names",
        )?;
        let scope: TestScope<'_, 'py> = TestScope {
            model_loop: self,
            context: &context,
            settings,
            catalog: TargetCatalog {
                models,
                sources,
                seeds,
                table_functions: known.table_functions.clone(),
                macros: classes.macros.macro_names()?,
            },
            functions: known.functions.clone(),
            table_function_arities,
        };
        let tests: Vec<Py<PyAny>> = scope.attached(rows.0)?;
        let scenarios: Vec<Py<PyAny>> = scope.attached_scenarios(rows.1)?;
        Ok((tests, scenarios))
    }

    /// Attach every SQL, then Python, function in order, checking table function call arities.
    pub(crate) fn run_functions<'py>(
        &self,
        host: &Bound<'py, ConstantRendering>,
        rows: (Vec<FunctionRow>, Vec<FunctionRow>),
        settings: FunctionSettings,
        models: &[(String, Vec<ReferenceRow>)],
    ) -> PyResult<Vec<AttachedFunction>> {
        let py: Python<'py> = host.py();
        let contracts: ContractClasses<'py> = self.contract_classes.bind(py).extract()?;
        let classes: LoopClasses<'py> = self.loop_classes.bind(py).extract()?;
        let context: LoopContext<'_, 'py> = LoopContext {
            host,
            classes: &classes,
            contracts: &contracts,
            syntax: &self.syntax,
        };
        let attached: Vec<AttachedFunction> = attached_functions(self, &context, rows, settings)?;
        checked_table_function_arities(&context, models, &attached)?;
        Ok(attached)
    }

    /// Run the model's steps in the attachment's order; `None` hands it to the attachment.
    fn finish<'py>(
        &self,
        py: Python<'py>,
        context: &LoopContext<'_, 'py>,
        model: LoopModel<'_, 'py>,
    ) -> PyResult<FinishedModel<'py>> {
        let LoopModel {
            row,
            config,
            visible,
            signatures,
            visibility,
        } = model;
        let ModelLoopRow {
            paths:
                ModelFileTexts {
                    name,
                    posix_path,
                    relative_path,
                    file_path,
                },
            authored_query,
            contents,
            scan,
            interpolation,
            model_file,
            header: header_facts,
        } = row;
        let values: Bound<'py, PyDict> =
            native_field(&config, VALUES_ATTRIBUTE)?.downcast_into()?;
        let schema: Option<ModelSchema<'py>> = model_schema(context, &values, name)?;
        let hooked: bool = hook_presence(&values)?;
        let python_hooks: Vec<Bound<'py, PyAny>> = if hooked {
            signatures.checked(context, &values, &visible.consumer, (name, relative_path))?
        } else {
            Vec::new()
        };
        let schema_usages: Vec<Bound<'py, PyAny>> = match &schema {
            Some(schema) => {
                let declaration = (&SCHEMA_DECLARATION, schema.name.as_str());
                named_usages(
                    context,
                    declaration,
                    &visible.consumer,
                    (name, relative_path),
                )?
            }
            None => Vec::new(),
        };
        let interpolated: String = applied_interpolation(context, interpolation)?;
        let (references, stop) = scan.clone().unwrap_or_default();
        let expansion_classes: ExpansionClasses<'py> = ExpansionClasses {
            usage_record: context.classes.usage_record.clone(),
            compile_error: context.classes.compile_error.clone(),
        };
        let mut declarations: ExpandedDeclarations<'py> =
            expand_declarations(&DeclarationExpansion {
                sql: &interpolated,
                references: &references,
                stop,
                context: &visible.declarations,
                consumer: ExpansionConsumer {
                    identity: Some(visible.consumer.clone()),
                    relationship_visibility: false,
                    path: file_path,
                },
                rendering: context.host.get(),
                classes: &expansion_classes,
                sql_macros: Some(SqlMacroSource {
                    loaded_macros: Some(context.classes.macros.loaded_macros.bind(py).as_any()),
                    origin: Some(SqlCallOrigin {
                        relative_path,
                        contents,
                        query: authored_query,
                    }),
                    scopes: Some(&context.classes.visibility),
                    syntax: Some(context.syntax),
                }),
            })?;
        let macros: Option<ExpandedMacros> = if calls_macros(&declarations.spliced.sql) {
            let model: MacroModel<'_> = MacroModel {
                file_path,
                relative_path,
                contents,
                authored_query,
                authored: interpolated.as_str() == &**authored_query,
                declaration_spans: &declarations.spliced.spans,
            };
            Some(expanded_macros(
                context,
                visible,
                &declarations.spliced.sql,
                &model,
            )?)
        } else {
            None
        };
        let (macro_sql, argument_references, macro_spans): (
            String,
            Vec<ReferenceRow>,
            Vec<SpanRow>,
        ) = match &macros {
            Some(macros) => (
                macros.output.sql.clone(),
                macros.references.clone(),
                macros
                    .output
                    .spans
                    .iter()
                    .map(|span| span_row(*span))
                    .collect(),
            ),
            None => (declarations.spliced.sql.clone(), Vec::new(), Vec::new()),
        };
        let query_sql: String = self
            .canonical_query(name, &macro_sql, &values)?
            .map_err(|message| compile_error(context.classes, &message, None))?;
        let typed_calls: Vec<TypedCall> = query_typed_calls(
            std::mem::take(&mut declarations.typed_calls),
            (&declarations.spliced.sql, &macro_sql, &query_sql),
            &macro_spans,
        );
        let opt_out_model: OptOutModel<'_, 'py> = OptOutModel {
            name,
            file_path,
            config: &config,
            values: &values,
            query_sql: &query_sql,
            relative_path,
            contents,
        };
        let (analysed, rejected_opt_out) = self.analysis(context, &opt_out_model)?;
        let scanned = match extract_sql_references(&query_sql, &self.syntax) {
            ReferenceExtraction::Extracted(scanned) => scanned,
            ReferenceExtraction::Failed(failure) => {
                let authored: bool =
                    interpolated.as_str() == &**authored_query && query_sql == macro_sql;
                let origin: ScanFaultOrigin<'_> = ScanFaultOrigin {
                    relative_path: posix_path,
                    contents,
                    query_sql: authored_query,
                    passes: authored.then(|| {
                        vec![
                            declarations.spliced.spans.clone(),
                            spliced_spans(&macro_spans),
                        ]
                    }),
                };
                let message: String =
                    located_scan_error(&failure, &query_sql, &origin, &self.syntax);
                return Err(compile_error(context.classes, &message, None));
            }
        };
        report_invalid_calls(
            context,
            &self.syntax,
            &scanned.invalid_calls,
            CallOrigin {
                name: Some(name),
                relative_path,
                file_path,
                contents,
            },
        )?;
        let analysis: (bool, Option<Bound<'py, PyAny>>) = (
            analysed && scanned.invalid_calls.is_empty(),
            rejected_opt_out,
        );
        let references: Vec<ReferenceRow> = merge_call_site_references(
            scanned.references.iter().map(reference_row).collect(),
            argument_references,
        );
        let declared_columns: Option<Vec<String>> =
            schema.as_ref().map(schema_column_names).transpose()?;
        self.validate(
            context,
            (&config, &values),
            (name, &query_sql, relative_path),
            (&references, declared_columns),
        )?;
        let (values, hook_usages) = if hooked {
            let hook_model: HookModel<'_, 'py> = HookModel {
                name,
                posix_path,
                relative_path,
                file_path,
                contents,
                authored_query,
                visible,
                visibility,
            };
            let hooks: ExpandedHooks<'py> = expanded_hooks(self, context, &values, &hook_model)?;
            for name in hooks.sql_macros {
                if !declarations.sql_macros.contains(&name) {
                    declarations.sql_macros.push(name);
                }
            }
            (hooks.values, hooks.usages)
        } else {
            (values, Vec::new())
        };
        let source: HeaderSource<'_, 'py> = HeaderSource {
            name,
            values: &values,
            config: &config,
            model_file: model_file.get(),
            relative_path,
            visible: (&visible.declarations, &visible.consumer),
            schema: schema.as_ref(),
            facts: header_facts.as_ref(),
            model_file_handle: model_file,
        };
        let header: AttachedHeader<'py> = header_schema(py, context, &source)?;
        let mut named_usages: Vec<Bound<'py, PyAny>> = hook_usages;
        named_usages.extend(schema_usages);
        named_usages.extend(python_hooks);
        Ok(FinishedModel {
            config,
            values,
            query_sql,
            header,
            references,
            declarations,
            analysis,
            named_usages,
            macros: macros.map(|macros| macros.output),
            typed_calls,
        })
    }

    /// Whether the model's SQL is analysed and its rejected opt-out; syntax is checked unless deferred.
    fn analysis<'py>(
        &self,
        context: &LoopContext<'_, 'py>,
        model: &OptOutModel<'_, 'py>,
    ) -> PyResult<(bool, Option<Bound<'py, PyAny>>)> {
        let (analysed, rejected) =
            match analysis_gate(self.settings, analysis_override(model.values)?) {
                AnalysisGate::Conflict => {
                    return Err(compile_error(
                        context.classes,
                        ANALYSIS_CONFLICT,
                        Some(ANALYSIS_CONFLICT_CODE),
                    ));
                }
                AnalysisGate::Enabled => (true, None),
                AnalysisGate::Disabled => match rejected_opt_out(self, context, model)? {
                    OptOut::Accepted => (false, None),
                    OptOut::Rejected(location) => (true, Some(location)),
                },
            };
        if analysed && !self.settings.defer_sql_validation {
            reject_unparseable_query(context, model)?;
        }
        Ok((analysed, rejected))
    }

    /// The query with canonical cursor intrinsics, or the message the attachment raises.
    fn canonical_query(
        &self,
        name: &str,
        sql: &str,
        values: &Bound<'_, PyDict>,
    ) -> PyResult<Result<String, String>> {
        let incremental: bool = values.get_item(MATERIALIZED_KEY)?.is_some_and(|value| {
            value
                .downcast::<PyString>()
                .is_ok_and(|text| text.to_string_lossy() == INCREMENTAL_MATERIALIZATION)
        });
        let cursor: Option<String> = match values.get_item(CURSOR_KEY)? {
            Some(value) => match value.downcast::<PyString>() {
                Ok(text) => Some(text.to_string_lossy().into_owned()),
                Err(_) => None,
            },
            None => None,
        };
        let model: IntrinsicModel<'_> = IntrinsicModel {
            name,
            incremental,
            cursor: cursor.as_deref(),
        };
        Ok(validated_model_intrinsics(
            sql,
            &self.reserved_markers,
            &model,
        ))
    }

    /// Validate the model natively, raising the validator's first error as Python does.
    fn validate(
        &self,
        context: &LoopContext<'_, '_>,
        config: (&Bound<'_, PyAny>, &Bound<'_, PyDict>),
        model: (&str, &str, &str),
        references: (&[ReferenceRow], Option<Vec<String>>),
    ) -> PyResult<()> {
        let (references, declared_columns) = references;
        let py: Python<'_> = context.host.py();
        let (config, values) = config;
        let external: Vec<Option<PyErr>> = external_errors(context, references, model.2)?;
        let rows: Vec<(&str, &str, bool)> = references
            .iter()
            .zip(&external)
            .map(|(ReferenceRow { kind, name, .. }, error)| {
                (kind.as_str(), name.as_str(), error.is_some())
            })
            .collect();
        let unmanaged: Bound<'_, PyAny> = native_field(
            &native_field(config, RETENTION_ATTRIBUTE)?,
            UNMANAGED_ATTRIBUTE,
        )?;
        let declared: Bound<'_, PyAny> = native_field(
            &native_field(config, TABLE_TYPE_ATTRIBUTE)?,
            DECLARED_ATTRIBUTE,
        )?;
        let outcome: Bound<'_, PyAny> = self.validator.bind(py).call_method1(
            VALIDATE_METHOD,
            (
                values.clone(),
                model,
                (rows, declared_columns, unmanaged, declared),
            ),
        )?;
        validation_outcome(&outcome, external, values)
    }
}

/// `_raise_validation_outcome`: nothing, the validator's error, or a rejected reference's error.
pub(crate) fn validation_outcome(
    outcome: &Bound<'_, PyAny>,
    external: Vec<Option<PyErr>>,
    values: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if outcome.is_none() {
        return Ok(());
    }
    if let Ok(index) = outcome.extract::<usize>()
        && let Some(error) = external.into_iter().nth(index).flatten()
    {
        return Err(error);
    }
    Err(config_error(outcome, Some(values)))
}

/// `applied_interpolation`: record the interpolation's template reads, then its SQL or error.
fn applied_interpolation(
    context: &LoopContext<'_, '_>,
    interpolation: &InterpolationRow,
) -> PyResult<String> {
    let InterpolationRow {
        sql, reads, error, ..
    } = interpolation;
    record_reads(context, reads)?;
    match error {
        Some(message) => Err(compile_error(context.classes, message, None)),
        None => Ok(sql.clone()),
    }
}

/// Record interpolation's environment and context reads for compile reuse.
pub(crate) fn record_reads(
    context: &LoopContext<'_, '_>,
    reads: &[(String, String)],
) -> PyResult<()> {
    if !reads.is_empty() {
        let py: Python<'_> = context.host.py();
        context
            .classes
            .config
            .record_template_reads
            .call1((PyTuple::new(py, reads)?,))?;
    }
    Ok(())
}

/// `CompileInputError(message, code=...)`, raised as the attachment raises it.
pub(crate) fn compile_error(classes: &LoopClasses<'_>, message: &str, code: Option<&str>) -> PyErr {
    let raised = (|| {
        let keywords: Bound<'_, PyDict> = PyDict::new(classes.compile_error.py());
        if let Some(code) = code {
            keywords.set_item(CODE_ARGUMENT, code)?;
        }
        classes.compile_error.call((message,), Some(&keywords))
    })();
    match raised {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

/// `CompileInputError(message, help=help)`.
pub(crate) fn helped_compile_error(
    classes: &LoopClasses<'_>,
    message: &str,
    help: Option<&str>,
) -> PyErr {
    let raised = (|| {
        let keywords: Bound<'_, PyDict> = PyDict::new(classes.compile_error.py());
        keywords.set_item(HELP_ARGUMENT, help)?;
        classes.compile_error.call((message,), Some(&keywords))
    })();
    match raised {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

/// The Python exception a native config error raises as, given the values it names.
pub(crate) fn config_error(error: &Bound<'_, PyAny>, values: Option<&Bound<'_, PyDict>>) -> PyErr {
    raised_config_exception(error, values.map(Bound::as_any))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelLoop>()
}

/// The spliced SQL's expression macro calls moved through macro expansion into the query's bytes.
fn query_typed_calls(
    calls: Vec<TypedCall>,
    (spliced, expanded, query): (&str, &str, &str),
    macro_spans: &[SpanRow],
) -> Vec<TypedCall> {
    if calls.is_empty() {
        return calls;
    }
    let calls: Vec<TypedCall> = if macro_spans.is_empty() {
        calls
    } else {
        spliced_typed_calls(calls, (spliced, expanded), &spliced_spans(macro_spans))
    };
    if expanded == query {
        return calls;
    }
    let Ok((canonical, spans)) =
        replace_intrinsics_with_spans(expanded, "", |name| format!("{name}()"))
    else {
        return Vec::new();
    };
    if canonical != query {
        return Vec::new();
    }
    let splices: Vec<TextSplice> = spans
        .into_iter()
        .map(|span| TextSplice {
            source: (span.source_start, span.source_end),
            output: (span.output_start, span.output_end),
        })
        .collect();
    moved_typed_calls(calls, &splices)
}

/// Span rows as the spliced spans the scan-fault locator maps through.
fn spliced_spans(rows: &[SpanRow]) -> Vec<SplicedSpan> {
    rows.iter()
        .map(|row| SplicedSpan {
            source_start: row.source_start,
            source_end: row.source_end,
            output_start: row.output_start,
            output_end: row.output_end,
        })
        .collect()
}
