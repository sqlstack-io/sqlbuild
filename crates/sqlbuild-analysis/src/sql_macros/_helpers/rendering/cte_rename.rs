//! A CTE-list macro body's CTEs renamed for one call's `__cte` name, located by parsing.

use sqlquack_syntax::expressions::names::models::{Identifier, TableRef};
use sqlquack_syntax::expressions::queries::models::With;
use sqlquack_syntax::{Expression, ExpressionWalk, Parsed};

use crate::analysis_route::types::NativeDialect;

use crate::sql_macros::_helpers::parsing::lexing::{Token, tokens};
use crate::sql_macros::_helpers::parsing::parameters::{
    ParameterReference, parameter_references, placeholder, with_placeholders,
};
use crate::sql_macros::_helpers::typing::typed_calls::spliced;
use crate::sql_macros::constants::{ARGUMENT_CODE, CTE_ARGUMENT, CTE_NAME_SEPARATOR};
use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::models::{
    CallArgument, ExpandedText, SqlMacroDefinition, SqlMacroForm, TextSplice,
};

/// The query a CTE list is completed with to parse it.
const CTE_SUFFIX: &str = "\nSELECT 1";

/// The call's `__cte` name, removed from its arguments, or why it cannot be used.
pub(crate) fn take_cte_argument(
    definition: &SqlMacroDefinition,
    arguments: Vec<CallArgument>,
) -> Result<(Option<String>, Vec<CallArgument>), SqlMacroError> {
    let mut name: Option<String> = None;
    let mut kept: Vec<CallArgument> = Vec::with_capacity(arguments.len());
    for argument in arguments {
        if !argument
            .name
            .as_deref()
            .is_some_and(|named| named.eq_ignore_ascii_case(CTE_ARGUMENT))
        {
            kept.push(argument);
            continue;
        }
        if definition.form != SqlMacroForm::CteList {
            return Err(SqlMacroError::new(
                ARGUMENT_CODE,
                format!(
                    "{CTE_ARGUMENT} names the CTEs a CTE-list macro inserts; {}() is {} {} macro",
                    definition.name,
                    if definition.form == SqlMacroForm::Expression {
                        "an"
                    } else {
                        "a"
                    },
                    definition.form.label()
                ),
            )
            .helped(format!("remove {CTE_ARGUMENT} := {}", argument.text.trim())));
        }
        if name.is_some() {
            return Err(SqlMacroError::new(
                ARGUMENT_CODE,
                format!(
                    "{CTE_ARGUMENT} is passed twice in a call to macro {}()",
                    definition.name
                ),
            ));
        }
        let text: &str = argument.text.trim();
        if !is_bare_identifier(text) {
            return Err(SqlMacroError::new(
                ARGUMENT_CODE,
                format!(
                    "{CTE_ARGUMENT} of macro {}() must be a bare CTE name; got {text}",
                    definition.name
                ),
            )
            .helped(format!("pass a name such as {CTE_ARGUMENT} := eu_orders")));
        }
        name = Some(text.to_owned());
    }
    Ok((name, kept))
}

/// Whether `text` is one unquoted identifier.
fn is_bare_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// The name each of a body's CTEs takes: the last takes `name`, the others `<name>__<cte>`.
fn renamed(ctes: &[String], name: &str, cte: &str) -> Option<String> {
    let index: usize = ctes
        .iter()
        .position(|defined| defined.eq_ignore_ascii_case(cte))?;
    Some(if index + 1 == ctes.len() {
        name.to_owned()
    } else {
        format!("{name}{CTE_NAME_SEPARATOR}{}", ctes[index])
    })
}

