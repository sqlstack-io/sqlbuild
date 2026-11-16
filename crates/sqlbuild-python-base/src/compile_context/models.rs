//! Public records of compile context.

use crate::boundary::main::imported::imported;
use crate::compile_context::_helpers::diagnostic_collector::active_collector;
use crate::compile_context::_helpers::image_codec::MAX_RESERVED_ITEMS;
use crate::compile_context::_helpers::image_codec::length;
use crate::compile_context::_helpers::image_codec::read;
use crate::compile_context::_helpers::input_reads::CompileInputReads;
use crate::compile_context::_helpers::input_reads::registry;
use crate::compile_context::_helpers::yaml_cache::document_key;
use crate::compile_context::_helpers::yaml_cache::encoded_document;
use crate::compile_context::_helpers::yaml_cache::stored_outcome;
use crate::compile_context::types::ModelShape;
use pyo3::PyErr;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;
use rustc_hash::FxHashMap;
use sqlbuild_analysis::assembly::analysis_session::models::RelationTables;
use sqlbuild_cache::store::models::NativeStore;
use sqlbuild_cache::store::models::RecordReader;
use sqlbuild_cache::store::models::RecordWriter;
use sqlbuild_discovery::header_changes::types::HeaderKeys;
use sqlbuild_discovery::yaml_files::models::YamlFileOutcome;
use sqlbuild_host::compile_context::types::ProcessClock;
use sqlbuild_rules::engine::main::retained_request::retained_request;
use sqlbuild_rules::models::EvaluateRequest;
use sqlbuild_rules::models::RetainedRows;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::PoisonError;

/// The process CPU clock of one interpreter, including its worker threads.
#[derive(Clone, Copy)]
pub struct PythonProcessClock<'py>(pub Python<'py>);

impl ProcessClock for PythonProcessClock<'_> {
    type Error = PyErr;

    fn process_time_ns(&self) -> PyResult<u64> {
        imported(self.0, "time", "process_time_ns")?
            .call0()?
            .extract()
    }
}

/// What one model contributes to the partial-check reasons, apart from its dependencies.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelCoverage {
    pub name: String,
    /// The model opts out of SQL analysis.
    pub disabled: bool,
    /// The model has a binding error, which makes every model reading it partial.
    pub blocked: bool,
    /// Its own reasons, sorted; those from invalid dependencies are derived.
    pub partial: Vec<String>,
}

impl ModelCoverage {
    /// Whether the model adds anything to the reasons.
    pub fn notable(&self) -> bool {
        self.disabled || self.blocked || !self.partial.is_empty()
    }
}

/// A stored compile's built-in rules request, as decoded for a delta compile.
#[derive(Clone, Debug)]
pub struct BaseRules {
    pub request: EvaluateRequest,
    /// Each model's payload digest, which keys its cached findings, in model order.
    pub digests: Vec<String>,
}

impl BaseRules {
    /// The rows a compile retained, decoded.
    pub fn decoded(retained: &RetainedRows) -> Option<Self> {
        Some(Self {
            request: retained_request(retained)?,
            digests: retained.digests.clone(),
        })
    }
}

/// A stored rules request, decoding on its own thread or decoded already.
#[derive(Debug)]
pub enum PendingRules {
    Decoding(std::thread::JoinHandle<Option<BaseRules>>),
    Ready(Box<BaseRules>),
}

impl PendingRules {
    /// The decoded request, or `None` when it could not be decoded.
    pub fn resolved(self) -> Option<BaseRules> {
        match self {
            Self::Decoding(handle) => {
                let Ok(rules) = handle.join() else {
                    return None;
                };
                rules
            }
            Self::Ready(rules) => Some(*rules),
        }
    }
}

