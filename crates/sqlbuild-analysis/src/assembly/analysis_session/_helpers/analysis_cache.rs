//! Per-model analysis cache keys over everything an analysis reads, and stored outcomes.

use std::collections::{BTreeSet, HashMap};

use sha2::{Digest, Sha256};
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::errors::StoreDecodeError;
use sqlbuild_cache::store::models::NativeStore;

use crate::assembly::analysis_session::constants::{
    ANALYSIS_CACHE_FORMAT, VARINT_CONTINUATION, VARINT_MAX_BYTES, VARINT_PAYLOAD_BITS,
    VARINT_PAYLOAD_MASK,
};
use crate::assembly::analysis_session::models::{
    AnalysisCache, AnalysisCacheStats, AnalysisSession, ColumnFact, DynamicFamily, LineageFacts,
    LineageRow, ModelAnalysis, ModelOutcome, ModelRequest, SessionRequest,
};
use crate::assembly::analysis_session::types::{Pairs, Shapes};
use crate::semantic_validation::models::DiagnosticRow;

const LINEAGE_NATIVE: u64 = 0;
const LINEAGE_FACTS: u64 = 1;

/// A stored outcome and the whole-table digest it requires, if it came from a legacy analysis.
pub(crate) type CachedOutcome = (ModelOutcome, Option<ContentDigest>);

impl AnalysisSession {
    /// Read and fill `store`; its environment already names the build and Python's settings.
    pub(crate) fn attach_cache(&mut self, store: NativeStore) {
        let models: usize = self.request.models.len();
        self.cache = Some(AnalysisCache {
            store,
            session_digest: session_digest(&self.request),
            keys: vec![None; models],
            hits: vec![false; models],
            legacy_tables: vec![None; models],
            stats: AnalysisCacheStats::default(),
        });
    }

    /// The store with this session's stored outcomes, and what the cache did.
    pub(crate) fn take_cache(&mut self) -> Option<(NativeStore, AnalysisCacheStats)> {
        self.cache.take().map(|cache| (cache.store, cache.stats))
    }

    /// Whether `model`'s outcome came from the cache, so its later passes are already applied.
    pub(crate) fn cached(&self, model: usize) -> bool {
        self.cache
            .as_ref()
            .is_some_and(|cache| cache.hits.get(model).copied().unwrap_or(false))
    }

    /// The names whose relation facts `model`'s analysis may read, in sorted order.
    pub(crate) fn model_relation_names(&self, model: usize) -> BTreeSet<&str> {
        let request: &ModelRequest = &self.request.models[model];
        request
            .references
            .iter()
            .map(|reference| reference.analysis_name.as_str())
            .chain(request.required_names.iter().map(String::as_str))
            .chain(
                request
                    .lineage_references
                    .iter()
                    .map(|(name, _, _)| name.as_str()),
            )
            .collect()
    }

    /// Digest of `name`'s current types, nullability, closed shape and catalog schema.
    pub(crate) fn relation_digest(&self, name: &str) -> ContentDigest {
        let mut key: KeyHasher = KeyHasher::default();
        key.text(name);
        key.optional_pairs(self.available_types.get(name));
        key.optional_pairs(self.available_nullability.get(name));
        key.optional_pairs(self.complete_shapes.get(name));
        key.optional_pairs(self.catalog.known_schema(name));
        key.finish()
    }

    /// The key of `model` analysed with `schema`, given its relations' digests.
    pub(crate) fn model_key(
        &self,
        session_digest: &ContentDigest,
        model: usize,
        schema: &Shapes,
        relations: &HashMap<&str, ContentDigest>,
    ) -> ContentDigest {
        let mut key: KeyHasher = KeyHasher::default();
        key.bytes(session_digest);
        key.model(&self.request.models[model]);
        key.shapes(schema);
        let names: BTreeSet<&str> = self.model_relation_names(model);
        key.count(names.len());
        for name in names {
            match relations.get(name) {
                Some(digest) => key.bytes(digest),
                None => key.bytes(&self.relation_digest(name)),
            }
        }
        key.flag(self.dependency_ordered);
        key.flag(self.cyclic);
        key.finish()
    }

    /// Digest of every relation's types and nullability, which a legacy analysis reads whole.
    pub(crate) fn tables_digest(&self) -> ContentDigest {
        let mut key: KeyHasher = KeyHasher::default();
        key.bytes(&self.available_types.chain());
        key.bytes(&self.available_nullability.chain());
        key.finish()
    }
}

/// SHA-256 over length-prefixed fields, the shared store's digest scheme, without buffering.
#[derive(Default)]
struct KeyHasher(Sha256);

