//! Format every authored SQL file and collect the violations that remain.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods};
use pyo3::{IntoPyObject, IntoPyObjectExt};
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_core::format_command::main::collect_lint_files::collect_lint_files;
use sqlbuild_rules::dsl_headers::models::{HeaderRulesConfig, HeaderViolation};
use sqlbuild_rules::lint_text::main::format_sql_bodies::format_sql_bodies;
use sqlbuild_rules::lint_text::main::reject_inline_suppressions::reject_inline_suppressions;
use sqlbuild_rules::lint_text::models::{
    FormatFault, FormatFile, FormattedBodies, SuppressionFinding,
};

use crate::bindings::_helpers::format_command::cache::FormatCache;
use crate::bindings::_helpers::format_command::files::{
    FormatChange, PendingWrite, persist_changes,
};
use crate::bindings::_helpers::format_command::stages::{
    QuoteFix, each_file, format_header_text, lint_header_text, on_format_workers, parse_faults,
    quote_fixed_text,
};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::compile_driver::constants::FORMAT_CACHE_HITS;
use sqlbuild_python_base::compile_driver::constants::FORMAT_CACHE_MISSES;
use sqlbuild_python_base::compile_driver::constants::FORMAT_LINT_BATCHES;
use sqlbuild_python_base::compile_driver::constants::FORMAT_LINT_FILES;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_compile::rules::main::checking::rules_config::rules_config;
use sqlbuild_python_compile::rules::main::lint::project_relative_path::project_relative_path;
use sqlbuild_python_compile::rules::main::lint::relative_parts::relative_parts;
use sqlbuild_python_compile::rules::main::lint::unsafe_fault::unsafe_fault;
use sqlbuild_python_compile::rules::main::sql::format_redundant_fixture_nulls::format_redundant_fixture_nulls;
use sqlbuild_python_compile::rules::models::LintViolationRow;
use sqlbuild_python_compile::rules::models::ViolationClasses;
use sqlbuild_python_front::discovery::main::records::externally_described_models::externally_described_models;

const DESCRIPTION_PRESENT: &str = "description-present";
/// The fix that rewrites descriptions escaping or doubling double quotes as `"""` strings.
const DESCRIPTION_QUOTES: &str = "description-quotes";
const DESCRIPTION_QUOTES_REASON: &str =
    "Escaped or doubled double quotes rewritten as a \"\"\"...\"\"\" string holding real quotes";
const DESCRIPTION_QUOTES_REFUSED_REASON: &str =
    "Quotes left as written: the file has header syntax errors the rewrite does not fix";
const APPLIED: &str = "applied";
const REFUSED: &str = "refused";
const LINE_FEED: &str = "\n";
const CARRIAGE_RETURN_LINE_FEED: &str = "\r\n";

/// What one format run formats and how.
pub(crate) struct FormatRequest<'a, 'py> {
    pub(crate) project_dir: &'a Bound<'py, PyAny>,
    pub(crate) config: &'a Bound<'py, PyAny>,
    pub(crate) adapter: Option<&'a Bound<'py, PyAny>>,
    pub(crate) selected: Option<&'a BTreeSet<PathBuf>>,
    pub(crate) discovered: Option<&'a Bound<'py, PyAny>>,
    pub(crate) fixtures_only: bool,
    pub(crate) write: bool,
    pub(crate) fix: bool,
    /// Whether files a previous run found clean are skipped, and clean files remembered.
    pub(crate) cache: bool,
    /// Why `--fix` could not discover the project, which Rule fixes need.
    pub(crate) discovery_error: Option<&'a PyErr>,
}

/// The outcome of one format run, with Python violations and Rule fix reports.
pub(crate) struct FormatOutcome<'py> {
    pub(crate) files_checked: usize,
    pub(crate) violations: Vec<Bound<'py, PyAny>>,
    pub(crate) changes: Vec<FormatChange<'py>>,
    pub(crate) rule_fixes: Vec<Bound<'py, PyAny>>,
    /// Paths whose formatting was declined, so their applied fixes were not kept.
    pub(crate) declined: BTreeSet<PathBuf>,
    /// The final text of every collected file a violation may quote.
    pub(crate) final_contents: HashMap<PathBuf, String>,
}

/// One file the run formats: where it is, the text read, and whether it was collected.
struct Entry {
    path: PathBuf,
    original: String,
    /// False for a file only Rule fixes introduced: it is written, but neither linted nor shown.
    collected: bool,
}

