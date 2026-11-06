//! A model's hooks, expanded and checked natively.

use std::cell::RefCell;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{
    PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods, PyTuple,
    PyTupleMethods,
};
use sqlbuild_analysis::compiled_project::main::located_scan_error::located_scan_error;
use sqlbuild_analysis::compiled_project::main::merge_call_site_references::merge_call_site_references;
use sqlbuild_analysis::compiled_project::models::ScanFaultOrigin;
use sqlbuild_attachments::audits::main::render_parameterized_sql::render_parameterized_sql;
use sqlbuild_attachments::audits::models::ParameterRenderStop;
use sqlbuild_attachments::cursor_intrinsics::main::intrinsic_free::intrinsic_free;
use sqlbuild_attachments::cursor_intrinsics::models::IntrinsicCheck;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::ReferenceExtraction;

use crate::bindings::_helpers::attachments::argument_values::argument_pairs;
use crate::bindings::_helpers::input_loops::model_loop::{
    LoopContext, NativeModelLoop, VisibleDeclarations, compile_error, record_reads,
    validation_outcome,
};
use crate::bindings::_helpers::input_loops::model_loop_calls::{CallOrigin, report_invalid_calls};
use crate::bindings::_helpers::input_loops::model_loop_external::external_errors;
use crate::bindings::_helpers::input_loops::model_loop_inputs::{
    HOOK_KEYS, present, reference_row,
};
use crate::bindings::_helpers::input_loops::model_loop_macros::{
    argument_references, expanded_with,
};
use crate::bindings::_helpers::input_loops::model_loop_named::{
    SQL_HOOK_DECLARATION, named_usages,
};
use crate::bindings::_helpers::input_loops::model_loop_visibility::ModelVisibility;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::render::declaration_expansion::{
    DeclarationExpansion, ExpandedDeclarations, ExpansionClasses, ExpansionConsumer,
    expand_declarations,
};
use crate::bindings::_helpers::render::sql_macro_host::SqlMacroSource;
use crate::bindings::_helpers::sqltext::declaration_references::declaration_scan;
use crate::bindings::_helpers::sqltext::static_sql::{InterpolationSources, interpolated};
use sqlbuild_project::models::ReferenceRow;

const REFERENCES_METHOD: &str = "references";
const STATEMENT_ATTRIBUTE: &str = "statement";
const NAME_ATTRIBUTE: &str = "name";
const KWARGS_ATTRIBUTE: &str = "kwargs";
const RELATIVE_PATH_ATTRIBUTE: &str = "relative_path";
const DEFINITION_SQL_ATTRIBUTE: &str = "definition_sql";
const DESCRIPTION_ATTRIBUTE: &str = "description";
const FILE_PATH_ATTRIBUTE: &str = "file_path";
const SQL_BODY_ATTRIBUTE: &str = "sql_body";
const READS_ATTRIBUTE: &str = "reads";
const KIND_ATTRIBUTE: &str = "kind";
const MACRO_TOKEN: char = '@';
/// `_VisibleModelDeclarations` fields an unscoped hook's `DeclarationResolutionContext` reads.
const UNSCOPED_FIELDS: [&str; 4] = [
    "enums",
    "constants",
    "inaccessible_enums",
    "inaccessible_constants",
];

/// What a model's SQL hooks are expanded with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct SqlHookClasses<'py> {
    named_sql_hook_entry: Bound<'py, PyAny>,
    python_hook_entry: Bound<'py, PyAny>,
    /// Discovered SQL hook files by name.
    definitions: Bound<'py, PyDict>,
    pub(crate) effective_vars: Bound<'py, PyDict>,
    /// `UnicodeEnvironment()` and `render_project_var_text`.
    pub(crate) environment: Bound<'py, PyAny>,
    pub(crate) render_project_var: Bound<'py, PyAny>,
    sql_resource_ref: Bound<'py, PyAny>,
    /// `SqlResourceRefKind` by the `SqlReferenceKind` value of an explicit reference.
    explicit_kinds: Bound<'py, PyDict>,
    declaration_identity: Bound<'py, PyAny>,
    /// `DeclarationKind.SQL_HOOK`.
    sql_hook_kind: Bound<'py, PyAny>,
}

