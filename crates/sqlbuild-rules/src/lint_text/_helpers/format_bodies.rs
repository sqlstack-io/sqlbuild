//! Body preparation, request building and result splicing for SQL body formatting.

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use regex::Regex;
use sqlbuild_attachments::ceremonial_select::main::omitted_select_offset::omitted_select_offset;
use sqlbuild_attachments::ceremonial_select::models::OmittedSelect;
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_core::text::main::python_splitlines::python_splitlines;
use sqlbuild_sqltext::sql_scan::main::dialect_non_code_ranges::dialect_non_code_ranges;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::dsl_headers::main::declares_table_returns::declares_table_returns;
use crate::dsl_headers::main::measurement_body_ranges::measurement_body_ranges;
use crate::dsl_headers::main::scan_headers::scan_headers;
use crate::dsl_headers::main::sql_body_ranges::sql_body_ranges;
use crate::dsl_headers::models::HeaderSpan;
use crate::lint_text::_helpers::sites::compiled;
use crate::lint_text::constants::{
    BLOCK_COMMENT_OPENER, DECLARATION_GROUP_DIRECTORY, DECLINED_WITHOUT_REASON,
    EXPRESSION_SELECT_PREFIX, FIXTURE_HEADER_KINDS, FORMAT_API_VERSION, FUNCTION_HEADER_KIND,
    GLOBAL_NAMED_ROOTS, GROUPED_NAMED_ROLES, LINE_COMMENT_PREFIX, LINTED_EXPRESSION_ALIAS,
    MACRO_HEADER_KIND, MAX_FUNCTION_CALL_DEPTH, MOVED_EXPRESSION_ALIAS, MOVED_OMITTED_SELECT,
    OMITTED_CEREMONIAL_SELECT_SQL, OMITTED_SELECT_SUFFIX_PATTERN,
};
use crate::lint_text::errors::LintTextError;
use crate::lint_text::main::neutralize_interpolation::neutralize_interpolation;
use crate::lint_text::main::restore_interpolation::restore_interpolation;
use crate::lint_text::models::{
    FormatFault, FormatFile, FormatRun, InterpolationSite, PreparedBody,
};
use sqlclean::sql_lint::models::{FormatRequest, FormatResponse};

static OMITTED_SELECT_SUFFIX: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(OMITTED_SELECT_SUFFIX_PATTERN));
/// The alias that closes a formatted expression body, with or without an `AS` before it.
static EXPRESSION_ALIAS_SUFFIX: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)\s+(?:AS\s+)?{LINTED_EXPRESSION_ALIAS}\s*\z")));

/// Deduplicated format requests, keyed by neutralized SQL and sentinel widths.
#[derive(Default)]
pub(crate) struct RequestTable {
    pub(crate) requests: Vec<FormatRequest>,
    index: HashMap<(String, Vec<(String, usize)>), usize>,
}

impl RequestTable {
    pub(crate) fn insert(
        &mut self,
        sql: String,
        widths: BTreeMap<String, usize>,
        options: (&str, usize),
    ) -> usize {
        let key: (String, Vec<(String, usize)>) = (
            sql.clone(),
            widths
                .iter()
                .map(|(name, width)| (name.clone(), *width))
                .collect(),
        );
        if let Some(index) = self.index.get(&key) {
            return *index;
        }
        self.requests.push(FormatRequest {
            version: FORMAT_API_VERSION,
            sql,
            dialect: options.0.to_string(),
            max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
            line_width: Some(options.1),
            indent: None,
            token_widths: widths.into_iter().collect(),
        });
        let _ = self.index.insert(key, self.requests.len() - 1);
        self.requests.len() - 1
    }
}

