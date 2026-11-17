//! Public records of compile delta.

use crate::compile_delta::_helpers::image::FamiliesByTable;
use crate::compile_delta::_helpers::image::RULE_ROWS_SECTION;
use crate::compile_delta::_helpers::image::RULES_SECTION;
use crate::compile_delta::_helpers::image::StoredFacts;
use crate::compile_delta::_helpers::image::decoded_rules;
use crate::compile_delta::_helpers::image::differing;
use crate::compile_delta::_helpers::image::encoded_strings;
use crate::compile_delta::_helpers::image::reads_any_model;
use crate::compile_delta::_helpers::image::restored;
use crate::compile_delta::_helpers::image::section;
use crate::compile_delta::constants::FACTS_SECTION;
use crate::compile_delta::constants::GATE_SECTION;
use crate::compile_delta::constants::IMAGE_FORMAT;
use crate::compile_delta::constants::RULE_DIGESTS_SECTION;
use crate::compile_driver::types::PhasesOutcome;
use pyo3::prelude::PyErr;
use sqlbuild_analysis::assembly::analysis_session::models::RelationTables;
use sqlbuild_analysis::assembly::analysis_session::types::Shapes;
use sqlbuild_cache::project_reuse::types::ImageSections;
use sqlbuild_discovery::header_changes::types::HeaderKeys;
use sqlbuild_python_base::compile_context::models::CapturedDiagnostic;
use sqlbuild_python_base::compile_context::models::CapturedModel;
use sqlbuild_python_base::compile_context::models::CapturedTest;
use sqlbuild_python_base::compile_context::models::Decoder;
use sqlbuild_python_base::compile_context::models::Encoder;
use sqlbuild_python_base::compile_context::models::ImageCapture;
use sqlbuild_python_base::compile_context::models::ModelCoverage;
use sqlbuild_python_base::compile_context::models::PendingRules;
use sqlbuild_python_base::compile_context::models::RecordMerge;
use sqlbuild_python_base::compile_context::types::ModelShape;
use sqlbuild_rules::models::RetainedRows;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// A delta compile that finished, ready to be stored and printed or compared.
#[derive(Debug)]
pub struct Finished<'py> {
    pub stdout: String,
    pub stderr_lines: Vec<String>,
    pub phases: PhasesOutcome<'py>,
    pub image: Option<ImageSections>,
    pub merge: RecordMerge,
    pub changed_files: usize,
    pub rerun_models: usize,
}

/// What one delta attempt came to.
#[derive(Debug)]
pub enum DeltaOutcome<'py> {
    /// The delta cannot reproduce this compile; the reason is for diagnostics only.
    Fallback(String),
    /// The compile stopped on an error from a changed file, exactly as a full compile would.
    Raised(PyErr),
    Finished(Box<Finished<'py>>),
}

/// Everything a delta compile reads of the compile it patches.
#[derive(Debug, Default)]
pub struct ProjectImage {
    pub format: String,
    pub json_output: bool,
    pub models: Vec<CapturedModel>,
    pub usages: BTreeMap<String, String>,
    pub tests: Vec<CapturedTest>,
    pub relations: ImageRelations,
    pub fact_digests: Vec<(Vec<String>, String)>,
    pub model_subjects: Option<Vec<(String, String, Option<String>)>>,
    pub diagnostics: Vec<CapturedDiagnostic>,
    pub text_models: Option<Vec<(String, bool, i64)>>,
    pub environment_names: Vec<String>,
    pub pinned_sources: Vec<String>,
    pub test_sources: BTreeMap<String, Vec<String>>,
    pub model_shapes: Vec<ModelShape>,
    pub coverage: Option<Vec<ModelCoverage>>,
    pub lint_model_keys: Option<Vec<(String, String)>>,
    pub lint_file_keys: Option<Vec<(String, Option<String>)>>,
    pub header_keys: BTreeMap<String, HeaderKeys>,
    pub audit_references: BTreeMap<String, Vec<String>>,
    pub rules: Option<ImageRules>,
}