impl KeyHasher {
    fn bytes(&mut self, value: &[u8]) {
        self.0.update((value.len() as u64).to_le_bytes());
        self.0.update(value);
    }

    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn count(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    fn flag(&mut self, value: bool) {
        self.count(usize::from(value));
    }

    fn finish(self) -> ContentDigest {
        self.0.finalize().into()
    }

    fn model(&mut self, model: &ModelRequest) {
        self.text(&model.name);
        self.text(&model.query_sql);
        self.pairs(&model.placeholders);
        self.count(model.references.len());
        for reference in &model.references {
            self.text(&reference.analysis_name);
            self.flag(reference.model_ref);
        }
        self.count(model.lineage_references.len());
        for (name, resource_type, resource) in &model.lineage_references {
            self.text(name);
            self.text(resource_type);
            self.text(resource);
        }
        self.count(model.required_names.len());
        for name in &model.required_names {
            self.text(name);
        }
        self.flag(model.recover_cte_facts);
        self.flag(model.has_set_operation);
        match &model.snapshot_columns {
            Some((valid_from, valid_to)) => {
                self.flag(true);
                self.text(valid_from);
                self.text(valid_to);
            }
            None => self.flag(false),
        }
        self.text(&model.pivot_sql);
        self.families(&model.dynamic_families);
    }

    fn families(&mut self, families: &[DynamicFamily]) {
        self.count(families.len());
        for family in families {
            self.text(&family.name);
            self.text(&family.pivot_column);
            self.text(&family.value_column);
            self.text(&family.aggregate);
            self.text(&family.data_type);
            match family.name_pattern.as_deref() {
                Some(pattern) => {
                    self.flag(true);
                    self.text(pattern);
                }
                None => self.flag(false),
            }
        }
    }

    fn pairs(&mut self, pairs: &Pairs) {
        self.count(pairs.len());
        for (name, value) in pairs {
            self.text(name);
            self.text(value);
        }
    }

    fn optional_pairs(&mut self, pairs: Option<&Pairs>) {
        match pairs {
            Some(pairs) => {
                self.flag(true);
                self.pairs(pairs);
            }
            None => self.flag(false),
        }
    }

    fn shapes(&mut self, shapes: &Shapes) {
        self.count(shapes.len());
        for (name, shape) in shapes {
            self.text(name);
            self.pairs(shape);
        }
    }
}

/// Digest of the request facts every model's analysis may read.
fn session_digest(request: &SessionRequest) -> ContentDigest {
    let mut key: KeyHasher = KeyHasher::default();
    key.text(ANALYSIS_CACHE_FORMAT);
    key.text(&request.dialect);
    key.flag(request.case_sensitive_shapes);
    key.pairs(&request.function_return_types);
    key.optional_pairs(request.nullability_rules.as_ref());
    key.flag(request.rich_type_inference);
    for shapes in [
        &request.column_types,
        &request.column_nullability,
        &request.complete_schemas,
        &request.catalog_schemas,
    ] {
        key.shapes(shapes);
    }
    key.count(request.dynamic_families_by_table.len());
    for (table, families) in &request.dynamic_families_by_table {
        key.text(table);
        key.families(families);
    }
    key.finish()
}

/// Varint fields over a per-entry string table, so repeated names are stored once.
#[derive(Default)]
struct Encoder<'a> {
    strings: Vec<&'a str>,
    ids: HashMap<&'a str, u64>,
    body: Vec<u8>,
}

impl<'a> Encoder<'a> {
    fn uint(&mut self, value: u64) {
        let (buffer, length) = varint(value);
        self.body.extend_from_slice(&buffer[..length]);
    }

    fn flag(&mut self, value: bool) {
        self.uint(u64::from(value));
    }

    fn text(&mut self, value: &'a str) {
        let next: u64 = self.strings.len() as u64;
        let id: u64 = *self.ids.entry(value).or_insert_with(|| {
            self.strings.push(value);
            next
        });
        self.uint(id);
    }

    fn optional_text(&mut self, value: Option<&'a str>) {
        match value {
            Some(value) => {
                self.flag(true);
                self.text(value);
            }
            None => self.flag(false),
        }
    }

