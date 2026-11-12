//! One compact analysis batch built, shared, run and projected exactly as Python's batch.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

use rayon::ThreadPool;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use regex::{Captures, Regex};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::assembly::analysis_session::_helpers::catalog_state::SessionCatalog;
use crate::assembly::analysis_session::_helpers::mappings::{ShapeTable, native_dialect};
use crate::assembly::analysis_session::constants::{
    ASCII_SPACES, BATCH_CHUNK_MEMBERS, BINDING_SEVERITIES, COMPACT_WORKERS, CONFIDENCE_CODES,
    DEFAULT_BINDING_MESSAGE, FACT_LENGTH, INVALID_NATIVE_RESPONSE, LEGACY_FALLBACK,
    LEGACY_RESPONSE_LENGTH, MIN_SHARED_MEMBERS, NULLABILITY_BY_CODE, QUALIFIED_SCAN_TOKENS,
    REFERENCE_MARKER_PATTERN, RELATION_STUB_PREFIX, RESPONSE_LENGTH, SOURCE_LENGTH,
    TRANSFORM_CODES, UNSTUBBED_REFERENCE_KIND,
};
use crate::assembly::analysis_session::models::{ColumnFact, LineageRow};
use crate::assembly::analysis_session::types::{Pairs, Shapes};
use crate::semantic_validation::main::diagnostics::binding_diagnostics;
use crate::semantic_validation::models::DiagnosticRow;
use crate::semantic_validation::types::{NormalizationRequest, Relations};

const INVALID_BATCH: &str = "native compact query analysis returned an invalid batch response";
const INVALID_TEMPLATE: &str = "native compact query analysis returned an invalid template";
const INVALID_COLUMN: &str = "native compact analysis returned invalid column facts";
static REFERENCE_MARKER: LazyLock<Result<Regex, String>> =
    LazyLock::new(|| Regex::new(REFERENCE_MARKER_PATTERN).map_err(|error| error.to_string()));

/// One query of a batch with the facts Python's preparation reads.
#[derive(Clone)]
pub(crate) struct BatchMember<'a> {
    pub(crate) query_sql: &'a str,
    pub(crate) placeholders: &'a Pairs,
    /// Every reference's analysis name, in reference order.
    pub(crate) analysis_names: Vec<&'a str>,
    pub(crate) lineage_references: &'a [(String, String, String)],
    pub(crate) recover_cte_facts: bool,
    pub(crate) binding_schema: Option<&'a Shapes>,
}

/// One batch with the inference profile settings and relation facts its analysis reads.
pub(crate) struct Batch<'a> {
    pub(crate) dialect: &'a str,
    pub(crate) function_return_types: &'a Pairs,
    pub(crate) rich_type_inference: bool,
    pub(crate) members: Vec<BatchMember<'a>>,
    pub(crate) types: &'a ShapeTable,
    pub(crate) nullability: &'a ShapeTable,
}

/// What the native engine returned for one member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MemberAnalysis {
    Projected {
        columns: Vec<ColumnFact>,
        lineage: Vec<LineageRow>,
        has_star: bool,
        star_resolved: bool,
    },
    /// Analysis failed; Python records an unsuccessful analysis.
    Failed,
    /// The engine asked for Python's legacy analysis, keeping lineage rows when it had them.
    Legacy { lineage: Option<Vec<LineageRow>> },
}

/// One member's projected native analysis.
#[derive(Debug, Clone)]
pub(crate) struct MemberResult {
    pub(crate) cleaned_sql: String,
    pub(crate) analysis: MemberAnalysis,
    pub(crate) binding_diagnostics: Option<Vec<DiagnosticRow>>,
    /// The failure Python logs at debug level.
    pub(crate) failure: Option<String>,
    /// The lineage before relation stubs were named, kept for a result later batches reuse.
    canonical: Option<CanonicalLineage>,
}

/// A shared result's lineage over relation stubs, and the stubs the engine named.
#[derive(Debug, Clone)]
struct CanonicalLineage {
    stubs: Vec<String>,
    lineage: Vec<LineageRow>,
}

/// The digest a remembered shared result is found by.
pub(crate) type MemoKey = [u8; 32];

/// Python's `BindingCatalog.shared_analyses` entry: an exact shared result later batches reuse.
#[derive(Debug, Clone)]
pub(crate) struct SharedResult {
    columns: Vec<ColumnFact>,
    canonical: CanonicalLineage,
    has_star: bool,
    star_resolved: bool,
    binding_diagnostics: Option<Vec<DiagnosticRow>>,
}

/// The batch's response tables Python's projection reads.
struct BatchResponse<'a> {
    strings: Vec<&'a str>,
    facts: &'a [Value],
    templates: &'a [Value],
}

/// The prepared request, each member's query index and the queries members share.
struct BatchPayload {
    request: Value,
    query_indexes: Vec<usize>,
    shared_queries: HashSet<usize>,
}

/// Python's `SharedBindingQuery`: one member's relation-stubbed query and its identity.
struct SharedQuery {
    sql: String,
    /// Each lineage reference's stub, in reference order.
    stubs: Vec<(String, String)>,
    key: String,
}

impl SharedQuery {
    fn stub<'a>(&'a self, name: &'a str) -> &'a str {
        stub_name(&self.stubs, name)
    }
}

/// One chunk of a batch: the member indexes it analyses, with the whole batch's inputs.
#[derive(Clone, Copy)]
struct ChunkInputs<'b, 'm> {
    batch: &'b Batch<'m>,
    members: &'b [usize],
    cleaned: &'b [String],
    keys: &'b [Option<MemoKey>],
}

/// One run's projected members and the shared members whose result Python recomputes.
struct BatchRun {
    results: Vec<MemberResult>,
    inexact: Vec<usize>,
}