/// A violation, kept as native data until the report needs Python records.
enum Finding<'py> {
    Header(usize, HeaderViolation),
    Unsafe(usize, FormatFault),
    Suppression(usize, SuppressionFinding),
    Python(Bound<'py, PyAny>),
}

/// Rule fix reports and the faults of fixes that could not be kept.
type RuleFixOutcome<'py> = (Vec<Bound<'py, PyAny>>, Vec<Bound<'py, PyAny>>);

/// Changed texts by entry, and the findings of declined bodies.
type FormattedTexts<'py> = (Vec<(usize, String)>, Vec<Finding<'py>>);

/// Rewritten texts by entry, in the order each entry was first rewritten.
#[derive(Default)]
struct Rewrites {
    texts: HashMap<usize, String>,
    order: Vec<usize>,
}

impl Rewrites {
    fn set(&mut self, index: usize, text: String) {
        if self.texts.insert(index, text).is_none() {
            self.order.push(index);
        }
    }

    fn get(&self, index: usize) -> Option<&str> {
        self.texts.get(&index).map(String::as_str)
    }
}

/// The rewrites a run kept, the faults that declined the others, and the lint of each rewrite.
struct Applied<'py> {
    rewrites: Rewrites,
    rejected: BTreeSet<usize>,
    faults: Vec<Finding<'py>>,
    rewrite_lints: HashMap<usize, Vec<HeaderViolation>>,
}

/// The native state of one format run.
struct Run<'a, 'r, 'py> {
    request: &'r FormatRequest<'a, 'py>,
    py: Python<'py>,
    directory: PathBuf,
    config: HeaderRulesConfig,
    entries: Vec<Entry>,
    index: HashMap<PathBuf, usize>,
    /// The entries headers parse in, which Rule fixes and formatting rewrite.
    safe: Vec<usize>,
    /// Texts Rule fixes changed.
    fixed: HashMap<usize, String>,
}

fn native<T: Send>(
    py: Python<'_>,
    operation: impl FnOnce() -> Result<T, String> + Send,
) -> PyResult<T> {
    py.compiler_detach(|| on_format_workers(operation))
        .map_err(value_error)
}

impl<'py> Run<'_, '_, 'py> {
    /// The text formatting starts from: the Rule-fixed text, else the text read.
    fn base_text(&self, index: usize) -> &str {
        self.fixed
            .get(&index)
            .map_or(self.entries[index].original.as_str(), String::as_str)
    }

    fn text<'t>(&'t self, index: usize, rewrites: &'t Rewrites) -> &'t str {
        rewrites.get(index).unwrap_or_else(|| self.base_text(index))
    }

    fn sorted(&self, indices: &[usize]) -> Vec<usize> {
        let mut sorted: Vec<usize> = indices.to_vec();
        sorted.sort_by(|left, right| self.entries[*left].path.cmp(&self.entries[*right].path));
        sorted
    }

    /// The entry of `path`, adding one for a path the run did not collect.
    fn entry_of(&mut self, path: PathBuf) -> usize {
        if let Some(index) = self.index.get(&path) {
            return *index;
        }
        let index: usize = self.entries.len();
        let _ = self.index.insert(path.clone(), index);
        self.entries.push(Entry {
            path,
            original: String::new(),
            collected: false,
        });
        index
    }

