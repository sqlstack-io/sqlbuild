//! Compile only changed models and SQL tests against the stored compile, or fall back.

use crate::compile_delta::constants::DELTA_VERIFY_ENV_VAR;
use crate::compile_delta::main::driver::env_enabled::env_enabled;
use crate::compile_delta::models::DeltaOutcome;
use crate::compile_delta::models::Finished;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyStringMethods;
use sqlbuild_analysis::assembly::analysis_session::models::RelationTables;
use sqlbuild_cache::project_reuse::main::read_delta_base::read_delta_base;
use sqlbuild_cache::project_reuse::main::read_delta_output::read_delta_output;
use sqlbuild_cache::project_reuse::models::{DeltaBase, DeltaCandidate, ProjectFileChanges};
use sqlbuild_cache::project_reuse::types::ImageSections;

use crate::compile_command::main::invocation::reuse_debug::reuse_debug;
use crate::compile_command::models::RecordedCompileOutput;
use crate::compile_delta::_helpers::splice::spliced_report;
use crate::compile_delta::models::ImageGate;
use crate::compile_delta::models::ImageRelations;
use crate::compile_delta::models::ImageRules;
use crate::compile_delta::models::ProjectImage;
use crate::compile_delta::models::SpliceScope;
use crate::compile_driver::main::stages::unrecorded_compile::unrecorded_compile;
use crate::compile_driver::types::PhasesOutcome;
use crate::compile_outputs::models::NativeReuseAttempt;
use crate::rules::main::custom::stored_custom_rule_refusal::stored_custom_rule_refusal;
use sqlbuild_discovery::header_changes::main::custom_rule_reads::custom_rule_reads;
use sqlbuild_discovery::header_changes::main::header_change::header_change;
use sqlbuild_discovery::header_changes::models::HeaderChange;
use sqlbuild_python_base::build_progress::main::progress::current_projector::current_projector;
use sqlbuild_python_base::build_progress::models::NativeProgressProjector;
use sqlbuild_python_base::compile_context::constants::ERRORS_ON_DELTA_PATH;
use sqlbuild_python_base::compile_context::main::delta::start_capture::start_capture;
use sqlbuild_python_base::compile_context::main::delta::take_capture::take_capture;
use sqlbuild_python_base::compile_context::main::delta::take_delta_unsupported::take_delta_unsupported;
use sqlbuild_python_base::compile_context::main::delta::with_injection::with_injection;
use sqlbuild_python_base::compile_context::models::BaseRules;
use sqlbuild_python_base::compile_context::models::CapturedDiagnostic;
use sqlbuild_python_base::compile_context::models::CapturedModel;
use sqlbuild_python_base::compile_context::models::CapturedTest;
use sqlbuild_python_base::compile_context::models::DeltaInjection;
use sqlbuild_python_base::compile_context::models::ImageCapture;
use sqlbuild_python_base::compile_context::models::PendingRules;
use sqlbuild_python_base::compile_context::models::RecordMerge;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) const ENABLED_VALUE: &str = "1";
/// Rerunning more models than this, or this share of the project, compiles the whole project.
const MAX_RERUN_MODELS: usize = 500;
const MAX_RERUN_PERCENT: usize = 5;
/// How many times a changed relation shape may pull its dependents into the delta.
const MAX_ROUNDS: usize = 4;
const MODELS_ROOT: &str = "models/";
/// How a rules diagnostic's report JSON names its phase; rules run over the whole project.
const RULE_PHASE: &str = "\"phase\": \"rule\"";
/// What every error for a reference to an undiscovered source says.
const UNKNOWN_SOURCE: &str = "unknown source";
/// The upstream kind of a source a model reads.
const SOURCE_KIND: &str = "source";
/// The upstream kind of a model a model reads.
const MODEL_KIND: &str = "model";
/// The custom-rule fact listing every project source.
const PROJECT_SOURCES_FACT: &str = "project.sources";
const TESTS_ROOT: &str = "tests/";
pub(crate) const STDERR: &str = "stderr";
const SQL_SUFFIX: &str = ".sql";
/// How generated SQL test CTE names mark their prefix, as in `__ref__orders`.
const CTE_PREFIX_MARK: &str = "__";
const QUOTE: char = '\'';
/// What errors for a reference to a model the project lacks say.
const MISSING_MODEL_PHRASES: [&str; 2] = ["unknown model", "which does not exist"];
const UNCOMPILED_MODEL: &str = "the delta compile reported a model it did not compile as missing";
/// How many changed, added and removed paths the candidates debug line names.
const CANDIDATES_LOGGED: usize = 5;
const UNREADABLE_IMAGE: &str = "the project image cannot be read";
const TOO_MANY_MODELS: &str = "too many models would compile again";
/// A compile whose Rules failed writes no SQL tests, functions or audits, so the next compile is full.
pub(crate) const AFTER_FAILED_RULES: &str = "the previous compile failed its Rules";
/// The attribute holding a Python enum member's value.
const ENUM_VALUE: &str = "value";
/// The lineage mode a delta compile reproduces; richer modes compile in full.
const FAST_LINEAGE: &str = "fast";
/// Under verification, the delta's own report is kept here for inspection.
const VERIFY_REPORT_DIRECTORY: &str = "target/cache/compiler-native-v1";
const VERIFY_REPORT_FILE: &str = "delta-report.out";