/// The facts a full compile leaves for its project image beyond its analysis record.
#[derive(Debug, Default)]
pub struct ImageCapture {
    /// The sources functions and audits read, which a delta compile always keeps.
    pub pinned_sources: BTreeSet<String>,
    /// The sources each SQL test file reads, which a delta compile keeps when it reruns it.
    pub test_sources: BTreeMap<String, BTreeSet<String>>,
    /// The relation facts the model analysis session ended with.
    pub relations: Option<RelationTables>,
    /// The built-in rules request and its raw findings, before suppressions.
    pub rules: Option<CapturedRules>,
    /// Each custom-rule fact digest the custom rule cache compared, by fact key.
    pub fact_digests: BTreeMap<Vec<String>, String>,
    /// Whether the JSON report fell back to the standard library's serializer.
    pub stdlib_json: bool,
    /// Each model's scope usage digest, by model file path.
    pub usages: BTreeMap<String, String>,
    /// Each SQL test's compiled-artifact chain, by test key text.
    pub test_chains: Vec<CapturedTest>,
    /// Each project model's `(path, name, materialized)`, when custom rules ran.
    pub model_subjects: Option<Vec<(String, String, Option<String>)>>,
    /// Each compiled model's `(name, relative path, upstream (kind, name) pairs)`.
    pub models: Vec<CapturedModel>,
    /// Each report diagnostic as its JSON item text, with what it names.
    pub diagnostics: Vec<CapturedDiagnostic>,
    /// The text report's model lines `(name, failed, column count)`, when it rendered one.
    pub text_models: Option<Vec<(String, bool, i64)>>,
    /// The template environment names the compile read.
    pub environment_names: Vec<String>,
    /// Each discovered model's header keys, by model file path.
    pub header_keys: BTreeMap<String, HeaderKeys>,
    /// The audit factories each model file's header attaches, for files attaching any.
    pub audit_references: BTreeMap<String, Vec<String>>,
    /// Each model file's SQL lint cache key, in project model order.
    pub lint_model_keys: Option<Vec<(String, String)>>,
    /// Each other SQL file's lint cache key, in walk order.
    pub lint_file_keys: Option<Vec<(String, Option<String>)>>,
    /// The closed shape each model published for semantic checks, in publication order.
    pub model_shapes: Option<Vec<ModelShape>>,
    /// The notable semantic coverage facts of every model, when semantic checks reported them.
    pub coverage: Option<Vec<ModelCoverage>>,
    /// Why this compile cannot be the base of a delta compile, when something ruled it out.
    pub unsupported: Option<String>,
}

/// The built-in rules request a compile evaluated and its raw findings.
#[derive(Debug)]
pub struct CapturedRules {
    pub retained: RetainedRows,
}

/// One compiled model's identity and references.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CapturedModel {
    pub name: String,
    pub relative_path: String,
    pub upstream: Vec<(String, String)>,
}

/// One report diagnostic: its JSON text and the resource and file it names.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CapturedDiagnostic {
    pub json: String,
    pub resource_name: Option<String>,
    pub paths: Vec<String>,
    /// Whether the diagnostic is an error.
    #[serde(default)]
    pub error: bool,
}

/// One SQL test's file and the models its compiled artifact is keyed by.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CapturedTest {
    pub key: String,
    pub relative_path: String,
    pub chain: Vec<String>,
}

/// What a delta compile hands the stages it reruns for the changed models.
#[derive(Debug, Default)]
pub struct DeltaInjection {
    /// Model files to discover, as project-relative POSIX paths.
    pub model_paths: BTreeSet<String>,
    /// SQL test files to discover, as project-relative POSIX paths.
    pub test_paths: BTreeSet<String>,
    /// The paths and names of every model the delta compile reruns.
    pub rerun_names: BTreeSet<String>,
    /// The project directory the stored compile and the delta compile both read.
    pub project_dir: std::path::PathBuf,
    /// Every model and SQL test file of the whole project, as project-relative POSIX paths.
    pub source_paths: BTreeSet<String>,
    /// Every model file of the whole project, which path defaults are matched against.
    pub project_model_paths: Vec<String>,
    /// Every model name of the whole project, which references resolve against.
    pub model_names: Vec<String>,
    /// The whole project's relation facts, injected for relations the delta does not analyse.
    pub relations: RelationTables,
    /// The whole project's built-in rules request, taken by the round that evaluates it.
    pub rules: Mutex<Option<PendingRules>>,
    /// The whole project's custom-rule fact digests.
    pub fact_digests: BTreeMap<Vec<String>, String>,
    /// Each project model's `(path, name, materialized)`, the custom-rule subjects.
    pub model_subjects: Option<Vec<(String, String, Option<String>)>>,
    /// The files that changed since the stored compile, as project-relative POSIX paths.
    pub changed_paths: BTreeSet<String>,
    /// Where loaded YAML documents are kept for later delta compiles.
    pub yaml_store: Option<std::path::PathBuf>,
    /// The only sources discovery keeps, those the rerun models read; `None` keeps every one.
    pub source_names: Option<BTreeSet<String>>,
    /// The whole project's SQL lint cache keys, as the image recorded them.
    pub lint_model_keys: Option<Vec<(String, String)>>,
    pub lint_file_keys: Option<Vec<(String, Option<String>)>>,
    /// The closed shapes the whole project's models published.
    pub model_shapes: Vec<ModelShape>,
    /// The whole project's text report model lines, with the rerun models' lines to replace.
    pub text_models: Vec<(String, bool, i64)>,
    /// The whole project's notable semantic coverage facts, as the image recorded them.
    pub coverage: Option<Vec<ModelCoverage>>,
    /// Every project model in project order, with the models it reads.
    pub coverage_order: Vec<(String, Vec<String>)>,
    /// The audit factories each model file's header attaches, for files attaching any.
    pub audit_references: BTreeMap<String, Vec<String>>,
}

