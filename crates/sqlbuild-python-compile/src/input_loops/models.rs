//! Public records of input loops.

use crate::input_loops::_helpers::model::model_loop::ANALYSIS_CONFLICT;
use crate::input_loops::_helpers::model::model_loop::ANALYSIS_CONFLICT_CODE;
use crate::input_loops::_helpers::model::model_loop::CURSOR_KEY;
use crate::input_loops::_helpers::model::model_loop::DECLARED_ATTRIBUTE;
use crate::input_loops::_helpers::model::model_loop::INCREMENTAL_MATERIALIZATION;
use crate::input_loops::_helpers::model::model_loop::LoopChecks;
use crate::input_loops::_helpers::model::model_loop::LoopClasses;
use crate::input_loops::_helpers::model::model_loop::LoopContext;
use crate::input_loops::_helpers::model::model_loop::LoopModel;
use crate::input_loops::_helpers::model::model_loop::LoopSettingsInput;
use crate::input_loops::_helpers::model::model_loop::MATERIALIZED_KEY;
use crate::input_loops::_helpers::model::model_loop::RETENTION_ATTRIBUTE;
use crate::input_loops::_helpers::model::model_loop::TABLE_TYPE_ATTRIBUTE;
use crate::input_loops::_helpers::model::model_loop::UNMANAGED_ATTRIBUTE;
use crate::input_loops::_helpers::model::model_loop::VALIDATE_METHOD;
use crate::input_loops::_helpers::model::model_loop::VALUES_ATTRIBUTE;
use crate::input_loops::_helpers::model::model_loop::VisibleDeclarations;
use crate::input_loops::_helpers::model::model_loop::applied_interpolation;
use crate::input_loops::_helpers::model::model_loop::compile_error;
use crate::input_loops::_helpers::model::model_loop::index_sql_hook_definitions;
use crate::input_loops::_helpers::model::model_loop::prepared_rows;
use crate::input_loops::_helpers::model::model_loop::query_typed_calls;
use crate::input_loops::_helpers::model::model_loop::spliced_spans;
use crate::input_loops::_helpers::model::model_loop::validation_outcome;
use crate::input_loops::_helpers::model::model_loop_calls::CallOrigin;
use crate::input_loops::_helpers::model::model_loop_calls::report_invalid_calls;
use crate::input_loops::_helpers::model::model_loop_config::ModelConfigs;
use crate::input_loops::_helpers::model::model_loop_external::external_errors;
use crate::input_loops::_helpers::model::model_loop_header::AttachedHeader;
use crate::input_loops::_helpers::model::model_loop_header::HeaderSource;
use crate::input_loops::_helpers::model::model_loop_header::header_schema;
use crate::input_loops::_helpers::model::model_loop_hooks::ExpandedHooks;
use crate::input_loops::_helpers::model::model_loop_hooks::HookModel;
use crate::input_loops::_helpers::model::model_loop_hooks::expanded_hooks;
use crate::input_loops::_helpers::model::model_loop_host::FinishedModel;
use crate::input_loops::_helpers::model::model_loop_host::ModelLoopRow;
use crate::input_loops::_helpers::model::model_loop_host::RecordInputs;
use crate::input_loops::_helpers::model::model_loop_host::finished_record;
use crate::input_loops::_helpers::model::model_loop_inputs::analysis_override;
use crate::input_loops::_helpers::model::model_loop_inputs::hook_presence;
use crate::input_loops::_helpers::model::model_loop_inputs::reference_row;
use crate::input_loops::_helpers::model_loop::model_loop_macros::ExpandedMacros;
use crate::input_loops::_helpers::model_loop::model_loop_macros::MacroModel;
use crate::input_loops::_helpers::model_loop::model_loop_macros::expanded_macros;
use crate::input_loops::_helpers::model_loop::model_loop_named::SCHEMA_DECLARATION;
use crate::input_loops::_helpers::model_loop::model_loop_named::named_usages;
use crate::input_loops::_helpers::model_loop::model_loop_opt_out::AuthoredQuery;
use crate::input_loops::_helpers::model_loop::model_loop_opt_out::GapQueue;
use crate::input_loops::_helpers::model_loop::model_loop_opt_out::OptOut;
use crate::input_loops::_helpers::model_loop::model_loop_opt_out::OptOutModel;
use crate::input_loops::_helpers::model_loop::model_loop_opt_out::gap_job;
use crate::input_loops::_helpers::model_loop::model_loop_opt_out::reject_unparseable_query;
use crate::input_loops::_helpers::model_loop::model_loop_opt_out::rejected_opt_out;
use crate::input_loops::_helpers::model_loop::model_loop_opt_out::skip_unavailable;
use crate::input_loops::_helpers::model_loop::model_loop_prepass::NativeHeader;
use crate::input_loops::_helpers::model_loop::model_loop_prepass::all_header_facts;
use crate::input_loops::_helpers::model_loop::model_loop_prepass::native_header;
use crate::input_loops::_helpers::model_loop::model_loop_python_hooks::HookSignatures;
use crate::input_loops::_helpers::model_loop::model_loop_schema::ModelSchema;
use crate::input_loops::_helpers::model_loop::model_loop_schema::model_schema;
use crate::input_loops::_helpers::model_loop::model_loop_schema::schema_column_names;
use crate::input_loops::_helpers::model_loop::model_loop_visibility::ModelVisibility;
use crate::input_loops::_helpers::model_loop::model_loop_visibility::VisibilityModel;
use crate::input_loops::_helpers::resources::audit_loop::attached_audits;
use crate::input_loops::_helpers::resources::function_loop::attached_functions;
use crate::input_loops::_helpers::resources::function_loop::checked_table_function_arities;
use crate::input_loops::_helpers::resources::source_loop::attached_sources;
use crate::input_loops::_helpers::resources::sql_test_loop::TestScope;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use sqlbuild_analysis::analysis_route::main::analysis_route::analysis_route;
use sqlbuild_analysis::analysis_route::models::AnalysisRoute;
use sqlbuild_analysis::compiled_project::main::analysis_gate::analysis_gate;
use sqlbuild_analysis::compiled_project::main::calls_macros::calls_macros;
use sqlbuild_analysis::compiled_project::main::located_scan_error::located_scan_error;
use sqlbuild_analysis::compiled_project::main::merge_call_site_references::merge_call_site_references;
use sqlbuild_analysis::compiled_project::models::AnalysisGate;
use sqlbuild_analysis::compiled_project::models::ModelLoopSettings;
use sqlbuild_analysis::compiled_project::models::ScanFaultOrigin;
use sqlbuild_analysis::compiled_project::models::SplicedSpan;
use sqlbuild_analysis::sql_macros::models::TypedCall;
use sqlbuild_attachments::cursor_intrinsics::main::validated_model_intrinsics::validated_model_intrinsics;
use sqlbuild_attachments::cursor_intrinsics::models::IntrinsicModel;
use sqlbuild_attachments::functions::models::KnownResources;
use sqlbuild_attachments::test_targets::models::TargetCatalog;
use sqlbuild_discovery::model_files::models::HeaderFacts;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::cache::models::NativeParseStore;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_front::discovery::models::ModelFileTexts;
use sqlbuild_python_front::discovery::models::NativeModelFile;
use sqlbuild_python_front::model_config::models::AuditInstance;
use sqlbuild_python_front::model_config::models::ContractClasses;
use sqlbuild_python_front::model_config::models::NativeModelValidator;
use sqlbuild_python_front::render::main::calls::span_row::span_row;
use sqlbuild_python_front::render::main::declarations::expand_declarations::expand_declarations;
use sqlbuild_python_front::render::models::ConstantRendering;
use sqlbuild_python_front::render::models::DeclarationExpansion;
use sqlbuild_python_front::render::models::DeclarationExpansionClasses;
use sqlbuild_python_front::render::models::ExpandedDeclarations;
use sqlbuild_python_front::render::models::ExpansionConsumer;
use sqlbuild_python_front::render::models::SqlCallOrigin;
use sqlbuild_python_front::render::models::SqlMacroSource;
use sqlbuild_python_front::render::types::SpanRow;
use sqlbuild_python_front::sqltext::models::LexicalSyntaxInput;
use sqlbuild_python_project::compiled_project::models::AuditFileRow;
use sqlbuild_python_project::compiled_project::models::AuditSettings;
use sqlbuild_python_project::compiled_project::models::FunctionRow;
use sqlbuild_python_project::compiled_project::models::FunctionSettings;
use sqlbuild_python_project::compiled_project::models::NativeCompiledProject;
use sqlbuild_python_project::compiled_project::models::ScenarioRow;
use sqlbuild_python_project::compiled_project::models::SqlTestSettings;
use sqlbuild_python_project::compiled_project::models::TestFileRow;
use sqlbuild_python_project::compiled_project::types::AttachedSqlTests;
use sqlbuild_python_project::compiled_project::types::SourceRow;
use sqlbuild_python_project::compiled_project::types::SourceSettings;
use sqlbuild_python_project::compiled_views::models::NativeModelInput;
use sqlbuild_python_project::resource_views::main::source_views::retain_sources::retain_sources;
use sqlbuild_python_project::resource_views::models::NativeSourceInput;
use sqlbuild_python_project::resource_views::types::AttachedAudit;
use sqlbuild_python_project::resource_views::types::AttachedFunction;
use sqlbuild_python_project::resource_views::types::AttachedSource;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::ReferenceExtraction;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