pub(crate) fn fallback<'py>(reason: impl Into<String>) -> PyResult<DeltaOutcome<'py>> {
    Ok(DeltaOutcome::Fallback(reason.into()))
}

/// Whether the invocation asks for anything a delta compile does not reproduce.
pub(crate) fn ineligible(request: &Bound<'_, PyAny>) -> PyResult<Option<&'static str>> {
    let empty = |name: &str| -> PyResult<bool> {
        let value: Bound<'_, PyAny> = native_field(request, name)?;
        Ok(value.is_none() || !value.is_truthy()?)
    };
    for (name, reason) in [
        ("select", "a selection was given"),
        ("exclude", "an exclusion was given"),
        ("dag_path", "a DAG artifact was requested"),
        ("manifest", "a manifest was requested"),
        ("defer_to", "deferral was requested"),
        ("no_sql_validation", "SQL validation is off"),
    ] {
        if !empty(name)? {
            return Ok(Some(reason));
        }
    }
    let lineage: Bound<'_, PyAny> = native_field(request, "lineage_mode")?;
    if !lineage.is_none() && native_field(&lineage, ENUM_VALUE)?.str()?.to_str()? != FAST_LINEAGE {
        return Ok(Some("rich lineage was requested"));
    }
    let flags: Bound<'_, PyAny> = native_field(request, "profile_flags")?;
    for name in [
        "skip_discovery_sql_analysis",
        "skip_column_inference",
        "skip_contracts",
        "skip_write",
    ] {
        if native_field(&flags, name)?.is_truthy()? {
            return Ok(Some("a profiling flag was given"));
        }
    }
    Ok(None)
}

/// A path the delta may recompile: an SQL file in a resource tree, not a declaration tree.
fn resource_sql(path: &str, root: &str) -> bool {
    path.starts_with(root)
        && path.ends_with(SQL_SUFFIX)
        && !path.split('/').any(|part| part.starts_with('_'))
}

/// The stored compile's inputs and image; its report is read only after the delta runs.
pub(crate) struct Base {
    store_path: PathBuf,
    stored: DeltaBase,
    image: ProjectImage,
    project_dir: PathBuf,
}

/// The project files the reuse check found changed, and the slot it checked.
fn changes_of(
    attempt: &Bound<'_, PyAny>,
) -> PyResult<Result<(ProjectFileChanges, PathBuf), String>> {
    let native: Bound<'_, PyAny> = native_field(attempt, "native")?;
    let Ok(native) = native.cast::<NativeReuseAttempt>() else {
        return Ok(Err("the reuse check did not run".to_owned()));
    };
    let (delta, store_path): (DeltaCandidate, PathBuf) = native
        .get()
        .with_attempt(|attempt| (attempt.delta.clone(), attempt.store_path.clone()))?;
    Ok(match delta {
        DeltaCandidate::Files(changes) => Ok((changes, store_path)),
        DeltaCandidate::Unavailable(reason) => Err(reason.to_owned()),
    })
}

/// The rerun set: models and SQL tests, with every path and name the delta recompiles.
#[derive(Clone, Debug, Default)]
pub(crate) struct Rerun {
    changed: BTreeSet<String>,
    /// The sources the rerun models read, the only ones discovery keeps; `None` keeps all.
    sources: Option<BTreeSet<String>>,
    models: BTreeSet<String>,
    model_paths: BTreeSet<String>,
    test_paths: BTreeSet<String>,
    tests: BTreeSet<String>,
}

impl Rerun {
    /// The rerun set grown by every SQL test it touches and every model those tests run.
    fn with_tests(mut self, image: &ProjectImage, changed_tests: &BTreeSet<String>) -> Self {
        self.test_paths = changed_tests.clone();
        let test_files: BTreeSet<&str> = image
            .tests
            .iter()
            .map(|test| test.relative_path.as_str())
            .collect();
        for diagnostic in &image.diagnostics {
            for path in &diagnostic.paths {
                if test_files.contains(path.as_str()) {
                    self.test_paths.insert(path.clone());
                }
            }
        }
        let model_paths: HashMap<&str, &str> = image
            .models
            .iter()
            .map(|model| (model.name.as_str(), model.relative_path.as_str()))
            .collect();
        let mut growing: bool = true;
        while growing {
            growing = false;
            for test in &image.tests {
                if !self.test_paths.contains(&test.relative_path)
                    && !test.chain.iter().any(|name| self.models.contains(name))
                {
                    continue;
                }
                self.test_paths.insert(test.relative_path.clone());
                for name in &test.chain {
                    if let Some(path) = model_paths.get(name.as_str())
                        && self.models.insert(name.clone())
                    {
                        self.model_paths.insert((*path).to_owned());
                        growing = true;
                    }
                }
            }
        }
        self.tests = image
            .tests
            .iter()
            .filter(|test| self.test_paths.contains(&test.relative_path))
            .map(|test| test.key.clone())
            .collect();
        self
    }

    fn with_models(mut self, image: &ProjectImage, added: Vec<String>) -> Self {
        let paths: HashMap<&str, &str> = image
            .models
            .iter()
            .map(|model| (model.name.as_str(), model.relative_path.as_str()))
            .collect();
        for name in added {
            if let Some(path) = paths.get(name.as_str()) {
                self.model_paths.insert((*path).to_owned());
            }
            self.models.insert(name);
        }
        self
    }

    fn names(&self) -> BTreeSet<String> {
        let mut names: BTreeSet<String> = self.models.clone();
        names.extend(self.model_paths.iter().cloned());
        names.extend(self.tests.iter().cloned());
        names.extend(self.test_paths.iter().cloned());
        names
    }

    fn owns(&self, diagnostic: &CapturedDiagnostic) -> bool {
        diagnostic
            .resource_name
            .as_ref()
            .is_some_and(|name| self.models.contains(name) || self.tests.contains(name))
            || diagnostic
                .paths
                .iter()
                .any(|path| self.model_paths.contains(path) || self.test_paths.contains(path))
    }
}

/// The injection of the image's whole-project facts, moved out of `image` while the delta runs.
fn lent_injection(
    mut image: ProjectImage,
    project_dir: PathBuf,
    store_path: &std::path::Path,
) -> (ProjectImage, DeltaInjection) {
    let injection: DeltaInjection = DeltaInjection {
        yaml_store: store_path
            .parent()
            .and_then(std::path::Path::parent)
            .map(sqlbuild_python_base::compile_context::main::yaml_cache::yaml_store_path::yaml_store_path),
        project_dir,
        source_paths: image
            .models
            .iter()
            .map(|model| model.relative_path.clone())
            .chain(image.tests.iter().map(|test| test.relative_path.clone()))
            .collect(),
        project_model_paths: image
            .models
            .iter()
            .map(|model| model.relative_path.clone())
            .collect(),
        model_names: image
            .models
            .iter()
            .map(|model| model.name.clone())
            .collect(),
        relations: std::mem::take(&mut image.relations).into(),
        rules: std::sync::Mutex::new(image.rules.take().and_then(ImageRules::pending)),
        fact_digests: std::mem::take(&mut image.fact_digests)
            .into_iter()
            .collect(),
        model_subjects: image.model_subjects.take(),
        text_models: image.text_models.clone().unwrap_or_default(),
        model_shapes: std::mem::take(&mut image.model_shapes),
        coverage: image.coverage.take(),
        coverage_order: image
            .models
            .iter()
            .map(|model| (model.name.clone(), upstream_models(&model.upstream)))
            .collect(),
        lint_model_keys: image.lint_model_keys.take(),
        lint_file_keys: image.lint_file_keys.take(),
        audit_references: image.audit_references.clone(),
        ..DeltaInjection::default()
    };
    (image, injection)
}

/// Return the facts `lent_injection` moved out of `image`.
fn returned(mut image: ProjectImage, injection: DeltaInjection) -> ProjectImage {
    image.relations = injection.relations.into();
    image.fact_digests = injection.fact_digests.into_iter().collect();
    image.model_subjects = injection.model_subjects;
    image.model_shapes = injection.model_shapes;
    image.coverage = injection.coverage;
    image.lint_model_keys = injection.lint_model_keys;
    image.lint_file_keys = injection.lint_file_keys;
    image
}

/// `injection` set up to rerun `rerun`.
fn for_rerun(
    mut injection: DeltaInjection,
    rerun: &Rerun,
    sources_read: Option<BTreeSet<String>>,
) -> DeltaInjection {
    injection.model_paths = rerun.model_paths.clone();
    injection.test_paths = rerun.test_paths.clone();
    injection.rerun_names = rerun.names();
    injection.changed_paths = rerun.changed.clone();
    injection.source_names = sources_read;
    injection
}

/// The relation facts of `name` that its dependents' analysis reads.
fn shape_of(relations: &RelationTables, name: &str) -> [Option<Vec<(String, String)>>; 3] {
    let find = |table: &[(String, Vec<(String, String)>)]| {
        table
            .iter()
            .find(|(relation, _)| relation == name)
            .map(|(_, shape)| shape.clone())
    };
    [
        find(&relations.types),
        find(&relations.nullability),
        find(&relations.complete),
    ]
}

/// `base`'s rows with each of `fresh`'s rows replacing the one of its name, new names appended.
fn merged_rows(
    mut base: Vec<(String, Vec<(String, String)>)>,
    fresh: Vec<(String, Vec<(String, String)>)>,
) -> Vec<(String, Vec<(String, String)>)> {
    let mut positions: HashMap<String, usize> = base
        .iter()
        .enumerate()
        .map(|(index, (name, _))| (name.clone(), index))
        .collect();
    for (name, shape) in fresh {
        match positions.get(&name) {
            Some(index) => base[*index].1 = shape,
            None => {
                positions.insert(name.clone(), base.len());
                base.push((name, shape));
            }
        }
    }
    base
}

/// One restricted compile of `rerun`, with what it captured.
pub(crate) struct Round<'py> {
    stdout: Option<String>,
    stderr_lines: Vec<String>,
    phases: PhasesOutcome<'py>,
    capture: ImageCapture,
}