impl SessionCatalog {
    /// Normalize, prepare, analyse and project a batch; an error is the one Python raises.
    pub(crate) fn analyze_batch(&mut self, batch: &Batch<'_>) -> Result<Vec<MemberResult>, String> {
        if batch.members.is_empty() {
            return Ok(Vec::new());
        }
        let mut required: Vec<String> = Vec::new();
        let mut seen: HashSet<&str> = HashSet::new();
        for name in batch
            .members
            .iter()
            .flat_map(|member| member.analysis_names.iter())
        {
            if seen.insert(name) {
                required.push((*name).to_owned());
            }
        }
        self.prepare_analysis(&required, batch.types, batch.nullability);
        let cleaned: Vec<String> = self.normalize_members(batch)?;
        let mut shared: Vec<Option<SharedQuery>> = self.shared_queries(batch, &cleaned)?;
        self.shared_members += shared.iter().flatten().count();
        let mut query_digests: HashMap<&str, MemoKey> = HashMap::new();
        let batch_digest: MemoKey = batch_key_digest(batch);
        let keys: Vec<Option<MemoKey>> = batch
            .members
            .iter()
            .zip(&shared)
            .map(|(member, share)| {
                let share: &SharedQuery = share.as_ref()?;
                let query: MemoKey = *query_digests
                    .entry(share.key.as_str())
                    .or_insert_with(|| Sha256::digest(share.key.as_bytes()).into());
                Some(shared_result_key(
                    batch,
                    member,
                    share,
                    (&query, &batch_digest),
                ))
            })
            .collect();
        for (member, sql) in batch.members.iter().zip(&cleaned) {
            if let Some(schema) = member.binding_schema {
                let _ = self.prepare(&[(sql.as_str(), schema)]);
            }
        }
        let mut results: Vec<Option<MemberResult>> = batch.members.iter().map(|_| None).collect();
        for chunk in batch_chunks(&shared) {
            let chunk_shared: Vec<Option<SharedQuery>> =
                chunk.iter().map(|index| shared[*index].take()).collect();
            let inputs = ChunkInputs {
                batch,
                members: &chunk,
                cleaned: &cleaned,
                keys: &keys,
            };
            let chunk_results: Vec<MemberResult> = self.analyze_chunk(&inputs, chunk_shared)?;
            for (index, result) in chunk.iter().zip(chunk_results) {
                results[*index] = Some(result);
            }
        }
        let results: Vec<MemberResult> = results
            .into_iter()
            .collect::<Option<_>>()
            .ok_or(INVALID_BATCH)?;
        self.with_separate_validation(&batch.members, results)
    }

    /// Reuse remembered shared results for `chunk`, analyse the rest and remember new ones.
    fn analyze_chunk(
        &mut self,
        inputs: &ChunkInputs<'_, '_>,
        shared: Vec<Option<SharedQuery>>,
    ) -> Result<Vec<MemberResult>, String> {
        let ChunkInputs {
            batch,
            members: chunk,
            cleaned,
            keys,
        } = *inputs;
        let mut results: Vec<Option<MemberResult>> = chunk.iter().map(|_| None).collect();
        let mut analyzed: Vec<usize> = Vec::with_capacity(chunk.len());
        let mut analyzed_shared: Vec<Option<SharedQuery>> = Vec::with_capacity(chunk.len());
        for (position, (index, share)) in chunk.iter().zip(shared).enumerate() {
            let reused: Option<MemberResult> = keys[*index]
                .as_ref()
                .and_then(|key| self.shared_results.get(key))
                .zip(share.as_ref())
                .and_then(|(remembered, share)| {
                    remembered.reused(&batch.members[*index], share, &cleaned[*index])
                });
            match reused {
                Some(result) => results[position] = Some(result),
                None => {
                    analyzed.push(position);
                    analyzed_shared.push(share);
                }
            }
        }
        if !analyzed.is_empty() {
            let analyzed_results: Vec<MemberResult> =
                self.analyze_unremembered(inputs, &analyzed, &analyzed_shared)?;
            for (position, result) in analyzed.iter().zip(analyzed_results) {
                results[*position] = Some(result);
            }
        }
        results
            .into_iter()
            .collect::<Option<_>>()
            .ok_or_else(|| INVALID_BATCH.to_owned())
    }

    /// Analyse the chunk members at `positions`, then remember their exact shared results.
    fn analyze_unremembered(
        &mut self,
        inputs: &ChunkInputs<'_, '_>,
        positions: &[usize],
        shared: &[Option<SharedQuery>],
    ) -> Result<Vec<MemberResult>, String> {
        let ChunkInputs {
            batch,
            members: chunk,
            cleaned,
            keys,
        } = *inputs;
        let analyzed: Vec<usize> = positions.iter().map(|position| chunk[*position]).collect();
        let subset = Batch {
            members: analyzed
                .iter()
                .map(|index| batch.members[*index].clone())
                .collect(),
            ..*batch
        };
        let subset_cleaned: Vec<String> = analyzed
            .iter()
            .map(|index| cleaned[*index].clone())
            .collect();
        let mut first: HashSet<MemoKey> = HashSet::new();
        let remember: Vec<bool> = analyzed
            .iter()
            .map(|index| {
                keys[*index]
                    .is_some_and(|key| !self.shared_results.contains_key(&key) && first.insert(key))
            })
            .collect();
        let BatchRun {
            results: mut analyzed_results,
            inexact,
        } = self.run_batch(&subset, subset_cleaned, shared, &remember)?;
        self.reanalysed_members += inexact.len();
        if !inexact.is_empty() {
            let rerun_subset = Batch {
                members: inexact
                    .iter()
                    .map(|index| subset.members[*index].clone())
                    .collect(),
                ..*batch
            };
            let rerun_cleaned: Vec<String> = inexact
                .iter()
                .map(|index| analyzed_results[*index].cleaned_sql.clone())
                .collect();
            let unshared: Vec<Option<SharedQuery>> = inexact.iter().map(|_| None).collect();
            let kept: Vec<bool> = inexact.iter().map(|_| false).collect();
            let rerun: BatchRun = self.run_batch(&rerun_subset, rerun_cleaned, &unshared, &kept)?;
            for (index, result) in inexact.iter().zip(rerun.results) {
                analyzed_results[*index] = result;
            }
        }
        for (position, (index, result)) in analyzed.iter().zip(&analyzed_results).enumerate() {
            if let Some(key) = keys[*index].as_ref()
                && !inexact.contains(&position)
                && let Some(remembered) = SharedResult::remembered(result)
            {
                self.shared_results.entry(*key).or_insert(remembered);
            }
        }
        Ok(analyzed_results)
    }