/// One resource's attached audits: its target, consumer path, and owner files and instances.
#[derive(Debug)]
pub struct AttachmentOwner<'py> {
    pub kind: &'static str,
    pub name: String,
    pub relation: String,
    pub path: String,
    /// `(owner file, column, instance)` in attachment order.
    pub instances: Vec<(String, Option<String>, AuditInstance<'py>)>,
}

/// Whether SQL analysis runs, is skipped, is deferred, and is required.
#[derive(Debug)]
pub struct LoopSwitches {
    pub sql_analysis: bool,
    pub no_sql_validation: bool,
    pub defer_sql_validation: bool,
    pub require_sql_analysis: bool,
}

impl<'py> pyo3::FromPyObject<'_, 'py> for LoopSwitches {
    type Error = pyo3::PyErr;

    fn extract(value: pyo3::Borrowed<'_, 'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let value: &pyo3::Bound<'py, pyo3::PyAny> = &value;
        let (sql_analysis, no_sql_validation, defer_sql_validation, require_sql_analysis): (
            bool,
            bool,
            bool,
            bool,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            sql_analysis,
            no_sql_validation,
            defer_sql_validation,
            require_sql_analysis,
        })
    }
}

/// The ordered model loop of one compile-input stage.
#[pyclass(module = "sqlbuild._native", frozen)]
pub struct NativeModelLoop {
    pub validator: Py<PyAny>,
    pub syntax: LexicalSyntax,
    pub reserved_markers: Vec<String>,
    pub settings: ModelLoopSettings,
    pub project_config_path: String,
    /// Whether the dialect's SQL is analysed by Polyglot or by its native module.
    pub route: AnalysisRoute,
    /// The adapter's analysis dialect name, which P026 names when no native parser reads it.
    pub dialect: Option<String>,
    pub(crate) contract_classes: Py<PyDict>,
    pub(crate) loop_classes: Py<PyDict>,
}

