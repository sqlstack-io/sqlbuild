//! Public records of render.

use crate::render::_helpers::calls::macro_call_classes::CallClass;
use crate::render::_helpers::calls::macro_call_classes::ClassInputs;
use crate::render::_helpers::calls::macro_call_memo::MacroCallMemo;
use crate::render::_helpers::calls::macro_call_memo::poisoned;
use crate::render::_helpers::declarations::declaration_expansion::usage_visibility;
use crate::render::_helpers::expansion::macro_evaluation::CallText;
use crate::render::_helpers::expansion::macro_evaluation::Evaluation;
use crate::render::_helpers::expansion::macro_evaluation::macro_argument_error;
use crate::render::_helpers::expansion::macro_expansion::CONSTANT_KIND;
use crate::render::_helpers::expansion::macro_expansion::DECLARATION_FORMS;
use crate::render::_helpers::expansion::macro_expansion::ExpansionClasses;
use crate::render::_helpers::expansion::macro_expansion::ExpansionRow;
use crate::render::_helpers::expansion::macro_expansion::LEXICAL_SYNTAX_ATTRIBUTE;
use crate::render::_helpers::expansion::macro_expansion::RELATION_PREFIX;
use crate::render::_helpers::expansion::macro_expansion::RELATION_SUFFIX;
use crate::render::_helpers::expansion::macro_expansion::SQL_LANGUAGE;
use crate::render::_helpers::expansion::macro_expansion::UNREAD_CALL_HELP;
use crate::render::_helpers::expansion::macro_expansion::coded;
use crate::render::_helpers::expansion::macro_expansion::expanded;
use crate::render::_helpers::expansion::macro_expansion::observe_new_modules;
use crate::render::_helpers::expansion::macro_expansion::raised;
use crate::render::_helpers::expansion::macro_expansion::reference_call_text;
use crate::render::_helpers::expansion::macro_expansion::relationship_consumer;
use crate::render::_helpers::expansion::macro_expansion::resource_key;
use crate::render::_helpers::expansion::macro_expansion::result_has_markers;
use crate::render::_helpers::expansion::macro_expansion::sorted_names;
use crate::render::_helpers::expansion::macro_expansion::top_level_output;
use crate::render::_helpers::expansion::macro_expansion::unread_call_detail;
use crate::render::_helpers::expansion::macro_expansion::written_call;
use crate::render::_helpers::expansion::macro_expansion_facts::ExpansionFacts;
use crate::render::_helpers::expansion::macro_expansion_facts::ResourceRef;
use crate::render::_helpers::expansion::macro_generated_references::GeneratedOrigin;
use crate::render::_helpers::expansion::macro_generated_references::report_generated_references;
use crate::render::main::calls::span_row::span_row;
use crate::render::main::calls::span_tuple::span_tuple;
use crate::render::main::declarations::constant_declaration_sql::constant_declaration_sql;
use crate::render::types::MacroScopes;
use crate::render::types::SpanRow;
use crate::render::types::StateTuple;
use crate::sqltext::main::optional_record_syntax::optional_record_syntax;
use crate::sqltext::models::ScannedReference;
use pyo3::FromPyObject;
use pyo3::exceptions::PyIndexError;
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
use pyo3::types::PyTuple;
use sqlbuild_analysis::compiled_project::models::SplicedSql;
use sqlbuild_analysis::sql_macros::constants::BARE_MACRO_CALL_CODE;
use sqlbuild_analysis::sql_macros::constants::MIXED_KIND_CODE;
use sqlbuild_analysis::sql_macros::models::SqlMacroCheck;
use sqlbuild_analysis::sql_macros::models::TypedCall;
use sqlbuild_core::text::main::is_decimal_digit::is_decimal_digit;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::field_text::field_text;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_render::macro_calls::errors::ScanError;
use sqlbuild_render::macro_calls::models::MacroCallEntry;
use sqlbuild_render::macro_calls::models::MacroCallEvent;
use sqlbuild_render::macro_calls::models::MacroCallScan;
use sqlbuild_render::macro_calls::models::MacroCallSite;
use sqlbuild_render::macro_calls::models::MacroScanFailure;
use sqlbuild_render::macro_calls::models::MacroSpliceSpan;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use std::cell::RefCell;

/// Authored SQL expanded through vars, declarations and macros, with each pass's spans.
#[derive(Debug)]
pub struct AuthoredExpansion {
    pub sql: String,
    /// Interpolation, declaration and macro spans, in pass order.
    pub passes: [Vec<SpanRow>; 3],
}