    fn optional_uint(&mut self, value: Option<usize>) {
        match value {
            Some(value) => {
                self.flag(true);
                self.uint(value as u64);
            }
            None => self.flag(false),
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        let mut bytes: Vec<u8> = push_varint(Vec::new(), self.strings.len() as u64);
        for value in &self.strings {
            bytes = push_varint(bytes, value.len() as u64);
            bytes.extend_from_slice(value.as_bytes());
        }
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

/// `value` as a little-endian base-128 varint: the buffer and how many bytes it uses.
fn varint(mut value: u64) -> ([u8; VARINT_MAX_BYTES], usize) {
    let mut buffer: [u8; VARINT_MAX_BYTES] = [0; VARINT_MAX_BYTES];
    let mut length: usize = 0;
    while value >= VARINT_CONTINUATION {
        buffer[length] = (value as u8 & VARINT_PAYLOAD_MASK) | VARINT_CONTINUATION as u8;
        value >>= VARINT_PAYLOAD_BITS;
        length += 1;
    }
    buffer[length] = value as u8;
    (buffer, length + 1)
}

fn push_varint(bytes: Vec<u8>, value: u64) -> Vec<u8> {
    let mut bytes: Vec<u8> = bytes;
    let (buffer, length) = varint(value);
    bytes.extend_from_slice(&buffer[..length]);
    bytes
}

/// Reads what an [`Encoder`] wrote; anything else is a decode error.
struct Decoder<'a> {
    bytes: &'a [u8],
    position: usize,
    strings: Vec<&'a str>,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, StoreDecodeError> {
        let mut decoder: Self = Self {
            bytes,
            position: 0,
            strings: Vec::new(),
        };
        let count: u64 = decoder.uint()?;
        for _ in 0..count {
            let length: usize = usize::try_from(decoder.uint()?).map_err(|_| StoreDecodeError)?;
            let end: usize = decoder
                .position
                .checked_add(length)
                .ok_or(StoreDecodeError)?;
            let raw: &'a [u8] = bytes.get(decoder.position..end).ok_or(StoreDecodeError)?;
            decoder
                .strings
                .push(std::str::from_utf8(raw).map_err(|_| StoreDecodeError)?);
            decoder.position = end;
        }
        Ok(decoder)
    }

    fn uint(&mut self) -> Result<u64, StoreDecodeError> {
        let mut value: u64 = 0;
        for shift in (0..u64::BITS).step_by(VARINT_PAYLOAD_BITS as usize) {
            let byte: u8 = *self.bytes.get(self.position).ok_or(StoreDecodeError)?;
            self.position += 1;
            value |= u64::from(byte & VARINT_PAYLOAD_MASK) << shift;
            if u64::from(byte) < VARINT_CONTINUATION {
                return Ok(value);
            }
        }
        Err(StoreDecodeError)
    }

    fn index(&mut self) -> Result<usize, StoreDecodeError> {
        usize::try_from(self.uint()?).map_err(|_| StoreDecodeError)
    }

    fn flag(&mut self) -> Result<bool, StoreDecodeError> {
        match self.uint()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(StoreDecodeError),
        }
    }

    fn text(&mut self) -> Result<String, StoreDecodeError> {
        let id: usize = self.index()?;
        self.strings
            .get(id)
            .map(|value| (*value).to_owned())
            .ok_or(StoreDecodeError)
    }

    fn optional_text(&mut self) -> Result<Option<String>, StoreDecodeError> {
        if self.flag()? {
            self.text().map(Some)
        } else {
            Ok(None)
        }
    }

    fn optional_index(&mut self) -> Result<Option<usize>, StoreDecodeError> {
        if self.flag()? {
            self.index().map(Some)
        } else {
            Ok(None)
        }
    }

    fn finish(&self) -> Result<(), StoreDecodeError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(StoreDecodeError)
        }
    }
}

/// The stored bytes of a finished native outcome, without its re-derivable cleaned SQL.
pub(crate) fn encode_outcome(
    outcome: &ModelOutcome,
    legacy_tables: Option<&ContentDigest>,
) -> Vec<u8> {
    let analysis: &ModelAnalysis = &outcome.analysis;
    let (tag, rows) = match &analysis.lineage {
        LineageFacts::Native(rows) => (LINEAGE_NATIVE, rows),
        LineageFacts::NativeFacts(rows) => (LINEAGE_FACTS, rows),
    };
    let mut encoder: Encoder<'_> = Encoder::default();
    match legacy_tables {
        Some(digest) => {
            encoder.flag(true);
            encoder.body.extend_from_slice(digest);
        }
        None => encoder.flag(false),
    }
    encoder.flag(analysis.analysis_succeeded);
    match &analysis.columns {
        Some(columns) => {
            encoder.flag(true);
            encoder.uint(columns.len() as u64);
            for column in columns {
                encoder.text(&column.name);
                encoder.optional_text(column.data_type.as_deref());
                encoder.text(&column.nullability);
            }
        }
        None => encoder.flag(false),
    }
    encoder.uint(tag);
    encoder.uint(rows.len() as u64);
    for row in rows {
        encoder.text(&row.output_column);
        encoder.uint(u64::from(row.transform_code));
        encoder.uint(u64::from(row.confidence_code));
        encoder.uint(row.sources.len() as u64);
        for (resource_type, name, column) in &row.sources {
            encoder.text(resource_type);
            encoder.text(name);
            encoder.text(column);
        }
    }
    encoder.flag(analysis.has_star);
    encoder.flag(analysis.star_resolved);
    encoder.uint(analysis.binding_diagnostics.len() as u64);
    for DiagnosticRow {
        code,
        message,
        line,
        column,
        start,
        end,
        severity,
    } in &analysis.binding_diagnostics
    {
        encoder.text(code);
        encoder.text(message);
        for position in [line, column, start, end] {
            encoder.optional_uint(*position);
        }
        encoder.text(severity);
    }
    encoder.flag(analysis.binding_validated);
    encoder.into_bytes()
}

