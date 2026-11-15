//! A model's Python lifecycle hooks: checked against their functions' signatures, and their usages.

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};

use crate::bindings::_helpers::input_loops::model_loop::{LoopContext, compile_error};
use crate::bindings::_helpers::input_loops::model_loop_named::{
    PYTHON_HOOK_DECLARATION, named_usages,
};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Hook keys in the sorted order `validate_python_hook_config` reads them.
const SORTED_HOOK_KEYS: [&str; 2] = ["post_hooks", "pre_hooks"];
/// Parameters SQLBuild injects with the hook context.
const CONTEXT_PARAMETERS: [&str; 4] = ["ctx", "context", "_ctx", "hook_context"];
const NAME_FIELD: &str = "name";
const KWARGS_FIELD: &str = "kwargs";
const FUNCTION_FIELD: &str = "function";
const POSITIONAL_ONLY: u8 = 0;
const POSITIONAL_OR_KEYWORD: u8 = 1;
const KEYWORD_ONLY: u8 = 3;
const VAR_KEYWORD: u8 = 4;

/// One parameter as `inspect.signature` reports it: name, `Parameter.kind`, and a default.
#[derive(Clone, FromPyObject)]
struct HookParameter(String, u8, bool);

/// The discovered hook functions, provider names, `PythonHookEntry`, and signature reader.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct HookClasses<'py> {
    /// Discovered hook functions by name.
    functions: Bound<'py, PyDict>,
    provider_names: Vec<String>,
    python_hook_entry: Bound<'py, PyAny>,
    /// Reads a hook function's parameters with `inspect.signature`.
    signature: Bound<'py, PyAny>,
}

/// Each hook function's parameters, read once per compile.
#[derive(Default)]
pub(crate) struct HookSignatures {
    by_name: RefCell<HashMap<String, Vec<HookParameter>>>,
}

/// One Python hook entry where it was authored.
struct HookSite<'a> {
    model_name: &'a str,
    hook_key: &'a str,
    index: usize,
    name: &'a str,
}

impl HookSignatures {
    /// `_checked_python_hooks`: validate each Python hook, then return their usages in order.
    pub(crate) fn checked<'py>(
        &self,
        context: &LoopContext<'_, 'py>,
        values: &Bound<'py, PyDict>,
        consumer: &Bound<'py, PyAny>,
        model: (&str, &str),
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let (model_name, _) = model;
        let entries: Vec<(&str, usize, Bound<'py, PyAny>)> = python_entries(context, values)?;
        let functions: &Bound<'py, PyDict> = &context.classes.hooks.functions;
        for (hook_key, index, entry) in &entries {
            let name: String = native_field(entry, NAME_FIELD)?.extract()?;
            let site: HookSite<'_> = HookSite {
                model_name,
                hook_key,
                index: *index,
                name: &name,
            };
            let Some(function) = functions.get_item(&name)? else {
                let mut known: Vec<String> = functions.keys().extract()?;
                known.sort_unstable();
                let known: String = if known.is_empty() {
                    "none discovered".to_owned()
                } else {
                    known.join(", ")
                };
                let message: String = format!(
                    "{} references an unknown hook. Discovered hooks: {known}",
                    site.prefix()
                );
                return Err(compile_error(context.classes, &message, None));
            };
            let kwargs: Vec<String> = native_field(entry, KWARGS_FIELD)?
                .downcast_into::<PyDict>()?
                .keys()
                .extract()?;
            let parameters: Vec<HookParameter> = self.parameters(context, &name, &function)?;
            if let Some(message) =
                signature_problem(&site, &parameters, &kwargs, &context.classes.hooks)
            {
                return Err(compile_error(context.classes, &message, None));
            }
        }
        let mut usages: Vec<Bound<'py, PyAny>> = Vec::new();
        for (_, _, entry) in &entries {
            let name: String = native_field(entry, NAME_FIELD)?.extract()?;
            usages.extend(named_usages(
                context,
                (&PYTHON_HOOK_DECLARATION, &name),
                consumer,
                model,
            )?);
        }
        Ok(usages)
    }

    /// The hook function's parameters, read through `inspect.signature` the first time.
    fn parameters(
        &self,
        context: &LoopContext<'_, '_>,
        name: &str,
        function: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<HookParameter>> {
        if let Some(parameters) = self.by_name.borrow().get(name) {
            return Ok(parameters.clone());
        }
        let parameters: Vec<HookParameter> = context
            .classes
            .hooks
            .signature
            .call1((native_field(function, FUNCTION_FIELD)?,))?
            .extract()?;
        self.by_name
            .borrow_mut()
            .insert(name.to_owned(), parameters.clone());
        Ok(parameters)
    }
}