/// The model whose hooks are expanded.
pub(crate) struct HookModel<'a, 'py> {
    pub(crate) name: &'a str,
    pub(crate) posix_path: &'a str,
    pub(crate) relative_path: &'a str,
    pub(crate) file_path: &'a str,
    pub(crate) contents: &'a str,
    pub(crate) authored_query: &'a str,
    pub(crate) visible: &'a VisibleDeclarations<'py>,
    pub(crate) visibility: &'a ModelVisibility<'py>,
}

/// The hook-expanded config values and the usages hook SQL recorded.
pub(crate) struct ExpandedHooks<'py> {
    pub(crate) values: Bound<'py, PyDict>,
    pub(crate) usages: Vec<Bound<'py, PyAny>>,
    /// The SQL macros the hook SQL called, top-level and nested, once each.
    pub(crate) sql_macros: Vec<String>,
}

/// `expand_model_hook_macros_result`, then the hooks' reference and intrinsic checks.
pub(crate) fn expanded_hooks<'py>(
    model_loop: &NativeModelLoop,
    context: &LoopContext<'_, 'py>,
    values: &Bound<'py, PyDict>,
    model: &HookModel<'_, 'py>,
) -> PyResult<ExpandedHooks<'py>> {
    let py: Python<'py> = values.py();
    let context_values: Bound<'py, PyDict> = context
        .classes
        .config
        .builder
        .get()
        .model_context_values(values, model.name, true)?;
    let run: HookRun<'_, '_, 'py> = HookRun {
        model_loop,
        context,
        model,
        context_values,
        usages: RefCell::new(Vec::new()),
        references: RefCell::new(Vec::new()),
        sql_macros: RefCell::new(Vec::new()),
    };
    let expanded: Bound<'py, PyDict> = values.copy()?;
    let model_scope: Scope<'_, 'py> = Scope {
        file_path: model.file_path,
        consumer: model.visible.consumer.clone(),
    };
    for key in HOOK_KEYS {
        let Some(hooks) = present(values, key)? else {
            continue;
        };
        expanded.set_item(key, run.value(&hooks, &model_scope, (Some(key), None))?)?;
    }
    let mut references: Vec<ReferenceRow> = Vec::new();
    for row in run.references.take() {
        if !references.contains(&row) {
            references.push(row);
        }
    }
    let external: Vec<Option<PyErr>> = external_errors(context, &references, model.relative_path)?;
    let rows: Vec<(&str, &str, bool)> = references
        .iter()
        .zip(&external)
        .map(|(ReferenceRow { kind, name, .. }, error)| {
            (kind.as_str(), name.as_str(), error.is_some())
        })
        .collect();
    let outcome: Bound<'py, PyAny> = model_loop
        .validator
        .bind(py)
        .call_method1(REFERENCES_METHOD, ((model.name, model.relative_path), rows))?;
    validation_outcome(&outcome, external, &PyDict::new(py))?;
    reject_hook_intrinsics(model_loop, context, model.name, &expanded)?;
    Ok(ExpandedHooks {
        values: expanded,
        usages: run.usages.take(),
        sql_macros: run.sql_macros.take(),
    })
}

/// The file a hook's SQL is authored in and the identity its usages are recorded for.
struct Scope<'a, 'py> {
    file_path: &'a str,
    consumer: Bound<'py, PyAny>,
}

/// `(hook key, index)` of the entry being expanded, for error labels.
type HookPosition<'a> = (Option<&'a str>, Option<usize>);