/// What a delta compile's reuse record keeps of the stored compile it patched.
#[derive(Debug, Default)]
pub struct RecordMerge {
    /// Each stored artifact's stamp, verified by the reuse check, kept unless the delta wrote it.
    pub kept: Vec<sqlbuild_cache::project_snapshot::models::PathStamp>,
    /// The template environment names the stored compile read.
    pub environment_names: Vec<String>,
    /// The module files the stored compile depended on, such as those behind stored macro calls.
    pub module_paths: Vec<String>,
}

/// Deduplicated compile diagnostics gathered while the collection is entered.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct CollectedCompileDiagnostics {
    pub(crate) by_key: Py<PyDict>,
    pub(crate) token: Mutex<Option<Py<PyAny>>>,
}

#[pymethods]
impl CollectedCompileDiagnostics {
    pub(crate) fn __enter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Self>> {
        let token: Bound<'_, PyAny> =
            active_collector(slf.py())?.call_method1("set", (slf.clone(),))?;
        *slf.get()
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(token.unbind());
        Ok(slf.clone())
    }

    #[pyo3(signature = (*_exception))]
    pub(crate) fn __exit__(
        &self,
        py: Python<'_>,
        _exception: &Bound<'_, PyTuple>,
    ) -> PyResult<bool> {
        let token: Option<Py<PyAny>> = self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(token) = token {
            active_collector(py)?.call_method1("reset", (token,))?;
        }
        Ok(false)
    }

    /// Record one diagnostic; a repeated key keeps the first diagnostic.
    #[pyo3(signature = (*, key, diagnostic))]
    pub(crate) fn add(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        diagnostic: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.by_key
            .bind(py)
            .call_method1("setdefault", (key, diagnostic))?;
        Ok(())
    }

    /// The recorded diagnostics in deterministic key order.
    #[getter]
    pub fn diagnostics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let by_key: &Bound<'py, PyDict> = self.by_key.bind(py);
        let keys: Bound<'py, PyAny> = imported(py, "builtins", "sorted")?.call1((by_key,))?;
        let diagnostics: Vec<Bound<'py, PyAny>> = keys
            .try_iter()?
            .map(|key| by_key.as_any().get_item(key?))
            .collect::<PyResult<_>>()?;
        PyTuple::new(py, diagnostics)
    }
}

/// Writes one section: its values refer to a table of the distinct strings they hold.
#[derive(Debug, Default)]
pub struct Encoder<'a> {
    pub(crate) ids: FxHashMap<&'a str, u64>,
    pub(crate) strings: Vec<&'a str>,
    pub(crate) body: RecordWriter,
}

impl<'a> Encoder<'a> {
    pub fn string(&mut self, value: &'a str) {
        let next: u64 = self.strings.len() as u64;
        let id: u64 = *self.ids.entry(value).or_insert(next);
        if id == next {
            self.strings.push(value);
        }
        self.body.put_u64(id);
    }

    pub fn optional_string(&mut self, value: Option<&'a str>) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.string(value);
        }
    }

    pub fn count(&mut self, count: usize) {
        self.body.put_u64(count as u64);
    }

    pub fn flag(&mut self, value: bool) {
        self.body.put_u64(u64::from(value));
    }

    pub fn bytes(&mut self, value: &[u8]) {
        self.body.put_bytes(value);
    }

    pub fn strings(&mut self, values: impl ExactSizeIterator<Item = &'a str>) {
        self.count(values.len());
        for value in values {
            self.string(value);
        }
    }

    pub fn pairs(&mut self, pairs: &'a [(String, String)]) {
        self.count(pairs.len());
        for (left, right) in pairs {
            self.string(left);
            self.string(right);
        }
    }

    /// The section: the string table, then the values.
    pub fn finish(self) -> Vec<u8> {
        let mut section: RecordWriter = RecordWriter::default();
        section.put_u64(self.strings.len() as u64);
        for value in &self.strings {
            section.put_str(value);
        }
        section.put_bytes(&self.body.into_bytes());
        section.into_bytes()
    }
}

/// Reads a section an [`Encoder`] wrote; any malformed value fails the whole read.
#[derive(Debug)]
pub struct Decoder<'a> {
    pub(crate) strings: Vec<String>,
    pub(crate) body: RecordReader<'a>,
}