/// An image's rules request: decoding from a stored image, or as a compile just retained it.
#[derive(Debug)]
pub enum ImageRules {
    Decoding(PendingRules),
    Retained(RetainedRows),
}

impl ImageRules {
    /// The rules request still decoding from a stored image, if this is one.
    pub fn pending(self) -> Option<PendingRules> {
        match self {
            Self::Decoding(rules) => Some(rules),
            Self::Retained(_) => None,
        }
    }

    /// The rows a compile just retained, if this is them.
    pub fn retained(self) -> Option<RetainedRows> {
        match self {
            Self::Retained(rules) => Some(rules),
            Self::Decoding(_) => None,
        }
    }
}

/// The analysis session's final relation facts, as stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageRelations {
    pub types: Shapes,
    pub nullability: Shapes,
    pub complete: Shapes,
    pub schemas: Shapes,
    pub dependency_ordered: bool,
    pub cyclic: bool,
    pub legacy_models: Vec<String>,
    pub families: FamiliesByTable,
}

impl From<RelationTables> for ImageRelations {
    fn from(tables: RelationTables) -> Self {
        Self {
            types: tables.types,
            nullability: tables.nullability,
            complete: tables.complete,
            schemas: tables.schemas,
            dependency_ordered: tables.dependency_ordered,
            cyclic: tables.cyclic,
            legacy_models: tables.legacy_models,
            families: tables.families,
        }
    }
}

/// What a delta compile reads first to decide, cheaply, whether it can patch the image.
#[derive(Clone, Debug, Default)]
pub struct ImageGate {
    pub format: String,
    pub json_output: bool,
    /// Whether the stored compile reported any error diagnostic.
    pub has_errors: bool,
    /// Whether custom rules recorded which models they select.
    pub has_model_subjects: bool,
    pub models: Vec<CapturedModel>,
    pub tests: Vec<CapturedTest>,
    pub fact_digests: Vec<(Vec<String>, String)>,
    pub header_keys: BTreeMap<String, HeaderKeys>,
}

impl ImageGate {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut encoder: Encoder<'_> = Encoder::default();
        encoder.string(&self.format);
        encoder.flag(self.json_output);
        encoder.flag(self.has_errors);
        encoder.flag(self.has_model_subjects);
        encoder.count(self.models.len());
        for model in &self.models {
            encoder.string(&model.name);
            encoder.string(&model.relative_path);
            encoder.pairs(&model.upstream);
        }
        encoder.count(self.tests.len());
        for test in &self.tests {
            encoder.string(&test.key);
            encoder.string(&test.relative_path);
            encoder.strings(test.chain.iter().map(String::as_str));
        }
        encoder.count(self.fact_digests.len());
        for (key, digest) in &self.fact_digests {
            encoder.strings(key.iter().map(String::as_str));
            encoder.string(digest);
        }
        encoder.count(self.header_keys.len());
        for (path, keys) in &self.header_keys {
            encoder.string(path);
            encoder.pairs(keys);
        }
        encoder.finish()
    }

    /// The gate stored in `sections`, or `None` when it is unreadable or of another format.
    pub fn decode(sections: &ImageSections) -> Option<Self> {
        let mut decoder: Decoder<'_> = Decoder::new(section(sections, GATE_SECTION)?)?;
        let format: String = decoder.string()?;
        if format != IMAGE_FORMAT {
            return None;
        }
        let gate: Self = Self {
            format,
            json_output: decoder.flag()?,
            has_errors: decoder.flag()?,
            has_model_subjects: decoder.flag()?,
            models: decoder.list(|decoder| {
                Some(CapturedModel {
                    name: decoder.string()?,
                    relative_path: decoder.string()?,
                    upstream: decoder.pairs()?,
                })
            })?,
            tests: decoder.list(|decoder| {
                Some(CapturedTest {
                    key: decoder.string()?,
                    relative_path: decoder.string()?,
                    chain: decoder.strings()?,
                })
            })?,
            fact_digests: decoder.list(|decoder| Some((decoder.strings()?, decoder.string()?)))?,
            header_keys: decoder
                .list(|decoder| Some((decoder.string()?, decoder.pairs()?)))?
                .into_iter()
                .collect(),
        };
        decoder.finish()?;
        Some(gate)
    }
}