impl std::fmt::Debug for NativeModelLoop {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeModelLoop")
            .finish_non_exhaustive()
    }
}

#[pymethods]
impl NativeModelLoop {
    #[new]
    pub fn new(
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
            dialect,
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
            route: analysis_route(dialect.as_deref()),
            dialect,
            contract_classes,
            loop_classes,
        })
    }
}

impl NativeModelLoop {
    pub(crate) fn run_models<'py>(
        &self,
        py: Python<'py>,
        project: Bound<'py, NativeCompiledProject>,
        host: Bound<'py, ConstantRendering>,
        (models, cache_dir): (Vec<ModelLoopRow>, Option<PathBuf>),
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
        let parsed: Option<NativeParseStore> = match (self.route, cache_dir) {
            (AnalysisRoute::Native(_), Some(cache_dir)) => {
                NativeParseStore::open_in(py, &cache_dir)?
            }
            _ => None,
        };
        let mut gaps: GapQueue = GapQueue::new(parsed);
        let attached: PyResult<()> = (|| -> PyResult<()> {
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
                let config: Bound<'py, PyAny> = configs.effective(
                    &context,
                    record,
                    (name, relative_path),
                    row.header.as_ref(),
                )?;
                let model: LoopModel<'_, 'py> = LoopModel {
                    row: &row,
                    config,
                    visible: &visible,
                    signatures: &signatures,
                    visibility: &visibility,
                };
                gaps.expect_record(project.borrow().inputs().len());
                let finished: FinishedModel<'py> = self.finish(py, &context, model, &mut gaps)?;
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
            Ok(())
        })();
        let settled: PyResult<()> = gaps.settle(self.settings, &context, &project);
        settled?;
        attached?;
        Ok(inputs)
    }
}