/// One model's hook expansion and the facts it collects.
struct HookRun<'a, 'b, 'py> {
    model_loop: &'a NativeModelLoop,
    context: &'a LoopContext<'b, 'py>,
    model: &'a HookModel<'a, 'py>,
    context_values: Bound<'py, PyDict>,
    usages: RefCell<Vec<Bound<'py, PyAny>>>,
    /// The SQL macros hook SQL called so far, once each.
    sql_macros: RefCell<Vec<String>>,
    references: RefCell<Vec<ReferenceRow>>,
}

impl<'py> HookRun<'_, '_, 'py> {
    fn classes(&self) -> &SqlHookClasses<'py> {
        &self.context.classes.sql_hooks
    }

    /// `expand_sql_macros_in_value`: hook container shapes expanded recursively.
    fn value(
        &self,
        value: &Bound<'py, PyAny>,
        scope: &Scope<'_, 'py>,
        position: HookPosition<'_>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = value.py();
        if let Ok(text) = value.downcast::<PyString>() {
            let expanded: String = self.text(text.to_str()?, scope, position)?;
            return Ok(PyString::new(py, &expanded).into_any());
        }
        if value.is_instance(&self.context.classes.sql_hook_entry)? {
            return self.inline_entry(value, scope, position);
        }
        if value.is_instance(&self.classes().named_sql_hook_entry)? {
            return self.named_entry(value, scope, position);
        }
        if value.is_instance(&self.classes().python_hook_entry)? {
            return Ok(value.clone());
        }
        let (key, _) = position;
        if let Ok(items) = value.downcast::<PyList>() {
            let mut expanded: Vec<Bound<'py, PyAny>> = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                expanded.push(self.value(&item, scope, (key, Some(index)))?);
            }
            return Ok(PyList::new(py, expanded)?.into_any());
        }
        if let Ok(items) = value.downcast::<PyTuple>() {
            let mut expanded: Vec<Bound<'py, PyAny>> = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                expanded.push(self.value(&item, scope, (key, Some(index)))?);
            }
            return Ok(PyTuple::new(py, expanded)?.into_any());
        }
        Ok(value.clone())
    }

    fn inline_entry(
        &self,
        entry: &Bound<'py, PyAny>,
        scope: &Scope<'_, 'py>,
        position: HookPosition<'_>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let start: usize = self.references.borrow().len();
        let statement: String = self
            .value(&native_field(entry, STATEMENT_ATTRIBUTE)?, scope, position)?
            .str()?
            .to_str()?
            .to_owned();
        self.require_payload(&statement, &inline_label(position), scope)?;
        let fields: Bound<'py, PyDict> = PyDict::new(entry.py());
        fields.set_item(STATEMENT_ATTRIBUTE, &statement)?;
        for field in [
            NAME_ATTRIBUTE,
            RELATIVE_PATH_ATTRIBUTE,
            DEFINITION_SQL_ATTRIBUTE,
            KWARGS_ATTRIBUTE,
            DESCRIPTION_ATTRIBUTE,
        ] {
            fields.set_item(field, native_field(entry, field)?)?;
        }
        fields.set_item(READS_ATTRIBUTE, self.reads_since(start)?)?;
        self.context.classes.sql_hook_entry.call((), Some(&fields))
    }

    fn named_entry(
        &self,
        entry: &Bound<'py, PyAny>,
        scope: &Scope<'_, 'py>,
        position: HookPosition<'_>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = entry.py();
        let name: String = native_field(entry, NAME_ATTRIBUTE)?.extract()?;
        let label: String = named_label(&name, position);
        let Some(definition) = self.classes().definitions.get_item(&name)? else {
            let mut known: Vec<String> = self
                .classes()
                .definitions
                .keys()
                .into_iter()
                .map(|key| key.extract())
                .collect::<PyResult<_>>()?;
            known.sort();
            let known: String = if known.is_empty() {
                "none discovered".to_owned()
            } else {
                known.join(", ")
            };
            let message: String = format!(
                "{label} in '{}' references an unknown SQL hook. Discovered SQL hooks: {known}. \
                 If this is an inline SQL statement, use inline_sql(\"...\").",
                scope.file_path
            );
            return Err(compile_error(self.context.classes, &message, None));
        };
        self.usages.borrow_mut().extend(named_usages(
            self.context,
            (&SQL_HOOK_DECLARATION, &name),
            &scope.consumer,
            (self.model.name, self.model.relative_path),
        )?);
        let kwargs: Bound<'py, PyDict> = native_field(entry, KWARGS_ATTRIBUTE)?.downcast_into()?;
        let body: String = native_field(&definition, SQL_BODY_ATTRIBUTE)?.extract()?;
        let definition_path: Bound<'py, PyAny> =
            native_field(&definition, RELATIVE_PATH_ATTRIBUTE)?;
        let owner: String = format!("{} {label}", scope.file_path);
        let definition_label: String =
            format!("SQL hook '{name}' in {}", definition_path.str()?.to_str()?);
        let rendered: String = render_parameterized_sql(&body, &argument_pairs(&kwargs)?, true)
            .map_err(|stop| {
                let message: String = match stop {
                    ParameterRenderStop::UnusedArguments(names) => format!(
                        "{owner} passes unknown argument(s) to {definition_label}: {}",
                        names.join(", ")
                    ),
                    ParameterRenderStop::MissingArgument(argument) => {
                        format!("{owner} is missing argument '{argument}' for {definition_label}")
                    }
                    ParameterRenderStop::UnsupportedValue(argument) => format!(
                        "{owner} {definition_label} argument '{argument}' uses an unsupported value"
                    ),
                };
                compile_error(self.context.classes, &message, None)
            })?;
        let start: usize = self.references.borrow().len();
        let hook_path: String = native_field(&definition, FILE_PATH_ATTRIBUTE)?
            .str()?
            .to_str()?
            .to_owned();
        let hook_scope: Scope<'_, 'py> = Scope {
            file_path: &hook_path,
            consumer: self
                .classes()
                .declaration_identity
                .call1((&self.classes().sql_hook_kind, &name))?,
        };
        let statement: String = self.text(&rendered, &hook_scope, position)?;
        self.require_payload(&statement, &label, scope)?;
        let fields: Bound<'py, PyDict> = PyDict::new(py);
        fields.set_item(STATEMENT_ATTRIBUTE, &statement)?;
        fields.set_item(NAME_ATTRIBUTE, &name)?;
        fields.set_item(RELATIVE_PATH_ATTRIBUTE, definition_path)?;
        fields.set_item(DEFINITION_SQL_ATTRIBUTE, &body)?;
        fields.set_item(KWARGS_ATTRIBUTE, kwargs.copy()?)?;
        fields.set_item(
            DESCRIPTION_ATTRIBUTE,
            native_field(&definition, DESCRIPTION_ATTRIBUTE)?,
        )?;
        fields.set_item(READS_ATTRIBUTE, self.reads_since(start)?)?;
        self.context.classes.sql_hook_entry.call((), Some(&fields))
    }

    fn require_payload(
        &self,
        statement: &str,
        label: &str,
        scope: &Scope<'_, 'py>,
    ) -> PyResult<()> {
        if !blank(statement) {
            return Ok(());
        }
        let message: String = format!(
            "{label} in '{}' must render a non-empty SQL payload",
            scope.file_path
        );
        Err(compile_error(self.context.classes, &message, None))
    }

    /// `expand_authored_sql_result` and the reference scan of one hook SQL text.
    fn text(
        &self,
        sql: &str,
        scope: &Scope<'_, 'py>,
        position: HookPosition<'_>,
    ) -> PyResult<String> {
        if uses_template(sql) {
            let message: String = format!(
                "{} in '{}' uses unsupported ${{...}} template syntax. Use @@CTX:..., \
                 @@ENV:..., or @@project_var inside SQL hooks.",
                inline_label(position),
                scope.file_path
            );
            return Err(compile_error(self.context.classes, &message, None));
        }
        let py: Python<'py> = scope.consumer.py();
        let hooks: &SqlHookClasses<'py> = self.classes();
        let (interpolated_sql, reads, error) = interpolated(
            InterpolationSources(
                hooks.effective_vars.clone(),
                hooks.environment.clone(),
                Some(self.context_values.clone()),
                hooks.render_project_var.clone(),
                Some(self.context.syntax.clone()),
            ),
            sql,
            scope.file_path,
        )?;
        let reads: Vec<(String, String)> = reads
            .into_iter()
            .map(|(kind, name)| (kind.to_owned(), name))
            .collect();
        record_reads(self.context, &reads)?;
        if let Some(message) = error {
            return Err(compile_error(self.context.classes, &message, None));
        }
        let interpolated_sql: String = interpolated_sql.unwrap_or_else(|| sql.to_owned());
        let declarations: Bound<'py, PyAny> = if self.context.classes.macros.scoped {
            self.model
                .visibility
                .resolved(self.context, scope.file_path)?
        } else {
            let fields: Bound<'py, PyDict> = PyDict::new(py);
            for field in UNSCOPED_FIELDS {
                fields.set_item(
                    field,
                    native_field(&self.model.visible.declarations, field)?,
                )?;
            }
            self.context
                .classes
                .macros
                .declaration_context
                .call((), Some(&fields))?
        };
        let expanded: ExpandedDeclarations<'py> =
            self.declarations(&interpolated_sql, &declarations, scope)?;
        {
            let mut called = self.sql_macros.borrow_mut();
            for name in &expanded.sql_macros {
                if !called.contains(name) {
                    called.push(name.clone());
                }
            }
        }
        let mut sql: String = expanded.spliced.sql;
        let mut usages: Vec<Bound<'py, PyAny>> = expanded.usages;
        let mut argument_rows: Vec<ReferenceRow> = Vec::new();
        if sql.contains(MACRO_TOKEN) {
            let macro_declarations: Bound<'py, PyAny> = if self.context.classes.macros.scoped {
                declarations
            } else {
                py.None().into_bound(py)
            };
            let output = expanded_with(
                self.context,
                (&macro_declarations, &scope.consumer),
                scope.file_path,
                &sql,
            )?;
            argument_rows = argument_references(self.context, &output)?;
            usages.extend(output.usages.into_iter().map(|usage| usage.into_bound(py)));
            sql = output.sql;
        }
        self.usages.borrow_mut().extend(usages);
        let scanned = match extract_sql_references(&sql, &self.model_loop.syntax) {
            ReferenceExtraction::Extracted(scanned) => scanned,
            ReferenceExtraction::Failed(failure) => {
                let origin: ScanFaultOrigin<'_> = ScanFaultOrigin {
                    relative_path: self.model.posix_path,
                    contents: self.model.contents,
                    query_sql: self.model.authored_query,
                    passes: None,
                };
                let message: String =
                    located_scan_error(&failure, &sql, &origin, &self.model_loop.syntax);
                return Err(compile_error(self.context.classes, &message, None));
            }
        };
        report_invalid_calls(
            self.context,
            &self.model_loop.syntax,
            &scanned.invalid_calls,
            CallOrigin {
                name: Some(self.model.name),
                relative_path: self.model.relative_path,
                file_path: self.model.file_path,
                contents: self.model.contents,
            },
        )?;
        self.references
            .borrow_mut()
            .extend(merge_call_site_references(
                scanned.references.iter().map(reference_row).collect(),
                argument_rows,
            ));
        Ok(sql)
    }

    fn declarations(
        &self,
        sql: &str,
        declarations: &Bound<'py, PyAny>,
        scope: &Scope<'_, 'py>,
    ) -> PyResult<ExpandedDeclarations<'py>> {
        let (references, stop) = declaration_scan(sql, Some(&self.model_loop.syntax));
        let classes: ExpansionClasses<'py> = ExpansionClasses {
            usage_record: self.context.classes.usage_record.clone(),
            compile_error: self.context.classes.compile_error.clone(),
        };
        expand_declarations(&DeclarationExpansion {
            sql,
            references: &references,
            stop,
            context: declarations,
            consumer: ExpansionConsumer {
                identity: Some(scope.consumer.clone()),
                relationship_visibility: false,
                path: scope.file_path,
            },
            rendering: self.context.host.get(),
            classes: &classes,
            sql_macros: Some(SqlMacroSource {
                loaded_macros: Some(
                    self.context
                        .classes
                        .macros
                        .loaded_macros
                        .bind(declarations.py())
                        .as_any(),
                ),
                origin: None,
                scopes: Some(&self.context.classes.visibility),
                syntax: Some(self.context.syntax),
            }),
        })
    }

    /// `resource_references`: the model, source and seed references read since `start`, once each.
    fn reads_since(&self, start: usize) -> PyResult<Bound<'py, PyTuple>> {
        let hooks: &SqlHookClasses<'py> = self.classes();
        let py: Python<'py> = hooks.explicit_kinds.py();
        let mut seen: Vec<(String, String)> = Vec::new();
        let mut reads: Vec<Bound<'py, PyAny>> = Vec::new();
        for ReferenceRow { kind, name, .. } in self.references.borrow().iter().skip(start) {
            let Some(resource_kind) = hooks.explicit_kinds.get_item(kind)? else {
                continue;
            };
            if seen.contains(&(kind.clone(), name.clone())) {
                continue;
            }
            seen.push((kind.clone(), name.clone()));
            let fields: Bound<'py, PyDict> = PyDict::new(py);
            fields.set_item(KIND_ATTRIBUTE, resource_kind)?;
            fields.set_item(NAME_ATTRIBUTE, name)?;
            reads.push(hooks.sql_resource_ref.call((), Some(&fields))?);
        }
        PyTuple::new(py, reads)
    }
}

