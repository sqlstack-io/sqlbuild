//! One authored string's macro expansion: scanned, memoised, evaluated and spliced natively.

use std::cell::RefCell;

use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyString, PyStringMethods, PyTuple};
use pyo3::{FromPyObject, pyclass, pymethods};
use sqlbuild_analysis::sql_macros::constants::{BARE_MACRO_CALL_CODE, MIXED_KIND_CODE};
use sqlbuild_core::text::main::is_decimal_digit::is_decimal_digit;
use sqlbuild_render::macro_calls::errors::ScanError;
use sqlbuild_render::macro_calls::main::scan_macro_call_sites::scan_macro_call_sites as scan_sites;
use sqlbuild_render::macro_calls::main::splice_macro_calls::splice_macro_calls as splice_calls;
use sqlbuild_render::macro_calls::models::{
    MacroCallEntry, MacroCallEvent, MacroCallScan, MacroCallSite, MacroScanFailure, MacroSpliceSpan,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::records::construct::new_helped_error;
use crate::bindings::_helpers::records::fields::{field as native_field, field_text};
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::_helpers::render::declaration_expansion::usage_visibility;
use crate::bindings::_helpers::render::macro_call_classes::{CallClass, ClassInputs};
use crate::bindings::_helpers::render::macro_call_memo::{MacroCallMemo, poisoned};
use crate::bindings::_helpers::render::macro_call_rows::{SpanTuple, span_row, span_tuple};
use crate::bindings::_helpers::render::macro_evaluation::{
    CallText, Evaluation, evaluate_call, macro_argument_error,
};
use crate::bindings::_helpers::render::macro_expansion_facts::{ExpansionFacts, ResourceRef};
use crate::bindings::_helpers::render::macro_generated_references::{
    GeneratedOrigin, GeneratedReferenceClasses, report_generated_references,
};
use crate::bindings::_helpers::sqltext::lexical_syntax::optional_record_syntax;

const RELATION_PREFIX: &str = "__sqlbuild_relation_";
const MEMO_ATTRIBUTE: &str = "memo";
const LEXICAL_SYNTAX_ATTRIBUTE: &str = "_lexical_syntax";
const OBSERVE_MODULES_METHOD: &str = "observe_modules";
const RELATION_SUFFIX: &str = "__";
const CONSTANT_KIND: &str = "constant";
const RELATIONSHIP_CONSUMER_KINDS: [&str; 2] = ["test", "scenario"];
const SQL_LANGUAGE: &str = "sql";
const DECLARATION_FORMS: [&str; 3] = ["const", "enum", "param"];
const UNREAD_CALL_HELP: &str = "Close every quote, block comment and parenthesis in the macro \
                                call, and follow its name with its opening parenthesis";

/// Why the scan could not read the call `@{called}`, if the scan stopped inside a call.
fn unread_call_detail(error: ScanError, called: &str) -> Option<String> {
    let reason: String = match error {
        ScanError::UnclosedQuote => format!("a quoted string in {called} is not closed"),
        ScanError::UnclosedComment => format!("a block comment in {called} is not closed"),
        ScanError::UnclosedParenthesis => format!("{called} is not closed with ')'"),
        ScanError::MissingParenthesis => {
            format!("{called} is not followed by its opening parenthesis")
        }
        ScanError::BareCall | ScanError::SqlCall => return None,
    };
    Some(format!("could not be read: {reason}"))
}

/// How a scanned call is written: `@py.name`, or `@const` and the other declaration forms.
fn written_call(name: &str) -> String {
    if DECLARATION_FORMS.contains(&name) {
        format!("@{name}")
    } else {
        format!("@py.{name}")
    }
}

positional_record! {
    into;
    /// `(sql, spans, argument references, dependencies, usages)` of one expansion.
    struct ExpansionRow {
        sql: String,
        spans: Vec<SpanTuple>,
        argument_references: Vec<ResourceRef>,
        dependencies: Vec<Py<PyAny>>,
        usages: Vec<Py<PyAny>>,
    }
}

/// The Python classes, enums and helpers an expansion builds facts, errors and contexts from.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ExpansionClasses<'py> {
    pub(crate) compile_error: Bound<'py, PyAny>,
    pub(crate) macro_argument_error: Bound<'py, PyAny>,
    mismatch_error: Bound<'py, PyAny>,
    declaration_identity: Bound<'py, PyAny>,
    declaration_kind: Bound<'py, PyAny>,
    macro_kind: Bound<'py, PyAny>,
    usage_record: Bound<'py, PyAny>,
    runtime_usage: Bound<'py, PyAny>,
    resource_identity: Bound<'py, PyAny>,
    model_kind: Bound<'py, PyAny>,
    pub(crate) resource_ref: Bound<'py, PyAny>,
    /// `SqlResourceRefKind` member by typed reference function, such as `__ref`.
    pub(crate) ref_kinds: Bound<'py, PyDict>,
    /// Typed reference function by `SqlResourceRefKind` value.
    ref_functions: Bound<'py, PyDict>,
    pub(crate) relation_renderer: Bound<'py, PyAny>,
    /// Builds the `MacroContext` a macro is called with from the visible declarations.
    pub(crate) invocation_context: Bound<'py, PyAny>,
    /// What reference calls a macro returned are reported with.
    generated: GeneratedReferenceClasses<'py>,
    pub(crate) int_type: Bound<'py, PyAny>,
    pub(crate) float_type: Bound<'py, PyAny>,
}