enum RoundOutcome<'py> {
    Ran(Box<Round<'py>>),
    Stop(DeltaOutcome<'py>),
}

/// Compile the rerun set once, keeping the longest round's lines: every phase a full compile runs.
fn round<'py>(
    request: &Bound<'py, PyAny>,
    attempt: &Bound<'py, PyAny>,
    (injection, rerun): (DeltaInjection, &Rerun),
) -> PyResult<(RoundOutcome<'py>, Option<DeltaInjection>)> {
    start_capture();
    let projector: Option<Py<NativeProgressProjector>> = current_projector(request.py())?;
    let earlier: Vec<String> = projector
        .as_ref()
        .map(|projector| projector.get().exchange_held_lines(Vec::new()))
        .unwrap_or_default();
    let (ran, injection) = with_injection(injection, || unrecorded_compile(request, attempt));
    if let Some(projector) = &projector {
        let latest: Vec<String> = projector.get().exchange_held_lines(Vec::new());
        let _ = projector
            .get()
            .exchange_held_lines(if latest.len() < earlier.len() {
                earlier
            } else {
                latest
            });
    }
    Ok((ran_round(ran, rerun)?, injection))
}

fn ran_round<'py>(
    ran: PyResult<(
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        PyResult<PhasesOutcome<'py>>,
    )>,
    rerun: &Rerun,
) -> PyResult<RoundOutcome<'py>> {
    let unsupported: Option<String> = take_delta_unsupported();
    let capture: Option<ImageCapture> = take_capture();
    if let Some(reason) = unsupported {
        return Ok(RoundOutcome::Stop(DeltaOutcome::Fallback(reason)));
    }
    let (_, output, phases) = match ran {
        Ok(ran) => ran,
        Err(error) => {
            return Ok(RoundOutcome::Stop(DeltaOutcome::Fallback(format!(
                "the delta compile failed: {error}"
            ))));
        }
    };
    let (stderr_lines, stdout): (Vec<String>, Option<String>) = output
        .cast::<RecordedCompileOutput>()?
        .get()
        .texts(output.py())?;
    let phases: PhasesOutcome<'py> = match phases {
        Ok(phases) => phases,
        Err(error) => {
            let message: String = error.to_string();
            let owned: bool = rerun
                .model_paths
                .iter()
                .chain(&rerun.test_paths)
                .any(|path| message.contains(path.as_str()));
            if rerun.sources.is_some() && message.contains(UNKNOWN_SOURCE) {
                return Ok(RoundOutcome::Stop(DeltaOutcome::Fallback(format!(
                    "the delta compile read a source it did not discover: {message}"
                ))));
            }
            return Ok(RoundOutcome::Stop(if owned && ERRORS_ON_DELTA_PATH {
                DeltaOutcome::Raised(error)
            } else {
                DeltaOutcome::Fallback(format!("the delta compile raised: {message}"))
            }));
        }
    };
    let Some(capture) = capture else {
        return Ok(RoundOutcome::Stop(DeltaOutcome::Fallback(
            "the delta compile captured nothing".to_owned(),
        )));
    };
    if let Some(reason) = &capture.unsupported {
        return Ok(RoundOutcome::Stop(DeltaOutcome::Fallback(reason.clone())));
    }
    Ok(RoundOutcome::Ran(Box::new(Round {
        stdout,
        stderr_lines,
        phases,
        capture,
    })))
}