/// The inputs authored SQL is expanded with outside a compile; the handles carry user values.
#[derive(Debug)]
pub struct SqlExpansionInputs {
    pub effective_vars: Py<PyAny>,
    pub loaded_macros: Py<PyAny>,
    pub macro_context: Py<PyAny>,
    pub enums: Py<PyAny>,
    pub constants: Py<PyAny>,
    pub value_renderer: Py<PyAny>,
    pub collection_rendering: Py<PyAny>,
    /// Each model file's own declarations, by file path.
    pub local_declarations: Py<PyDict>,
    pub declaration_resolver: Py<PyAny>,
}

impl SqlExpansionInputs {
    /// The inputs a `SqlExpansionContext` record holds.
    pub fn from_record(record: &Bound<'_, PyAny>) -> PyResult<Self> {
        let field =
            |name: &str| -> PyResult<Py<PyAny>> { Ok(native_field(record, name)?.unbind()) };
        Ok(Self {
            effective_vars: field("effective_vars")?,
            loaded_macros: field("loaded_macros")?,
            macro_context: field("macro_context")?,
            enums: field("enums")?,
            constants: field("constants")?,
            value_renderer: field("value_renderer")?,
            collection_rendering: field("collection_rendering")?,
            local_declarations: native_field(record, "local_declarations")?
                .cast_into::<PyDict>()?
                .unbind(),
            declaration_resolver: field("declaration_resolver")?,
        })
    }

    /// These inputs as a `SqlExpansionContext` record.
    pub fn record<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        new_record_with(
            py,
            RecordId::SqlExpansionContext,
            &[
                ("effective_vars", self.effective_vars.bind(py).clone()),
                ("loaded_macros", self.loaded_macros.bind(py).clone()),
                ("macro_context", self.macro_context.bind(py).clone()),
                ("enums", self.enums.bind(py).clone()),
                ("constants", self.constants.bind(py).clone()),
                ("value_renderer", self.value_renderer.bind(py).clone()),
                (
                    "collection_rendering",
                    self.collection_rendering.bind(py).clone(),
                ),
                (
                    "local_declarations",
                    self.local_declarations.bind(py).clone().into_any(),
                ),
                (
                    "declaration_resolver",
                    self.declaration_resolver.bind(py).clone(),
                ),
            ],
        )
    }
}

/// The Python classes expansion builds usages and errors from.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct DeclarationExpansionClasses<'py> {
    pub usage_record: Bound<'py, PyAny>,
    pub compile_error: Bound<'py, PyAny>,
}

/// Who reads the declarations, whether it prefers relationship visibility, and its error path.
#[derive(Debug, Clone)]
pub struct ExpansionConsumer<'a, 'py> {
    pub identity: Option<Bound<'py, PyAny>>,
    pub relationship_visibility: bool,
    pub path: &'a str,
}

/// One expansion: the SQL, its scan, the visible declarations and the adapter's constant rendering.
#[derive(Debug)]
pub struct DeclarationExpansion<'a, 'py> {
    pub sql: &'a str,
    pub references: &'a [ScannedReference],
    pub stop: Option<u8>,
    pub context: &'a Bound<'py, PyAny>,
    pub consumer: ExpansionConsumer<'a, 'py>,
    pub rendering: &'a ConstantRendering,
    pub classes: &'a DeclarationExpansionClasses<'py>,
    /// What SQL macro calls read beyond the context; `None` where SQL macros are not expanded.
    pub sql_macros: Option<SqlMacroSource<'a, 'py>>,
}

/// The expanded SQL with its spans, and the crate::render::_helpers::declarations::declaration_expansion::usage records it adds in reference order.
#[derive(Debug)]
pub struct ExpandedDeclarations<'py> {
    pub spliced: SplicedSql,
    pub usages: Vec<Bound<'py, PyAny>>,
    /// The SQL macros the SQL called, top-level and nested, once each.
    pub sql_macros: Vec<String>,
    /// What the analysed SQL must show for each top-level call's arguments to fit.
    pub sql_macro_checks: Vec<SqlMacroCheck>,
    /// Each expression macro call in the spliced SQL, by byte range.
    pub typed_calls: Vec<TypedCall>,
}

/// The adapter's constant rendering a compile's declaration expansions share.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct ConstantRendering {
    pub(crate) value_renderer: Py<PyAny>,
    pub(crate) collection_rendering: Py<PyAny>,
}