/// Whether a project-relative path lies in a generic or singular audit role.
fn is_audit_path(parts: &[String]) -> bool {
    let directories: &[String] = &parts[..parts.len().saturating_sub(1)];
    for (root, audit) in GLOBAL_NAMED_ROOTS {
        if starts_with(directories, root) {
            return audit;
        }
    }
    for (index, part) in directories.iter().enumerate() {
        if part != DECLARATION_GROUP_DIRECTORY {
            continue;
        }
        for (role, audit) in GROUPED_NAMED_ROLES {
            if starts_with(&directories[index + 1..], role) {
                return audit;
            }
        }
    }
    false
}

fn starts_with(parts: &[String], prefix: &[&str]) -> bool {
    parts.len() >= prefix.len() && parts.iter().zip(prefix).all(|(part, name)| part == name)
}

/// The lintable body ranges of one file, measurement blocks first for audits.
fn body_ranges(file: &FormatFile, text: &[char], headers: &[HeaderSpan]) -> Vec<(usize, usize)> {
    if file.relative_parts.as_deref().is_some_and(is_audit_path) {
        let ranges: Vec<(usize, usize)> = measurement_body_ranges(text);
        if !ranges.is_empty() {
            return ranges;
        }
    }
    sql_body_ranges(text, headers)
}

/// One prepared body with its neutralized SQL and sentinel widths, awaiting its request index.
pub(crate) type PendingBody = (PreparedBody, String, BTreeMap<String, usize>);

/// Prepare every body of one file for formatting.
pub(crate) fn prepare_file(
    file: &FormatFile,
    dialect: &str,
) -> Result<Vec<PendingBody>, LintTextError> {
    let text: Vec<char> = file.contents.chars().collect();
    let headers: Vec<HeaderSpan> = scan_headers(&text, false);
    let omits: bool = headers
        .iter()
        .any(|header| FIXTURE_HEADER_KINDS.contains(&header.kind.as_str()));
    let sql_macro: bool = headers
        .iter()
        .any(|header| header.kind == MACRO_HEADER_KIND);
    let scalar_function: bool = headers.iter().any(|header| {
        header.kind == FUNCTION_HEADER_KIND
            && !declares_table_returns(&text[header.start..header.end])
    });
    let syntax: LexicalSyntax = LexicalSyntax {
        line_comment_prefixes: vec![LINE_COMMENT_PREFIX.to_string()],
        ..LexicalSyntax::default()
    };
    let mut prepared: Vec<PendingBody> = Vec::new();
    for (start, end) in body_ranges(file, &text, &headers) {
        let body: &[char] = &text[start..end];
        let core_end: usize = body
            .iter()
            .rposition(|character| !python_isspace(*character))
            .map_or(0, |index| index + 1);
        let mut core: String = body[..core_end].iter().collect();
        let mut trailing: String = body[core_end..].iter().collect();
        let form: Option<MacroBodyForm> =
            (sql_macro || scalar_function).then(|| macro_body_form(&core));
        let wraps_expression: bool = matches!(form, Some(MacroBodyForm::Expression));
        let omitted: Option<usize> = if sql_macro {
            matches!(form, Some(MacroBodyForm::CteList)).then(|| core.chars().count())
        } else if omits {
            match omitted_select_offset(&core, &syntax) {
                OmittedSelect::At(offset) => Some(offset),
                OmittedSelect::Absent => None,
            }
        } else {
            None
        };
        if let Some(offset) = omitted {
            let characters: Vec<char> = core.chars().collect();
            trailing = characters[offset..].iter().collect::<String>() + &trailing;
            core = characters[..offset].iter().collect();
        }
        if wraps_expression && let Some(statement) = core.strip_suffix(';') {
            trailing = format!(";{trailing}");
            core = statement.trim_end().to_string();
        }
        let neutralized = neutralize_interpolation(&core, dialect, true)?;
        let mut sql: String = if wraps_expression {
            format!(
                "{EXPRESSION_SELECT_PREFIX}{}\n{LINTED_EXPRESSION_ALIAS}",
                neutralized.text
            )
        } else {
            neutralized.text
        };
        if omitted.is_some() {
            sql.push_str(OMITTED_CEREMONIAL_SELECT_SQL);
        }
        let mut widths: BTreeMap<String, usize> = sentinel_widths(&neutralized.sites);
        if wraps_expression {
            let _ = widths.insert(LINTED_EXPRESSION_ALIAS.to_uppercase(), 0);
        }
        let body: PreparedBody = PreparedBody {
            start,
            end,
            trailing,
            request: 0,
            sites: neutralized.sites,
            omits_select: omitted.is_some(),
            wraps_expression,
        };
        prepared.push((body, sql, widths));
    }
    Ok(prepared)
}

