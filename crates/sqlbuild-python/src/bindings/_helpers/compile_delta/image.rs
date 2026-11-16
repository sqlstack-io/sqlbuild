//! The project image a full or delta compile stores beside its reuse slot.

use std::collections::{BTreeMap, BTreeSet};

use sqlbuild_analysis::assembly::analysis_session::models::{DynamicFamily, RelationTables};
use sqlbuild_analysis::assembly::analysis_session::types::Shapes;
use sqlbuild_cache::project_reuse::types::ImageSections;
use sqlbuild_rules::models::RetainedRows;

use sqlbuild_discovery::header_changes::types::HeaderKeys;
use sqlbuild_python_base::compile_context::models::BaseRules;
use sqlbuild_python_base::compile_context::models::CapturedDiagnostic;
use sqlbuild_python_base::compile_context::models::CapturedModel;
use sqlbuild_python_base::compile_context::models::CapturedTest;
use sqlbuild_python_base::compile_context::models::Decoder;
use sqlbuild_python_base::compile_context::models::Encoder;
use sqlbuild_python_base::compile_context::models::ImageCapture;
use sqlbuild_python_base::compile_context::models::ModelCoverage;
use sqlbuild_python_base::compile_context::models::PendingRules;
use sqlbuild_python_base::compile_context::types::ModelShape;

/// Changes whenever the image's meaning changes, so an older image is never read.
pub(crate) const IMAGE_FORMAT: &str = "sqlbuild-project-image-v9";
pub(crate) const GATE_SECTION: &str = "gate";
/// The upstream kind of a model another model reads.
const MODEL_KIND: &str = "model";
pub(crate) const FACTS_SECTION: &str = "facts";
const RULES_SECTION: &str = "rules";
pub(crate) const RULE_DIGESTS_SECTION: &str = "rule-digests";
const RULE_ROWS_SECTION: &str = "rule-rows";

/// Everything a delta compile reads of the compile it patches.
#[derive(Debug, Default)]
pub(crate) struct ProjectImage {
    pub(crate) format: String,
    pub(crate) json_output: bool,
    pub(crate) models: Vec<CapturedModel>,
    pub(crate) usages: BTreeMap<String, String>,
    pub(crate) tests: Vec<CapturedTest>,
    pub(crate) relations: ImageRelations,
    pub(crate) fact_digests: Vec<(Vec<String>, String)>,
    pub(crate) model_subjects: Option<Vec<(String, String, Option<String>)>>,
    pub(crate) diagnostics: Vec<CapturedDiagnostic>,
    pub(crate) text_models: Option<Vec<(String, bool, i64)>>,
    pub(crate) environment_names: Vec<String>,
    pub(crate) pinned_sources: Vec<String>,
    pub(crate) test_sources: BTreeMap<String, Vec<String>>,
    pub(crate) model_shapes: Vec<ModelShape>,
    pub(crate) coverage: Option<Vec<ModelCoverage>>,
    pub(crate) lint_model_keys: Option<Vec<(String, String)>>,
    pub(crate) lint_file_keys: Option<Vec<(String, Option<String>)>>,
    pub(crate) header_keys: BTreeMap<String, HeaderKeys>,
    pub(crate) audit_references: BTreeMap<String, Vec<String>>,
    pub(crate) rules: Option<ImageRules>,
}

/// An image's rules request: decoding from a stored image, or as a compile just retained it.
#[derive(Debug)]
pub(crate) enum ImageRules {
    Decoding(PendingRules),
    Retained(RetainedRows),
}

impl ImageRules {
    /// The rules request still decoding from a stored image, if this is one.
    pub(crate) fn pending(self) -> Option<PendingRules> {
        match self {
            Self::Decoding(rules) => Some(rules),
            Self::Retained(_) => None,
        }
    }

    /// The rows a compile just retained, if this is them.
    pub(crate) fn retained(self) -> Option<RetainedRows> {
        match self {
            Self::Retained(rules) => Some(rules),
            Self::Decoding(_) => None,
        }
    }
}

/// The value `bytes` hold as JSON, or `None` when they are malformed.
fn from_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    let Ok(value) = serde_json::from_slice(bytes) else {
        return None;
    };
    Some(value)
}

fn decoded_rules(request_json: Vec<u8>, rows: Vec<u8>, digests: &[u8]) -> Option<BaseRules> {
    let retained: RetainedRows = RetainedRows {
        request_json,
        rows,
        digests: decoded_strings(digests)?,
    };
    BaseRules::decoded(&retained)
}

