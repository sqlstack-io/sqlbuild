//! SQL macro calls expanded in the declaration pass, reading the caller's visible declarations.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::sql_macros::constants::{
    BARE_MACRO_CALL_CODE, BODY_SCOPE_CODE, BODY_TYPE_CODE, MIXED_KIND_CODE, RETURNS_MISMATCH_CODE,
};
use sqlbuild_analysis::sql_macros::errors::ExpansionFailure;
use sqlbuild_analysis::sql_macros::errors::SqlMacroError;
use sqlbuild_analysis::sql_macros::main::bare_macro_calls::bare_macro_calls;
use sqlbuild_analysis::sql_macros::main::byte_text_splices::byte_text_splices;
use sqlbuild_analysis::sql_macros::main::declaration_visible_from_macro::declaration_visible_from_macro;
use sqlbuild_analysis::sql_macros::main::expand_sql_macro_call::expand_sql_macro_call;
use sqlbuild_analysis::sql_macros::main::parse_sql_macro::parse_sql_macro;
use sqlbuild_analysis::sql_macros::main::render_inline_collection::render_inline_collection;
use sqlbuild_analysis::sql_macros::main::sql_macro_calls::sql_macro_calls;
use sqlbuild_analysis::sql_macros::models::{
    ExpandedText, InsertedNames, RenderedDeclarations, SqlMacroCall, SqlMacroCheck,
    SqlMacroDefinition, SqlMacroLocation, TextContext, TextSplice,
};
use sqlbuild_analysis::sql_macros::types::{ConstantShape, SqlMacroHost, SqlMacroScope};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::records::construct::{
    enum_member, new_helped_error, new_record, new_record_with,
};
use crate::bindings::_helpers::records::fields::{field as native_field, field_text, path_posix};
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::render::declaration_expansion::{
    DeclarationExpansion, ExpandedDeclarations, expand_declarations, usage_visibility,
};
use crate::bindings::_helpers::sqltext::declaration_references::declaration_scan;

const SQL_LANGUAGE: &str = "sql";
const UNKNOWN_CODE: &str = "P001";
const INACCESSIBLE_CODE: &str = "S006";
const ENUM_REFERENCE_KIND: u8 = 0;
const SQL_MACRO_REFERENCE_KIND: u8 = 2;

const MODEL_KIND: &str = "model";

/// A macro the caller sees, and one it cannot see, by name.
type LoadedMacro<'py> = (Option<Bound<'py, PyAny>>, Option<Bound<'py, PyAny>>);

/// Where the caller's text came from, to locate calls in its authored file.
#[derive(Clone, Copy)]
pub(crate) struct SqlCallOrigin<'a> {
    /// The authored file's project-relative path.
    pub(crate) relative_path: &'a str,
    /// The authored file's contents.
    pub(crate) contents: &'a str,
    /// The authored query the expanded text came from, within `contents`.
    pub(crate) query: &'a str,
}

/// What SQL macro calls in one declaration pass read beyond the declaration context.
#[derive(Clone, Copy)]
pub(crate) struct SqlMacroSource<'a, 'py> {
    /// Every loaded macro by name, for unscoped compiles whose context holds none.
    pub(crate) loaded_macros: Option<&'a Bound<'py, PyAny>>,
    pub(crate) origin: Option<SqlCallOrigin<'a>>,
    /// Where macro bodies resolve their declarations; `None` outside the input loops.
    pub(crate) scopes: Option<&'a dyn MacroScopes<'py>>,
    /// The lexical rules the authored SQL is written in; `None` reads it under the generic rules.
    pub(crate) syntax: Option<&'a LexicalSyntax>,
}

/// The declaration contexts of macro files in a scoped compile.
pub(crate) trait MacroScopes<'py> {
    /// The context the file at `path` sees, or `None` when the compile is unscoped.
    fn file_context(&self, path: &str) -> PyResult<Option<Bound<'py, PyAny>>>;
}

/// The facts a declaration pass's SQL macro calls recorded.
#[derive(Default)]
pub(crate) struct SqlMacroFacts<'py> {
    pub(crate) usages: Vec<Bound<'py, PyAny>>,
    /// The SQL macros the calls used, top-level and nested, once each.
    pub(crate) names: Vec<String>,
    /// What the analysed SQL must show for each top-level call's arguments to fit.
    pub(crate) checks: Vec<SqlMacroCheck>,
}