/// How a SQL macro or function body starts, which decides how it is formatted.
enum MacroBodyForm {
    /// A `WITH` list, formatted with a final query added and removed again.
    CteList,
    /// A `SELECT` list, formatted as it stands.
    ColumnList,
    /// An expression, formatted as the one aliased item of a `SELECT` that is removed again.
    Expression,
}

/// The form of a SQL macro or function body, read from its first word after comments.
fn macro_body_form(body: &str) -> MacroBodyForm {
    let mut rest: &str = body;
    loop {
        rest = rest.trim_start();
        if let Some(comment) = rest.strip_prefix(LINE_COMMENT_PREFIX) {
            rest = comment.split_once('\n').map_or("", |(_, after)| after);
        } else if let Some(comment) = rest.strip_prefix("/*") {
            rest = comment.split_once("*/").map_or("", |(_, after)| after);
        } else {
            break;
        }
    }
    let word: String = rest
        .chars()
        .take_while(|character| character.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    match word.as_str() {
        "WITH" => MacroBodyForm::CteList,
        "SELECT" => MacroBodyForm::ColumnList,
        _ => MacroBodyForm::Expression,
    }
}

/// The printed width of the SQLBuild syntax each sentinel stands in for, where it differs.
fn sentinel_widths(sites: &[InterpolationSite]) -> BTreeMap<String, usize> {
    sites
        .iter()
        .filter(|site| {
            site.original_text.chars().count() != site.sentinel.chars().count()
                || site.original_text.contains('\n')
        })
        .map(|site| {
            (
                site.sentinel.to_uppercase(),
                printed_width(&site.original_text),
            )
        })
        .collect()
}

/// The widest line of `text`, in characters.
fn printed_width(text: &str) -> usize {
    let lines: Vec<&str> = python_splitlines(text);
    lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

/// The formatted text replacing one body, or why the body was left unchanged.
pub(crate) fn formatted_body(
    prepared: &PreparedBody,
    response: &FormatResponse,
    syntax: &LexicalSyntax,
) -> Result<Option<String>, String> {
    if !response.formatted {
        return Err(response
            .unavailable
            .clone()
            .or_else(|| response.reason.clone())
            .filter(|reason| !reason.is_empty())
            .unwrap_or_else(|| DECLINED_WITHOUT_REASON.to_string()));
    }
    let mut sql: &str = &response.sql;
    let mut changed: bool = response.changed;
    if prepared.omits_select {
        let pattern: &Regex =
            compiled(&OMITTED_SELECT_SUFFIX).map_err(|error| error.to_string())?;
        let Some(suffix) = pattern.find(sql) else {
            return Err(MOVED_OMITTED_SELECT.to_string());
        };
        sql = &sql[..suffix.start()];
        changed = true;
    }
    let unwrapped: String;
    if prepared.wraps_expression {
        let pattern: &Regex =
            compiled(&EXPRESSION_ALIAS_SUFFIX).map_err(|error| error.to_string())?;
        let Some(alias) = pattern.find(sql) else {
            return Err(MOVED_EXPRESSION_ALIAS.to_string());
        };
        unwrapped = unwrapped_expression(&sql[..alias.start()], syntax)
            .ok_or_else(|| MOVED_EXPRESSION_ALIAS.to_string())?;
        sql = &unwrapped;
        changed = true;
    }
    if !changed {
        return Ok(None);
    }
    restore_interpolation(sql, &prepared.sites)
        .map(|restored| Some(restored + &prepared.trailing))
        .map_err(|error| error.to_string())
}

/// One file's text with every formatted body spliced in, if it changed, and its declined bodies.
pub(crate) fn splice_file(
    index: usize,
    contents: &str,
    bodies: &[PreparedBody],
    run: &FormatRun<'_>,
) -> (Option<String>, Vec<FormatFault>) {
    let original: Vec<char> = contents.chars().collect();
    let mut updated: Vec<char> = original.clone();
    let mut faults: Vec<FormatFault> = Vec::new();
    for body in bodies.iter().rev() {
        match formatted_body(body, &run.responses[body.request], run.syntax) {
            Ok(Some(replacement)) => {
                let _ = updated.splice(body.start..body.end, replacement.chars());
            }
            Ok(None) => {}
            Err(reason) => faults.push(FormatFault {
                file: index,
                line: original[..body.start]
                    .iter()
                    .filter(|character| **character == '\n')
                    .count()
                    + 1,
                reason,
                unavailable: run.responses[body.request].unavailable.is_some(),
            }),
        }
    }
    let formatted: Option<String> = (updated != original).then(|| updated.into_iter().collect());
    (formatted, faults)
}

/// The expression of a formatted one-item `SELECT`, dedented except inside comments and strings.
pub(crate) fn unwrapped_expression(formatted: &str, syntax: &LexicalSyntax) -> Option<String> {
    let trimmed: &str = formatted.trim_start();
    let keyword: &str = trimmed.get(..EXPRESSION_SELECT_PREFIX.trim_end().len())?;
    if !keyword.eq_ignore_ascii_case(EXPRESSION_SELECT_PREFIX.trim_end()) {
        return None;
    }
    let rest: &str = &trimmed[keyword.len()..];
    let opaque: Vec<(usize, usize)> = dialect_non_code_ranges(rest, syntax);
    let mut offset: usize = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let mut lines: Vec<(usize, &str)> = Vec::new();
    for line in rest[offset..].split('\n') {
        lines.push((offset, line.strip_suffix('\r').unwrap_or(line)));
        offset += line.len() + 1;
    }
    if lines.len() > 1 && lines[0].1.trim().is_empty() {
        let _ = lines.remove(0);
    }
    let (_, first): (usize, &str) = lines[0];
    let code: &str = lines
        .iter()
        .find(|&&(start, line)| {
            let lead: usize = start + line.len() - line.trim_start().len();
            !line.trim().is_empty() && !in_comment(rest, &opaque, lead, syntax)
        })
        .map_or(first, |&(_, line)| line);
    let indent: usize = code.len() - code.trim_start().len();
    let mut ranges = opaque.iter().peekable();
    let mut dedented: Vec<&str> = vec![first.trim_start()];
    for &(start, line) in &lines[1..] {
        while ranges.next_if(|&&(_, end)| end <= start).is_some() {}
        if ranges.peek().is_some_and(|&&(open, _)| open < start) {
            dedented.push(line);
        } else {
            let leading: usize = line.len() - line.trim_start().len();
            dedented.push(&line[leading.min(indent)..]);
        }
    }
    Some(dedented.join("\n").trim_end().to_owned())
}

/// Whether offset `at` of `text` lies in one of its non-code `ranges` that is a comment.
fn in_comment(text: &str, ranges: &[(usize, usize)], at: usize, syntax: &LexicalSyntax) -> bool {
    ranges
        .iter()
        .any(|&(open, end)| open <= at && at < end && is_comment(&text[open..], syntax))
}

/// Whether the non-code text at the start of `text` is a comment rather than quoted text.
fn is_comment(text: &str, syntax: &LexicalSyntax) -> bool {
    text.starts_with(BLOCK_COMMENT_OPENER)
        || syntax
            .line_comment_prefixes
            .iter()
            .any(|prefix| text.starts_with(prefix.as_str()))
}