    /// Python's `_attach_compiled_bindings`: decode the batch's fused binding validations.
    fn attach_validations(
        &self,
        response: &Value,
        query_indexes: &[usize],
        dialect: &str,
        mut results: Vec<MemberResult>,
    ) -> Result<Vec<MemberResult>, String> {
        let pool: Arc<ThreadPool> = self.native.analysis_pool()?;
        let Some(validations) = response.get("validations").filter(|value| !value.is_null()) else {
            return Ok(results);
        };
        let query_count: usize = query_indexes.iter().max().map_or(0, |index| index + 1);
        let validations: &Vec<Value> = validations
            .as_array()
            .filter(|validations| validations.len() == query_count)
            .ok_or("native compilation returned an invalid binding batch")?;
        let decoded: Vec<Option<Result<Vec<DiagnosticRow>, String>>> = pool.install(|| {
            results
                .par_iter()
                .zip(query_indexes)
                .map(|(result, query_index)| {
                    let validation: &Value = &validations[*query_index];
                    (!validation.is_null())
                        .then(|| decode_validation(&result.cleaned_sql, dialect, validation))
                })
                .collect()
        });
        for (result, diagnostics) in results.iter_mut().zip(decoded) {
            if let Some(diagnostics) = diagnostics {
                result.binding_diagnostics = Some(diagnostics?);
            }
        }
        Ok(results)
    }

    /// Python's `_analyze_compact_inputs` run: prepare, analyse, project and find inexact sharing.
    fn run_batch(
        &mut self,
        batch: &Batch<'_>,
        cleaned: Vec<String>,
        shared: &[Option<SharedQuery>],
        remember: &[bool],
    ) -> Result<BatchRun, String> {
        let payload: BatchPayload = self.batch_payload(batch, &cleaned, shared)?;
        let job = self.native.prepare_compact(&payload.request.to_string())?;
        let analysis = job
            .take_analysis()
            .ok_or("compact analysis job already ran")?;
        let response: Value =
            serde_json::from_str(&job.run(analysis)?).map_err(|error| error.to_string())?;
        let inexact: Vec<usize> = inexact_shared_members(&response, &payload);
        let results: Vec<MemberResult> = self.attach_validations(
            &response,
            &payload.query_indexes,
            batch.dialect,
            project_response(&response, cleaned, remember)?,
        )?;
        Ok(BatchRun { results, inexact })
    }

    /// Python's `prepare_binding_queries` shared forms, for keys at least two members match.
    fn shared_queries(
        &self,
        batch: &Batch<'_>,
        cleaned: &[String],
    ) -> Result<Vec<Option<SharedQuery>>, String> {
        let pool = self.native.analysis_pool()?;
        let prekeys: Vec<Option<String>> = pool.install(|| {
            batch
                .members
                .par_iter()
                .map(member_prekey)
                .collect::<Result<_, _>>()
        })?;
        let mut prekey_counts: HashMap<&str, usize> = HashMap::new();
        for prekey in prekeys.iter().flatten() {
            *prekey_counts.entry(prekey.as_str()).or_default() += 1;
        }
        let candidates: Vec<(usize, Vec<(String, String)>)> = pool.install(|| {
            batch
                .members
                .par_iter()
                .zip(&prekeys)
                .enumerate()
                .filter_map(|(index, (member, prekey))| {
                    let (Some(schema), Some(prekey)) = (member.binding_schema, prekey) else {
                        return None;
                    };
                    if prekey_counts.get(prekey.as_str()).copied().unwrap_or(0) < MIN_SHARED_MEMBERS
                    {
                        return None;
                    }
                    let names: Vec<String> =
                        self.shared_reference_names(&cleaned[index], member, schema)?;
                    Some((index, relation_stubs(names)))
                })
                .collect()
        });
        let requests: Vec<NormalizationRequest> = candidates
            .iter()
            .map(|(index, stubs)| {
                let member: &BatchMember<'_> = &batch.members[*index];
                (
                    member.query_sql.to_owned(),
                    stubs.iter().cloned().collect(),
                    member.placeholders.iter().cloned().collect(),
                )
            })
            .collect();
        let stubbed: Vec<Result<String, String>> = if requests.is_empty() {
            Vec::new()
        } else {
            self.native
                .normalize_analysis_sqls(batch.dialect, requests)?
        };
        let mut shared: Vec<Option<SharedQuery>> = batch.members.iter().map(|_| None).collect();
        let keyed: Vec<(usize, SharedQuery)> = pool.install(|| {
            candidates
                .into_par_iter()
                .zip(stubbed)
                .filter_map(|((index, stubs), sql)| {
                    let (Ok(sql), Some(schema)) = (sql, batch.members[index].binding_schema) else {
                        return None;
                    };
                    let key: String = self.shared_key(
                        batch.dialect,
                        &sql,
                        &stubs,
                        (schema, batch.members[index].recover_cte_facts),
                    );
                    Some((index, SharedQuery { sql, stubs, key }))
                })
                .collect()
        });
        for (index, query) in keyed {
            shared[index] = Some(query);
        }
        let mut key_counts: HashMap<String, usize> = HashMap::new();
        for query in shared.iter().flatten() {
            *key_counts.entry(query.key.clone()).or_default() += 1;
        }
        Ok(shared
            .into_iter()
            .map(|query| with_partner(query, &key_counts))
            .collect())
    }

    /// Python's `_shared_reference_names`, None where stubbing could change the analysis.
    fn shared_reference_names(
        &self,
        cleaned_sql: &str,
        member: &BatchMember<'_>,
        schema: &Shapes,
    ) -> Option<Vec<String>> {
        let names: Vec<String> = member
            .lineage_references
            .iter()
            .map(|(name, _, _)| name.clone())
            .collect();
        let folded: HashSet<String> = names.iter().map(|name| name.to_lowercase()).collect();
        let plain_text: bool = cleaned_sql.is_ascii() && names.iter().all(|name| name.is_ascii());
        let quoted: bool = cleaned_sql.contains('"') || schema_quoted(schema);
        let collides: bool = names
            .iter()
            .any(|name| name.starts_with(RELATION_STUB_PREFIX))
            || cleaned_sql
                .to_ascii_lowercase()
                .contains(RELATION_STUB_PREFIX);
        let unshaped: bool = names
            .iter()
            .any(|name| self.session_analysis_shape(name).is_none());
        let usable: bool = !names.is_empty()
            && folded.len() == names.len()
            && plain_text
            && !quoted
            && !collides
            && !unshaped
            && !names_qualify(cleaned_sql, &names);
        usable.then_some(names)
    }