/// The analysis session's final relation facts, as stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ImageRelations {
    pub(crate) types: Shapes,
    pub(crate) nullability: Shapes,
    pub(crate) complete: Shapes,
    pub(crate) schemas: Shapes,
    pub(crate) dependency_ordered: bool,
    pub(crate) cyclic: bool,
    pub(crate) legacy_models: Vec<String>,
    pub(crate) families: FamiliesByTable,
}

/// Each table's dynamic column families, in the order the analysis request listed them.
pub(crate) type FamiliesByTable = Vec<(String, Vec<DynamicFamily>)>;

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

impl From<ImageRelations> for RelationTables {
    fn from(relations: ImageRelations) -> Self {
        Self {
            types: relations.types,
            nullability: relations.nullability,
            complete: relations.complete,
            schemas: relations.schemas,
            dependency_ordered: relations.dependency_ordered,
            cyclic: relations.cyclic,
            legacy_models: relations.legacy_models,
            families: relations.families,
        }
    }
}

/// Rows of `table` that differ from the same relation's `types` row; `None` where equal.
fn differing(table: &Shapes, types: &Shapes) -> StoredShapes {
    let types: std::collections::HashMap<&str, &Vec<(String, String)>> = types
        .iter()
        .map(|(name, shape)| (name.as_str(), shape))
        .collect();
    table
        .iter()
        .map(|(name, shape)| {
            let same: bool = types
                .get(name.as_str())
                .is_some_and(|typed| *typed == shape);
            (name.clone(), (!same).then(|| shape.clone()))
        })
        .collect()
}

/// `differing`'s rows restored against `types`.
fn restored(rows: StoredShapes, types: &Shapes) -> Option<Shapes> {
    let types: std::collections::HashMap<&str, &Vec<(String, String)>> = types
        .iter()
        .map(|(name, shape)| (name.as_str(), shape))
        .collect();
    rows.into_iter()
        .map(|(name, shape)| {
            let shape: Vec<(String, String)> = match shape {
                Some(shape) => shape,
                None => (*types.get(name.as_str())?).clone(),
            };
            Some((name, shape))
        })
        .collect()
}

/// A shape table as stored: `None` where a relation's row equals its `types` row.
type StoredShapes = Vec<(String, Option<Vec<(String, String)>>)>;

/// What a delta compile reads first to decide, cheaply, whether it can patch the image.
#[derive(Clone, Debug, Default)]
pub(crate) struct ImageGate {
    pub(crate) format: String,
    pub(crate) json_output: bool,
    /// Whether the stored compile reported any error diagnostic.
    pub(crate) has_errors: bool,
    /// Whether custom rules recorded which models they select.
    pub(crate) has_model_subjects: bool,
    pub(crate) models: Vec<CapturedModel>,
    pub(crate) tests: Vec<CapturedTest>,
    pub(crate) fact_digests: Vec<(Vec<String>, String)>,
    pub(crate) header_keys: BTreeMap<String, HeaderKeys>,
}

/// The rest of the image's facts as stored; shapes equal to a relation's types are stored once.
struct StoredFacts {
    usages: BTreeMap<String, String>,
    types: Shapes,
    nullability: Shapes,
    complete: StoredShapes,
    schemas: StoredShapes,
    dependency_ordered: bool,
    cyclic: bool,
    legacy_models: Vec<String>,
    families: FamiliesByTable,
    model_subjects: Option<Vec<(String, String, Option<String>)>>,
    diagnostics: Vec<CapturedDiagnostic>,
    text_models: Option<Vec<(String, bool, i64)>>,
    environment_names: Vec<String>,
    pinned_sources: Vec<String>,
    test_sources: BTreeMap<String, Vec<String>>,
    model_shapes: StoredShapes,
    coverage: Option<Vec<ModelCoverage>>,
    lint_model_keys: Option<Vec<(String, String)>>,
    lint_file_keys: Option<Vec<(String, Option<String>)>>,
    audit_references: BTreeMap<String, Vec<String>>,
}

fn section<'a>(sections: &'a ImageSections, name: &str) -> Option<&'a [u8]> {
    sections
        .iter()
        .find(|(section, _)| section == name)
        .map(|(_, bytes)| bytes.as_slice())
}

/// The diagnostics and report lines an image keeps as one JSON array, being small and varied.
struct StoredMisc {
    diagnostics: Vec<CapturedDiagnostic>,
    text_models: Option<Vec<(String, bool, i64)>>,
    environment_names: Vec<String>,
    coverage: Option<Vec<ModelCoverage>>,
}

impl<'de> serde::Deserialize<'de> for StoredMisc {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_tuple(4, StoredMiscVisitor)
    }
}