/// Whether Python's `str.strip()` leaves nothing.
fn blank(statement: &str) -> bool {
    statement.chars().all(char::is_whitespace)
}

/// `\$\{[^}]+\}`: a `${` followed by at least one character before its closing `}`.
fn uses_template(sql: &str) -> bool {
    sql.match_indices("${").any(|(start, _)| {
        let rest: &str = &sql[start + 2..];
        rest.find('}').is_some_and(|end| end > 0)
    })
}

fn inline_label(position: HookPosition<'_>) -> String {
    match position {
        (None, _) => "inline_sql(\"...\") hook".to_owned(),
        (Some(key), None) => format!("{key} inline_sql(\"...\")"),
        (Some(key), Some(index)) => format!("{key}[{index}] inline_sql(\"...\")"),
    }
}

fn named_label(name: &str, position: HookPosition<'_>) -> String {
    match position {
        (None, _) => format!("sql(\"{name}\") hook"),
        (Some(key), None) => format!("{key} sql(\"{name}\")"),
        (Some(key), Some(index)) => format!("{key}[{index}] sql(\"{name}\")"),
    }
}

/// Reject cursor intrinsics in each SQL hook statement.
fn reject_hook_intrinsics(
    model_loop: &NativeModelLoop,
    context: &LoopContext<'_, '_>,
    name: &str,
    values: &Bound<'_, PyDict>,
) -> PyResult<()> {
    for key in HOOK_KEYS {
        let Some(hooks) = present(values, key)? else {
            continue;
        };
        if !(hooks.is_instance_of::<PyList>() || hooks.is_instance_of::<PyTuple>()) {
            continue;
        }
        let hook_context: String = format!("Model '{name}' {key}");
        for entry in hooks.try_iter()? {
            let entry: Bound<'_, PyAny> = entry?;
            if !entry.is_instance(&context.classes.sql_hook_entry)? {
                continue;
            }
            let statement: String = native_field(&entry, STATEMENT_ATTRIBUTE)?.extract()?;
            if let IntrinsicCheck::Rejected(message) =
                intrinsic_free(&statement, &model_loop.reserved_markers, &hook_context)
            {
                return Err(compile_error(context.classes, &message, None));
            }
        }
    }
    Ok(())
}