    /// Python's `SharedBindingQuery.key`, with the CTE fact flag its query key adds.
    fn shared_key(
        &self,
        dialect: &str,
        sql: &str,
        stubs: &[(String, String)],
        (schema, recover_cte_facts): (&Shapes, bool),
    ) -> String {
        let analysis_shapes: Vec<Value> = stubs
            .iter()
            .map(|(name, stub)| {
                let (types, nullability) = self
                    .session_analysis_shape(name)
                    .cloned()
                    .unwrap_or_default();
                json!([stub, types, nullability])
            })
            .collect();
        let mut binding_shapes: Vec<String> = schema
            .iter()
            .map(|(relation, columns)| {
                let alias: &str = stub_name(stubs, relation);
                json!([
                    alias,
                    !columns.is_empty(),
                    self.effective_binding_shape(relation, columns)
                ])
                .to_string()
            })
            .collect();
        binding_shapes.sort_unstable();
        json!([
            sql,
            dialect,
            analysis_shapes,
            binding_shapes,
            recover_cte_facts
        ])
        .to_string()
    }

    fn normalize_members(&self, batch: &Batch<'_>) -> Result<Vec<String>, String> {
        self.normalize_each(batch.dialect, &batch.members)?
            .into_iter()
            .collect()
    }

    /// Each member's cleaned analysis SQL, or why it cannot be cleaned.
    pub(crate) fn normalize_each(
        &self,
        dialect: &str,
        members: &[BatchMember<'_>],
    ) -> Result<Vec<Result<String, String>>, String> {
        let requests: Vec<NormalizationRequest> = members
            .iter()
            .map(|member| {
                (
                    member.query_sql.to_owned(),
                    HashMap::new(),
                    member.placeholders.iter().cloned().collect(),
                )
            })
            .collect();
        self.native.normalize_analysis_sqls(dialect, requests)
    }

    /// Python's `_prepare_compact_analysis_batch` request and each member's query index.
    fn batch_payload(
        &mut self,
        batch: &Batch<'_>,
        cleaned: &[String],
        shared: &[Option<SharedQuery>],
    ) -> Result<BatchPayload, String> {
        let function_return_types: Map<String, Value> = batch
            .function_return_types
            .iter()
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect();
        let mut queries: Vec<Value> = Vec::new();
        let mut query_keys: HashMap<String, usize> = HashMap::new();
        let mut shared_queries: HashSet<usize> = HashSet::new();
        let mut templates: Vec<Value> = Vec::new();
        let mut template_keys: HashMap<String, usize> = HashMap::new();
        let mut projections: Vec<Value> = Vec::with_capacity(batch.members.len());
        let mut query_indexes: Vec<usize> = Vec::with_capacity(batch.members.len());
        for ((member, sql), share) in batch.members.iter().zip(cleaned).zip(shared) {
            let query_index: usize = match share {
                Some(share) => {
                    let binding = self.member_binding(member, sql)?;
                    let index: usize = match query_keys.get(&share.key) {
                        Some(index) => *index,
                        None => {
                            let query: Value =
                                self.shared_member_query(batch.dialect, member, share, binding);
                            queries.push(query);
                            query_keys.insert(share.key.clone(), queries.len() - 1);
                            queries.len() - 1
                        }
                    };
                    shared_queries.insert(index);
                    index
                }
                None => {
                    let query: Value = self.member_query(batch.dialect, member, sql)?;
                    *query_keys.entry(query.to_string()).or_insert_with(|| {
                        queries.push(query);
                        queries.len() - 1
                    })
                }
            };
            query_indexes.push(query_index);
            let references: Map<String, Value> = lineage_resources(member, share.as_ref());
            let declared: Value = json!(declared_column_order(member, batch.nullability));
            let member_template_identity: String =
                json!([query_index, references, declared, member.recover_cte_facts]).to_string();
            let template_index: usize = *template_keys
                .entry(member_template_identity)
                .or_insert_with(|| {
                    templates.push(json!({
                        "queryIndex": query_index,
                        "references": references,
                        "functionReturnTypes": function_return_types,
                        "declaredColumnOrder": declared,
                        "recoverCteFacts": member.recover_cte_facts,
                        "richTypeInference": batch.rich_type_inference,
                    }));
                    templates.len() - 1
                });
            let resource_names: Map<String, Value> = member
                .lineage_references
                .iter()
                .map(|(name, _, resource)| {
                    let canonical: &str = share
                        .as_ref()
                        .map_or(name.as_str(), |share| share.stub(name));
                    (canonical.to_owned(), Value::String(resource.clone()))
                })
                .collect();
            projections
                .push(json!({"templateIndex": template_index, "resourceNames": resource_names}));
        }
        Ok(BatchPayload {
            request: json!({
                "queries": queries,
                "templates": templates,
                "projections": projections,
                "workers": COMPACT_WORKERS,
            }),
            query_indexes,
            shared_queries,
        })
    }

    /// A bound member's sorted binding references and overrides, preparing its relations.
    fn member_binding(
        &mut self,
        member: &BatchMember<'_>,
        sql: &str,
    ) -> Result<(Vec<(String, bool)>, Relations), String> {
        let schema: &Shapes = member
            .binding_schema
            .ok_or("a shared member has no binding schema")?;
        let (_, references, overrides) = self
            .prepare(&[(sql, schema)])
            .pop()
            .ok_or("binding preparation returned no request")?;
        Ok((references, overrides))
    }

    /// Python's shared query: stubbed SQL with references, aliases and binding sorted by stub.
    fn shared_member_query(
        &mut self,
        dialect: &str,
        member: &BatchMember<'_>,
        share: &SharedQuery,
        (references, overrides): (Vec<(String, bool)>, Relations),
    ) -> Value {
        let mut lineage_names: Vec<&str> = member
            .lineage_references
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect();
        lineage_names.sort_by_key(|name| share.stub(name));
        let analysis_references: Vec<[&str; 2]> = lineage_names
            .iter()
            .map(|name| [*name, share.stub(name)])
            .collect();
        let mut references: Vec<(String, bool)> = references;
        references.sort_by(|(left, _), (right, _)| share.stub(left).cmp(share.stub(right)));
        let aliases: Map<String, Value> = references
            .iter()
            .filter(|(name, _)| share.stub(name) != name.as_str())
            .map(|(name, _)| (name.clone(), Value::String(share.stub(name).to_owned())))
            .collect();
        let mut query: Value = json!({
            "sql": share.sql,
            "dialect": dialect,
            "analysis_references": analysis_references,
            "binding_references": references,
            "binding_aliases": aliases,
        });
        if !overrides.is_empty() {
            query["binding_override"] = json!(self.register_override(overrides));
        }
        query
    }

    /// One member's query: analysis references and, when bound, its catalog binding.
    fn member_query(
        &mut self,
        dialect: &str,
        member: &BatchMember<'_>,
        sql: &str,
    ) -> Result<Value, String> {
        let mut lineage_names: Vec<&str> = member
            .lineage_references
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect();
        lineage_names.sort_unstable();
        let analysis_references: Vec<[&str; 2]> =
            lineage_names.iter().map(|name| [*name, *name]).collect();
        let mut query: Value = json!({
            "sql": sql,
            "dialect": dialect,
            "analysis_references": analysis_references,
        });
        match member.binding_schema {
            Some(schema) => {
                let (_, references, overrides) = self
                    .prepare(&[(sql, schema)])
                    .pop()
                    .ok_or("binding preparation returned no request")?;
                query["binding_references"] = json!(references);
                if !overrides.is_empty() {
                    query["binding_override"] = json!(self.register_override(overrides));
                }
            }
            None if !member.lineage_references.is_empty() => {
                return Err("unbound members with relations are not supported".to_owned());
            }
            None => {}
        }
        Ok(query)
    }

    /// Python's separate schema validation for bound members the batch did not validate.
    fn with_separate_validation(
        &mut self,
        members: &[BatchMember<'_>],
        mut results: Vec<MemberResult>,
    ) -> Result<Vec<MemberResult>, String> {
        let mut indexes: Vec<usize> = Vec::new();
        let mut requests: Vec<(&str, &Shapes)> = Vec::new();
        for (index, (member, result)) in members.iter().zip(&results).enumerate() {
            if let (Some(schema), None) = (member.binding_schema, &result.binding_diagnostics) {
                indexes.push(index);
                requests.push((result.cleaned_sql.as_str(), schema));
            }
        }
        if indexes.is_empty() {
            return Ok(results);
        }
        let prepared: Vec<_> = self.prepare(&requests);
        let rows: Vec<Vec<DiagnosticRow>> = self.native.binding_results(prepared)?;
        if rows.len() != indexes.len() {
            return Err(
                "native SQL schema validation returned an invalid batch response".to_owned(),
            );
        }
        for (index, diagnostics) in indexes.into_iter().zip(rows) {
            results[index].binding_diagnostics = Some(diagnostics);
        }
        Ok(results)
    }
}

/// Bounded member chunks in batch order, each shared query's members in its first chunk.
fn batch_chunks(shared: &[Option<SharedQuery>]) -> Vec<Vec<usize>> {
    let mut chunks: Vec<Vec<usize>> = Vec::new();
    let mut chunk_by_key: HashMap<&str, usize> = HashMap::new();
    for (index, query) in shared.iter().enumerate() {
        let grouped: Option<usize> = query
            .as_ref()
            .and_then(|query| chunk_by_key.get(query.key.as_str()).copied());
        let chunk: usize = match grouped {
            Some(chunk) => chunk,
            None => {
                if chunks
                    .last()
                    .is_none_or(|chunk| chunk.len() >= BATCH_CHUNK_MEMBERS)
                {
                    chunks.push(Vec::new());
                }
                let chunk: usize = chunks.len() - 1;
                if let Some(query) = query {
                    chunk_by_key.insert(query.key.as_str(), chunk);
                }
                chunk
            }
        };
        chunks[chunk].push(index);
    }
    chunks
}

impl SharedResult {
    /// Python's `remembered_shared_results` entry; none with member-located diagnostics.
    fn remembered(result: &MemberResult) -> Option<Self> {
        if result
            .binding_diagnostics
            .as_ref()
            .is_some_and(|diagnostics| !diagnostics.is_empty())
        {
            return None;
        }
        let MemberAnalysis::Projected {
            columns,
            has_star,
            star_resolved,
            ..
        } = &result.analysis
        else {
            return None;
        };
        Some(Self {
            columns: columns.clone(),
            canonical: result.canonical.clone()?,
            has_star: *has_star,
            star_resolved: *star_resolved,
            binding_diagnostics: result.binding_diagnostics.clone(),
        })
    }

    /// Python's `reused_shared_results` projection onto `member`, or None for a foreign stub.
    fn reused(
        &self,
        member: &BatchMember<'_>,
        share: &SharedQuery,
        cleaned_sql: &str,
    ) -> Option<MemberResult> {
        let by_stub: HashMap<&str, &str> = member
            .lineage_references
            .iter()
            .map(|(name, _, resource)| (share.stub(name), resource.as_str()))
            .collect();
        let mut names: HashMap<&str, &str> = HashMap::new();
        for stub in &self.canonical.stubs {
            names.insert(stub.as_str(), by_stub.get(stub.as_str())?);
        }
        let lineage: Vec<LineageRow> = self
            .canonical
            .lineage
            .iter()
            .map(|row| LineageRow {
                output_column: row.output_column.clone(),
                transform_code: row.transform_code,
                confidence_code: row.confidence_code,
                sources: named_sources(&row.sources, &names),
                expression_range: None,
            })
            .collect();
        Some(MemberResult {
            cleaned_sql: cleaned_sql.to_owned(),
            analysis: MemberAnalysis::Projected {
                columns: self.columns.clone(),
                lineage,
                has_star: self.has_star,
                star_resolved: self.star_resolved,
            },
            binding_diagnostics: self.binding_diagnostics.clone(),
            failure: None,
            canonical: None,
        })
    }
}

/// Each relation name with its stub, numbered by position.
fn relation_stubs(names: Vec<String>) -> Vec<(String, String)> {
    names
        .into_iter()
        .enumerate()
        .map(|(position, name)| (name, format!("{RELATION_STUB_PREFIX}{position}")))
        .collect()
}

/// `sources` with each stubbed relation renamed to the resource `names` maps it to.
fn named_sources(
    sources: &[(String, String, String)],
    names: &HashMap<&str, &str>,
) -> Vec<(String, String, String)> {
    sources
        .iter()
        .map(|(resource_type, name, column)| {
            let resource: String = names
                .get(name.as_str())
                .map_or_else(|| name.clone(), |resource| (*resource).to_owned());
            (resource_type.clone(), resource, column.clone())
        })
        .collect()
}

/// Digest of the settings every member of `batch` shares, hashed once per batch.
fn batch_key_digest(batch: &Batch<'_>) -> MemoKey {
    let shared: String =
        json!([batch.rich_type_inference, batch.function_return_types]).to_string();
    Sha256::digest(shared.as_bytes()).into()
}

/// Digest of Python's `shared_result_keys` entry, given its shared query's and batch's digests.
fn shared_result_key(
    batch: &Batch<'_>,
    member: &BatchMember<'_>,
    share: &SharedQuery,
    (query, batch_digest): (&MemoKey, &MemoKey),
) -> MemoKey {
    let reference_types: Vec<(&str, &str)> = member
        .lineage_references
        .iter()
        .map(|(name, resource_type, _)| (share.stub(name), resource_type.as_str()))
        .collect();
    let rest: String = json!([
        member.recover_cte_facts,
        declared_column_order(member, batch.nullability),
        reference_types,
    ])
    .to_string();
    let mut hasher: Sha256 = Sha256::new();
    hasher.update(query);
    hasher.update(batch_digest);
    hasher.update(rest.as_bytes());
    hasher.finalize().into()
}

fn lineage_resources(member: &BatchMember<'_>, share: Option<&SharedQuery>) -> Map<String, Value> {
    member
        .lineage_references
        .iter()
        .map(|(name, resource_type, _)| {
            let canonical: &str = share.map_or(name.as_str(), |share| share.stub(name));
            (
                canonical.to_owned(),
                json!({"resourceType": resource_type, "resourceName": canonical}),
            )
        })
        .collect()
}

/// Python's `binding_share_prekey`: authored SQL with lineage reference markers as stubs.
fn share_prekey(member: &BatchMember<'_>) -> Result<String, String> {
    let marker: &Regex = REFERENCE_MARKER.as_ref().map_err(Clone::clone)?;
    let positions: HashMap<&str, usize> = member
        .lineage_references
        .iter()
        .enumerate()
        .map(|(position, (name, _, _))| (name.as_str(), position))
        .collect();
    Ok(marker
        .replace_all(member.query_sql, |captures: &Captures<'_>| {
            let whole: &str = captures.get(0).map_or("", |found| found.as_str());
            let kind: &str = captures.get(1).map_or("", |found| found.as_str());
            match captures
                .get(2)
                .and_then(|found| positions.get(found.as_str()))
            {
                Some(position) if kind != UNSTUBBED_REFERENCE_KIND => {
                    format!("{RELATION_STUB_PREFIX}{position}")
                }
                _ => whole.to_owned(),
            }
        })
        .into_owned())
}

/// Python's `_qualified_reference_names` is non-empty; text its scanner reads counts as qualified.
fn names_qualify(sql: &str, names: &[String]) -> bool {
    if !sql.contains('.') {
        return false;
    }
    if !sql.is_ascii()
        || QUALIFIED_SCAN_TOKENS
            .iter()
            .any(|token| sql.contains(token))
    {
        return true;
    }
    let folded: String = sql.to_ascii_lowercase();
    names.iter().any(|name| {
        let name: String = name.to_lowercase();
        plain_identifier(&name) && is_qualifier(&folded, &name)
    })
}

/// Python's `_PLAIN_IDENTIFIER_PATTERN`.
fn plain_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == '_' || first == '$')
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '$'
        })
}