/// Reads [`StoredMisc`] from its four-element array.
struct StoredMiscVisitor;

impl<'de> serde::de::Visitor<'de> for StoredMiscVisitor {
    type Value = StoredMisc;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a tuple of size 4")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<StoredMisc, A::Error> {
        let missing = |index: usize| serde::de::Error::invalid_length(index, &self);
        Ok(StoredMisc {
            diagnostics: seq.next_element()?.ok_or_else(|| missing(0))?,
            text_models: seq.next_element()?.ok_or_else(|| missing(1))?,
            environment_names: seq.next_element()?.ok_or_else(|| missing(2))?,
            coverage: seq.next_element()?.ok_or_else(|| missing(3))?,
        })
    }
}

fn put_shapes<'a>(encoder: &mut Encoder<'a>, shapes: &'a Shapes) {
    encoder.count(shapes.len());
    for (name, shape) in shapes {
        encoder.string(name);
        encoder.pairs(shape);
    }
}

fn put_stored_shapes<'a>(encoder: &mut Encoder<'a>, shapes: &'a StoredShapes) {
    encoder.count(shapes.len());
    for (name, shape) in shapes {
        encoder.string(name);
        encoder.flag(shape.is_some());
        if let Some(shape) = shape {
            encoder.pairs(shape);
        }
    }
}

fn shapes(decoder: &mut Decoder<'_>) -> Option<Shapes> {
    decoder.list(|decoder| Some((decoder.string()?, decoder.pairs()?)))
}

fn stored_shapes(decoder: &mut Decoder<'_>) -> Option<StoredShapes> {
    decoder.list(|decoder| {
        let name: String = decoder.string()?;
        let shape: Option<Vec<(String, String)>> = if decoder.flag()? {
            Some(decoder.pairs()?)
        } else {
            None
        };
        Some((name, shape))
    })
}

fn put_families<'a>(encoder: &mut Encoder<'a>, families: &'a FamiliesByTable) {
    encoder.count(families.len());
    for (table, declared) in families {
        encoder.string(table);
        encoder.count(declared.len());
        for family in declared {
            encoder.string(&family.name);
            encoder.string(&family.pivot_column);
            encoder.string(&family.value_column);
            encoder.string(&family.aggregate);
            encoder.string(&family.data_type);
            encoder.optional_string(family.name_pattern.as_deref());
        }
    }
}

fn families(decoder: &mut Decoder<'_>) -> Option<FamiliesByTable> {
    decoder.list(|decoder| {
        let table: String = decoder.string()?;
        let declared: Vec<DynamicFamily> = decoder.list(|decoder| {
            Some(DynamicFamily {
                name: decoder.string()?,
                pivot_column: decoder.string()?,
                value_column: decoder.string()?,
                aggregate: decoder.string()?,
                data_type: decoder.string()?,
                name_pattern: decoder.optional_string()?,
            })
        })?;
        Some((table, declared))
    })
}