/// A delta compile ready to run its rounds: the stored compile, the changes and the first rerun.
pub(crate) struct Prepared {
    base: Base,
    pub(crate) changed_files: usize,
    rerun: Rerun,
    changed_tests: BTreeSet<String>,
    limit: usize,
}

/// The first `CANDIDATES_LOGGED` of `paths`.
fn first_paths(paths: &[String]) -> &[String] {
    &paths[..paths.len().min(CANDIDATES_LOGGED)]
}

/// Why `changes` rule a delta out before anything is read, if they do.
fn changes_ruled_out(changes: &ProjectFileChanges) -> Option<String> {
    if !changes.added.is_empty() || !changes.removed.is_empty() {
        return Some("files were added or removed".to_owned());
    }
    if changes.changed.is_empty() {
        return Some("no project file changed".to_owned());
    }
    changes
        .changed
        .iter()
        .find(|path| !resource_sql(path, MODELS_ROOT) && !resource_sql(path, TESTS_ROOT))
        .map(|path| format!("{path} is not a model or SQL test file"))
}

/// The stored compile, its image and the first rerun set, or why the gate rules a delta out.
pub(crate) fn prepared(
    request: &Bound<'_, PyAny>,
    attempt: &Bound<'_, PyAny>,
) -> PyResult<Result<Prepared, String>> {
    let (changes, store_path): (ProjectFileChanges, PathBuf) = match changes_of(attempt)? {
        Ok(found) => found,
        Err(reason) => return Ok(Err(reason)),
    };
    reuse_debug(
        request.py(),
        &format!(
            "Delta compile candidates: changed {:?}, added {:?}, removed {:?}",
            first_paths(&changes.changed),
            first_paths(&changes.added),
            first_paths(&changes.removed),
        ),
    )?;
    if let Some(reason) = changes_ruled_out(&changes) {
        return Ok(Err(reason));
    }
    let Some(mut stored) = read_delta_base(&store_path) else {
        return Ok(Err("the stored compile has no project image".to_owned()));
    };
    let Some(gate) = ImageGate::decode(&stored.image) else {
        return Ok(Err(UNREADABLE_IMAGE.to_owned()));
    };
    if !ERRORS_ON_DELTA_PATH && gate.has_errors {
        return Ok(Err("the stored compile reported errors".to_owned()));
    }
    if gate.json_output != native_field(request, "json_output")?.is_truthy()? {
        return Ok(Err(
            "the stored compile has the other output format".to_owned()
        ));
    }
    let project_dir: PathBuf = native_field(attempt, "project_dir")?.extract()?;
    let (rerun, changed_tests) = match first_rerun(&gate, &changes, &project_dir) {
        Ok(found) => found,
        Err(reason) => return Ok(Err(reason)),
    };
    let all_sources: bool = gate
        .fact_digests
        .iter()
        .any(|(key, _)| lists_all_sources(key));
    if let Some(reason) = stored_custom_rule_refusal(
        &project_dir,
        (&rerun.model_paths, &rerun.changed),
        all_sources,
    ) {
        return Ok(Err(reason.to_owned()));
    }
    let limit: usize = MAX_RERUN_MODELS.max(gate.models.len() * MAX_RERUN_PERCENT / 100);
    if rerun.models.len() > limit {
        return Ok(Err(TOO_MANY_MODELS.to_owned()));
    }
    let Some(image) = ProjectImage::decode(std::mem::take(&mut stored.image), gate) else {
        return Ok(Err(UNREADABLE_IMAGE.to_owned()));
    };
    Ok(Ok(Prepared {
        base: Base {
            store_path,
            stored,
            image,
            project_dir,
        },
        changed_files: changes.changed.len(),
        rerun,
        changed_tests,
        limit,
    }))
}