impl ConstantRendering {
    /// Rendering with the adapter's value renderer and the project's collection rendering.
    pub fn new(value_renderer: Py<PyAny>, collection_rendering: Py<PyAny>) -> Self {
        Self {
            value_renderer,
            collection_rendering,
        }
    }

    /// The adapter name the value renderer renders for, empty when it names none.
    pub fn adapter_name(&self, py: Python<'_>) -> String {
        native_field(self.value_renderer.bind(py), "adapter_name")
            .and_then(|name| name.extract::<String>())
            .unwrap_or_default()
    }

    /// The SQL analysis dialect of the adapter the value renderer renders for, if it has one.
    pub fn analysis_dialect(&self, py: Python<'_>) -> PyResult<Option<String>> {
        native_field(self.value_renderer.bind(py), "sql_analysis_dialect_name")?.extract()
    }

    /// One visible constant as SQL, its failures located at `file_path`.
    pub fn render(&self, declaration: &Bound<'_, PyAny>, file_path: &str) -> PyResult<String> {
        let py: Python<'_> = declaration.py();
        constant_declaration_sql(
            declaration,
            self.value_renderer.bind(py),
            (self.collection_rendering.bind(py), Some(file_path)),
        )
    }
}

/// A span as the tuple Python reads: source start and end, output start and end.
#[derive(Debug)]
pub struct SpanTuple {
    pub source_start: usize,
    pub source_end: usize,
    pub output_start: usize,
    pub output_end: usize,
}

impl<'py> pyo3::IntoPyObject<'py> for SpanTuple {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.source_start,
                self.source_end,
                self.output_start,
                self.output_end,
            ),
            py,
        )
    }
}

/// One authored string's expansion inputs and the facts it records.
#[pyclass(module = "sqlbuild._native", unsendable)]
#[derive(Debug)]
pub struct NativeMacroExpansion {
    pub file_path: Py<PyAny>,
    pub file_label: String,
    pub loaded_macros: Py<PyDict>,
    pub overrides: Py<PyDict>,
    pub macro_context: Py<PyAny>,
    /// The visible `DeclarationResolutionContext`, or `None`.
    pub declarations: Py<PyAny>,
    /// The consumer usages are recorded for, or `None`.
    pub consumer: Py<PyAny>,
    pub(crate) enforce_explicit: bool,
    pub(crate) classes: Py<PyDict>,
    pub(crate) facts: RefCell<ExpansionFacts>,
    /// The lexical rules the authored SQL is written in, from the macro context, if it has them.
    pub syntax: Option<LexicalSyntax>,
}

/// The `(macros, overrides, context, declarations, consumer)` one expansion reads.
#[derive(Debug)]
pub struct StateInput {
    pub loaded_macros: Py<PyDict>,
    pub overrides: Py<PyDict>,
    pub macro_context: Py<PyAny>,
    pub declarations: Py<PyAny>,
    pub consumer: Py<PyAny>,
}

impl<'py> pyo3::FromPyObject<'_, 'py> for StateInput {
    type Error = pyo3::PyErr;

    fn extract(value: pyo3::Borrowed<'_, 'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let value: &pyo3::Bound<'py, pyo3::PyAny> = &value;
        let (loaded_macros, overrides, macro_context, declarations, consumer): StateTuple =
            pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            loaded_macros,
            overrides,
            macro_context,
            declarations,
            consumer,
        })
    }
}

#[pymethods]
impl NativeMacroExpansion {
    #[new]
    pub fn new(
        file_path: &Bound<'_, PyAny>,
        state: StateInput,
        enforce_explicit: bool,
        classes: Py<PyDict>,
    ) -> PyResult<Self> {
        let StateInput {
            loaded_macros,
            overrides,
            macro_context,
            declarations,
            consumer,
        } = state;
        let context: &Bound<'_, PyAny> = macro_context.bind(file_path.py());
        let syntax: Option<LexicalSyntax> = if context.is_none() {
            None
        } else {
            optional_record_syntax(&native_field(context, LEXICAL_SYNTAX_ATTRIBUTE)?)?
        };
        Ok(Self {
            syntax,
            file_path: file_path.clone().unbind(),
            file_label: file_path.str()?.to_string(),
            loaded_macros,
            overrides,
            macro_context,
            declarations,
            consumer,
            enforce_explicit,
            classes,
            facts: RefCell::new(ExpansionFacts::default()),
        })
    }