impl<'a> Decoder<'a> {
    pub fn new(section: &'a [u8]) -> Option<Self> {
        let mut reader: RecordReader<'a> = RecordReader::new(section);
        let count: usize = length(read(reader.u64())?)?;
        let mut strings: Vec<String> = Vec::with_capacity(count.min(section.len()));
        for _ in 0..count {
            strings.push(read(reader.string())?);
        }
        let body: &'a [u8] = read(reader.bytes())?;
        read(reader.finish())?;
        Some(Self {
            strings,
            body: RecordReader::new(body),
        })
    }

    pub fn string(&mut self) -> Option<String> {
        let id: usize = length(read(self.body.u64())?)?;
        self.strings.get(id).cloned()
    }

    pub fn optional_string(&mut self) -> Option<Option<String>> {
        Some(if self.flag()? {
            Some(self.string()?)
        } else {
            None
        })
    }

    pub fn count(&mut self) -> Option<usize> {
        length(read(self.body.u64())?)
    }

    pub fn flag(&mut self) -> Option<bool> {
        match read(self.body.u64())? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    pub fn bytes(&mut self) -> Option<&'a [u8]> {
        read(self.body.bytes())
    }

    pub fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let count: usize = self.count()?;
        let mut items: Vec<T> = Vec::with_capacity(count.min(MAX_RESERVED_ITEMS));
        for _ in 0..count {
            items.push(item(self)?);
        }
        Some(items)
    }

    pub fn strings(&mut self) -> Option<Vec<String>> {
        self.list(Self::string)
    }

    pub fn pairs(&mut self) -> Option<Vec<(String, String)>> {
        self.list(|decoder| Some((decoder.string()?, decoder.string()?)))
    }

    /// Whether every value was read.
    pub fn finish(self) -> Option<()> {
        read(self.body.finish())
    }
}

/// Records environment variables and run-identity reads while entered.
#[pyclass(frozen, module = "sqlbuild._native")]
#[derive(Debug)]
pub struct CompileInputRecording {
    pub(crate) recorder: Mutex<Option<Py<CompileInputReads>>>,
}

#[pymethods]
impl CompileInputRecording {
    pub(crate) fn __enter__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, CompileInputReads>> {
        let recorder: Bound<'py, CompileInputReads> = Bound::new(py, CompileInputReads::new())?;
        registry(py)?.get().locked().push(recorder.clone().unbind());
        *self.recorder.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(recorder.clone().unbind());
        Ok(recorder)
    }

    #[pyo3(signature = (*_exception))]
    pub(crate) fn __exit__(
        &self,
        py: Python<'_>,
        _exception: &Bound<'_, PyTuple>,
    ) -> PyResult<bool> {
        if let Some(recorder) = self
            .recorder
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            registry(py)?
                .get()
                .locked()
                .retain(|item| !item.is(&recorder));
        }
        Ok(false)
    }
}

/// The store a delta compile reads and fills, opened beside its reuse slots.
#[derive(Debug)]
pub struct DocumentStore {
    pub store: NativeStore,
    pub path: PathBuf,
}

impl DocumentStore {
    /// Each file's document, decoded from the store or loaded by `load` and stored.
    pub fn load_with<E>(
        &mut self,
        paths: &[String],
        absolute: impl Fn(&str) -> PathBuf,
        load: impl FnOnce(&[String]) -> Result<Vec<YamlFileOutcome>, E>,
    ) -> Result<Vec<YamlFileOutcome>, E> {
        let mut outcomes: Vec<Option<YamlFileOutcome>> = Vec::with_capacity(paths.len());
        let mut missed: Vec<String> = Vec::new();
        for path in paths {
            let stored: Option<YamlFileOutcome> = stored_outcome(&mut self.store, &absolute(path));
            if stored.is_none() {
                missed.push(path.clone());
            }
            outcomes.push(stored);
        }
        let mut loaded = if missed.is_empty() {
            Vec::new()
        } else {
            load(&missed)?
        }
        .into_iter();
        let mut complete: Vec<YamlFileOutcome> = Vec::with_capacity(paths.len());
        for outcome in outcomes {
            let outcome: YamlFileOutcome = match outcome {
                Some(outcome) => outcome,
                None => {
                    let Some(outcome) = loaded.next() else {
                        break;
                    };
                    if let YamlFileOutcome::Loaded { contents, value } = &outcome {
                        self.store
                            .put(document_key(contents), encoded_document(value));
                    }
                    outcome
                }
            };
            complete.push(outcome);
        }
        Ok(complete)
    }
}

impl std::fmt::Debug for PythonProcessClock<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PythonProcessClock")
    }
}
