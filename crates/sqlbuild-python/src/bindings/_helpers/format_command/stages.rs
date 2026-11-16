//! The per-file native work of a format run: header lint and header formatting.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rayon::ThreadPool;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_analysis::sql_macros::errors::SqlMacroError;
use sqlbuild_analysis::sql_macros::main::parse_sql_macro::parse_sql_macro;
use sqlbuild_core::format_command::main::collect_lint_files::collect_lint_files;
use sqlbuild_rules::dsl_headers::constants::RULE_HEADER_PARSE;
use sqlbuild_rules::dsl_headers::main::fix_escaped_quotes::fix_escaped_quotes;
use sqlbuild_rules::dsl_headers::main::format_headers::format_headers;
use sqlbuild_rules::dsl_headers::main::scan_headers::scan_headers;
use sqlbuild_rules::dsl_headers::models::{HeaderRulesConfig, HeaderViolation};

use sqlbuild_python_compile::rules::main::checking::header_violations::header_violations;
use sqlbuild_python_compile::rules::models::NativeHeaderParser;

/// The header kind of a SQL macro file.
const MACRO_HEADER_KIND: &str = "MACRO";
/// The name and path a rewritten macro file is checked under; neither affects whether it parses.
const MACRO_CHECK_NAME: &str = "rewritten";
const MACRO_CHECK_PATH: &str = "rewritten.sql";

/// Parsers recurse deeply on nested SQL, so format workers get the stack the main thread has.
const FORMAT_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

static FORMAT_WORKERS: OnceLock<Result<ThreadPool, String>> = OnceLock::new();

/// Run `operation` on the format worker threads, which every parallel pass inside it uses.
pub(crate) fn on_format_workers<T: Send>(
    operation: impl FnOnce() -> Result<T, String> + Send,
) -> Result<T, String> {
    FORMAT_WORKERS
        .get_or_init(|| {
            rayon::ThreadPoolBuilder::new()
                .thread_name(|index| format!("sqlbuild-format-{index}"))
                .stack_size(FORMAT_WORKER_STACK_BYTES)
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)?
        .install(operation)
}

/// Every header rule violation of `text`.
pub(crate) fn lint_header_text(
    text: &str,
    config: HeaderRulesConfig,
) -> Result<Vec<HeaderViolation>, String> {
    let characters: Vec<char> = text.chars().collect();
    header_violations(&characters, &scan_headers(&characters, false), config)
}

/// The header-parse faults among `violations`.
pub(crate) fn parse_faults(violations: &[HeaderViolation]) -> Vec<HeaderViolation> {
    violations
        .iter()
        .filter(|violation| violation.code == RULE_HEADER_PARSE)
        .cloned()
        .collect()
}

/// The description quote fixes of one file.
#[derive(Clone)]
pub(crate) struct QuoteFix {
    /// The rewritten file, or `None` when it would still not parse, so nothing is rewritten.
    pub(crate) text: Option<String>,
    /// The one-based lines of the descriptions the fixes rewrite.
    pub(crate) lines: Vec<usize>,
}

/// Whether every header of `text`, `MACRO` headers included, parses.
fn headers_parse(text: &str, config: HeaderRulesConfig) -> Result<bool, String> {
    if !parse_faults(&lint_header_text(text, config)?).is_empty() {
        return Ok(false);
    }
    let characters: Vec<char> = text.chars().collect();
    let defines_macro: bool = scan_headers(&characters, false)
        .iter()
        .any(|header| header.kind == MACRO_HEADER_KIND);
    if !defines_macro {
        return Ok(true);
    }
    match parse_sql_macro(MACRO_CHECK_NAME, MACRO_CHECK_PATH, text) {
        Ok(_) => Ok(true),
        Err(SqlMacroError { .. }) => Ok(false),
    }
}

/// The description quote fixes of `text`, applied only when the whole rewritten file parses.
pub(crate) fn quote_fixed_text(
    text: &str,
    config: HeaderRulesConfig,
) -> Result<Option<QuoteFix>, String> {
    let characters: Vec<char> = text.chars().collect();
    let (fixed, lines) = fix_escaped_quotes(&characters);
    if lines.is_empty() {
        return Ok(None);
    }
    let fixed: String = fixed.into_iter().collect();
    let parses: bool = headers_parse(&fixed, config)?;
    Ok(Some(QuoteFix {
        text: parses.then_some(fixed),
        lines,
    }))
}

/// The number of authored SQL files of the project with a description escaping or doubling quotes.
pub(crate) fn quote_fixable_file_count(directory: &Path) -> Result<usize, String> {
    let files: Vec<(PathBuf, String)> =
        collect_lint_files(directory, None).map_err(|error| error.to_string())?;
    Ok(files
        .iter()
        .filter(|(_, text)| {
            (text.contains("\\\"") || text.contains("\"\"")) && escapes_quotes(text)
        })
        .count())
}

/// Whether a header description of `text` escapes or doubles quotes that a fix can rewrite.
fn escapes_quotes(text: &str) -> bool {
    !fix_escaped_quotes(&text.chars().collect::<Vec<char>>())
        .1
        .is_empty()
}

/// `text` with header formatting fixes applied, and the faults relocation reported.
pub(crate) fn format_header_text(
    text: &str,
    config: HeaderRulesConfig,
) -> Result<(String, Vec<HeaderViolation>), String> {
    let characters: Vec<char> = text.chars().collect();
    let (formatted, faults) = format_headers(&characters, config, &NativeHeaderParser)?;
    Ok((formatted.into_iter().collect(), faults))
}

/// `operation` over every item on the worker threads; results and the first error keep item order.
pub(crate) fn each_file<T: Sync, R: Send>(
    items: &[T],
    operation: impl Fn(&T) -> Result<R, String> + Sync + Send,
) -> Result<Vec<R>, String> {
    let results: Vec<Result<R, String>> = items.par_iter().map(operation).collect();
    results.into_iter().collect()
}