/// `body` with every CTE it defines, and every reference to one, renamed for `name`.
pub(crate) fn renamed_ctes(
    parser: &dyn NativeDialect,
    body: &ExpandedText,
    name: &str,
) -> Result<ExpandedText, String> {
    let sql: &str = &body.sql;
    let scanned: Vec<Token> =
        tokens(sql).map_err(|_| "it has an unclosed quote or comment".to_owned())?;
    let references: Vec<ParameterReference> = parameter_references(sql, &scanned);
    let parsed_text: String = format!("{}{CTE_SUFFIX}", with_placeholders(sql, &references));
    let statement: Expression = parsed(parser, &parsed_text)?;
    let Expression::Select(select) = &statement else {
        return Err("it is not a WITH list".to_owned());
    };
    let Some(with) = &select.with else {
        return Err("it is not a WITH list".to_owned());
    };
    let ctes: Vec<String> = with.ctes.iter().map(|cte| cte.alias.name.clone()).collect();
    let mut identifiers: Vec<&Identifier> = with.ctes.iter().map(|cte| &cte.alias).collect();
    let mut walk = CteReferences {
        frames: Vec::new(),
        scopes: Vec::new(),
        found: Vec::new(),
    };
    walk.visit(&statement, true);
    identifiers.extend(walk.found);
    let offsets: CharOffsets = CharOffsets::new(&parsed_text);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for identifier in identifiers {
        let Some(new_name) = renamed(&ctes, name, &identifier.name) else {
            continue;
        };
        let Some(span) = identifier.span else {
            return Err(format!(
                "the {} parser gives no position for CTE name {}, so it cannot be renamed",
                parser.name(),
                identifier.name
            ));
        };
        let start: usize = original_offset(offsets.byte(span.start), sql, &references);
        let end: usize = original_offset(offsets.byte(span.end), sql, &references);
        let text: String = if identifier.quoted {
            format!("\"{new_name}\"")
        } else {
            new_name
        };
        edits.push((start, end, text));
    }
    edits.sort_by_key(|(start, _, _)| *start);
    edits.dedup_by_key(|(start, _, _)| *start);
    let mut renamed_sql: String = String::with_capacity(sql.len() + edits.len() * name.len());
    let mut splices: Vec<TextSplice> = Vec::with_capacity(edits.len());
    let mut cursor: usize = 0;
    for (start, end, text) in edits {
        if start < cursor || end > sql.len() {
            continue;
        }
        renamed_sql.push_str(&sql[cursor..start]);
        let output: usize = renamed_sql.len();
        renamed_sql.push_str(&text);
        splices.push(TextSplice {
            source: (start, end),
            output: (output, renamed_sql.len()),
        });
        cursor = end;
    }
    renamed_sql.push_str(&sql[cursor..]);
    Ok(ExpandedText {
        calls: spliced(body.calls.clone(), &splices),
        sql: renamed_sql,
    })
}

/// One relation a query reads by name: the name it is read as, and whether that is the body's CTE.
struct ScopeSource {
    name: String,
    body_cte: bool,
}

/// The table names and qualifiers that read a body's own CTEs, resolved scope by scope.
struct CteReferences<'a> {
    /// The CTE names of each enclosing `WITH`, innermost last, and whether they are the body's.
    frames: Vec<(Vec<String>, bool)>,
    /// The relations of each enclosing query, innermost last.
    scopes: Vec<Vec<ScopeSource>>,
    found: Vec<&'a Identifier>,
}

impl<'a> CteReferences<'a> {
    fn visit(&mut self, node: &'a Expression, top: bool) {
        let with = match node {
            Expression::Select(select) => select.with.as_ref(),
            Expression::Union(union) => union.with.as_ref(),
            Expression::Intersect(intersect) => intersect.with.as_ref(),
            Expression::Except(except) => except.with.as_ref(),
            _ => None,
        };
        if let Some(with) = with {
            let names: Vec<String> = with.ctes.iter().map(|cte| cte.alias.name.clone()).collect();
            for (index, cte) in with.ctes.iter().enumerate() {
                let visible: usize = if with.recursive { index + 1 } else { index };
                self.frames.push((names[..visible].to_vec(), top));
                self.visit(&cte.this, false);
                self.frames.pop();
            }
            self.frames.push((names, top));
        }
        let scoped: bool = if let Expression::Select(select) = node {
            let mut sources: Vec<ScopeSource> = Vec::new();
            for source in select
                .from
                .iter()
                .flat_map(|from| &from.expressions)
                .chain(select.joins.iter().map(|join| &join.this))
            {
                sources.extend(self.scope_sources(source));
            }
            self.scopes.push(sources);
            true
        } else {
            false
        };
        match node {
            Expression::Table(table) if self.is_body_cte(table) => self.found.push(&table.name),
            Expression::Column(column) => self.qualifier(column.table.as_ref()),
            Expression::Star(star) => self.qualifier(star.table.as_ref()),
            _ => {}
        }
        for child in node.children() {
            if !is_cte_body(with, child) {
                self.visit(child, false);
            }
        }
        if scoped {
            self.scopes.pop();
        }
        if with.is_some() {
            self.frames.pop();
        }
    }