/// The outcome [`encode_outcome`] stored, validated against `schema`, without cleaned SQL.
pub(crate) fn decode_outcome(
    bytes: &[u8],
    schema: &Shapes,
) -> Result<CachedOutcome, StoreDecodeError> {
    let mut decoder: Decoder<'_> = Decoder::new(bytes)?;
    let legacy_tables: Option<ContentDigest> = if decoder.flag()? {
        let end: usize = decoder.position + 32;
        let digest: ContentDigest = bytes
            .get(decoder.position..end)
            .ok_or(StoreDecodeError)?
            .try_into()
            .map_err(|_| StoreDecodeError)?;
        decoder.position = end;
        Some(digest)
    } else {
        None
    };
    let analysis_succeeded: bool = decoder.flag()?;
    let columns: Option<Vec<ColumnFact>> = if decoder.flag()? {
        let count: usize = decoder.index()?;
        let mut columns: Vec<ColumnFact> = Vec::with_capacity(count.min(bytes.len()));
        for _ in 0..count {
            columns.push(ColumnFact {
                name: decoder.text()?,
                data_type: decoder.optional_text()?,
                nullability: decoder.text()?,
            });
        }
        Some(columns)
    } else {
        None
    };
    let tag: u64 = decoder.uint()?;
    let row_count: usize = decoder.index()?;
    let mut rows: Vec<LineageRow> = Vec::with_capacity(row_count.min(bytes.len()));
    for _ in 0..row_count {
        let output_column: String = decoder.text()?;
        let transform_code: u8 = u8::try_from(decoder.uint()?).map_err(|_| StoreDecodeError)?;
        let confidence_code: u8 = u8::try_from(decoder.uint()?).map_err(|_| StoreDecodeError)?;
        let source_count: usize = decoder.index()?;
        let mut sources: Vec<(String, String, String)> =
            Vec::with_capacity(source_count.min(bytes.len()));
        for _ in 0..source_count {
            sources.push((decoder.text()?, decoder.text()?, decoder.text()?));
        }
        rows.push(LineageRow {
            output_column,
            transform_code,
            confidence_code,
            sources,
        });
    }
    let lineage: LineageFacts = match tag {
        LINEAGE_NATIVE => LineageFacts::Native(rows),
        LINEAGE_FACTS => LineageFacts::NativeFacts(rows),
        _ => return Err(StoreDecodeError),
    };
    let has_star: bool = decoder.flag()?;
    let star_resolved: bool = decoder.flag()?;
    let diagnostic_count: usize = decoder.index()?;
    let mut binding_diagnostics: Vec<DiagnosticRow> = Vec::new();
    for _ in 0..diagnostic_count {
        binding_diagnostics.push(DiagnosticRow {
            code: decoder.text()?,
            message: decoder.text()?,
            line: decoder.optional_index()?,
            column: decoder.optional_index()?,
            start: decoder.optional_index()?,
            end: decoder.optional_index()?,
            severity: decoder.text()?,
        });
    }
    let binding_validated: bool = decoder.flag()?;
    decoder.finish()?;
    let outcome = ModelOutcome {
        sql_macro_findings: Vec::new(),
        analysis: ModelAnalysis {
            analysis_succeeded,
            columns,
            lineage,
            has_star,
            star_resolved,
            binding_diagnostics,
            binding_validated,
        },
        cleaned_sql: String::new(),
        deferred_cleaning: None,
        validated_schema: schema.clone(),
        fused_binding_validated: true,
    };
    Ok((outcome, legacy_tables))
}