    /// `{Path: text}` of the safe entries, as the Python formatting passes read them.
    fn safe_dictionary(&self, rewrites: &Rewrites) -> PyResult<Bound<'py, PyDict>> {
        let dictionary: Bound<'py, PyDict> = PyDict::new(self.py);
        for index in &self.safe {
            dictionary.set_item(&self.entries[*index].path, self.text(*index, rewrites))?;
        }
        Ok(dictionary)
    }

    /// Rule fixes planned in memory and verified by compilation: reports and faults.
    fn plan_rule_fixes(&mut self) -> PyResult<RuleFixOutcome<'py>> {
        let py: Python<'py> = self.py;
        let request: &FormatRequest<'_, 'py> = self.request;
        let none = || py.None().into_bound(py);
        let result: Bound<'py, PyAny> =
            sqlbuild_python_compile::rules::main::fix::plan_rule_fixes::plan_rule_fixes(
                py,
                Some(keywords(
                    py,
                    &[
                        ("project_dir", request.project_dir.clone()),
                        (
                            "files",
                            self.safe_dictionary(&Rewrites::default())?.into_any(),
                        ),
                        ("config", request.config.clone()),
                        ("adapter", request.adapter.map_or_else(none, Clone::clone)),
                        (
                            "discovered_inputs",
                            request.discovered.map_or_else(none, Clone::clone),
                        ),
                        ("enabled", PyBool::new(py, true).to_owned().into_any()),
                    ],
                )?),
            )?;
        let mut safe: Vec<usize> = Vec::new();
        for (path, contents) in result.get_item(0)?.cast_into::<PyDict>()?.iter() {
            let index: usize = self.entry_of(path.extract()?);
            let text: String = contents.extract()?;
            if text != self.entries[index].original || !self.entries[index].collected {
                let _ = self.fixed.insert(index, text);
            }
            safe.push(index);
        }
        self.safe = safe;
        Ok((
            result.get_item(1)?.try_iter()?.collect::<PyResult<_>>()?,
            result.get_item(2)?.try_iter()?.collect::<PyResult<_>>()?,
        ))
    }

    /// Fixture-null rewrites proven safe by the contracts, by entry, in the order Python made them.
    fn fixture_autofix(
        &mut self,
        (discovered, syntax): &(Bound<'py, PyAny>, Bound<'py, PyAny>),
        rewrites: &Rewrites,
    ) -> PyResult<Vec<(usize, String)>> {
        let updated: Bound<'py, PyAny> = format_redundant_fixture_nulls(
            &self.safe_dictionary(rewrites)?,
            &self.request.project_dir.clone(),
            &discovered.clone(),
            &syntax.clone(),
        )?
        .into_bound_py_any(self.py)?;
        let mut texts: Vec<(usize, String)> = Vec::new();
        for (path, contents) in updated.cast_into::<PyDict>()?.iter() {
            let index: usize = self.entry_of(path.extract()?);
            texts.push((index, contents.extract()?));
        }
        Ok(texts)
    }

    /// The SQL bodies of `items` formatted natively: changed texts and declined-body faults.
    fn format_bodies(&self, items: &[(usize, &str)]) -> PyResult<FormattedTexts<'py>> {
        count_stage(self.py, FORMAT_LINT_BATCHES, 1)?;
        count_stage(self.py, FORMAT_LINT_FILES, items.len() as u64)?;
        let inputs: Vec<FormatFile> = items
            .iter()
            .map(|(index, contents)| FormatFile {
                relative_parts: relative_parts(&self.entries[*index].path, &self.directory),
                contents: (*contents).to_owned(),
            })
            .collect();
        let dialect: String = native_field(self.request.config, "dialect")?.extract()?;
        let line_width: usize = native_field(self.request.config, "line_width")?.extract()?;
        let result: FormattedBodies = native(self.py, || {
            format_sql_bodies(&inputs, &dialect, line_width).map_err(|error| error.to_string())
        })?;
        Ok((
            result
                .formatted
                .into_iter()
                .map(|(position, text)| (items[position].0, text))
                .collect(),
            result
                .faults
                .into_iter()
                .map(|fault| Finding::Unsafe(items[fault.file].0, fault))
                .collect(),
        ))
    }

    /// Contents after header, SQL body and fixture fixes, and the faults of declined rewrites.
    fn apply_fixes(
        &mut self,
        fixture_inputs: Option<&(Bound<'py, PyAny>, Bound<'py, PyAny>)>,
    ) -> PyResult<Applied<'py>> {
        let mut applied: Applied<'py> = Applied {
            rewrites: Rewrites::default(),
            rejected: BTreeSet::new(),
            faults: Vec::new(),
            rewrite_lints: HashMap::new(),
        };
        if let Some(inputs) = fixture_inputs {
            for (index, text) in self.fixture_autofix(inputs, &applied.rewrites)? {
                applied.rewrites.set(index, text);
            }
        }
        if self.request.fixtures_only {
            return Ok(applied);
        }
        let sorted: Vec<usize> = self.sorted(&self.safe);
        let config: HeaderRulesConfig = self.config;
        let headers: Vec<(String, Vec<HeaderViolation>)> = {
            let texts: Vec<&str> = sorted
                .iter()
                .map(|index| self.text(*index, &applied.rewrites))
                .collect();
            native(self.py, || {
                each_file(&texts, |text| format_header_text(text, config))
            })?
        };
        for (index, (formatted, faults)) in sorted.iter().zip(headers) {
            applied.faults.extend(
                faults
                    .into_iter()
                    .map(|fault| Finding::Header(*index, fault)),
            );
            if formatted != self.text(*index, &applied.rewrites) {
                applied.rewrites.set(*index, formatted);
            }
        }
        let (formatted, faults) = {
            let items: Vec<(usize, &str)> = sorted
                .iter()
                .map(|index| (*index, self.text(*index, &applied.rewrites)))
                .collect();
            self.format_bodies(&items)?
        };
        applied.faults.extend(faults);
        for (index, text) in formatted {
            applied.rewrites.set(index, text);
        }
        if let Some(inputs) = fixture_inputs {
            let fixtures: Vec<(usize, String)> = self.fixture_autofix(inputs, &applied.rewrites)?;
            if !fixtures.is_empty() {
                let mut items: Vec<(usize, &str)> = fixtures
                    .iter()
                    .map(|(index, text)| (*index, text.as_str()))
                    .collect();
                items.sort_by(|left, right| {
                    self.entries[left.0].path.cmp(&self.entries[right.0].path)
                });
                let (formatted, faults) = self.format_bodies(&items)?;
                applied.faults.extend(faults);
                for (index, text) in fixtures.into_iter().chain(formatted) {
                    applied.rewrites.set(index, text);
                }
            }
        }
        self.reject_unparseable_rewrites(applied)
    }

    /// Decline every rewrite whose header no longer parses, adding its header-parse faults.
    fn reject_unparseable_rewrites(&self, mut applied: Applied<'py>) -> PyResult<Applied<'py>> {
        let config: HeaderRulesConfig = self.config;
        let order: Vec<usize> = applied.rewrites.order.clone();
        let lints: Vec<Vec<HeaderViolation>> = {
            let texts: Vec<&str> = order
                .iter()
                .map(|index| applied.rewrites.get(*index).unwrap_or_default())
                .collect();
            native(self.py, || {
                each_file(&texts, |text| lint_header_text(text, config))
            })?
        };
        for (index, violations) in order.into_iter().zip(lints) {
            let faults: Vec<HeaderViolation> = parse_faults(&violations);
            if faults.is_empty() {
                let _ = applied.rewrite_lints.insert(index, violations);
                continue;
            }
            let _ = applied.rejected.insert(index);
            applied.faults.extend(
                faults
                    .into_iter()
                    .map(|fault| Finding::Header(index, fault)),
            );
        }
        Ok(applied)
    }
}