/// A fallback that stops the rounds.
pub(crate) fn stopped<'py, T>(reason: impl Into<String>) -> PyResult<Result<T, DeltaOutcome<'py>>> {
    Ok(Err(DeltaOutcome::Fallback(reason.into())))
}

/// Run rounds until no rerun model changes the shape a model outside the rerun set reads.
pub(crate) fn rounds<'py>(
    request: &Bound<'py, PyAny>,
    attempt: &Bound<'py, PyAny>,
    prepared: Prepared,
) -> PyResult<Result<(Round<'py>, Rerun, Base), DeltaOutcome<'py>>> {
    let Prepared {
        base,
        mut rerun,
        changed_tests,
        limit,
        ..
    } = prepared;
    let Base {
        store_path,
        stored,
        image,
        project_dir,
    } = base;
    let (image, mut injection): (ProjectImage, DeltaInjection) =
        lent_injection(image, project_dir.clone(), &store_path);
    for _ in 0..MAX_ROUNDS {
        if rerun.models.len() > limit {
            return stopped(TOO_MANY_MODELS);
        }
        rerun = rerun.with_tests(&image, &changed_tests);
        if image
            .diagnostics
            .iter()
            .any(|diagnostic| kept_only_by_full_compiles(&rerun, diagnostic))
        {
            return stopped("the stored compile reports problems in files the edit leaves alone");
        }
        rerun.sources = if reads_all_sources(&injection) {
            None
        } else {
            sources_read(&image, &rerun)
        };
        let injection_for_round: DeltaInjection =
            for_rerun(injection, &rerun, rerun.sources.clone());
        let (outcome, back) = round(request, attempt, (injection_for_round, &rerun))?;
        let model_names: &[String] = back
            .as_ref()
            .map_or(&[], |injection| injection.model_names.as_slice());
        let ran: Round<'py> = match outcome {
            RoundOutcome::Ran(ran) => *ran,
            RoundOutcome::Stop(DeltaOutcome::Raised(error))
                if names_uncompiled_model(&error.to_string(), model_names, &rerun.models) =>
            {
                return stopped(UNCOMPILED_MODEL);
            }
            RoundOutcome::Stop(outcome) => return Ok(Err(outcome)),
        };
        if ran
            .capture
            .diagnostics
            .iter()
            .any(|diagnostic| names_uncompiled_model(&diagnostic.json, model_names, &rerun.models))
        {
            return stopped(UNCOMPILED_MODEL);
        }
        if let Some(path) = rechained_test(&ran.capture.test_chains, &image.tests, &changed_tests) {
            return stopped(format!("{path} now covers other models"));
        }
        let Some(back) = back else {
            return stopped("the delta compile's injection is still in use");
        };
        injection = back;
        let added: Vec<String> = match reshaped_dependents(&ran, &image, &injection, &rerun) {
            Ok(added) => added,
            Err(reason) => return stopped(reason),
        };
        if added.is_empty() {
            let base: Base = Base {
                store_path,
                stored,
                image: returned(image, injection),
                project_dir,
            };
            return Ok(Ok((ran, rerun, base)));
        }
        let rules: Option<BaseRules> = ran
            .capture
            .rules
            .as_ref()
            .and_then(|rules| BaseRules::decoded(&rules.retained));
        let Some(rules) = rules else {
            return stopped("the delta compile retained no rules request");
        };
        injection.rules = std::sync::Mutex::new(Some(PendingRules::Ready(Box::new(rules))));
        rerun = rerun.with_models(&image, added);
    }
    stopped("changed shapes reach too far")
}

/// Whether a custom rule read the project's whole source list, which a delta must then keep.
fn reads_all_sources(injection: &DeltaInjection) -> bool {
    injection
        .fact_digests
        .keys()
        .any(|key| lists_all_sources(key))
}

/// Whether a custom-rule fact key is the project's whole source list.
fn lists_all_sources(key: &[String]) -> bool {
    matches!(key, [name] if name == PROJECT_SOURCES_FACT)
}