struct PyHost<'r, 'a, 'py> {
    request: &'r DeclarationExpansion<'a, 'py>,
    source: SqlMacroSource<'a, 'py>,
    facts: RefCell<SqlMacroFacts<'py>>,
    adapter: String,
}

/// Whether a scanned reference is a SQL macro call.
pub(crate) fn is_sql_macro_reference(kind: u8) -> bool {
    kind == SQL_MACRO_REFERENCE_KIND
}

/// The replacement text, with its expression macro calls, of each SQL macro call at `starts`.
pub(crate) fn expanded_sql_macro_calls<'py>(
    request: &DeclarationExpansion<'_, 'py>,
    starts: &[usize],
) -> PyResult<(Vec<ExpandedText>, SqlMacroFacts<'py>)> {
    let sql: &str = request.sql;
    let syntax: Option<&LexicalSyntax> = request.sql_macros.and_then(|source| source.syntax);
    let calls: Vec<SqlMacroCall> =
        sql_macro_calls(sql, syntax).map_err(|error| macro_error(request, &error))?;
    let by_char: HashMap<usize, &SqlMacroCall> = calls
        .iter()
        .map(|call| (sql[..call.start].chars().count(), call))
        .collect();
    let source: SqlMacroSource<'_, 'py> = request.sql_macros.unwrap_or(SqlMacroSource {
        loaded_macros: None,
        origin: None,
        scopes: None,
        syntax: None,
    });
    let host: PyHost<'_, '_, 'py> = PyHost {
        request,
        source,
        facts: RefCell::new(SqlMacroFacts::default()),
        adapter: request.rendering.adapter_name(request.context.py()),
    };
    let owner: String = owner_label(request)?;
    let context = TextContext {
        text: sql,
        scope: &SqlMacroScope::Caller,
        stack: &[],
        owner: &owner,
    };
    let mut inserted: InsertedNames = InsertedNames::default();
    let mut replacements: Vec<ExpandedText> = Vec::with_capacity(starts.len());
    for start in starts {
        let Some(call) = by_char.get(start) else {
            return Err(macro_error(
                request,
                &SqlMacroError {
                    code: UNKNOWN_CODE,
                    message: "a SQL macro call could not be read".to_owned(),
                    locations: Vec::new(),
                    help: None,
                },
            ));
        };
        match expand_sql_macro_call(&host, &context, inserted, call) {
            Ok((text, updated)) => {
                replacements.push(text);
                inserted = updated;
            }
            Err(ExpansionFailure::Host(error)) => return Err(error),
            Err(ExpansionFailure::Macro(error)) => {
                let located: SqlMacroError = match host.call_location((call.start, call.end)) {
                    Some(location) if is_macro_file_error(&error) => error.at(location),
                    Some(location) => error.at_call(location),
                    None => error,
                };
                return Err(macro_error(request, &located));
            }
        }
    }
    Ok((replacements, host.facts.into_inner()))
}

/// Where the call spanning bytes `span` of `sql` starts in the authored file `origin`.
fn authored_location(
    origin: SqlCallOrigin<'_>,
    sql: &str,
    span: (usize, usize),
) -> SqlMacroLocation {
    authored_span(origin, sql, span).0
}

/// Where the call at bytes `start..end` of `sql` starts and ends in the authored file `origin`.
fn authored_span(
    origin: SqlCallOrigin<'_>,
    sql: &str,
    (start, end): (usize, usize),
) -> (SqlMacroLocation, (usize, usize)) {
    let query_byte: usize = if origin.contents.ends_with(origin.query) {
        origin.contents.len() - origin.query.len()
    } else {
        origin.contents.find(origin.query).unwrap_or(0)
    };
    let snippet: &str = &sql[start..end];
    let authored: usize = if sql == origin.query {
        query_byte + start
    } else {
        let ordinal: usize = sql[..start].match_indices(snippet).count();
        origin.contents[query_byte..]
            .match_indices(snippet)
            .nth(ordinal)
            .map_or(query_byte, |(found, _)| query_byte + found)
    };
    let (line, column) = byte_position(origin.contents, authored);
    (
        SqlMacroLocation {
            path: origin.relative_path.to_owned(),
            line,
            column,
        },
        byte_position(origin.contents, authored + snippet.len()),
    )
}