/// Python's `_is_qualifier`: `name` as a whole identifier followed by `.` after any spaces.
fn is_qualifier(folded_sql: &str, name: &str) -> bool {
    let identifier = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
    let bytes: &[u8] = folded_sql.as_bytes();
    folded_sql.match_indices(name).any(|(start, _)| {
        let mut end: usize = start + name.len();
        let bounded: bool = (start == 0 || !identifier(bytes[start - 1]))
            && (end == bytes.len() || !identifier(bytes[end]));
        while bounded && end < bytes.len() && ASCII_SPACES.as_bytes().contains(&bytes[end]) {
            end += 1;
        }
        bounded && bytes.get(end) == Some(&b'.')
    })
}

/// Python's `_inexact_shared_members`: shared members whose result depends on relation names.
fn inexact_shared_members(response: &Value, payload: &BatchPayload) -> Vec<usize> {
    if payload.shared_queries.is_empty() {
        return Vec::new();
    }
    let (Some(analyses), Some(templates)) = (
        response.get("analyses").and_then(Value::as_array),
        response.get("templates").and_then(Value::as_array),
    ) else {
        return Vec::new();
    };
    let validations: Option<&Vec<Value>> = response.get("validations").and_then(Value::as_array);
    payload
        .query_indexes
        .iter()
        .zip(analyses)
        .enumerate()
        .filter(|(_, (query, analysis))| {
            payload.shared_queries.contains(query) && {
                let template: Option<&Value> = analysis_template(analysis, templates);
                let validation: Option<&Value> =
                    validations.and_then(|validations| validations.get(**query));
                !shared_result_is_exact(template, validation)
            }
        })
        .map(|(member, _)| member)
        .collect()
}