/// The sources the rerun models, functions, audits and rerun SQL tests read; `None` for all.
fn sources_read(image: &ProjectImage, rerun: &Rerun) -> Option<BTreeSet<String>> {
    if image.diagnostics.iter().any(outside_resources) {
        return None;
    }
    Some(
        image
            .models
            .iter()
            .filter(|model| rerun.models.contains(&model.name))
            .flat_map(|model| &model.upstream)
            .filter(|(kind, _)| kind == SOURCE_KIND)
            .map(|(_, name)| name.clone())
            .chain(image.pinned_sources.iter().cloned())
            .chain(
                rerun
                    .test_paths
                    .iter()
                    .filter_map(|path| image.test_sources.get(path))
                    .flatten()
                    .cloned(),
            )
            .collect(),
    )
}

/// The models outside `rerun` whose inputs' shapes the round changed, or why it cannot go on.
fn reshaped_dependents(
    ran: &Round<'_>,
    image: &ProjectImage,
    injection: &DeltaInjection,
    rerun: &Rerun,
) -> Result<Vec<String>, String> {
    if rerun.models.is_empty() {
        return Ok(Vec::new());
    }
    let Some(relations) = ran.capture.relations.as_ref() else {
        return Err("the delta compile analysed no models".to_owned());
    };
    let stored_relations: &RelationTables = &injection.relations;
    if relations.cyclic != stored_relations.cyclic
        || relations.dependency_ordered != stored_relations.dependency_ordered
    {
        return Err("the project's analysis order changed".to_owned());
    }
    let stored_models: HashMap<&str, &CapturedModel> = image
        .models
        .iter()
        .map(|model| (model.name.as_str(), model))
        .collect();
    if let Some(model) = ran
        .capture
        .models
        .iter()
        .find(|model| stored_models.get(model.name.as_str()) != Some(model))
    {
        return Err(format!("{} changed its references", model.name));
    }
    if let Some(path) = rerun
        .model_paths
        .iter()
        .find(|path| ran.capture.usages.get(*path) != image.usages.get(*path))
    {
        return Err(format!("{path} changed the declarations it uses"));
    }
    let reshaped: BTreeSet<String> = rerun
        .models
        .iter()
        .filter(|name| shape_of(relations, name) != shape_of(stored_relations, name))
        .cloned()
        .collect();
    Ok(image
        .dependents(&reshaped)
        .into_iter()
        .filter(|name| !rerun.models.contains(name))
        .collect())
}

/// The models and SQL tests the changed files rerun, or why the gate rules a delta out.
fn first_rerun(
    gate: &ImageGate,
    changes: &ProjectFileChanges,
    project_dir: &std::path::Path,
) -> Result<(Rerun, BTreeSet<String>), String> {
    let model_by_path: HashMap<&str, &str> = gate
        .models
        .iter()
        .map(|model| (model.relative_path.as_str(), model.name.as_str()))
        .collect();
    let test_files: BTreeSet<&str> = gate
        .tests
        .iter()
        .map(|test| test.relative_path.as_str())
        .collect();
    let mut rerun: Rerun = Rerun {
        changed: changes.changed.iter().cloned().collect(),
        ..Rerun::default()
    };
    let mut changed_tests: BTreeSet<String> = BTreeSet::new();
    for path in &changes.changed {
        if resource_sql(path, MODELS_ROOT)
            && let Some(name) = model_by_path.get(path.as_str())
        {
            rerun.models.insert((*name).to_owned());
            rerun.model_paths.insert(path.clone());
        } else if resource_sql(path, TESTS_ROOT) && test_files.contains(path.as_str()) {
            changed_tests.insert(path.clone());
        } else {
            return Err(format!("{path} is not a model or SQL test file"));
        }
    }
    let upstream_by_path: HashMap<&str, &[(String, String)]> = gate
        .models
        .iter()
        .map(|model| (model.relative_path.as_str(), model.upstream.as_slice()))
        .collect();
    for path in &rerun.model_paths {
        let name: &str = model_by_path
            .get(path.as_str())
            .copied()
            .unwrap_or_default();
        let upstream: &[(String, String)] = upstream_by_path
            .get(path.as_str())
            .copied()
            .unwrap_or_default();
        match header_change(project_dir, path, (gate.header_keys.get(path), upstream)) {
            HeaderChange::Unchanged => {}
            HeaderChange::Unparsed if !ERRORS_ON_DELTA_PATH => {
                return Err(format!("{path} does not parse"));
            }
            HeaderChange::Unparsed => {}
            HeaderChange::Unsupported(reason) => return Err(reason),
            HeaderChange::Patched(keys) => {
                if gate.has_model_subjects && keys.contains("materialized") {
                    return Err("custom rules select models by their materialization".to_owned());
                }
                if let Some(reason) = custom_rule_reads(&gate.fact_digests, (path, name), &keys) {
                    return Err(reason);
                }
            }
        }
    }
    let path_by_model: HashMap<&str, &str> = model_by_path
        .iter()
        .map(|(path, name)| (*name, *path))
        .collect();
    let upstream_by_name: HashMap<&str, &[(String, String)]> = gate
        .models
        .iter()
        .map(|model| (model.name.as_str(), model.upstream.as_slice()))
        .collect();
    for test in &gate.tests {
        if !changed_tests.contains(&test.relative_path) {
            continue;
        }
        let ring = test.chain.iter().flat_map(|name| {
            upstream_models(
                upstream_by_name
                    .get(name.as_str())
                    .copied()
                    .unwrap_or_default(),
            )
        });
        let named: BTreeSet<String> =
            test_model_references(project_dir, &test.relative_path, &path_by_model)?;
        for name in test.chain.iter().cloned().chain(ring).chain(named) {
            let Some(path) = path_by_model.get(name.as_str()) else {
                return Err(format!("{} tests a model the image lacks", test.key));
            };
            rerun.model_paths.insert((*path).to_owned());
            rerun.models.insert(name);
        }
    }
    Ok((rerun, changed_tests))
}