/// The one-based line and character column of byte `offset` of `text`.
fn byte_position(text: &str, offset: usize) -> (usize, usize) {
    let mut offset: usize = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before: &str = &text[..offset];
    let line_start: usize = before.rfind('\n').map_or(0, |newline| newline + 1);
    (
        before.matches('\n').count() + 1,
        before[line_start..].chars().count() + 1,
    )
}

/// Reject the first call in an authored text that names no macro kind, located in its file.
pub(crate) fn reject_bare_macro_calls(request: &DeclarationExpansion<'_, '_>) -> PyResult<()> {
    let Some(origin) = request.sql_macros.and_then(|source| source.origin) else {
        return Ok(());
    };
    let Some((name, start)) = bare_macro_calls(
        request.sql,
        request.sql_macros.and_then(|source| source.syntax),
    )
    .into_iter()
    .next() else {
        return Ok(());
    };
    let error: SqlMacroError = SqlMacroError::new_public(
        BARE_MACRO_CALL_CODE,
        format!("macro call @{name}() does not say which kind of macro it calls"),
    )
    .with_help(format!(
        "write @sql.{name}(...) to call a SQL macro or @py.{name}(...) to call a Python macro"
    ))
    .at_call(authored_location(
        origin,
        request.sql,
        (start, start + 1 + name.len()),
    ));
    Err(macro_error(request, &error))
}

/// `kind:name` of the consumer, or its file path.
fn owner_label(request: &DeclarationExpansion<'_, '_>) -> PyResult<String> {
    match &request.consumer.identity {
        Some(identity) if identity.hasattr("kind")? => Ok(format!(
            "{}:{}",
            native_field(&native_field(identity, "kind")?, "value")?.str()?,
            native_field(identity, "name")?.str()?
        )),
        _ => Ok(request.consumer.path.to_owned()),
    }
}

/// The consumer's model name, when it is a model.
fn model_name(request: &DeclarationExpansion<'_, '_>) -> PyResult<Option<String>> {
    let Some(identity) = request.consumer.identity.as_ref() else {
        return Ok(None);
    };
    let kind: String = native_field(&native_field(identity, "kind")?, "value")?.extract()?;
    if kind != MODEL_KIND {
        return Ok(None);
    }
    Ok(Some(native_field(identity, "name")?.extract()?))
}

/// `CompileInputError(message, code=..., help=...)` for a SQL macro error.
fn macro_error(request: &DeclarationExpansion<'_, '_>, error: &SqlMacroError) -> PyErr {
    let model: Option<String> = match model_name(request) {
        Ok(model) => model,
        Err(failure) => return failure,
    };
    new_helped_error(
        request.classes.compile_error.py(),
        ErrorId::CompileInput,
        (error.located_message(model.as_deref()), error.code),
        error.help.as_deref(),
    )
}

fn failure(code: &'static str, message: String) -> ExpansionFailure<PyErr> {
    ExpansionFailure::Macro(SqlMacroError {
        code,
        message,
        locations: Vec::new(),
        help: None,
    })
}

fn helped(code: &'static str, message: String, help: String) -> ExpansionFailure<PyErr> {
    ExpansionFailure::Macro(SqlMacroError {
        code,
        message,
        locations: Vec::new(),
        help: Some(help),
    })
}

fn host<T>(result: PyResult<T>) -> Result<T, ExpansionFailure<PyErr>> {
    result.map_err(ExpansionFailure::Host)
}

/// A mapping field of the context, or an empty dictionary when the context has none.
fn mapping<'py>(context: &Bound<'py, PyAny>, field: &str) -> PyResult<Bound<'py, PyDict>> {
    if context.is_none() || !context.hasattr(field)? {
        return Ok(PyDict::new(context.py()));
    }
    Ok(native_field(context, field)?.downcast_into()?)
}