    /// Expand `sql`, through the bridge's memo when one is active.
    pub(crate) fn expand<'py>(
        slf: &Bound<'py, Self>,
        sql: &Bound<'py, PyString>,
        bridge: &Bound<'py, PyAny>,
    ) -> PyResult<ExpansionRow> {
        let output: ExpansionOutput = expanded(slf, sql.to_str()?, bridge)?;
        Ok(ExpansionRow {
            sql: output.sql,
            spans: output
                .spans
                .into_iter()
                .map(|span| span_tuple(&span_row(span)))
                .collect(),
            argument_references: output.argument_references,
            dependencies: output.dependencies,
            usages: output.usages,
        })
    }

    /// `render_relation`: an approved call-site reference's placeholder, else its reference call.
    pub(crate) fn render_relation(&self, reference: &Bound<'_, PyAny>) -> PyResult<String> {
        let py: Python<'_> = reference.py();
        let classes: ExpansionClasses<'_> = self.classes.bind(py).extract()?;
        if !reference.is_instance(&classes.resource_ref)? {
            return Err(raised(
                &classes.compile_error,
                "Only typed resource references render as relations",
            ));
        }
        let key: ResourceRef = resource_key(reference)?;
        let index: Option<usize> = self.facts.borrow_mut().relation_index(&key);
        match index {
            Some(index) => Ok(format!("{RELATION_PREFIX}{index}{RELATION_SUFFIX}")),
            None => reference_call_text(&classes, &key),
        }
    }

    /// `_record_macro_declaration_usage`: a macro read a constant or enum through its context.
    pub(crate) fn declaration_read(&self, kind: &str, name: &str, py: Python<'_>) -> PyResult<()> {
        let classes: ExpansionClasses<'_> = self.classes.bind(py).extract()?;
        self.record_declaration_read(&classes, kind, name)
    }

    /// Mark the executing call as dependent on its consumer, so it is never replayed.
    pub(crate) fn block_memo(&self) {
        self.facts.borrow_mut().block_memo();
    }
}