fn code_is(violation: &Bound<'_, PyAny>, code: &str) -> PyResult<bool> {
    Ok(native_field(violation, "code")?.extract::<String>()? == code)
}

/// Drop `description-present` faults for models described outside their header.
fn drop_resolved_description_faults<'py>(
    project_dir: &Bound<'py, PyAny>,
    violations: Vec<Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut has_description_fault: bool = false;
    for violation in &violations {
        has_description_fault |= code_is(violation, DESCRIPTION_PRESENT)?;
    }
    if !has_description_fault {
        return Ok(violations);
    }
    let described: BTreeSet<PathBuf> = externally_described_models(project_dir)?
        .try_iter()?
        .map(|path| path?.call_method0("resolve")?.extract())
        .collect::<PyResult<_>>()?;
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::new();
    for violation in violations {
        let resolved: PathBuf = native_field(&violation, "file_path")?
            .call_method0("resolve")?
            .extract()?;
        if !code_is(&violation, DESCRIPTION_PRESENT)? || !described.contains(&resolved) {
            kept.push(violation);
        }
    }
    Ok(kept)
}

/// Python `LintViolation` records of `findings`, in order, sharing one path object per file.
fn violation_records<'py>(
    py: Python<'py>,
    entries: &[Entry],
    findings: Vec<Finding<'py>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let classes: ViolationClasses<'py> = ViolationClasses::new(py)?;
    let mut paths: HashMap<usize, Bound<'py, PyAny>> = HashMap::new();
    let mut path = |index: usize| -> PyResult<Bound<'py, PyAny>> {
        if let Some(path) = paths.get(&index) {
            return Ok(path.clone());
        }
        let path: Bound<'py, PyAny> = entries[index].path.clone().into_pyobject(py)?.into_any();
        let _ = paths.insert(index, path.clone());
        Ok(path)
    };
    findings
        .into_iter()
        .map(|finding| match finding {
            Finding::Header(index, violation) => {
                classes.violation(&path(index)?, &LintViolationRow::header(0, violation))
            }
            Finding::Unsafe(index, fault) => unsafe_fault(&path(index)?, &fault),
            Finding::Suppression(index, finding) => {
                classes.violation(&path(index)?, &LintViolationRow::suppression(finding))
            }
            Finding::Python(violation) => Ok(violation),
        })
        .collect()
}