impl<'py> PyHost<'_, '_, 'py> {
    fn py(&self) -> Python<'py> {
        self.request.classes.compile_error.py()
    }

    /// The authored location of the call spanning bytes `span` of the caller's text.
    fn call_location(&self, span: (usize, usize)) -> Option<SqlMacroLocation> {
        Some(authored_location(
            self.source.origin?,
            self.request.sql,
            span,
        ))
    }

    /// The loaded macro `name`: visible to the caller, inaccessible to it, or unknown.
    fn loaded(&self, name: &str) -> PyResult<LoadedMacro<'py>> {
        let context: &Bound<'py, PyAny> = self.request.context;
        if let Some(found) = mapping(context, "macros")?.get_item(name)? {
            return Ok((Some(found), None));
        }
        let inaccessible: Option<Bound<'py, PyAny>> =
            mapping(context, "inaccessible_macros")?.get_item(name)?;
        if inaccessible.is_none()
            && let Some(all) = self.source.loaded_macros
            && !all.is_none()
            && all.contains(name)?
        {
            return Ok((Some(all.get_item(name)?), None));
        }
        Ok((None, inaccessible))
    }

    /// The loaded macro `name` as the SQL macro file at `macro_path` sees it.
    fn loaded_from_macro(&self, name: &str, macro_path: &str) -> PyResult<LoadedMacro<'py>> {
        let found: Option<Bound<'py, PyAny>> = match self.source.loaded_macros {
            Some(all) if !all.is_none() && all.contains(name)? => Some(all.get_item(name)?),
            _ => mapping(self.request.context, "macros")?.get_item(name)?,
        };
        let Some(found) = found else {
            return self.loaded(name);
        };
        let path: String = path_posix(&native_field(&found, "relative_path")?)?;
        let scoped: bool = match self.source.scopes {
            Some(scopes) => scopes.file_context(macro_path)?.is_some(),
            None => false,
        };
        if !scoped || declaration_visible_from_macro(&path, macro_path) {
            return Ok((Some(found), None));
        }
        Ok((None, Some(found)))
    }

    /// The declaration context of `scope`: the caller's, or the macro file's own folder's.
    fn scope_context(&self, scope: &SqlMacroScope) -> PyResult<Bound<'py, PyAny>> {
        let (SqlMacroScope::Macro(definition), Some(scopes)) = (scope, self.source.scopes) else {
            return Ok(self.request.context.clone());
        };
        Ok(scopes
            .file_context(&definition.path)?
            .unwrap_or_else(|| self.request.context.clone()))
    }

    fn record_use(&self, name: &str) -> PyResult<()> {
        let mut facts = self.facts.borrow_mut();
        if !facts.names.iter().any(|known| known == name) {
            facts.names.push(name.to_owned());
        }
        let Some(consumer) = self.request.consumer.identity.clone() else {
            return Ok(());
        };
        let py: Python<'py> = self.py();
        let context: &Bound<'py, PyAny> = self.request.context;
        let identity: Bound<'py, PyAny> = match mapping(context, "macro_records")?.get_item(name)? {
            Some(record) => native_field(&record, "identity")?,
            None => new_record(
                py,
                RecordId::DeclarationIdentity,
                (enum_member(py, RecordId::DeclarationKind, "MACRO")?, name),
                None,
            )?,
        };
        let visibility: Bound<'py, PyAny> = mapping(context, "macro_visibility")?
            .get_item(name)?
            .unwrap_or_else(|| PyTuple::empty(py).into_any());
        let runtime: Bound<'py, PyAny> = enum_member(py, RecordId::UsageKind, "RUNTIME")?;
        let records: Vec<Bound<'py, PyAny>> =
            usage_visibility(&visibility, self.request.consumer.relationship_visibility)?;
        let throughs: Vec<Bound<'py, PyAny>> = if records.is_empty() {
            vec![py.None().into_bound(py)]
        } else {
            records
                .iter()
                .map(|record| native_field(record, "through"))
                .collect::<PyResult<_>>()?
        };
        for through in throughs {
            let fields: [(&str, Bound<'py, PyAny>); 4] = [
                ("consumer", consumer.clone()),
                ("declaration", identity.clone()),
                ("kind", runtime.clone()),
                ("through", through),
            ];
            facts
                .usages
                .push(new_record_with(py, RecordId::UsageRecord, &fields)?);
        }
        Ok(())
    }
}