impl NativeMacroExpansion {
    /// Each site's call class, or `None` when the scan failed or a tree name is unresolvable.
    pub(crate) fn call_classes<'py>(
        &self,
        classes: &ExpansionClasses<'py>,
        memo: &MacroCallMemo,
        scan: &MacroCallScan,
    ) -> PyResult<Option<Vec<CallClass>>> {
        if scan.failure.is_some() {
            return Ok(None);
        }
        let py: Python<'py> = classes.compile_error.py();
        let inputs: ClassInputs<'_, 'py> = ClassInputs {
            declarations: self.declarations.bind(py),
            loaded_macros: self.loaded_macros.bind(py),
            macro_context: self.macro_context.bind(py),
        };
        let mut state = memo.classes.lock().map_err(poisoned)?;
        let mut found: Vec<CallClass> = Vec::with_capacity(scan.sites.len());
        for site in &scan.sites {
            let Some(tree_names) = &site.tree_names else {
                return Ok(None);
            };
            let Some(call_class) = state.call_class(tree_names, &inputs)? else {
                return Ok(None);
            };
            found.push(call_class);
        }
        Ok(Some(found))
    }

    /// `_bridged_call_output`: a recorded call replayed, or the call evaluated and recorded.
    pub(crate) fn bridged_output<'py>(
        &self,
        run: &Evaluation<'_, 'py>,
        bridge: (&Bound<'py, PyAny>, &MacroCallMemo),
        call: (&CallText<'_>, &MacroCallSite),
        call_class: &CallClass,
    ) -> PyResult<String> {
        let (bridge, memo) = bridge;
        let (text, site) = call;
        let classes: &ExpansionClasses<'py> = run.classes;
        let py: Python<'py> = bridge.py();
        if self.mocked(site, py)? {
            return self.evaluated_output(run, (text, site));
        }
        let call_text: &str = text.slice(site.start, site.end);
        let prior: Option<Vec<ResourceRef>> = site
            .typed_reference_text
            .then(|| self.facts.borrow().relations.clone());
        let (class_id, class_text) = memo
            .classes
            .lock()
            .map_err(poisoned)?
            .class_id(call_class, prior.as_deref());
        let recorded: Option<MacroCallEntry> = {
            let mut inner = memo.inner.lock().map_err(poisoned)?;
            if let Some(class_text) = class_text {
                inner.set_persistent_class(class_id, &class_text);
            }
            inner
                .lookup(class_id, call_text)
                .map(|entry| MacroCallEntry::clone(&entry))
        };
        if let Some(entry) = recorded {
            for event in entry.events {
                self.replay(classes, event)?;
            }
            for relation in &entry.relations {
                let _ = self.facts.borrow_mut().relation_slot(relation);
            }
            return self.render_placeholders(classes, &entry.sql);
        }
        let known: usize = self.facts.borrow().relations.len();
        self.facts.borrow_mut().begin_event_log();
        let result: PyResult<String> = top_level_output(run, (text, site));
        let (events, replayable) = self.facts.borrow_mut().end_event_log();
        let output: String = result?;
        let added: Vec<ResourceRef> = self.facts.borrow().relations[known..].to_vec();
        if replayable && (prior.is_some() || added.is_empty()) {
            observe_new_modules(bridge, memo)?;
            let entry: MacroCallEntry = MacroCallEntry {
                sql: output.clone(),
                relations: added,
                events,
            };
            memo.inner
                .lock()
                .map_err(poisoned)?
                .record(class_id, call_text.to_owned(), entry);
        }
        self.render_placeholders(classes, &output)
    }

    /// `_unmemoised_sql_macros`: every call evaluated in order, then the scan's failure raised.
    pub(crate) fn unmemoised_outputs(
        &self,
        run: &Evaluation<'_, '_>,
        text: &CallText<'_>,
        scan: &MacroCallScan,
    ) -> PyResult<Vec<String>> {
        let mut outputs: Vec<String> = Vec::with_capacity(scan.sites.len());
        for site in &scan.sites {
            outputs.push(self.evaluated_output(run, (text, site))?);
        }
        if let Some(failure) = &scan.failure {
            return Err(self.failed_scan(run.classes, (text, failure), None, run.handle.py()));
        }
        Ok(outputs)
    }

    pub(crate) fn evaluated_output(
        &self,
        run: &Evaluation<'_, '_>,
        call: (&CallText<'_>, &MacroCallSite),
    ) -> PyResult<String> {
        let output: String = top_level_output(run, call)?;
        self.render_placeholders(run.classes, &output)
    }

    /// Whether a test overrides any macro in the call's tree.
    pub(crate) fn mocked(&self, site: &MacroCallSite, py: Python<'_>) -> PyResult<bool> {
        let overrides: &Bound<'_, PyDict> = self.overrides.bind(py);
        for name in site.tree_names.iter().flatten() {
            if overrides.contains(name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `raise_failed_macro_call`: the lookup error, the unreadable call located, or the scan's error.
    pub(crate) fn failed_scan(
        &self,
        classes: &ExpansionClasses<'_>,
        (text, failure): (&CallText<'_>, &MacroScanFailure),
        enclosing: Option<&str>,
        py: Python<'_>,
    ) -> PyErr {
        let name: &str = failure.call_name.as_deref().unwrap_or_default();
        match failure.error {
            ScanError::BareCall => {
                return coded(
                    &classes.compile_error,
                    &format!("macro call @{name}() does not say which kind of macro it calls"),
                    (
                        BARE_MACRO_CALL_CODE,
                        &format!(
                            "write @sql.{name}(...) to call a SQL macro or @py.{name}(...) to call a Python macro"
                        ),
                    ),
                );
            }
            ScanError::SqlCall => {
                return coded(
                    &classes.compile_error,
                    &format!(
                        "SQL macro call @sql.{name}() is not expanded in '{}'",
                        self.file_label
                    ),
                    (
                        MIXED_KIND_CODE,
                        "SQL macro calls are expanded in authored SQL, not in Python macro output",
                    ),
                );
            }
            _ => {}
        }
        if let Some(name) = &failure.call_name
            && let Err(error) = self.resolved_macro(classes, name, py)
        {
            return error;
        }
        let written: String = written_call(name);
        if let (Some(start), Some(detail)) = (
            failure.call_start,
            unread_call_detail(failure.error, &written),
        ) {
            let labelled: &str = match enclosing {
                Some(enclosing) => enclosing,
                None if DECLARATION_FORMS.contains(&name) => &written,
                None => name,
            };
            return macro_argument_error(
                (self, classes),
                text,
                (labelled, start),
                (&detail, UNREAD_CALL_HELP),
            );
        }
        raised(&classes.compile_error, failure.error.message())
    }

    /// `_resolved_macro`: the visible macro `name`, or Python's inaccessible or unknown error.
    pub(crate) fn resolved_macro<'py>(
        &self,
        classes: &ExpansionClasses<'py>,
        name: &str,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let declarations: &Bound<'py, PyAny> = self.declarations.bind(py);
        let macros: Bound<'py, PyAny> = if declarations.is_none() {
            self.loaded_macros.bind(py).clone().into_any()
        } else {
            native_field(declarations, "macros")?
        };
        if !declarations.is_none() {
            let inaccessible: Bound<'py, PyAny> =
                native_field(declarations, "inaccessible_macros")?;
            if inaccessible.contains(name)? {
                let record: Bound<'py, PyAny> = inaccessible.get_item(name)?;
                let owning: Bound<'py, PyAny> = native_field(&record, "owning_path")?;
                let owner: String = if owning.is_truthy()? {
                    owning.str()?.to_string()
                } else {
                    "global".to_owned()
                };
                let message: String = format!(
                    "Macro '@py.{name}' in '{}' is inaccessible. Defined at '{}:{}:{}' with scope \
                     owner '{owner}'; consumer/definition path is '{}'. Visible macros: {}",
                    self.file_label,
                    native_field(&record, "path")?.str()?,
                    native_field(&record, "line")?.str()?,
                    native_field(&record, "column")?.str()?,
                    self.file_label,
                    sorted_names(&macros)?,
                );
                return Err(raised(&classes.compile_error, &message));
            }
        }
        if macros.contains(name)? {
            let found: Bound<'py, PyAny> = macros.get_item(name)?;
            if field_text(&found, "language")? == SQL_LANGUAGE {
                return Err(coded(
                    &classes.compile_error,
                    &format!("{name}() is a SQL macro, called as a Python macro with @py.{name}()"),
                    (MIXED_KIND_CODE, &format!("call it as @sql.{name}(...)")),
                ));
            }
            return Ok(found);
        }
        let message: String = format!(
            "Unknown macro '@py.{name}' in '{}'. Available macros: {}",
            self.file_label,
            sorted_names(&macros)?
        );
        Err(raised(&classes.compile_error, &message))
    }

    /// `_record_macro_use`: the macro's dependency and its runtime crate::render::_helpers::expansion::macro_expansion::usage by the consumer.
    pub(crate) fn record_macro_use<'py>(
        &self,
        classes: &ExpansionClasses<'py>,
        name: &str,
        identity: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py: Python<'py> = identity.py();
        {
            let mut facts = self.facts.borrow_mut();
            facts.note_event(MacroCallEvent::MacroUse {
                name: name.to_owned(),
            });
            facts.dependencies.push(identity.clone().unbind());
        }
        let consumer: &Bound<'py, PyAny> = self.consumer.bind(py);
        if consumer.is_none() {
            return Ok(());
        }
        let declarations: &Bound<'py, PyAny> = self.declarations.bind(py);
        let visibility: Bound<'py, PyAny> = if declarations.is_none() {
            PyTuple::empty(py).into_any()
        } else {
            native_field(declarations, "macro_visibility")?
                .call_method1("get", (name, PyTuple::empty(py)))?
        };
        let usages: Vec<Bound<'py, PyAny>> = if visibility.is_truthy()? {
            let mut usages: Vec<Bound<'py, PyAny>> = Vec::new();
            for visible in usage_visibility(&visibility, relationship_consumer(classes, consumer)?)?
            {
                usages.push(crate::render::_helpers::expansion::macro_expansion::usage(
                    classes,
                    (consumer, identity),
                    Some(&native_field(&visible, "through")?),
                )?);
            }
            usages
        } else {
            vec![crate::render::_helpers::expansion::macro_expansion::usage(
                classes,
                (consumer, identity),
                None,
            )?]
        };
        self.facts
            .borrow_mut()
            .usages
            .extend(usages.into_iter().map(Bound::unbind));
        Ok(())
    }

    /// `_record_macro_declaration_usage` for a constant or enum the macro read.
    pub(crate) fn record_declaration_read(
        &self,
        classes: &ExpansionClasses<'_>,
        kind: &str,
        name: &str,
    ) -> PyResult<()> {
        let py: Python<'_> = classes.compile_error.py();
        self.facts
            .borrow_mut()
            .note_event(MacroCallEvent::DeclarationRead {
                kind: kind.to_owned(),
                name: name.to_owned(),
            });
        let declarations: &Bound<'_, PyAny> = self.declarations.bind(py);
        let own_consumer: &Bound<'_, PyAny> = self.consumer.bind(py);
        let consumer: Bound<'_, PyAny> = if own_consumer.is_truthy()? {
            own_consumer.clone()
        } else {
            native_field(declarations, "consumer")?
        };
        if consumer.is_none() {
            return Ok(());
        }
        let constant: bool = kind == CONSTANT_KIND;
        let visibility: Bound<'_, PyAny> = declarations
            .getattr(if constant {
                "constant_visibility"
            } else {
                "enum_visibility"
            })?
            .call_method1("get", (name, PyTuple::empty(py)))?;
        let mut usages: Vec<Bound<'_, PyAny>> = Vec::new();
        if visibility.is_truthy()? {
            for visible in
                usage_visibility(&visibility, relationship_consumer(classes, &consumer)?)?
            {
                usages.push(crate::render::_helpers::expansion::macro_expansion::usage(
                    classes,
                    (&consumer, &native_field(&visible, "declaration")?),
                    Some(&native_field(&visible, "through")?),
                )?);
            }
        } else {
            let declaration: Bound<'_, PyAny> = declarations
                .getattr(if constant { "constants" } else { "enums" })?
                .get_item(name)?;
            let model_name: Bound<'_, PyAny> = native_field(&declaration, "model_name")?;
            let owner: Option<Bound<'_, PyAny>> = if model_name.is_none() {
                None
            } else {
                let fields: Bound<'_, PyDict> = PyDict::new(py);
                fields.set_item("kind", &classes.model_kind)?;
                fields.set_item("name", model_name)?;
                Some(classes.resource_identity.call((), Some(&fields))?)
            };
            let fields: Bound<'_, PyDict> = PyDict::new(py);
            fields.set_item("kind", classes.declaration_kind.call1((kind,))?)?;
            fields.set_item("name", name)?;
            fields.set_item("owner", owner)?;
            let identity: Bound<'_, PyAny> =
                classes.declaration_identity.call((), Some(&fields))?;
            usages.push(crate::render::_helpers::expansion::macro_expansion::usage(
                classes,
                (&consumer, &identity),
                None,
            )?);
        }
        self.facts
            .borrow_mut()
            .usages
            .extend(usages.into_iter().map(Bound::unbind));
        Ok(())
    }

    /// `_report_generated_references` for a macro's returned SQL.
    pub(crate) fn report_generated(
        &self,
        classes: &ExpansionClasses<'_>,
        name: &str,
        loaded_macro: &Bound<'_, PyAny>,
        result: &str,
    ) -> PyResult<()> {
        if result_has_markers(classes, result) {
            self.facts
                .borrow_mut()
                .note_event(MacroCallEvent::GeneratedSql {
                    macro_name: name.to_owned(),
                    sql: result.to_owned(),
                });
        }
        let py: Python<'_> = loaded_macro.py();
        let origin: GeneratedOrigin<'_, '_> = GeneratedOrigin {
            file_path: self.file_path.bind(py),
            consumer: self.consumer.bind(py),
            enforce_explicit: self.enforce_explicit,
        };
        report_generated_references(&classes.generated, &origin, loaded_macro, result)
    }

    /// `_replay_macro_call_event`: one recorded fact, recorded again for this consumer.
    pub(crate) fn replay(
        &self,
        classes: &ExpansionClasses<'_>,
        event: MacroCallEvent,
    ) -> PyResult<()> {
        let py: Python<'_> = classes.compile_error.py();
        match event {
            MacroCallEvent::MacroUse { name } => {
                let identity: Bound<'_, PyAny> = self.macro_identity(classes, &name, py)?;
                self.record_macro_use(classes, &name, &identity)
            }
            MacroCallEvent::DeclarationRead { kind, name } => {
                if self.declarations.bind(py).is_none() {
                    return Err(raised(
                        &classes.mismatch_error,
                        "Replayed a declaration read without declarations",
                    ));
                }
                self.record_declaration_read(classes, &kind, &name)
            }
            MacroCallEvent::GeneratedSql { macro_name, sql } => {
                let declarations: &Bound<'_, PyAny> = self.declarations.bind(py);
                let loaded_macro: Bound<'_, PyAny> = if declarations.is_none() {
                    self.loaded_macros
                        .bind(py)
                        .get_item(&macro_name)?
                        .ok_or_else(|| {
                            PyErr::new::<pyo3::exceptions::PyKeyError, _>(macro_name.clone())
                        })?
                } else {
                    native_field(declarations, "macros")?.get_item(&macro_name)?
                };
                self.report_generated(classes, &macro_name, &loaded_macro, &sql)
            }
            MacroCallEvent::ArgumentReference { kind, name } => {
                self.facts
                    .borrow_mut()
                    .record_call_site_refs(&[(kind, name)]);
                Ok(())
            }
        }
    }

    /// The macro's declaration identity: its indexed record's, else an unowned macro identity.
    pub(crate) fn macro_identity<'py>(
        &self,
        classes: &ExpansionClasses<'py>,
        name: &str,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let declarations: &Bound<'py, PyAny> = self.declarations.bind(py);
        if !declarations.is_none() {
            return native_field(declarations, "macro_records")?
                .get_item(name)?
                .getattr("identity");
        }
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item("kind", &classes.macro_kind)?;
        fields.set_item("name", name)?;
        classes.declaration_identity.call((), Some(&fields))
    }

    /// `render_relation_placeholders`: each placeholder as the reference call it stands for.
    pub(crate) fn render_placeholders(
        &self,
        classes: &ExpansionClasses<'_>,
        sql: &str,
    ) -> PyResult<String> {
        let relations: Vec<ResourceRef> = self.facts.borrow().relations.clone();
        if relations.is_empty() || !sql.contains(RELATION_PREFIX) {
            return Ok(sql.to_owned());
        }
        let mut rendered: String = String::with_capacity(sql.len());
        let mut cursor: usize = 0;
        let mut search: usize = 0;
        while let Some(found) = sql[search..].find(RELATION_PREFIX) {
            let start: usize = search + found;
            let digits_start: usize = start + RELATION_PREFIX.len();
            let digits_end: usize = sql[digits_start..]
                .char_indices()
                .find(|(_, character)| !is_decimal_digit(*character))
                .map_or(sql.len(), |(offset, _)| digits_start + offset);
            if digits_end == digits_start || !sql[digits_end..].starts_with(RELATION_SUFFIX) {
                search = start + 1;
                continue;
            }
            let index: usize = classes
                .int_type
                .call1((&sql[digits_start..digits_end],))?
                .extract()?;
            let relation: &ResourceRef = relations
                .get(index)
                .ok_or_else(|| PyIndexError::new_err("tuple index out of range"))?;
            rendered.push_str(&sql[cursor..start]);
            rendered.push_str(&reference_call_text(classes, relation)?);
            cursor = digits_end + RELATION_SUFFIX.len();
            search = cursor;
        }
        rendered.push_str(&sql[cursor..]);
        Ok(rendered)
    }
}