/// The path a fault declines the formatting of.
fn fault_path(entries: &[Entry], fault: &Finding<'_>) -> PyResult<PathBuf> {
    match fault {
        Finding::Header(index, _) | Finding::Unsafe(index, _) | Finding::Suppression(index, _) => {
            Ok(entries[*index].path.clone())
        }
        Finding::Python(violation) => native_field(violation, "file_path")?.extract(),
    }
}

/// Header violations of each collected file's final text, in path order, reusing earlier lints.
fn lint_final_contents<'py>(
    run: &Run<'_, '_, 'py>,
    (final_texts, original_lints): (&HashMap<usize, &str>, &[Vec<HeaderViolation>]),
    applied: &Applied<'py>,
) -> PyResult<Vec<Finding<'py>>> {
    let collected: Vec<usize> = (0..run.entries.len())
        .filter(|index| run.entries[*index].collected)
        .collect();
    let sorted: Vec<usize> = run.sorted(&collected);
    let final_text = |index: usize| -> &str {
        final_texts
            .get(&index)
            .copied()
            .unwrap_or(run.entries[index].original.as_str())
    };
    let known = |index: usize| -> Option<&Vec<HeaderViolation>> {
        let text: &str = final_text(index);
        if text == run.entries[index].original {
            return original_lints.get(index);
        }
        applied
            .rewrite_lints
            .get(&index)
            .filter(|_| applied.rewrites.get(index) == Some(text))
    };
    let unknown: Vec<usize> = sorted
        .iter()
        .copied()
        .filter(|index| known(*index).is_none())
        .collect();
    let config: HeaderRulesConfig = run.config;
    let computed: Vec<Vec<HeaderViolation>> = {
        let texts: Vec<&str> = unknown.iter().map(|index| final_text(*index)).collect();
        native(run.py, || {
            each_file(&texts, |text| lint_header_text(text, config))
        })?
    };
    let computed: HashMap<usize, Vec<HeaderViolation>> =
        unknown.into_iter().zip(computed).collect();
    let mut findings: Vec<Finding<'py>> = Vec::new();
    for index in sorted {
        let violations: &Vec<HeaderViolation> = match known(index) {
            Some(violations) => violations,
            None => &computed[&index],
        };
        findings.extend(
            violations
                .iter()
                .cloned()
                .map(|violation| Finding::Header(index, violation)),
        );
    }
    Ok(findings)
}