    /// Whether `table` is an unqualified name whose innermost binding is one of the body's CTEs.
    fn is_body_cte(&self, table: &TableRef) -> bool {
        if table.schema.is_some() || table.catalog.is_some() {
            return false;
        }
        self.frames
            .iter()
            .rev()
            .find(|(names, _)| includes_name(names, &table.name.name))
            .is_some_and(|(_, body)| *body)
    }

    /// Each name a FROM or JOIN item is read as, and whether it is an unaliased body CTE.
    fn scope_sources(&self, source: &Expression) -> Vec<ScopeSource> {
        let aliased = |alias: &Identifier| ScopeSource {
            name: alias.name.clone(),
            body_cte: false,
        };
        match source {
            Expression::Table(table) => vec![match &table.alias {
                Some(alias) => aliased(alias),
                None => ScopeSource {
                    name: table.name.name.clone(),
                    body_cte: self.is_body_cte(table),
                },
            }],
            Expression::Subquery(subquery) => subquery.alias.iter().map(aliased).collect(),
            Expression::JoinedTable(joined) => match &joined.alias {
                Some(alias) => vec![aliased(alias)],
                None => {
                    let mut sources: Vec<ScopeSource> = self.scope_sources(&joined.left);
                    for join in &joined.joins {
                        sources.extend(self.scope_sources(&join.this));
                    }
                    sources
                }
            },
            Expression::Paren(paren) => self.scope_sources(&paren.this),
            _ => Vec::new(),
        }
    }

    /// Records `qualifier` when the relation it names, innermost scope first, is a body CTE.
    fn qualifier(&mut self, qualifier: Option<&'a Identifier>) {
        let Some(qualifier) = qualifier else {
            return;
        };
        let source: Option<&ScopeSource> = self
            .scopes
            .iter()
            .rev()
            .find_map(|sources| source_named(sources, &qualifier.name));
        if source.is_some_and(|source| source.body_cte) {
            self.found.push(qualifier);
        }
    }
}

/// Whether `child` is the body of one of `with`'s CTEs, which are visited in their own frames.
fn is_cte_body(with: Option<&With>, child: &Expression) -> bool {
    let Some(with) = with else {
        return false;
    };
    for cte in &with.ctes {
        if std::ptr::eq(&cte.this, child) {
            return true;
        }
    }
    false
}

/// Whether `names` holds `name`, ignoring case.
fn includes_name(names: &[String], name: &str) -> bool {
    names
        .iter()
        .any(|defined| defined.eq_ignore_ascii_case(name))
}

/// The relation of `sources` read as `name`, ignoring case.
fn source_named<'s>(sources: &'s [ScopeSource], name: &str) -> Option<&'s ScopeSource> {
    sources
        .iter()
        .find(|source| source.name.eq_ignore_ascii_case(name))
}

/// The one statement `text` parses to in the project's parser, or why it cannot read it.
fn parsed(parser: &dyn NativeDialect, text: &str) -> Result<Expression, String> {
    let Parsed {
        mut statements,
        gaps,
    } = parser.parse(text).map_err(|gap| gap.message)?;
    if let Some(gap) = gaps.first() {
        return Err(format!(
            "the {} parser cannot read {}",
            parser.name(),
            gap.construct
        ));
    }
    match (statements.pop(), statements.is_empty()) {
        (Some(statement), true) => Ok(statement),
        _ => Err("it holds more than one statement".to_owned()),
    }
}

/// The byte offset in `sql` of byte `offset` of `sql` with its references as placeholders.
fn original_offset(offset: usize, sql: &str, references: &[ParameterReference]) -> usize {
    let mut moved: isize = 0;
    for reference in references {
        let placed_start: isize = reference.start as isize + moved;
        if (offset as isize) <= placed_start {
            break;
        }
        moved += placeholder(&reference.name).len() as isize
            - (reference.end - reference.start) as isize;
    }
    usize::try_from(offset as isize - moved)
        .unwrap_or(0)
        .min(sql.len())
}

/// Byte offsets of a text's character offsets.
struct CharOffsets {
    bytes: Option<Vec<usize>>,
}

impl CharOffsets {
    fn new(text: &str) -> Self {
        if text.is_ascii() {
            return Self { bytes: None };
        }
        let mut bytes: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
        bytes.push(text.len());
        Self { bytes: Some(bytes) }
    }

    fn byte(&self, character: usize) -> usize {
        match &self.bytes {
            Some(bytes) => bytes.get(character).copied().unwrap_or(character),
            None => character,
        }
    }
}