/// The first edited SQL test whose model chain the delta resolved differently than stored.
fn rechained_test<'a>(
    fresh: &[CapturedTest],
    stored: &'a [CapturedTest],
    changed_tests: &BTreeSet<String>,
) -> Option<&'a str> {
    let sorted = |chain: &[String]| -> BTreeSet<String> { chain.iter().cloned().collect() };
    let fresh: HashMap<&str, BTreeSet<String>> = fresh
        .iter()
        .map(|test| (test.key.as_str(), sorted(&test.chain)))
        .collect();
    stored
        .iter()
        .filter(|test| changed_tests.contains(&test.relative_path))
        .find(|test| fresh.get(test.key.as_str()) != Some(&sorted(&test.chain)))
        .map(|test| test.relative_path.as_str())
}

/// Every project model the SQL test file at `path` names, a harmless superset of its references.
fn test_model_references(
    project_dir: &std::path::Path,
    path: &str,
    path_by_model: &HashMap<&str, &str>,
) -> Result<BTreeSet<String>, String> {
    let Ok(text) = std::fs::read_to_string(project_dir.join(path)) else {
        return Err(format!("{path} cannot be read"));
    };
    let mut names: BTreeSet<String> = BTreeSet::new();
    for word in text.split(|character: char| !(character.is_alphanumeric() || character == '_')) {
        let unprefixed: &str = word
            .strip_prefix(CTE_PREFIX_MARK)
            .and_then(|rest| rest.split_once(CTE_PREFIX_MARK))
            .map_or(word, |(_, name)| name);
        for candidate in [word, unprefixed] {
            if path_by_model.contains_key(candidate) {
                names.insert(candidate.to_owned());
            }
        }
    }
    Ok(names)
}

/// Whether `message` reports as missing a project model the delta did not compile.
pub(crate) fn names_uncompiled_model(
    message: &str,
    model_names: &[String],
    rerun_models: &BTreeSet<String>,
) -> bool {
    MISSING_MODEL_PHRASES
        .iter()
        .any(|phrase| message.contains(phrase))
        && message
            .split(QUOTE)
            .skip(1)
            .step_by(2)
            .filter(|name| !rerun_models.contains(*name))
            .any(|name| project_model(model_names, name))
}

fn project_model(model_names: &[String], name: &str) -> bool {
    model_names.iter().any(|model| model == name)
}

/// The names of the models among `upstream`.
fn upstream_models(upstream: &[(String, String)]) -> Vec<String> {
    upstream
        .iter()
        .filter(|(kind, _)| kind == MODEL_KIND)
        .map(|(_, name)| name.clone())
        .collect()
}

/// Whether only a full compile would repeat a stored non-rule diagnostic outside the rerun.
fn kept_only_by_full_compiles(rerun: &Rerun, diagnostic: &CapturedDiagnostic) -> bool {
    !rerun.owns(diagnostic)
        && !diagnostic.json.contains(RULE_PHASE)
        && diagnostic
            .paths
            .iter()
            .any(|path| path.starts_with(MODELS_ROOT) || path.starts_with(TESTS_ROOT))
}

/// Whether a stored diagnostic concerns a file other than a model or SQL test.
fn outside_resources(diagnostic: &CapturedDiagnostic) -> bool {
    diagnostic
        .paths
        .iter()
        .any(|path| !path.starts_with(MODELS_ROOT) && !path.starts_with(TESTS_ROOT))
        || diagnostic.paths.is_empty() && diagnostic.resource_name.is_none()
}

/// Every diagnostic not owned by the rerun set, which the delta must leave as it was.
fn unowned<'a>(rerun: &Rerun, diagnostics: &'a [CapturedDiagnostic]) -> Vec<&'a str> {
    diagnostics
        .iter()
        .filter(|diagnostic| !rerun.owns(diagnostic))
        .map(|diagnostic| diagnostic.json.as_str())
        .collect()
}