/// The entries `findings` are about; Python findings come only from uncached Rule fixes.
fn dirty_entries<'f>(findings: &'f [Finding<'_>]) -> impl Iterator<Item = usize> + 'f {
    findings.iter().filter_map(|finding| match finding {
        Finding::Header(index, _) | Finding::Unsafe(index, _) | Finding::Suppression(index, _) => {
            Some(*index)
        }
        Finding::Python(_) => None,
    })
}

/// The effective settings that shape formatting, part of every cache key.
fn cache_settings(request: &FormatRequest<'_, '_>) -> PyResult<String> {
    let config: &Bound<'_, PyAny> = request.config;
    let mut settings: Vec<String> = Vec::new();
    for name in [
        "line_width",
        "max_description_lines",
        "max_literal_length",
        "max_ranking_order_by",
    ] {
        settings.push(native_field(config, name)?.extract::<i64>()?.to_string());
    }
    settings.push(native_field(config, "dialect")?.extract()?);
    settings.push(request.fixtures_only.to_string());
    Ok(settings.join("\0"))
}

/// SQL test files fixture-null formatting rewrites from model contracts, never skipped.
fn fixture_paths(
    directory: &Path,
    fixture_inputs: Option<&(Bound<'_, PyAny>, Bound<'_, PyAny>)>,
) -> PyResult<BTreeSet<PathBuf>> {
    let Some((discovered, _)) = fixture_inputs else {
        return Ok(BTreeSet::new());
    };
    let mut paths: BTreeSet<PathBuf> = BTreeSet::new();
    for file in native_field(discovered, "test_files")?.try_iter()? {
        let relative: PathBuf = native_field(&file?, "relative_path")?.extract()?;
        let path: PathBuf = directory.join(relative);
        let _ = paths.insert(fs::canonicalize(&path).unwrap_or_else(|_| path.clone()));
        let _ = paths.insert(path);
    }
    Ok(paths)
}

/// Collected files not cached as clean, each with its cache key, or none when not skippable.
fn uncached_entries(
    py: Python<'_>,
    (cache, uncacheable): (&mut FormatCache, &BTreeSet<PathBuf>),
    directory: &Path,
    collected: Vec<(PathBuf, String)>,
) -> PyResult<(Vec<Entry>, Vec<Option<ContentDigest>>)> {
    let shared: &FormatCache = cache;
    let keys: Vec<Option<ContentDigest>> = native(py, || {
        each_file(&collected, |(path, contents)| {
            let skippable: bool = uncacheable.is_empty()
                || !(uncacheable.contains(path)
                    || fs::canonicalize(path)
                        .is_ok_and(|resolved| uncacheable.contains(&resolved)));
            Ok(skippable.then(|| shared.key(&project_relative_path(path, directory), contents)))
        })
    })?;
    let mut entries: Vec<Entry> = Vec::new();
    let mut kept: Vec<Option<ContentDigest>> = Vec::new();
    let (mut hits, mut misses): (u64, u64) = (0, 0);
    for ((path, original), key) in collected.into_iter().zip(keys) {
        if let Some(key) = key.as_ref() {
            if cache.is_clean(key) {
                hits += 1;
                continue;
            }
            misses += 1;
        }
        entries.push(Entry {
            path,
            original,
            collected: true,
        });
        kept.push(key);
    }
    count_stage(py, FORMAT_CACHE_HITS, hits)?;
    count_stage(py, FORMAT_CACHE_MISSES, misses)?;
    Ok((entries, kept))
}

/// One applied `description-quotes` fix report per rewritten description, in file order.
fn quote_fix_reports<'py>(
    py: Python<'py>,
    entries: &[Entry],
    fixes: &[Option<QuoteFix>],
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut reports: Vec<Bound<'py, PyAny>> = Vec::new();
    for (entry, fix) in entries.iter().zip(fixes) {
        let Some(fix) = fix else {
            continue;
        };
        let (status, reason): (&str, &str) = if fix.text.is_some() {
            (APPLIED, DESCRIPTION_QUOTES_REASON)
        } else {
            (REFUSED, DESCRIPTION_QUOTES_REFUSED_REASON)
        };
        for line in &fix.lines {
            reports.push(new_record_with(
                py,
                RecordId::RuleFixResult,
                &[
                    (
                        "file_path",
                        entry.path.clone().into_pyobject(py)?.into_any(),
                    ),
                    ("code", DESCRIPTION_QUOTES.into_pyobject(py)?.into_any()),
                    ("line", line.into_pyobject(py)?.into_any()),
                    ("status", status.into_pyobject(py)?.into_any()),
                    ("reason", reason.into_pyobject(py)?.into_any()),
                ],
            )?);
        }
    }
    Ok(reports)
}

/// The project-file warning that Rule fixes, which compile the saved files, wait for quote fixes.
fn rule_fixes_skipped<'py>(py: Python<'py>, directory: &Path) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::LintViolation,
        &[
            (
                "file_path",
                directory
                    .join(PROJECT_CONFIG_FILENAME)
                    .into_pyobject(py)?
                    .into_any(),
            ),
            ("line", 1_i64.into_pyobject(py)?.into_any()),
            ("column", 1_i64.into_pyobject(py)?.into_any()),
            ("code", "format-fix-skipped".into_pyobject(py)?.into_any()),
            (
                "message",
                "Rule fixes were skipped: the project compiles only once the rewritten \
                 descriptions are saved"
                    .into_pyobject(py)?
                    .into_any(),
            ),
            ("severity", "warning".into_pyobject(py)?.into_any()),
            ("engine", "native".into_pyobject(py)?.into_any()),
            (
                "remediation",
                "Rerun `sqb format --fix` to apply Rule fixes."
                    .into_pyobject(py)?
                    .into_any(),
            ),
        ],
    )
}