/// The template an analysis names, None where Python reads the analysis as a failure.
fn analysis_template<'a>(analysis: &Value, templates: &'a [Value]) -> Option<&'a Value> {
    let index: u64 = analysis.as_array()?.first()?.as_u64()?;
    match usize::try_from(index) {
        Ok(index) => templates.get(index),
        Err(_) => None,
    }
}

/// `stubs`' stub for `name`, or `name` itself when it has none.
fn stub_name<'a>(stubs: &'a [(String, String)], name: &'a str) -> &'a str {
    for (reference, stub) in stubs {
        if reference == name {
            return stub;
        }
    }
    name
}

/// A bound member's prekey; members without a binding schema never share.
fn member_prekey(member: &BatchMember<'_>) -> Result<Option<String>, String> {
    match member.binding_schema {
        Some(_) => share_prekey(member).map(Some),
        None => Ok(None),
    }
}

/// `query` when another member shares its key.
fn with_partner(
    query: Option<SharedQuery>,
    counts: &HashMap<String, usize>,
) -> Option<SharedQuery> {
    let query: SharedQuery = query?;
    (counts.get(&query.key).copied().unwrap_or(0) >= MIN_SHARED_MEMBERS).then_some(query)
}

/// Whether a binding relation or column name carries a double quote.
fn schema_quoted(schema: &Shapes) -> bool {
    for (relation, columns) in schema {
        if relation.contains('"') {
            return true;
        }
        for (column, _) in columns {
            if column.contains('"') {
                return true;
            }
        }
    }
    false
}

