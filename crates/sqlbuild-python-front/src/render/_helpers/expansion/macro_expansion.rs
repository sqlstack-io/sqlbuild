//! One authored string's macro expansion: scanned, memoised, evaluated and spliced natively.

use crate::render::models::ExpansionOutput;
use crate::render::models::NativeMacroExpansion;
use sqlbuild_python_base::positional_record;

use pyo3::FromPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyRef, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList};
use sqlbuild_render::macro_calls::errors::ScanError;
use sqlbuild_render::macro_calls::main::scan_macro_call_sites::scan_macro_call_sites as scan_sites;
use sqlbuild_render::macro_calls::main::splice_macro_calls::splice_macro_calls as splice_calls;
use sqlbuild_render::macro_calls::models::{MacroCallScan, MacroCallSite};

use crate::render::_helpers::calls::macro_call_classes::CallClass;
use crate::render::_helpers::calls::macro_call_memo::{MacroCallMemo, poisoned};
use crate::render::_helpers::expansion::macro_evaluation::{CallText, Evaluation, evaluate_call};
use crate::render::_helpers::expansion::macro_expansion_facts::ResourceRef;
use crate::render::_helpers::expansion::macro_generated_references::GeneratedReferenceClasses;
use crate::render::models::SpanTuple;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

pub(crate) const RELATION_PREFIX: &str = "__sqlbuild_relation_";
const MEMO_ATTRIBUTE: &str = "memo";
pub(crate) const LEXICAL_SYNTAX_ATTRIBUTE: &str = "_lexical_syntax";
const OBSERVE_MODULES_METHOD: &str = "observe_modules";
pub(crate) const RELATION_SUFFIX: &str = "__";
pub(crate) const CONSTANT_KIND: &str = "constant";
const RELATIONSHIP_CONSUMER_KINDS: [&str; 2] = ["test", "scenario"];
pub(crate) const SQL_LANGUAGE: &str = "sql";
pub(crate) const DECLARATION_FORMS: [&str; 3] = ["const", "enum", "param"];
pub(crate) const UNREAD_CALL_HELP: &str = "Close every quote, block comment and parenthesis in the macro \
                                call, and follow its name with its opening parenthesis";

/// Why the scan could not read the call `@{called}`, if the scan stopped inside a call.
pub(crate) fn unread_call_detail(error: ScanError, called: &str) -> Option<String> {
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
pub(crate) fn written_call(name: &str) -> String {
    if DECLARATION_FORMS.contains(&name) {
        format!("@{name}")
    } else {
        format!("@py.{name}")
    }
}

positional_record! {
    into;
    /// `(sql, spans, argument references, dependencies, usages)` of one expansion.
    pub(crate) struct ExpansionRow {
        pub(crate) sql: String,
        pub(crate) spans: Vec<SpanTuple>,
        pub(crate) argument_references: Vec<ResourceRef>,
        pub(crate) dependencies: Vec<Py<PyAny>>,
        pub(crate) usages: Vec<Py<PyAny>>,
    }
}

/// The Python classes, enums and helpers an expansion builds facts, errors and contexts from.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct ExpansionClasses<'py> {
    pub(crate) compile_error: Bound<'py, PyAny>,
    pub(crate) macro_argument_error: Bound<'py, PyAny>,
    pub(crate) mismatch_error: Bound<'py, PyAny>,
    pub(crate) declaration_identity: Bound<'py, PyAny>,
    pub(crate) declaration_kind: Bound<'py, PyAny>,
    pub(crate) macro_kind: Bound<'py, PyAny>,
    pub(crate) usage_record: Bound<'py, PyAny>,
    pub(crate) runtime_usage: Bound<'py, PyAny>,
    pub(crate) resource_identity: Bound<'py, PyAny>,
    pub(crate) model_kind: Bound<'py, PyAny>,
    pub(crate) resource_ref: Bound<'py, PyAny>,
    /// `SqlResourceRefKind` member by typed reference function, such as `__ref`.
    pub(crate) ref_kinds: Bound<'py, PyDict>,
    /// Typed reference function by `SqlResourceRefKind` value.
    pub(crate) ref_functions: Bound<'py, PyDict>,
    pub(crate) relation_renderer: Bound<'py, PyAny>,
    /// Builds the `MacroContext` a macro is called with from the visible declarations.
    pub(crate) invocation_context: Bound<'py, PyAny>,
    /// What reference calls a macro returned are reported with.
    pub(crate) generated: GeneratedReferenceClasses<'py>,
    pub(crate) int_type: Bound<'py, PyAny>,
    pub(crate) float_type: Bound<'py, PyAny>,
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
    let (rendered, spans) = splice_calls(text.text, &bounds, &outputs, this.syntax.as_ref())
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
pub(crate) fn top_level_output(
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
pub(crate) fn reference_call_text(
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
pub(crate) fn relationship_consumer(
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
pub(crate) fn usage<'py>(
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
pub(crate) fn sorted_names(names: &Bound<'_, PyAny>) -> PyResult<String> {
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
pub(crate) fn coded(class: &Bound<'_, PyAny>, message: &str, (code, help): (&str, &str)) -> PyErr {
    new_error(
        class.py(),
        ErrorId::CompileInput,
        message,
        (Some(code), Some(help)),
    )
}

/// The exception `class(message)` builds.
pub(crate) fn raised(class: &Bound<'_, PyAny>, message: &str) -> PyErr {
    match class.call1((message,)) {
        Ok(instance) => PyErr::from_value(instance),
        Err(error) => error,
    }
}

/// Whether a macro's result calls any reference function.
pub(crate) fn result_has_markers(classes: &ExpansionClasses<'_>, result: &str) -> bool {
    classes
        .generated
        .markers()
        .any(|marker| result.contains(marker))
}

/// Observe modules a recorded call imported, so the store is not saved over changed code.
pub(crate) fn observe_new_modules(bridge: &Bound<'_, PyAny>, memo: &MacroCallMemo) -> PyResult<()> {
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