impl<'py> SqlMacroHost for PyHost<'_, '_, 'py> {
    type Error = PyErr;

    fn lexical_syntax(&self) -> Option<&LexicalSyntax> {
        self.source.syntax
    }

    fn definition(
        &self,
        scope: &SqlMacroScope,
        name: &str,
    ) -> Result<Arc<SqlMacroDefinition>, ExpansionFailure<PyErr>> {
        let (loaded, inaccessible) = match scope {
            SqlMacroScope::Macro(calling) => host(self.loaded_from_macro(name, &calling.path))?,
            SqlMacroScope::Caller => host(self.loaded(name))?,
        };
        let loaded: Bound<'py, PyAny> = match (loaded, inaccessible) {
            (Some(loaded), _) => loaded,
            (None, Some(record)) => {
                if let SqlMacroScope::Macro(calling) = scope {
                    return Err(helped(
                        BODY_SCOPE_CODE,
                        format!("macro {name} is not visible from {}", calling.path),
                        "a SQL macro body calls macros visible from its own folder; move the called macro where the calling macro can see it".to_owned(),
                    ));
                }
                let path: String = host(field_text(&record, "path"))?;
                return Err(helped(
                    INACCESSIBLE_CODE,
                    format!("SQL macro {name}() defined at {path} is not visible from {}", self.request.consumer.path),
                    "move the macro to a folder the caller can see, or call a macro visible from here".to_owned(),
                ));
            }
            (None, None) => {
                return Err(failure(
                    UNKNOWN_CODE,
                    format!(
                        "Unknown SQL macro '@sql.{name}' in '{}'",
                        self.request.consumer.path
                    ),
                ));
            }
        };
        let path: String =
            host(native_field(&loaded, "relative_path").and_then(|path| path_posix(&path)))?;
        let language: String = host(field_text(&loaded, "language"))?;
        if language != SQL_LANGUAGE {
            return Err(helped(
                MIXED_KIND_CODE,
                format!("{name}() is a Python macro, called as a SQL macro with @sql.{name}()"),
                format!("call it as @py.{name}(...)"),
            ));
        }
        let source: String = host(field_text(&loaded, "raw_source"))?;
        let definition: Arc<SqlMacroDefinition> = Arc::new(parse_sql_macro(name, &path, &source)?);
        host(self.record_use(name))?;
        Ok(definition)
    }

    fn render_declarations(
        &self,
        scope: &SqlMacroScope,
        text: &str,
    ) -> Result<RenderedDeclarations, ExpansionFailure<PyErr>> {
        let (references, stop) = declaration_scan(text, self.source.syntax);
        if references.is_empty() && stop.is_none() {
            return Ok(RenderedDeclarations {
                sql: text.to_owned(),
                splices: Vec::new(),
            });
        }
        let context: Bound<'py, PyAny> = host(self.scope_context(scope))?;
        if let SqlMacroScope::Macro(definition) = scope {
            for reference in &references {
                let noun: &str = if reference.kind == ENUM_REFERENCE_KIND {
                    "enum"
                } else {
                    "constant"
                };
                let visible: bool =
                    match host(declaration_path(&context, reference.kind, &reference.name))? {
                        None => true,
                        Some(None) => false,
                        Some(Some(path)) => declaration_visible_from_macro(&path, &definition.path),
                    };
                if visible {
                    continue;
                }
                let example: String = if reference.kind == ENUM_REFERENCE_KIND {
                    format!(
                        "value ENUM {}, and pass @enum(\"{}\").MEMBER",
                        reference.name, reference.name
                    )
                } else {
                    format!(
                        "value <type> CONST, and pass @const(\"{}\")",
                        reference.name
                    )
                };
                let mut error: SqlMacroError = SqlMacroError {
                    code: BODY_SCOPE_CODE,
                    message: format!(
                        "{noun} {} is not visible from {}",
                        reference.name, definition.path
                    ),
                    locations: Vec::new(),
                    help: Some(format!(
                        "a SQL macro body resolves {noun}s from its own folder, not its caller's; make it a parameter, e.g. {example} from the model"
                    )),
                };
                let reference_text: String = text
                    .chars()
                    .skip(reference.start)
                    .take(reference.end - reference.start)
                    .collect();
                if let Some(offset) = definition.body.find(&reference_text) {
                    let position =
                        text_position(&definition.body, definition.body[..offset].chars().count());
                    error = error.at(SqlMacroLocation {
                        path: definition.path.clone(),
                        line: definition.body_line + position.line - 1,
                        column: if position.line == 1 {
                            definition.body_column + position.column - 1
                        } else {
                            position.column
                        },
                    });
                }
                return Err(ExpansionFailure::Macro(error));
            }
        }
        let expanded: ExpandedDeclarations<'py> =
            host(expand_declarations(&DeclarationExpansion {
                sql: text,
                references: &references,
                stop,
                context: &context,
                consumer: self.request.consumer.clone(),
                rendering: self.request.rendering,
                classes: self.request.classes,
                sql_macros: None,
            }))?;
        self.facts.borrow_mut().usages.extend(expanded.usages);
        let splices: Vec<TextSplice> =
            byte_text_splices(text, &expanded.spliced.sql, &expanded.spliced.spans);
        Ok(RenderedDeclarations {
            sql: expanded.spliced.sql,
            splices,
        })
    }

    fn constant_shape(
        &self,
        scope: &SqlMacroScope,
        name: &str,
    ) -> Result<ConstantShape, ExpansionFailure<PyErr>> {
        let context: Bound<'py, PyAny> = host(self.scope_context(scope))?;
        let constants: Bound<'py, PyDict> = host(mapping(&context, "constants"))?;
        let Some(declaration) = host(constants.get_item(name))? else {
            return Ok(ConstantShape::Scalar);
        };
        let kind: String = host(
            native_field(&declaration, "value")
                .and_then(|value| native_field(&value, "kind"))
                .and_then(|kind| native_field(&kind, "value"))
                .and_then(|value| value.extract()),
        )?;
        Ok(match kind.as_str() {
            "list" | "set" => ConstantShape::Collection,
            "object" => ConstantShape::Object,
            _ => ConstantShape::Scalar,
        })
    }

    fn render_collection(&self, text: &str) -> Result<String, ExpansionFailure<PyErr>> {
        Ok(render_inline_collection(text)?)
    }

    fn adapter(&self) -> &str {
        &self.adapter
    }

    fn record_check(&self, check: SqlMacroCheck, call: (usize, usize)) {
        let Some(origin) = self.source.origin else {
            return;
        };
        let (location, end) = authored_span(origin, self.request.sql, call);
        self.facts.borrow_mut().checks.push(SqlMacroCheck {
            call: Some((location, end)),
            ..check
        });
    }
}

/// The path of the declaration a reference names, as `context` sees it, if it is file-based.
fn declaration_path(
    context: &Bound<'_, PyAny>,
    kind: u8,
    name: &str,
) -> PyResult<Option<Option<String>>> {
    let (visible, inaccessible) = if kind == ENUM_REFERENCE_KIND {
        ("enums", "inaccessible_enums")
    } else {
        ("constants", "inaccessible_constants")
    };
    if let Some(declaration) = mapping(context, visible)?.get_item(name)? {
        if !native_field(&declaration, "model_name")?.is_none() {
            return Ok(Some(None));
        }
        let path: String = path_posix(&native_field(&declaration, "relative_path")?)?;
        return Ok(Some(Some(path)));
    }
    if let Some(record) = mapping(context, inaccessible)?.get_item(name)? {
        if !native_field(&native_field(&record, "identity")?, "owner")?.is_none() {
            return Ok(Some(None));
        }
        return Ok(Some(Some(native_field(&record, "path")?.extract()?)));
    }
    Ok(None)
}

/// Whether `error` reports the macro's own body rather than one call of it.
fn is_macro_file_error(error: &SqlMacroError) -> bool {
    [RETURNS_MISMATCH_CODE, BODY_TYPE_CODE].contains(&error.code)
}