/// Python's `shared_result_is_exact`: an analysed template without any binding diagnostic.
fn shared_result_is_exact(template: Option<&Value>, validation: Option<&Value>) -> bool {
    let analysed: bool = template.is_some_and(|template| !template.is_string());
    analysed
        && match validation {
            None | Some(Value::Null) => true,
            Some(validation) => validation
                .get("errors")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
        }
}

/// A single-reference member's declared column order, as Python's template records it.
fn declared_column_order<'a>(
    member: &BatchMember<'_>,
    nullability: &'a ShapeTable,
) -> Vec<&'a str> {
    let [single] = member.analysis_names.as_slice() else {
        return Vec::new();
    };
    let Some(columns) = nullability.get(single) else {
        return Vec::new();
    };
    columns.iter().map(|(name, _)| name.as_str()).collect()
}

fn to_index(value: u64) -> Result<usize, String> {
    usize::try_from(value).map_err(|error| error.to_string())
}

/// Python's `_project_compact_analysis_batch`.
fn project_response(
    response: &Value,
    cleaned: Vec<String>,
    remember: &[bool],
) -> Result<Vec<MemberResult>, String> {
    let strings: Vec<&str> = response
        .get("strings")
        .and_then(Value::as_array)
        .ok_or(INVALID_BATCH)?
        .iter()
        .map(|value| value.as_str().ok_or(INVALID_BATCH))
        .collect::<Result<_, _>>()?;
    let batch = BatchResponse {
        strings,
        facts: response
            .get("facts")
            .and_then(Value::as_array)
            .ok_or(INVALID_BATCH)?,
        templates: response
            .get("templates")
            .and_then(Value::as_array)
            .ok_or(INVALID_BATCH)?,
    };
    let analyses = response
        .get("analyses")
        .and_then(Value::as_array)
        .filter(|analyses| analyses.len() == cleaned.len())
        .ok_or(INVALID_BATCH)?;
    let mut results: Vec<MemberResult> = Vec::with_capacity(cleaned.len());
    for ((cleaned_sql, analysis), remember) in cleaned.into_iter().zip(analyses).zip(remember) {
        let (analysis, failure, canonical) = project_member(&batch, analysis, *remember)?;
        results.push(MemberResult {
            cleaned_sql,
            analysis,
            binding_diagnostics: None,
            failure,
            canonical,
        });
    }
    Ok(results)
}

fn project_member(
    batch: &BatchResponse<'_>,
    analysis: &Value,
    keep_canonical: bool,
) -> Result<(MemberAnalysis, Option<String>, Option<CanonicalLineage>), String> {
    let entry: Option<&Vec<Value>> = analysis
        .as_array()
        .filter(|entry| entry.len() == RESPONSE_LENGTH);
    let (Some(template_index), Some(mappings)) = (
        entry.and_then(|entry| entry[0].as_i64()),
        entry.and_then(|entry| entry[1].as_array()),
    ) else {
        return Ok((
            MemberAnalysis::Failed,
            Some(INVALID_NATIVE_RESPONSE.to_owned()),
            None,
        ));
    };
    let template: &Value = u64::try_from(template_index)
        .map_err(|error| error.to_string())
        .and_then(to_index)
        .map(|index| batch.templates.get(index))?
        .ok_or("native compact query analysis returned an invalid template index")?;
    if let Some(error) = template.as_str() {
        let analysis = if error == LEGACY_FALLBACK {
            MemberAnalysis::Legacy { lineage: None }
        } else {
            MemberAnalysis::Failed
        };
        return Ok((analysis, Some(error.to_owned()), None));
    }
    let parts: &Vec<Value> = template.as_array().ok_or(INVALID_TEMPLATE)?;
    let flagged: bool = parts.len() == LEGACY_RESPONSE_LENGTH;
    let legacy: bool = flagged && parts[2].as_str() == Some(LEGACY_FALLBACK);
    let (Some(rows), Some(has_star)) = (
        parts.first().and_then(Value::as_array),
        parts.get(1).and_then(Value::as_bool),
    ) else {
        return Err(INVALID_TEMPLATE.to_owned());
    };
    if !(parts.len() == RESPONSE_LENGTH || legacy || (flagged && parts[2].is_boolean())) {
        return Err(INVALID_TEMPLATE.to_owned());
    }
    let resource_indexes: HashMap<u64, u64> = resource_name_indexes(batch, mappings)?;
    let (columns, lineage) = project_rows(batch, rows, &resource_indexes)?;
    if legacy {
        return Ok((
            MemberAnalysis::Legacy {
                lineage: Some(lineage),
            },
            None,
            None,
        ));
    }
    let canonical: Option<CanonicalLineage> = if keep_canonical {
        let mut stubs: Vec<String> = Vec::with_capacity(resource_indexes.len());
        for stub in resource_indexes.keys() {
            stubs.push(pooled(batch, *stub)?.to_owned());
        }
        Some(CanonicalLineage {
            stubs,
            lineage: project_rows(batch, rows, &HashMap::new())?.1,
        })
    } else {
        None
    };
    Ok((
        MemberAnalysis::Projected {
            columns,
            lineage,
            has_star,
            star_resolved: flagged && parts[2].as_bool() == Some(true),
        },
        None,
        canonical,
    ))
}