impl NativeModelLoop {
    /// Build every model in order into `project`; return each model's input view.
    pub fn run<'py>(
        slf: &Bound<'py, Self>,
        project: Bound<'py, NativeCompiledProject>,
        host: Bound<'py, ConstantRendering>,
        (model_files, cache_dir): (Vec<Py<NativeModelFile>>, Option<PathBuf>),
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
        slf.get().run_models(py, project, host, (models, cache_dir))
    }

    /// Attach every discovered source in order into `project`; return each source's view.
    pub fn run_sources<'py>(
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
    pub fn run_audits<'py>(
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
    pub fn run_sql_tests<'py>(
        &self,
        host: &Bound<'py, ConstantRendering>,
        rows: (&[TestFileRow], &[ScenarioRow]),
        settings: &SqlTestSettings,
        (table_function_arities, whole_partition): (HashMap<String, usize>, HashSet<String>),
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
        let validator: Bound<'py, NativeModelValidator> = self.validator.bind(py).cast()?.clone();
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
            whole_partition,
        };
        let tests: Vec<Py<PyAny>> = scope.attached(rows.0)?;
        let scenarios: Vec<Py<PyAny>> = scope.attached_scenarios(rows.1)?;
        Ok((tests, scenarios))
    }

    /// Attach every SQL, then Python, function in order, checking table function call arities.
    pub fn run_functions<'py>(
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
    pub(crate) fn finish<'py>(
        &self,
        py: Python<'py>,
        context: &LoopContext<'_, 'py>,
        model: LoopModel<'_, 'py>,
        gaps: &mut GapQueue,
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
        let values: Bound<'py, PyDict> = native_field(&config, VALUES_ATTRIBUTE)?.cast_into()?;
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
        let expansion_classes: DeclarationExpansionClasses<'py> = DeclarationExpansionClasses {
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
        let authored_passes: Vec<Vec<SplicedSpan>> = vec![
            declarations.spliced.spans.clone(),
            spliced_spans(&macro_spans),
        ];
        let authored_body: Option<AuthoredQuery<'_>> = (interpolated.as_str() == &**authored_query)
            .then_some(AuthoredQuery {
                query: authored_query,
                macro_sql: &macro_sql,
                passes: &authored_passes,
            });
        let opt_out_model: OptOutModel<'_, 'py> = OptOutModel {
            name,
            file_path,
            config: &config,
            values: &values,
            query_sql: &query_sql,
            relative_path,
            contents,
            authored: authored_body,
        };
        let legacy_gate = match self.route {
            AnalysisRoute::Legacy => Some(self.analysis(context, &opt_out_model, true, gaps)?),
            AnalysisRoute::Native(_) | AnalysisRoute::Unavailable => None,
        };
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
        let (analysed, rejected_opt_out, analysis_skipped) = match legacy_gate {
            Some(gate) => gate,
            None => {
                let expanded_model: OptOutModel<'_, 'py> = OptOutModel {
                    name,
                    file_path,
                    config: &config,
                    values: &values,
                    query_sql: &query_sql,
                    relative_path,
                    contents,
                    authored: authored_body,
                };
                let calls_valid: bool = scanned.invalid_calls.is_empty();
                self.analysis(context, &expanded_model, calls_valid, gaps)?
            }
        };
        let analysis: (bool, Option<Bound<'py, PyAny>>) = (
            analysed && scanned.invalid_calls.is_empty(),
            rejected_opt_out,
        );
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
            analysis_skipped,
            named_usages,
            macros: macros.map(|macros| macros.output),
            typed_calls,
        })
    }

    /// Analysed, rejected opt-out, skipped; a native parse queued on `gaps` unmarks it on a gap.
    pub(crate) fn analysis<'py>(
        &self,
        context: &LoopContext<'_, 'py>,
        model: &OptOutModel<'_, 'py>,
        calls_valid: bool,
        gaps: &mut GapQueue,
    ) -> PyResult<(bool, Option<Bound<'py, PyAny>>, bool)> {
        let mut skipped: bool = false;
        let (analysed, rejected) =
            match analysis_gate(self.settings, analysis_override(model.values)?) {
                AnalysisGate::Conflict => {
                    return Err(compile_error(
                        context.classes,
                        ANALYSIS_CONFLICT,
                        Some(ANALYSIS_CONFLICT_CODE),
                    ));
                }
                AnalysisGate::Enabled => match self.route {
                    AnalysisRoute::Legacy => (true, None),
                    AnalysisRoute::Native(_) | AnalysisRoute::Unavailable if !calls_valid => {
                        (false, None)
                    }
                    AnalysisRoute::Native(dialect) => {
                        gaps.queue(gap_job(dialect, context, model)?);
                        (true, None)
                    }
                    AnalysisRoute::Unavailable => {
                        skip_unavailable(self.settings, self.dialect.as_deref(), context, model)?;
                        skipped = true;
                        (false, None)
                    }
                },
                AnalysisGate::Disabled => match rejected_opt_out(self, context, model)? {
                    OptOut::Accepted => (false, None),
                    OptOut::Rejected(location) => (true, Some(location)),
                },
            };
        let legacy: bool = matches!(self.route, AnalysisRoute::Legacy);
        if analysed && legacy && !self.settings.defer_sql_validation {
            reject_unparseable_query(context, model)?;
        }
        Ok((analysed, rejected, skipped))
    }

    /// The query with canonical cursor intrinsics, or the message the attachment raises.
    pub(crate) fn canonical_query(
        &self,
        name: &str,
        sql: &str,
        values: &Bound<'_, PyDict>,
    ) -> PyResult<Result<String, String>> {
        let incremental: bool = values.get_item(MATERIALIZED_KEY)?.is_some_and(|value| {
            value
                .cast::<PyString>()
                .is_ok_and(|text| text.to_string_lossy() == INCREMENTAL_MATERIALIZATION)
        });
        let cursor: Option<String> = match values.get_item(CURSOR_KEY)? {
            Some(value) => match value.cast::<PyString>() {
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
    pub(crate) fn validate(
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