impl ImageGate {
    fn encode(&self) -> Vec<u8> {
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
    pub(crate) fn decode(sections: &ImageSections) -> Option<Self> {
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

impl StoredFacts {
    fn encode(&self) -> Result<Vec<u8>, String> {
        let misc: Vec<u8> = serde_json::to_vec(&(
            &self.diagnostics,
            &self.text_models,
            &self.environment_names,
            &self.coverage,
        ))
        .map_err(|error| error.to_string())?;
        let mut encoder: Encoder<'_> = Encoder::default();
        encoder.count(self.usages.len());
        for (path, digest) in &self.usages {
            encoder.string(path);
            encoder.string(digest);
        }
        put_shapes(&mut encoder, &self.types);
        put_shapes(&mut encoder, &self.nullability);
        put_stored_shapes(&mut encoder, &self.complete);
        put_stored_shapes(&mut encoder, &self.schemas);
        put_stored_shapes(&mut encoder, &self.model_shapes);
        encoder.flag(self.dependency_ordered);
        encoder.flag(self.cyclic);
        encoder.strings(self.legacy_models.iter().map(String::as_str));
        put_families(&mut encoder, &self.families);
        encoder.flag(self.model_subjects.is_some());
        if let Some(subjects) = &self.model_subjects {
            encoder.count(subjects.len());
            for (path, name, materialized) in subjects {
                encoder.string(path);
                encoder.string(name);
                encoder.optional_string(materialized.as_deref());
            }
        }
        encoder.flag(self.lint_model_keys.is_some());
        if let Some(keys) = &self.lint_model_keys {
            encoder.pairs(keys);
        }
        encoder.flag(self.lint_file_keys.is_some());
        if let Some(keys) = &self.lint_file_keys {
            encoder.count(keys.len());
            for (path, key) in keys {
                encoder.string(path);
                encoder.optional_string(key.as_deref());
            }
        }
        encoder.strings(self.pinned_sources.iter().map(String::as_str));
        encoder.count(self.test_sources.len());
        for (path, names) in &self.test_sources {
            encoder.string(path);
            encoder.strings(names.iter().map(String::as_str));
        }
        encoder.count(self.audit_references.len());
        for (path, names) in &self.audit_references {
            encoder.string(path);
            encoder.strings(names.iter().map(String::as_str));
        }
        encoder.bytes(&misc);
        Ok(encoder.finish())
    }

    fn decode(section: &[u8]) -> Option<Self> {
        let mut decoder: Decoder<'_> = Decoder::new(section)?;
        let usages: BTreeMap<String, String> = decoder
            .list(|decoder| Some((decoder.string()?, decoder.string()?)))?
            .into_iter()
            .collect();
        let types: Shapes = shapes(&mut decoder)?;
        let nullability: Shapes = shapes(&mut decoder)?;
        let complete: StoredShapes = stored_shapes(&mut decoder)?;
        let schemas: StoredShapes = stored_shapes(&mut decoder)?;
        let model_shapes: StoredShapes = stored_shapes(&mut decoder)?;
        let dependency_ordered: bool = decoder.flag()?;
        let cyclic: bool = decoder.flag()?;
        let legacy_models: Vec<String> = decoder.strings()?;
        let families: FamiliesByTable = families(&mut decoder)?;
        let model_subjects = if decoder.flag()? {
            Some(decoder.list(|decoder| {
                Some((
                    decoder.string()?,
                    decoder.string()?,
                    decoder.optional_string()?,
                ))
            })?)
        } else {
            None
        };
        let lint_model_keys = if decoder.flag()? {
            Some(decoder.pairs()?)
        } else {
            None
        };
        let lint_file_keys = if decoder.flag()? {
            Some(decoder.list(|decoder| Some((decoder.string()?, decoder.optional_string()?)))?)
        } else {
            None
        };
        let pinned_sources: Vec<String> = decoder.strings()?;
        let test_sources: BTreeMap<String, Vec<String>> = decoder
            .list(|decoder| Some((decoder.string()?, decoder.strings()?)))?
            .into_iter()
            .collect();
        let audit_references: BTreeMap<String, Vec<String>> = decoder
            .list(|decoder| Some((decoder.string()?, decoder.strings()?)))?
            .into_iter()
            .collect();
        let StoredMisc {
            diagnostics,
            text_models,
            environment_names,
            coverage,
        }: StoredMisc = from_json(decoder.bytes()?)?;
        decoder.finish()?;
        Some(Self {
            usages,
            types,
            nullability,
            complete,
            schemas,
            dependency_ordered,
            cyclic,
            legacy_models,
            families,
            model_subjects,
            diagnostics,
            text_models,
            environment_names,
            pinned_sources,
            test_sources,
            model_shapes,
            coverage,
            lint_model_keys,
            lint_file_keys,
            audit_references,
        })
    }
}

fn encoded_strings(values: &[String]) -> Vec<u8> {
    let mut encoder: Encoder<'_> = Encoder::default();
    encoder.strings(values.iter().map(String::as_str));
    encoder.finish()
}

fn decoded_strings(section: &[u8]) -> Option<Vec<String>> {
    let mut decoder: Decoder<'_> = Decoder::new(section)?;
    let values: Vec<String> = decoder.strings()?;
    decoder.finish()?;
    Some(values)
}

impl ProjectImage {
    /// The image of a finished compile's capture, or why it cannot be a delta base.
    pub(crate) fn from_capture(capture: ImageCapture, json_output: bool) -> Result<Self, String> {
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
    pub(crate) fn encode(self) -> Result<ImageSections, String> {
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
    pub(crate) fn decode(mut sections: ImageSections, gate: ImageGate) -> Option<Self> {
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
    pub(crate) fn dependents(&self, names: &BTreeSet<String>) -> BTreeSet<String> {
        self.models
            .iter()
            .filter(|model| !names.contains(&model.name))
            .filter(|model| reads_any_model(model, names))
            .map(|model| model.name.clone())
            .collect()
    }
}

/// Whether `model` reads any of the models `names`.
fn reads_any_model(model: &CapturedModel, names: &BTreeSet<String>) -> bool {
    model
        .upstream
        .iter()
        .any(|(kind, name)| kind == MODEL_KIND && names.contains(name))
}
