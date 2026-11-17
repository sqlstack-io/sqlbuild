//! The project image a full or delta compile stores beside its reuse slot.

use crate::compile_delta::models::ImageRelations;
use std::collections::{BTreeMap, BTreeSet};

use sqlbuild_analysis::assembly::analysis_session::models::{DynamicFamily, RelationTables};
use sqlbuild_analysis::assembly::analysis_session::types::Shapes;
use sqlbuild_cache::project_reuse::types::ImageSections;
use sqlbuild_rules::models::RetainedRows;

use sqlbuild_python_base::compile_context::models::BaseRules;
use sqlbuild_python_base::compile_context::models::CapturedDiagnostic;
use sqlbuild_python_base::compile_context::models::CapturedModel;
use sqlbuild_python_base::compile_context::models::Decoder;
use sqlbuild_python_base::compile_context::models::Encoder;
use sqlbuild_python_base::compile_context::models::ModelCoverage;

/// The upstream kind of a model another model reads.
const MODEL_KIND: &str = "model";

pub(crate) const RULES_SECTION: &str = "rules";

pub(crate) const RULE_ROWS_SECTION: &str = "rule-rows";

/// The value `bytes` hold as JSON, or `None` when they are malformed.
fn from_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    let Ok(value) = serde_json::from_slice(bytes) else {
        return None;
    };
    Some(value)
}

pub(crate) fn decoded_rules(
    request_json: Vec<u8>,
    rows: Vec<u8>,
    digests: &[u8],
) -> Option<BaseRules> {
    let retained: RetainedRows = RetainedRows {
        request_json,
        rows,
        digests: decoded_strings(digests)?,
    };
    BaseRules::decoded(&retained)
}

/// Each table's dynamic column families, in the order the analysis request listed them.
pub(crate) type FamiliesByTable = Vec<(String, Vec<DynamicFamily>)>;

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
pub(crate) fn differing(table: &Shapes, types: &Shapes) -> StoredShapes {
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
pub(crate) fn restored(rows: StoredShapes, types: &Shapes) -> Option<Shapes> {
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

/// The rest of the image's facts as stored; shapes equal to a relation's types are stored once.
pub(crate) struct StoredFacts {
    pub(crate) usages: BTreeMap<String, String>,
    pub(crate) types: Shapes,
    pub(crate) nullability: Shapes,
    pub(crate) complete: StoredShapes,
    pub(crate) schemas: StoredShapes,
    pub(crate) dependency_ordered: bool,
    pub(crate) cyclic: bool,
    pub(crate) legacy_models: Vec<String>,
    pub(crate) families: FamiliesByTable,
    pub(crate) model_subjects: Option<Vec<(String, String, Option<String>)>>,
    pub(crate) diagnostics: Vec<CapturedDiagnostic>,
    pub(crate) text_models: Option<Vec<(String, bool, i64)>>,
    pub(crate) environment_names: Vec<String>,
    pub(crate) pinned_sources: Vec<String>,
    pub(crate) test_sources: BTreeMap<String, Vec<String>>,
    pub(crate) model_shapes: StoredShapes,
    pub(crate) coverage: Option<Vec<ModelCoverage>>,
    pub(crate) lint_model_keys: Option<Vec<(String, String)>>,
    pub(crate) lint_file_keys: Option<Vec<(String, Option<String>)>>,
    pub(crate) audit_references: BTreeMap<String, Vec<String>>,
}

pub(crate) fn section<'a>(sections: &'a ImageSections, name: &str) -> Option<&'a [u8]> {
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

pub(crate) fn shapes(decoder: &mut Decoder<'_>) -> Option<Shapes> {
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

pub(crate) fn families(decoder: &mut Decoder<'_>) -> Option<FamiliesByTable> {
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

impl StoredFacts {
    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
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

    pub(crate) fn decode(section: &[u8]) -> Option<Self> {
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

pub(crate) fn encoded_strings(values: &[String]) -> Vec<u8> {
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

/// Whether `model` reads any of the models `names`.
pub(crate) fn reads_any_model(model: &CapturedModel, names: &BTreeSet<String>) -> bool {
    model
        .upstream
        .iter()
        .any(|(kind, name)| kind == MODEL_KIND && names.contains(name))
}