fn resource_name_indexes(
    batch: &BatchResponse<'_>,
    mappings: &[Value],
) -> Result<HashMap<u64, u64>, String> {
    let mut indexes: HashMap<u64, u64> = HashMap::new();
    for mapping in mappings {
        let pair: &Vec<Value> = mapping
            .as_array()
            .filter(|pair| pair.len() == RESPONSE_LENGTH)
            .ok_or("native compact query analysis returned an invalid resource mapping")?;
        let (Some(canonical), Some(resource)) = (pair[0].as_u64(), pair[1].as_u64()) else {
            return Err(
                "native compact query analysis returned an invalid resource mapping".to_owned(),
            );
        };
        ensure_pooled(batch, canonical)?;
        ensure_pooled(batch, resource)?;
        indexes.insert(canonical, resource);
    }
    Ok(indexes)
}

fn pooled<'a>(batch: &BatchResponse<'a>, index: u64) -> Result<&'a str, String> {
    batch
        .strings
        .get(to_index(index)?)
        .copied()
        .ok_or_else(|| "native compact analysis returned an out-of-range index".to_owned())
}

fn ensure_pooled(batch: &BatchResponse<'_>, index: u64) -> Result<(), String> {
    pooled(batch, index).map(drop)
}

/// Python's `_compact_projected_analysis_result` columns and resolved lineage rows.
fn project_rows(
    batch: &BatchResponse<'_>,
    rows: &[Value],
    resource_indexes: &HashMap<u64, u64>,
) -> Result<(Vec<ColumnFact>, Vec<LineageRow>), String> {
    let mut columns: Vec<ColumnFact> = Vec::with_capacity(rows.len());
    let mut lineage: Vec<LineageRow> = Vec::with_capacity(rows.len());
    for row in rows {
        let fact: &Vec<Value> = batch
            .facts
            .get(to_index(row.as_u64().ok_or(INVALID_COLUMN)?)?)
            .and_then(Value::as_array)
            .filter(|fact| fact.len() == FACT_LENGTH)
            .ok_or("native compact analysis returned an invalid column")?;
        let (column, lineage_row) = project_fact(batch, fact, resource_indexes)?;
        columns.push(column);
        lineage.push(lineage_row);
    }
    Ok((columns, lineage))
}

fn project_fact(
    batch: &BatchResponse<'_>,
    fact: &[Value],
    resource_indexes: &HashMap<u64, u64>,
) -> Result<(ColumnFact, LineageRow), String> {
    let (Some(name), Some(nullability), Some(transform), Some(confidence), Some(sources)) = (
        fact[0].as_u64(),
        fact[2].as_u64(),
        fact[3].as_u64(),
        fact[4].as_u64(),
        fact[5].as_array(),
    ) else {
        return Err(INVALID_COLUMN.to_owned());
    };
    let nullability: &str = NULLABILITY_BY_CODE
        .get(to_index(nullability)?)
        .copied()
        .filter(|_| transform < TRANSFORM_CODES && confidence < CONFIDENCE_CODES)
        .ok_or("native compact analysis returned an invalid fact code")?;
    let data_type: Option<String> = match &fact[1] {
        Value::Null => None,
        value => Some(
            pooled(
                batch,
                value
                    .as_u64()
                    .ok_or("native compact analysis returned an invalid type index")?,
            )?
            .to_owned(),
        ),
    };
    let output_column: String = pooled(batch, name)?.to_owned();
    let mut resolved: Vec<(String, String, String)> = Vec::with_capacity(sources.len());
    for source in sources {
        resolved.push(project_source(batch, source, resource_indexes)?);
    }
    Ok((
        ColumnFact {
            name: output_column.clone(),
            data_type,
            nullability: nullability.to_owned(),
        },
        LineageRow {
            output_column,
            transform_code: u8::try_from(transform).map_err(|error| error.to_string())?,
            confidence_code: u8::try_from(confidence).map_err(|error| error.to_string())?,
            sources: resolved,
            expression_range: None,
        },
    ))
}

fn project_source(
    batch: &BatchResponse<'_>,
    source: &Value,
    resource_indexes: &HashMap<u64, u64>,
) -> Result<(String, String, String), String> {
    const INVALID_SOURCE: &str = "native compact analysis returned invalid upstream lineage";
    let parts: &Vec<Value> = source
        .as_array()
        .filter(|parts| parts.len() == SOURCE_LENGTH)
        .ok_or(INVALID_SOURCE)?;
    let (Some(resource_type), Some(resource), Some(column)) =
        (parts[0].as_u64(), parts[1].as_u64(), parts[2].as_u64())
    else {
        return Err(INVALID_SOURCE.to_owned());
    };
    ensure_pooled(batch, resource)?;
    Ok((
        pooled(batch, resource_type)?.to_owned(),
        pooled(
            batch,
            resource_indexes.get(&resource).copied().unwrap_or(resource),
        )?
        .to_owned(),
        pooled(batch, column)?.to_owned(),
    ))
}

/// Python's `_binding_result` over one native validation.
fn decode_validation(
    sql: &str,
    dialect: &str,
    validation: &Value,
) -> Result<Vec<DiagnosticRow>, String> {
    let errors: &Vec<Value> = validation
        .get("errors")
        .and_then(Value::as_array)
        .ok_or("native SQL schema validation returned an invalid response")?;
    let mut rows: Vec<DiagnosticRow> = Vec::new();
    for error in errors {
        if let Some(row) = diagnostic_row(error)? {
            rows.push(row);
        }
    }
    binding_diagnostics(sql, native_dialect(dialect), rows)
}

fn diagnostic_row(error: &Value) -> Result<Option<DiagnosticRow>, String> {
    let (Some(severity), Some(code)) = (
        error.get("severity").and_then(Value::as_str),
        error.get("code").and_then(Value::as_str),
    ) else {
        return Ok(None);
    };
    if !BINDING_SEVERITIES.contains(&severity) {
        return Ok(None);
    }
    let message: String = match error.get("message") {
        Some(Value::String(message)) if !message.is_empty() => message.clone(),
        _ => DEFAULT_BINDING_MESSAGE.to_owned(),
    };
    Ok(Some(DiagnosticRow {
        code: code.to_owned(),
        message,
        line: position(error, "line")?,
        column: position(error, "column")?,
        start: position(error, "start")?,
        end: position(error, "end")?,
        severity: severity.to_owned(),
    }))
}

fn position(error: &Value, key: &str) -> Result<Option<usize>, String> {
    error
        .get(key)
        .and_then(Value::as_u64)
        .map(to_index)
        .transpose()
}