/// One authored string's expansion inputs and the facts it records.
#[pyclass(module = "sqlbuild._native", unsendable)]
pub(crate) struct NativeMacroExpansion {
    pub(crate) file_path: Py<PyAny>,
    pub(crate) file_label: String,
    pub(crate) loaded_macros: Py<PyDict>,
    pub(crate) overrides: Py<PyDict>,
    pub(crate) macro_context: Py<PyAny>,
    /// The visible `DeclarationResolutionContext`, or `None`.
    pub(crate) declarations: Py<PyAny>,
    /// The consumer usages are recorded for, or `None`.
    pub(crate) consumer: Py<PyAny>,
    enforce_explicit: bool,
    classes: Py<PyDict>,
    pub(crate) facts: RefCell<ExpansionFacts>,
    /// The lexical rules the authored SQL is written in, from the macro context, if it has them.
    pub(crate) syntax: Option<LexicalSyntax>,
}

positional_record! {
    extract;
    /// The `(macros, overrides, context, declarations, consumer)` one expansion reads.
    pub(crate) struct StateInput {
        pub(crate) loaded_macros: Py<PyDict>,
        pub(crate) overrides: Py<PyDict>,
        pub(crate) macro_context: Py<PyAny>,
        pub(crate) declarations: Py<PyAny>,
        pub(crate) consumer: Py<PyAny>,
    }
}