pub(crate) fn finished<'py>(
    ran: Round<'py>,
    base: Base,
    rerun: &Rerun,
    (py, changed_files): (Python<'py>, usize),
) -> PyResult<DeltaOutcome<'py>> {
    if !ERRORS_ON_DELTA_PATH && ran.phases.0.exit_code != 0 {
        let first: &str = ran
            .capture
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.error)
            .map_or("no error diagnostic", |diagnostic| diagnostic.json.as_str());
        return fallback(format!("the edit reports errors: {first}"));
    }
    if rerun.sources.is_some()
        && ran
            .capture
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.json.contains(UNKNOWN_SOURCE))
    {
        return fallback("the delta compile read a source it did not discover");
    }
    let (stored, fresh) = (
        unowned(rerun, &base.image.diagnostics),
        unowned(rerun, &ran.capture.diagnostics),
    );
    if stored != fresh {
        let only = |left: &[&str], right: &[&str]| {
            left.iter()
                .find(|item| !right.contains(item))
                .map_or(String::new(), |item| (*item).to_owned())
        };
        return fallback(format!(
            "diagnostics outside the changed files differ: stored {} / delta {}",
            only(&stored, &fresh),
            only(&fresh, &stored)
        ));
    }
    let Some(delta_stdout) = ran.stdout else {
        return fallback("the delta compile recorded no report");
    };
    if env_enabled(DELTA_VERIFY_ENV_VAR) {
        let directory: PathBuf = base.project_dir.join(VERIFY_REPORT_DIRECTORY);
        let _ = std::fs::create_dir_all(&directory);
        let _ = std::fs::write(directory.join(VERIFY_REPORT_FILE), &delta_stdout);
    }
    let stderr_lines: Vec<String> = ran.stderr_lines;
    let merge: RecordMerge = RecordMerge {
        kept: base.stored.inputs.target_files.clone(),
        environment_names: base.image.environment_names.clone(),
        module_paths: base
            .stored
            .inputs
            .modules
            .iter()
            .map(|(path, _, _)| path.clone())
            .collect(),
    };
    let json_output: bool = base.image.json_output;
    let Base {
        store_path,
        stored,
        image,
        ..
    } = base;
    let capture: ImageCapture = ran.capture;
    let (image, stdout): (Option<ImageSections>, Result<String, String>) = py.detach(|| {
        rayon::join(
            || {
                let Ok(sections) = patched_image(image, rerun, capture).encode() else {
                    return None;
                };
                Some(sections)
            },
            || {
                if !json_output {
                    return Ok(delta_stdout);
                }
                let Some(stored_output) = read_delta_output(&store_path, &stored) else {
                    return Err("the stored report changed".to_owned());
                };
                let scope: SpliceScope<'_> = SpliceScope {
                    models: &rerun.models,
                    tests: &rerun.tests,
                    sources: rerun.sources.as_ref(),
                };
                spliced_report(&stored_output.stdout, &delta_stdout, &scope)
            },
        )
    });
    let stdout: String = match stdout {
        Ok(stdout) => stdout,
        Err(reason) => return fallback(reason),
    };
    let _ = py;
    Ok(DeltaOutcome::Finished(Box::new(Finished {
        stdout,
        stderr_lines,
        phases: ran.phases,
        image,
        merge,
        changed_files,
        rerun_models: rerun.models.len(),
    })))
}

/// The stored image with the rerun models' and tests' facts replaced by the delta's.
fn patched_image(base: ProjectImage, rerun: &Rerun, capture: ImageCapture) -> ProjectImage {
    let mut image: ProjectImage = base;
    let fresh_models: HashMap<String, _> = capture
        .models
        .into_iter()
        .map(|model| (model.name.clone(), model))
        .collect();
    for model in &mut image.models {
        if let Some(fresh) = fresh_models.get(&model.name) {
            *model = fresh.clone();
        }
    }
    image.usages.extend(capture.usages);
    let mut fresh_tests: HashMap<String, CapturedTest> = capture
        .test_chains
        .into_iter()
        .map(|test| (test.key.clone(), test))
        .collect();
    for test in &mut image.tests {
        if rerun.tests.contains(&test.key)
            && let Some(fresh) = fresh_tests.remove(&test.key)
        {
            *test = fresh;
        }
    }
    if let Some(relations) = capture.relations {
        let fresh: ImageRelations = relations.into();
        image.relations.types =
            merged_rows(std::mem::take(&mut image.relations.types), fresh.types);
        image.relations.nullability = merged_rows(
            std::mem::take(&mut image.relations.nullability),
            fresh.nullability,
        );
        image.relations.complete = merged_rows(
            std::mem::take(&mut image.relations.complete),
            fresh.complete,
        );
        image.relations.schemas =
            merged_rows(std::mem::take(&mut image.relations.schemas), fresh.schemas);
        let mut legacy: BTreeSet<String> = image.relations.legacy_models.iter().cloned().collect();
        for name in &rerun.models {
            legacy.remove(name);
        }
        legacy.extend(fresh.legacy_models);
        image.relations.legacy_models = legacy.into_iter().collect();
    }
    let mut digests: BTreeMap<Vec<String>, String> = image.fact_digests.into_iter().collect();
    digests.extend(capture.fact_digests);
    image.fact_digests = digests.into_iter().collect();
    image.diagnostics = capture.diagnostics;
    if capture.text_models.is_some() {
        image.text_models = capture.text_models;
    }
    image.rules = capture
        .rules
        .map(|rules| ImageRules::Retained(rules.retained));
    if let Some(shapes) = capture.model_shapes {
        image.model_shapes = shapes;
    }
    if capture.coverage.is_some() {
        image.coverage = capture.coverage;
    }
    image.header_keys.extend(capture.header_keys);
    image
        .audit_references
        .retain(|path, _| !rerun.model_paths.contains(path));
    image.audit_references.extend(capture.audit_references);
    let mut pinned: BTreeSet<String> = std::mem::take(&mut image.pinned_sources)
        .into_iter()
        .collect();
    pinned.extend(capture.pinned_sources);
    image.pinned_sources = pinned.into_iter().collect();
    for (path, names) in capture.test_sources {
        image.test_sources.insert(path, names.into_iter().collect());
    }
    image.lint_model_keys = capture.lint_model_keys;
    image.lint_file_keys = capture.lint_file_keys;
    image
}