impl ProjectImage {
    /// The image of a finished compile's capture, or why it cannot be a delta base.
    pub fn from_capture(capture: ImageCapture, json_output: bool) -> Result<Self, String> {
        if let Some(reason) = capture.unsupported {
            return Err(reason);
        }
        if capture.stdlib_json {
            return Err("the report fell back to the standard library serializer".to_owned());
        }
        let Some(relations) = capture.relations else {
            return Err("no model analysis session ran".to_owned());
        };
        if !json_output && capture.text_models.is_none() {
            return Err("no text report was rendered".to_owned());
        }
        let Some(model_shapes) = capture.model_shapes else {
            return Err("no semantic shapes were published".to_owned());
        };
        let mut environment_names: Vec<String> = capture.environment_names;
        environment_names.sort();
        environment_names.dedup();
        Ok(Self {
            format: IMAGE_FORMAT.to_owned(),
            json_output,
            models: capture.models,
            usages: capture.usages,
            tests: capture.test_chains,
            relations: relations.into(),
            fact_digests: capture.fact_digests.into_iter().collect(),
            model_subjects: capture.model_subjects,
            diagnostics: capture.diagnostics,
            text_models: capture.text_models,
            environment_names,
            pinned_sources: capture.pinned_sources.into_iter().collect(),
            test_sources: capture
                .test_sources
                .into_iter()
                .map(|(path, names)| (path, names.into_iter().collect()))
                .collect(),
            model_shapes,
            coverage: capture.coverage,
            lint_model_keys: capture.lint_model_keys,
            lint_file_keys: capture.lint_file_keys,
            header_keys: capture.header_keys,
            audit_references: capture.audit_references,
            rules: capture
                .rules
                .map(|rules| ImageRules::Retained(rules.retained)),
        })
    }

    /// The image as the sections of its file.
    pub fn encode(self) -> Result<ImageSections, String> {
        let gate: ImageGate = ImageGate {
            format: self.format,
            json_output: self.json_output,
            has_errors: self.diagnostics.iter().any(|diagnostic| diagnostic.error),
            has_model_subjects: self.model_subjects.is_some(),
            models: self.models,
            tests: self.tests,
            fact_digests: self.fact_digests,
            header_keys: self.header_keys,
        };
        let relations: ImageRelations = self.relations;
        let types: &Shapes = &relations.types;
        let facts: StoredFacts = StoredFacts {
            usages: self.usages,
            complete: differing(&relations.complete, types),
            schemas: differing(&relations.schemas, types),
            model_shapes: differing(&self.model_shapes, types),
            types: relations.types,
            nullability: relations.nullability,
            dependency_ordered: relations.dependency_ordered,
            cyclic: relations.cyclic,
            legacy_models: relations.legacy_models,
            families: relations.families,
            model_subjects: self.model_subjects,
            diagnostics: self.diagnostics,
            text_models: self.text_models,
            environment_names: self.environment_names,
            coverage: self.coverage,
            pinned_sources: self.pinned_sources,
            test_sources: self.test_sources,
            lint_model_keys: self.lint_model_keys,
            lint_file_keys: self.lint_file_keys,
            audit_references: self.audit_references,
        };
        let rules: Option<RetainedRows> = self.rules.and_then(ImageRules::retained);
        let (gate, facts) = rayon::join(|| gate.encode(), || facts.encode());
        let rules = rules.map(|rules| {
            (
                rules.request_json,
                rules.rows,
                encoded_strings(&rules.digests),
            )
        });
        let mut sections: ImageSections = vec![
            (GATE_SECTION.to_owned(), gate),
            (FACTS_SECTION.to_owned(), facts?),
        ];
        if let Some((request, rows, digests)) = rules {
            sections.push((RULES_SECTION.to_owned(), request));
            sections.push((RULE_ROWS_SECTION.to_owned(), rows));
            sections.push((RULE_DIGESTS_SECTION.to_owned(), digests));
        }
        Ok(sections)
    }