impl HookSite<'_> {
    fn prefix(&self) -> String {
        format!(
            "model '{}' {}[{}] python(\"{}\")",
            self.model_name, self.hook_key, self.index, self.name
        )
    }
}

/// Every `PythonHookEntry` of the sorted hook keys, with its key and index.
fn python_entries<'py>(
    context: &LoopContext<'_, 'py>,
    values: &Bound<'py, PyDict>,
) -> PyResult<Vec<(&'static str, usize, Bound<'py, PyAny>)>> {
    let mut entries: Vec<(&'static str, usize, Bound<'py, PyAny>)> = Vec::new();
    for hook_key in SORTED_HOOK_KEYS {
        let Some(raw) = values.get_item(hook_key)? else {
            continue;
        };
        if !(raw.is_instance_of::<PyList>() || raw.is_instance_of::<PyTuple>()) {
            continue;
        }
        for (index, entry) in raw.try_iter()?.enumerate() {
            let entry: Bound<'py, PyAny> = entry?;
            if entry.is_instance(&context.classes.hooks.python_hook_entry)? {
                entries.push((hook_key, index, entry));
            }
        }
    }
    Ok(entries)
}

/// `validate_python_hook_signature`: the first problem with the configured arguments, if any.
fn signature_problem(
    site: &HookSite<'_>,
    parameters: &[HookParameter],
    kwargs: &[String],
    classes: &HookClasses<'_>,
) -> Option<String> {
    let prefix: String = site.prefix();
    let context_parameter = |name: &str| CONTEXT_PARAMETERS.contains(&name);
    let provider = |name: &str| classes.provider_names.iter().any(|known| known == name);
    let mut sorted_kwargs: Vec<&str> = kwargs.iter().map(String::as_str).collect();
    sorted_kwargs.sort_unstable();
    if let Some(conflict) = sorted_kwargs.iter().find(|name| context_parameter(name)) {
        return Some(format!(
            "{prefix} argument '{conflict}' conflicts with reserved context parameter \
             '{conflict}'. Rename the hook argument; context parameters are injected by SQLBuild."
        ));
    }
    if let Some(conflict) = sorted_kwargs.iter().find(|name| provider(name)) {
        return Some(format!(
            "{prefix} argument '{conflict}' conflicts with provider injection for parameter \
             '{conflict}'. Rename the hook argument or remove it to let SQLBuild inject the \
             provider."
        ));
    }
    let keyword = |parameter: &HookParameter| {
        (parameter.1 == POSITIONAL_OR_KEYWORD || parameter.1 == KEYWORD_ONLY)
            && !context_parameter(&parameter.0)
    };
    let accepts_var_keyword: bool = parameters
        .iter()
        .any(|parameter| parameter.1 == VAR_KEYWORD);
    let mut accepted: Vec<&str> = parameters
        .iter()
        .filter(|parameter| keyword(parameter))
        .map(|parameter| parameter.0.as_str())
        .collect();
    accepted.sort_unstable();
    accepted.dedup();
    let unknown: Vec<&str> = sorted_kwargs
        .iter()
        .copied()
        .filter(|name| !accepts_var_keyword && !accepted.contains(name))
        .collect();
    if !unknown.is_empty() {
        let accepted: String = if accepted.is_empty() {
            "none".to_owned()
        } else {
            accepted.join(", ")
        };
        return Some(format!(
            "{prefix} has unknown argument(s): {}. Accepted configured arguments: {accepted}",
            unknown.join(", ")
        ));
    }
    let required_positional_only: Vec<&str> = parameters
        .iter()
        .filter(|parameter| {
            parameter.1 == POSITIONAL_ONLY && !parameter.2 && !context_parameter(&parameter.0)
        })
        .map(|parameter| parameter.0.as_str())
        .collect();
    if !required_positional_only.is_empty() {
        return Some(format!(
            "{prefix} cannot be configured because the hook function has required \
             positional-only parameter(s): {}. Use keyword-capable parameters for values \
             supplied from MODEL hooks.",
            required_positional_only.join(", ")
        ));
    }
    let missing: Vec<&str> = parameters
        .iter()
        .filter(|parameter| {
            keyword(parameter)
                && !parameter.2
                && !provider(&parameter.0)
                && !kwargs.contains(&parameter.0)
        })
        .map(|parameter| parameter.0.as_str())
        .collect();
    if !missing.is_empty() {
        return Some(format!(
            "{prefix} is missing required argument(s): {}",
            missing.join(", ")
        ));
    }
    None
}
