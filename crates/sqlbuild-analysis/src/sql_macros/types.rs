//! What SQL macro expansion asks of its host: definitions, declarations and rendering.

use std::sync::Arc;

use crate::sql_macros::errors::ExpansionFailure;
use crate::sql_macros::models::{RenderedDeclarations, SqlMacroCheck, SqlMacroDefinition};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// Whose scope a piece of SQL resolves its declarations and macros in.
#[derive(Clone, Debug)]
pub enum SqlMacroScope {
    /// The calling resource's own scope.
    Caller,
    /// A SQL macro file's scope: its body and its defaults.
    Macro(Arc<SqlMacroDefinition>),
}

/// What a constant reference names, as a typed argument checks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstantShape {
    Scalar,
    Collection,
    Object,
}

/// The project facts SQL macro expansion reads, and the usages it records as it reads them.
pub trait SqlMacroHost {
    type Error;

    /// The SQL macro `name` visible from `scope`, recorded as used by the caller.
    fn definition(
        &self,
        scope: &SqlMacroScope,
        name: &str,
    ) -> Result<Arc<SqlMacroDefinition>, ExpansionFailure<Self::Error>>;

    /// `text` with its `@const` and `@enum` references rendered from `scope`, and where each landed.
    fn render_declarations(
        &self,
        scope: &SqlMacroScope,
        text: &str,
    ) -> Result<RenderedDeclarations, ExpansionFailure<Self::Error>>;

    /// What the constant `name` visible from `scope` holds.
    fn constant_shape(
        &self,
        scope: &SqlMacroScope,
        name: &str,
    ) -> Result<ConstantShape, ExpansionFailure<Self::Error>>;

    /// An inline `[...]` list or `{...}` set rendered as a constant of that value would be.
    fn render_collection(&self, text: &str) -> Result<String, ExpansionFailure<Self::Error>>;

    /// The project's adapter name for type rules and literal positions; empty for generic SQL.
    fn adapter(&self) -> &str {
        ""
    }

    /// The lexical rules the project's SQL is authored in; `None` reads it under generic rules.
    fn lexical_syntax(&self) -> Option<&LexicalSyntax> {
        None
    }

    /// Keep what the caller's analysed SQL must show for a call the caller wrote to fit.
    fn record_check(&self, _check: SqlMacroCheck, _call: (usize, usize)) {}
}

/// The macros on the current call path, and the macros already known to be cycle-free.
pub(crate) type Walk<'a> = (Vec<&'a str>, Vec<&'a str>);

/// A probe's `(column, type)` list, or `None` where its analysis did not know every column.
pub type ProbeColumns = Option<Vec<(String, Option<String>)>>;
/// Each bound relation's name and `(column, type)` list.
pub type RelationShapes = [(String, Vec<(String, String)>)];
/// One model's analysed SQL, its bound relations and its SQL macro calls' checks.
pub type ArgumentCheckInput<'a> = (&'a str, &'a RelationShapes, &'a [SqlMacroCheck]);