    /// The image behind `gate`, its rules request decoding on its own thread; `None` if unreadable.
    pub fn decode(mut sections: ImageSections, gate: ImageGate) -> Option<Self> {
        let mut taken = |name: &str| {
            sections
                .iter_mut()
                .find(|(section, _)| section == name)
                .map(|(_, bytes)| std::mem::take(bytes))
        };
        let stored_rules = (
            taken(RULES_SECTION),
            taken(RULE_ROWS_SECTION),
            taken(RULE_DIGESTS_SECTION),
        );
        let rules: Option<ImageRules> = match stored_rules {
            (Some(request), Some(rows), Some(digests)) => {
                Some(ImageRules::Decoding(PendingRules::Decoding(
                    std::thread::spawn(move || decoded_rules(request, rows, &digests)),
                )))
            }
            (None, None, None) => None,
            _ => return None,
        };
        let facts: StoredFacts = StoredFacts::decode(section(&sections, FACTS_SECTION)?)?;
        let types: Shapes = facts.types;
        let relations: ImageRelations = ImageRelations {
            complete: restored(facts.complete, &types)?,
            schemas: restored(facts.schemas, &types)?,
            nullability: facts.nullability,
            dependency_ordered: facts.dependency_ordered,
            cyclic: facts.cyclic,
            legacy_models: facts.legacy_models,
            families: facts.families,
            types: Shapes::new(),
        };
        let model_shapes: Shapes = restored(facts.model_shapes, &types)?;
        Some(Self {
            format: gate.format,
            json_output: gate.json_output,
            models: gate.models,
            usages: facts.usages,
            tests: gate.tests,
            relations: ImageRelations { types, ..relations },
            fact_digests: gate.fact_digests,
            model_subjects: facts.model_subjects,
            diagnostics: facts.diagnostics,
            text_models: facts.text_models,
            environment_names: facts.environment_names,
            pinned_sources: facts.pinned_sources,
            test_sources: facts.test_sources,
            model_shapes,
            coverage: facts.coverage,
            lint_model_keys: facts.lint_model_keys,
            lint_file_keys: facts.lint_file_keys,
            header_keys: gate.header_keys,
            audit_references: facts.audit_references,
            rules,
        })
    }

    /// Every model whose reads include a relation of `names`, outside `names`.
    pub fn dependents(&self, names: &BTreeSet<String>) -> BTreeSet<String> {
        self.models
            .iter()
            .filter(|model| !names.contains(&model.name))
            .filter(|model| reads_any_model(model, names))
            .map(|model| model.name.clone())
            .collect()
    }
}

/// What the splice replaces: the rerun models, their audits and the rerun SQL tests.
#[derive(Debug)]
pub struct SpliceScope<'a> {
    pub models: &'a BTreeSet<String>,
    pub tests: &'a BTreeSet<String>,
    /// The sources the delta discovered, when it discovered only some; `None` when all.
    pub sources: Option<&'a BTreeSet<String>>,
}

/// What the delta compile produced, held until the full compile finishes.
#[derive(Debug, Default)]
pub struct Expected {
    pub stdout: Option<String>,
    pub exit_code: Option<i64>,
    pub raised: Option<String>,
    /// Every file under the compiled root after the delta, with its native digest.
    pub compiled: BTreeMap<String, String>,
    pub compiled_root: String,
    pub artifacts_written: bool,
    /// Every file under `target/compiled-failed/` after the delta, with its native digest.
    pub failed: BTreeMap<String, String>,
    pub failed_root: String,
}