/// Format the project's authored SQL files and report what remains.
pub(crate) fn run_format<'py>(request: &FormatRequest<'_, 'py>) -> PyResult<FormatOutcome<'py>> {
    let py: Python<'py> = request.project_dir.py();
    let fixture_inputs: Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)> =
        match (request.discovered, request.adapter) {
            (Some(discovered), Some(adapter)) => Some((
                discovered.clone(),
                native_field(adapter, "sql_lexical_syntax")?,
            )),
            (Some(_), None) => {
                return Err(error_class(py, ErrorId::Lint)?
                    .call1(("fixture-null formatting requires the project adapter",))
                    .map(pyo3::PyErr::from_value)?);
            }
            _ => None,
        };
    let directory: PathBuf = request.project_dir.extract()?;
    let collected: Vec<(PathBuf, String)> = collect_lint_files(&directory, request.selected)
        .map_err(|error| pyo3::exceptions::PyOSError::new_err(error.to_string()))?;
    let files_checked: usize = collected.len();
    let mut cache: Option<FormatCache> = if request.cache {
        FormatCache::open(py, &directory, cache_settings(request)?)?
    } else {
        None
    };
    let (entries, keys): (Vec<Entry>, Vec<Option<ContentDigest>>) = match cache.as_mut() {
        Some(cache) => {
            let uncacheable: BTreeSet<PathBuf> =
                fixture_paths(&directory, fixture_inputs.as_ref())?;
            uncached_entries(py, (cache, &uncacheable), &directory, collected)?
        }
        None => {
            let entries: Vec<Entry> = collected
                .into_iter()
                .map(|(path, original)| Entry {
                    path,
                    original,
                    collected: true,
                })
                .collect();
            let keys: Vec<Option<ContentDigest>> = vec![None; entries.len()];
            (entries, keys)
        }
    };
    let config: HeaderRulesConfig = rules_config(request.config)?;
    let original_lints: Vec<Vec<HeaderViolation>> = native(py, || {
        each_file(&entries, |entry| lint_header_text(&entry.original, config))
    })?;
    let quote_fixes: Vec<Option<QuoteFix>> = if request.fix {
        native(py, || {
            each_file(&entries, |entry| quote_fixed_text(&entry.original, config))
        })?
    } else {
        vec![None; entries.len()]
    };
    let quote_reports: Vec<Bound<'py, PyAny>> = quote_fix_reports(py, &entries, &quote_fixes)?;
    let quotes_fixed: bool = quote_fixes.iter().flatten().any(|fix| fix.text.is_some());
    let mut run: Run<'_, '_, 'py> = Run {
        request,
        py,
        directory,
        config,
        index: entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.path.clone(), index))
            .collect(),
        safe: safe_files(&quote_fixes, &original_lints),
        entries,
        fixed: quote_fixed_texts(quote_fixes),
    };
    let (rule_fixes, rule_faults) = if request.fix {
        match request.discovery_error {
            Some(error) if !quotes_fixed => return Err(error.clone_ref(py)),
            Some(_) => (quote_reports, vec![rule_fixes_skipped(py, &run.directory)?]),
            None => match run.plan_rule_fixes() {
                Ok((verified, faults)) => {
                    (quote_reports.into_iter().chain(verified).collect(), faults)
                }
                Err(error) if error.is_instance_of::<PyException>(py) && quotes_fixed => {
                    let skipped: Bound<'py, PyAny> = rule_fixes_skipped(py, &run.directory)?;
                    (quote_reports, vec![skipped])
                }
                Err(error) => return Err(error),
            },
        }
    } else {
        (Vec::new(), Vec::new())
    };
    let mut applied: Applied<'py> = run.apply_fixes(fixture_inputs.as_ref())?;
    applied
        .faults
        .extend(rule_faults.into_iter().map(Finding::Python));
    let declined: BTreeSet<PathBuf> = applied
        .faults
        .iter()
        .map(|fault| fault_path(&run.entries, fault))
        .collect::<PyResult<_>>()?;
    let final_texts: HashMap<usize, &str> = run
        .safe
        .iter()
        .filter(|index| !declined.contains(&run.entries[**index].path))
        .map(|index| {
            let text: &str = if applied.rejected.contains(index) {
                run.base_text(*index)
            } else {
                run.text(*index, &applied.rewrites)
            };
            (*index, text)
        })
        .collect();
    let writes: Vec<PendingWrite<'_>> = run
        .sorted(&final_texts.keys().copied().collect::<Vec<usize>>())
        .into_iter()
        .map(|index| {
            let entry: &Entry = &run.entries[index];
            PendingWrite {
                entry: index,
                path: &entry.path,
                authored: &entry.original,
                contents: final_texts[&index],
                newline: if entry.collected && entry.original.contains(CARRIAGE_RETURN_LINE_FEED) {
                    CARRIAGE_RETURN_LINE_FEED
                } else {
                    LINE_FEED
                },
            }
        })
        .collect();
    let changes: Vec<FormatChange<'py>> = persist_changes(py, &writes, request.write)?;
    let mut findings: Vec<Finding<'py>> = if request.fixtures_only {
        Vec::new()
    } else {
        lint_final_contents(&run, (&final_texts, &original_lints), &applied)?
    };
    findings.append(&mut applied.faults);
    let findings_seen: BTreeSet<usize> = dirty_entries(&findings).collect();
    let python_findings: bool = findings
        .iter()
        .any(|finding| matches!(finding, Finding::Python(_)));
    let violations: Vec<Bound<'py, PyAny>> = drop_resolved_description_faults(
        request.project_dir,
        violation_records(py, &run.entries, findings)?,
    )?;
    let collected: Vec<usize> = (0..run.entries.len())
        .filter(|index| run.entries[*index].collected)
        .collect();
    let relative: Vec<String> = collected
        .iter()
        .map(|index| project_relative_path(&run.entries[*index].path, &run.directory))
        .collect();
    let sources: Vec<(&str, &str)> = collected
        .iter()
        .zip(&relative)
        .map(|(index, path)| {
            (
                path.as_str(),
                final_texts
                    .get(index)
                    .copied()
                    .unwrap_or(run.entries[*index].original.as_str()),
            )
        })
        .collect();
    let suppressions: Vec<Finding<'py>> = native(py, || {
        reject_inline_suppressions(&sources).map_err(|error| error.to_string())
    })?
    .into_iter()
    .map(|finding| Finding::Suppression(collected[finding.file], finding))
    .collect();
    if let Some(cache) = cache.as_mut() {
        let dirty: BTreeSet<usize> = changes
            .iter()
            .map(|change| change.entry)
            .chain(findings_seen.iter().copied())
            .chain(dirty_entries(&suppressions))
            .collect();
        for (index, key) in keys.iter().enumerate() {
            if let Some(key) = key.filter(|_| !dirty.contains(&index)) {
                cache.record_clean(key);
            }
        }
        cache.save(py);
    }
    let reported: BTreeSet<usize> = findings_seen
        .iter()
        .copied()
        .chain(dirty_entries(&suppressions))
        .collect();
    let mut violations: Vec<Bound<'py, PyAny>> = violations;
    violations.extend(violation_records(py, &run.entries, suppressions)?);
    let final_contents: HashMap<PathBuf, String> = collected
        .iter()
        .filter(|index| python_findings || reported.contains(index))
        .map(|index| {
            let entry: &Entry = &run.entries[*index];
            let text: &str = final_texts
                .get(index)
                .copied()
                .unwrap_or(entry.original.as_str());
            (entry.path.clone(), text.to_owned())
        })
        .collect();
    Ok(FormatOutcome {
        files_checked,
        violations,
        changes,
        rule_fixes,
        declined,
        final_contents,
    })
}

/// The indexes of files whose headers parse as written, or wholly once their quotes are fixed.
fn safe_files(quote_fixes: &[Option<QuoteFix>], lints: &[Vec<HeaderViolation>]) -> Vec<usize> {
    let mut safe: Vec<usize> = Vec::new();
    for (index, (fix, violations)) in quote_fixes.iter().zip(lints).enumerate() {
        let fixed: bool = fix.as_ref().is_some_and(|fix| fix.text.is_some());
        if fixed || parse_faults(violations).is_empty() {
            safe.push(index);
        }
    }
    safe
}

/// The quote-fixed text of each file a quote fix rewrote, by index.
fn quote_fixed_texts(quote_fixes: Vec<Option<QuoteFix>>) -> HashMap<usize, String> {
    let mut fixed: HashMap<usize, String> = HashMap::new();
    for (index, fix) in quote_fixes.into_iter().enumerate() {
        if let Some(text) = fix.and_then(|fix| fix.text) {
            fixed.insert(index, text);
        }
    }
    fixed
}