#[pymethods]
impl NativeMacroExpansion {
    #[new]
    pub(crate) fn new(
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
    fn expand<'py>(
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
    fn render_relation(&self, reference: &Bound<'_, PyAny>) -> PyResult<String> {
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
    fn declaration_read(&self, kind: &str, name: &str, py: Python<'_>) -> PyResult<()> {
        let classes: ExpansionClasses<'_> = self.classes.bind(py).extract()?;
        self.record_declaration_read(&classes, kind, name)
    }

    /// Mark the executing call as dependent on its consumer, so it is never replayed.
    fn block_memo(&self) {
        self.facts.borrow_mut().block_memo();
    }
}

impl NativeMacroExpansion {
    /// Each site's call class, or `None` when the scan failed or a tree name is unresolvable.
    fn call_classes<'py>(
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
    fn bridged_output<'py>(
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
    fn unmemoised_outputs(
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

    fn evaluated_output(
        &self,
        run: &Evaluation<'_, '_>,
        call: (&CallText<'_>, &MacroCallSite),
    ) -> PyResult<String> {
        let output: String = top_level_output(run, call)?;
        self.render_placeholders(run.classes, &output)
    }

    /// Whether a test overrides any macro in the call's tree.
    fn mocked(&self, site: &MacroCallSite, py: Python<'_>) -> PyResult<bool> {
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

    /// `_record_macro_use`: the macro's dependency and its runtime usage by the consumer.
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
                usages.push(usage(
                    classes,
                    (consumer, identity),
                    Some(&native_field(&visible, "through")?),
                )?);
            }
            usages
        } else {
            vec![usage(classes, (consumer, identity), None)?]
        };
        self.facts
            .borrow_mut()
            .usages
            .extend(usages.into_iter().map(Bound::unbind));
        Ok(())
    }

    /// `_record_macro_declaration_usage` for a constant or enum the macro read.
    fn record_declaration_read(
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
                usages.push(usage(
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
            usages.push(usage(classes, (&consumer, &identity), None)?);
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
    fn replay(&self, classes: &ExpansionClasses<'_>, event: MacroCallEvent) -> PyResult<()> {
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
    fn render_placeholders(&self, classes: &ExpansionClasses<'_>, sql: &str) -> PyResult<String> {
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
pub(crate) struct ExpansionOutput {
    pub(crate) sql: String,
    pub(crate) spans: Vec<MacroSpliceSpan>,
    pub(crate) argument_references: Vec<ResourceRef>,
    pub(crate) dependencies: Vec<Py<PyAny>>,
    pub(crate) usages: Vec<Py<PyAny>>,
}

/// Expand `sql` with the expansion's inputs, through the bridge's memo when one is active.
pub(crate) fn expanded<'py>(
    expansion: &Bound<'py, NativeMacroExpansion>,
    sql: &str,
    bridge: &Bound<'py, PyAny>,
) -> PyResult<ExpansionOutput> {
    let py: Python<'py> = expansion.py();
    let this: PyRef<'py, NativeMacroExpansion> = expansion.borrow();
    let classes: ExpansionClasses<'py> = this.classes.bind(py).extract()?;
    let handle: Bound<'py, PyAny> = expansion.as_any().clone();
    let run: Evaluation<'_, 'py> = Evaluation {
        expansion: &this,
        handle: &handle,
        classes: &classes,
    };
    let text: CallText<'_> = CallText::new(sql);
    let scan: MacroCallScan = scan_sites(text.text, this.syntax.as_ref());
    let memo: Option<Bound<'py, MacroCallMemo>> = if bridge.is_none() {
        None
    } else {
        Some(bridge.getattr(MEMO_ATTRIBUTE)?.downcast_into()?)
    };
    let call_classes: Option<Vec<CallClass>> = match &memo {
        Some(memo) => this.call_classes(&classes, memo.get(), &scan)?,
        None => None,
    };
    let outputs: Vec<String> = match (call_classes, &memo) {
        (Some(call_classes), Some(memo)) => {
            let mut outputs: Vec<String> = Vec::with_capacity(scan.sites.len());
            for (site, call_class) in scan.sites.iter().zip(call_classes) {
                outputs.push(this.bridged_output(
                    &run,
                    (bridge, memo.get()),
                    (&text, site),
                    &call_class,
                )?);
            }
            outputs
        }
        _ => this.unmemoised_outputs(&run, &text, &scan)?,
    };
    let bounds: Vec<(usize, usize)> = scan
        .sites
        .iter()
        .map(|site| (site.start, site.end))
        .collect();
    let (rendered, spans) = splice_calls(text.text, &bounds, &outputs)
        .ok_or_else(|| PyValueError::new_err("macro call sites do not fit the SQL"))?;
    let facts = this.facts.borrow();
    Ok(ExpansionOutput {
        sql: rendered,
        spans,
        argument_references: facts.argument_references.clone(),
        dependencies: facts
            .dependencies
            .iter()
            .map(|item| item.clone_ref(py))
            .collect(),
        usages: facts.usages.iter().map(|item| item.clone_ref(py)).collect(),
    })
}

/// One top-level call's SQL.
fn top_level_output(
    run: &Evaluation<'_, '_>,
    call: (&CallText<'_>, &MacroCallSite),
) -> PyResult<String> {
    evaluate_call(run, call, true)?.extract()
}

/// A `SqlResourceRef`'s kind value and name.
pub(crate) fn resource_key(reference: &Bound<'_, PyAny>) -> PyResult<ResourceRef> {
    Ok((
        native_field(&native_field(reference, "kind")?, "value")?.extract()?,
        native_field(reference, "name")?.extract()?,
    ))
}

/// `reference_call_text`: the reference call naming `reference`, such as `__ref("orders")`.
fn reference_call_text(
    classes: &ExpansionClasses<'_>,
    reference: &ResourceRef,
) -> PyResult<String> {
    let (kind, name) = reference;
    let function: String = classes
        .ref_functions
        .get_item(kind)?
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyKeyError, _>(kind.clone()))?
        .extract()?;
    Ok(format!("{function}(\"{name}\")"))
}

/// Whether the consumer is a test or scenario, which records relationship visibility.
fn relationship_consumer(
    classes: &ExpansionClasses<'_>,
    consumer: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if !consumer.is_instance(&classes.resource_identity)? {
        return Ok(false);
    }
    let kind: String = native_field(&native_field(consumer, "kind")?, "value")?.extract()?;
    Ok(RELATIONSHIP_CONSUMER_KINDS.contains(&kind.as_str()))
}

/// A runtime `UsageRecord` of `declaration` by `consumer`, through a relationship if any.
fn usage<'py>(
    classes: &ExpansionClasses<'py>,
    usage: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    through: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let (consumer, declaration) = usage;
    let keywords: Bound<'py, PyDict> = PyDict::new(consumer.py());
    if let Some(through) = through {
        keywords.set_item("through", through)?;
    }
    classes.usage_record.call(
        (consumer, declaration, &classes.runtime_usage),
        Some(&keywords),
    )
}

/// `", ".join(sorted(names)) or "none"`.
fn sorted_names(names: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut sorted: Vec<String> =
        PyList::new(names.py(), names.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.extract()?;
    sorted.sort_unstable();
    Ok(if sorted.is_empty() {
        "none".to_owned()
    } else {
        sorted.join(", ")
    })
}

/// The `CompileInputError(message, code=code, help=help)` the classes' compile error builds.
fn coded(class: &Bound<'_, PyAny>, message: &str, (code, help): (&str, &str)) -> PyErr {
    new_helped_error(
        class.py(),
        ErrorId::CompileInput,
        (message.to_owned(), code),
        Some(help),
    )
}

/// The exception `class(message)` builds.
pub(crate) fn raised(class: &Bound<'_, PyAny>, message: &str) -> PyErr {
    match class.call1((message,)) {
        Ok(instance) => PyErr::from_value(instance),
        Err(error) => error,
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeMacroExpansion>()?;
    Ok(())
}

/// Whether a macro's result calls any reference function.
fn result_has_markers(classes: &ExpansionClasses<'_>, result: &str) -> bool {
    classes
        .generated
        .markers()
        .any(|marker| result.contains(marker))
}

/// Observe modules a recorded call imported, so the store is not saved over changed code.
fn observe_new_modules(bridge: &Bound<'_, PyAny>, memo: &MacroCallMemo) -> PyResult<()> {
    let Some(observed) = *memo.observed_modules.lock().map_err(poisoned)? else {
        return Ok(());
    };
    let loaded: usize = PyModule::import(bridge.py(), "sys")?
        .getattr("modules")?
        .len()?;
    if loaded != observed {
        let _ = bridge.call_method0(OBSERVE_MODULES_METHOD)?;
    }
    Ok(())
}