/// One expansion's SQL, substitution spans, call-site references, dependencies and usages.
#[derive(Debug)]
pub struct ExpansionOutput {
    pub sql: String,
    pub spans: Vec<MacroSpliceSpan>,
    pub argument_references: Vec<ResourceRef>,
    pub dependencies: Vec<Py<PyAny>>,
    pub usages: Vec<Py<PyAny>>,
}

/// One expansion's inputs, as Python's `expand_sql_macros_result` takes them.
#[derive(Debug)]
pub struct ExpansionRequest<'py> {
    pub sql: Bound<'py, PyString>,
    pub file_path: Bound<'py, PyAny>,
    pub loaded_macros: Bound<'py, PyDict>,
    pub macro_overrides: Option<Bound<'py, PyDict>>,
    pub macro_context: Bound<'py, PyAny>,
    pub declaration_resolver: Option<Bound<'py, PyAny>>,
    pub declarations: Option<Bound<'py, PyAny>>,
    pub consumer: Option<Bound<'py, PyAny>>,
}

/// Where the caller's text came from, to locate calls in its authored file.
#[derive(Debug, Clone, Copy)]
pub struct SqlCallOrigin<'a> {
    /// The authored file's project-relative path.
    pub relative_path: &'a str,
    /// The authored file's contents.
    pub contents: &'a str,
    /// The authored query the expanded text came from, within `contents`.
    pub query: &'a str,
}

/// What SQL macro calls in one declaration pass read beyond the declaration context.
#[derive(Clone, Copy)]
pub struct SqlMacroSource<'a, 'py> {
    /// Every loaded macro by name, for unscoped compiles whose context holds none.
    pub loaded_macros: Option<&'a Bound<'py, PyAny>>,
    pub origin: Option<SqlCallOrigin<'a>>,
    /// Where macro bodies resolve their declarations; `None` outside the input loops.
    pub scopes: Option<&'a dyn MacroScopes<'py>>,
    /// The lexical rules the authored SQL is written in; `None` reads it under the generic rules.
    pub syntax: Option<&'a LexicalSyntax>,
}

impl std::fmt::Debug for SqlMacroSource<'_, '_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqlMacroSource")
            .finish_non_exhaustive()
    }
}
