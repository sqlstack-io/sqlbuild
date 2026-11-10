//! One native analysis shared by the models of a wave whose analyses differ only in relation names.
//!
//! Models share when their analysis SQL is the same text once each referenced relation is renamed
//! to its position, the relations they read have the same shapes under that renaming, and their
//! reference names sort in the same order, since lineage lists sources by relation name. The
//! dialect, declared functions, session functions and quoted-name setting are the wave's own, so
//! every member shares them. A model whose key cannot be built exactly is analysed on its own.

use std::collections::{HashMap, HashSet};

use rayon::ThreadPool;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use sha2::{Digest, Sha256};
use sqlbuild_core::identifier_text::main::is_lowercase_identifier::is_lowercase_identifier;
use sqlquack_semantics::lineage::models::SchemaColumn;

use crate::analysis_route::main::model_outcome::{analysed_sql, native_outcome, native_sql};
use crate::analysis_route::models::NativeAnalysis;
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::{ModelOutcome, ModelRequest, SessionSchema};
use crate::assembly::analysis_session::types::Shapes;

/// Spelling of a referenced relation in a share key's SQL, followed by its position; never parsed.
const RELATION_STUB: &str = "__sqlbuild_shared_relation_";
/// Opens the resource name the shared analysis gives a reference's position in its lineage rows.
const REFERENCE_TOKEN: char = '\u{0}';
/// Letter the name probe spells its same-length stand-in with.
const PROBE_LETTER: char = 'q';

/// Digest of what decides a model's native analysis besides the wave's settings: its analysis SQL
/// with each reference spelled as its position's stub, the relations it reads under that renaming,
/// and the order its reference names sort in.
type ShareKey = [u8; 32];

/// The relations a model's analysis reads, built from its binding schema when needed.
pub(crate) type RelationsOf<'a> = &'a (dyn Fn(&Shapes) -> SessionSchema + Sync);

/// One model of a native wave, prepared for analysis.
pub(crate) struct NativeMember<'a> {
    pub(crate) model: &'a ModelRequest,
    pub(crate) schema: Shapes,
    pub(crate) cleaned: Result<String, String>,
    pub(crate) key: Option<ShareKey>,
}

/// How many models took a shared analysis, and how many were analysed.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ShareCounts {
    pub(crate) shared: usize,
    pub(crate) unshared: usize,
}

/// `model` prepared over `schema`, with its share key when one can be built exactly.
pub(crate) fn native_member<'a>(
    dialect: &dyn NativeDialect,
    model: &'a ModelRequest,
    schema: Shapes,
    relations_of: RelationsOf<'_>,
) -> NativeMember<'a> {
    let cleaned: Result<String, String> = native_sql(dialect, model, &HashMap::new());
    let key: Option<ShareKey> = cleaned
        .as_ref()
        .ok()
        .and_then(|sql| share_key(dialect, model, sql, &relations_of(&schema)));
    NativeMember {
        model,
        schema,
        cleaned,
        key,
    }
}

/// Each member's outcome and failure, one analysis per share key that two members hold.
pub(crate) fn analysed_members(
    dialect: &dyn NativeDialect,
    pool: &ThreadPool,
    mut members: Vec<NativeMember<'_>>,
    (relations_of, settings): (RelationsOf<'_>, (&[String], bool)),
) -> (Vec<(ModelOutcome, Option<String>)>, ShareCounts) {
    drop_unprobed_keys(dialect, pool, &mut members);
    let leaders: Vec<Option<usize>> = share_leaders(&members);
    let led: HashSet<usize> = leaders.iter().flatten().copied().collect();
    let first: Vec<(usize, Result<NativeAnalysis, String>)> = pool.install(|| {
        (0..members.len())
            .into_par_iter()
            .filter(|index| leaders[*index].is_none())
            .map(|index| {
                let member: &NativeMember<'_> = &members[index];
                let analysed = if led.contains(&index) {
                    shared_analysis(dialect, member, (relations_of, settings))
                } else {
                    own_analysis(dialect, member, (relations_of, settings))
                };
                (index, analysed)
            })
            .collect()
    });
    let mut analyses: Vec<Option<Result<NativeAnalysis, String>>> =
        members.iter().map(|_| None).collect();
    let mut templates: HashMap<usize, Result<NativeAnalysis, String>> = HashMap::new();
    let mut alone: Vec<usize> = Vec::new();
    for (index, analysed) in first {
        if !led.contains(&index) {
            analyses[index] = Some(analysed);
            continue;
        }
        let own: Result<NativeAnalysis, String> =
            mapped(&analysed, &members[index].model.lineage_references);
        if own.is_err() && analysed.is_ok() {
            alone.push(index);
        } else {
            analyses[index] = Some(own);
        }
        let _ = templates.insert(index, analysed);
    }
    let mut counts: ShareCounts = ShareCounts {
        shared: 0,
        unshared: members.len(),
    };
    for (index, leader) in leaders.iter().enumerate() {
        let Some(leader) = leader else { continue };
        let template: Option<&Result<NativeAnalysis, String>> = templates
            .get(leader)
            .filter(|template| exact_for_others(template, members[*leader].model));
        match template.map(|template| mapped(template, &members[index].model.lineage_references)) {
            Some(Ok(analysis)) => {
                analyses[index] = Some(Ok(analysis));
                counts.shared += 1;
                counts.unshared -= 1;
            }
            _ => alone.push(index),
        }
    }
    let second: Vec<(usize, Result<NativeAnalysis, String>)> = pool.install(|| {
        alone
            .par_iter()
            .map(|index| {
                let analysed = own_analysis(dialect, &members[*index], (relations_of, settings));
                (*index, analysed)
            })
            .collect()
    });
    for (index, analysed) in second {
        analyses[index] = Some(analysed);
    }
    let outcomes: Vec<(ModelOutcome, Option<String>)> = members
        .into_iter()
        .zip(analyses)
        .map(|(member, analysed)| {
            let analysed: Result<NativeAnalysis, String> =
                analysed.unwrap_or_else(|| Err("a native model was left unanalysed".to_owned()));
            native_outcome(
                member.model,
                member.schema,
                member.cleaned.unwrap_or_default(),
                analysed,
            )
        })
        .collect();
    (outcomes, counts)
}

/// The member's analysis with its own references, or why it failed.
fn own_analysis(
    dialect: &dyn NativeDialect,
    member: &NativeMember<'_>,
    (relations_of, settings): (RelationsOf<'_>, (&[String], bool)),
) -> Result<NativeAnalysis, String> {
    let sql: &str = member.cleaned.as_ref().map_err(Clone::clone)?;
    let relations: SessionSchema = relations_of(&member.schema);
    analysed_sql(
        dialect,
        sql,
        (&relations, &member.model.lineage_references),
        settings,
    )
}

/// The member's analysis with each reference's resource named by its position, for every sharer.
fn shared_analysis(
    dialect: &dyn NativeDialect,
    member: &NativeMember<'_>,
    (relations_of, settings): (RelationsOf<'_>, (&[String], bool)),
) -> Result<NativeAnalysis, String> {
    let sql: &str = member.cleaned.as_ref().map_err(Clone::clone)?;
    let positional: Vec<(String, String, String)> = member
        .model
        .lineage_references
        .iter()
        .enumerate()
        .map(|(position, (name, _, _))| {
            let token: String = format!("{REFERENCE_TOKEN}{position}");
            (name.clone(), token.clone(), token)
        })
        .collect();
    let relations: SessionSchema = relations_of(&member.schema);
    analysed_sql(dialect, sql, (&relations, &positional), settings)
}

/// `template`'s analysis with each positional resource renamed to `references`' resource there.
///
/// A failed analysis stays failed; a source no position names leaves the result unmapped (`Err`
/// with an empty reason), which sends the model to its own analysis.
fn mapped(
    template: &Result<NativeAnalysis, String>,
    references: &[(String, String, String)],
) -> Result<NativeAnalysis, String> {
    let mut analysis: NativeAnalysis = template.clone()?;
    analysis.row_sources = Vec::new();
    for row in &mut analysis.lineage {
        let mut sources: Vec<(String, String, String)> = Vec::with_capacity(row.sources.len());
        for (_, token, column) in &row.sources {
            let reference: Option<&(String, String, String)> = token
                .strip_prefix(REFERENCE_TOKEN)
                .and_then(|position| position.parse::<usize>().ok())
                .and_then(|position| references.get(position));
            let Some((_, resource_type, resource_name)) = reference else {
                return Err(String::new());
            };
            let source = (resource_type.clone(), resource_name.clone(), column.clone());
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        row.sources = sources;
    }
    Ok(analysis)
}

/// Whether `template`, the analysis of `leader`'s own SQL, holds for every model sharing its key:
/// it succeeded, found nothing (findings carry positions and names), and spells no relation name.
fn exact_for_others(template: &Result<NativeAnalysis, String>, leader: &ModelRequest) -> bool {
    let Ok(analysis) = template else {
        return false;
    };
    let names: Vec<&str> = leader
        .lineage_references
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();
    let spells = |text: &str| {
        let folded: String = text.to_ascii_lowercase();
        names.iter().any(|name| folded.contains(name))
    };
    analysis.diagnostics.is_empty()
        && !analysis
            .columns
            .iter()
            .any(|column| spells(&column.name) || column.data_type.as_deref().is_some_and(spells))
        && !analysis.lineage.iter().any(|row| {
            spells(&row.output_column) || row.sources.iter().any(|(_, _, column)| spells(column))
        })
}

/// Each member's leader: the first earlier member with its key, when another member holds it.
fn share_leaders(members: &[NativeMember<'_>]) -> Vec<Option<usize>> {
    let mut first: HashMap<&ShareKey, usize> = HashMap::new();
    members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let key: &ShareKey = member.key.as_ref()?;
            let leader: usize = *first.entry(key).or_insert(index);
            (leader != index).then_some(leader)
        })
        .collect()
}

/// Forget the key of each member referencing a relation whose name the parser does not read as an
/// ordinary table name.
fn drop_unprobed_keys(
    dialect: &dyn NativeDialect,
    pool: &ThreadPool,
    members: &mut [NativeMember<'_>],
) {
    let names: HashSet<&str> = members
        .iter()
        .filter(|member| member.key.is_some())
        .flat_map(|member| member.model.lineage_references.iter())
        .map(|(name, _, _)| name.as_str())
        .collect();
    let ordinary: HashSet<String> = pool.install(|| {
        names
            .par_iter()
            .filter(|name| ordinary_name(dialect, name))
            .map(|name| (*name).to_owned())
            .collect()
    });
    for member in members.iter_mut() {
        if member
            .model
            .lineage_references
            .iter()
            .any(|(name, _, _)| !ordinary.contains(name))
        {
            member.key = None;
        }
    }
}

/// Whether the parser reads `FROM name` as it reads `FROM` a same-length plain stand-in, so no
/// keyword or special form hides behind the name.
fn ordinary_name(dialect: &dyn NativeDialect, name: &str) -> bool {
    let stand_in: String = PROBE_LETTER.to_string().repeat(name.len());
    let probe = |relation: &str| {
        dialect
            .read_whole(&format!("SELECT * FROM {relation}"))
            .map(|tree| format!("{tree:?}"))
    };
    match (probe(name), probe(&stand_in)) {
        (Ok(named), Ok(standing)) => named.replace(name, &stand_in) == standing,
        _ => false,
    }
}

/// The key of `model`, whose cleaned SQL is `cleaned`, or `None` where renaming its references
/// could change its analysis.
fn share_key(
    dialect: &dyn NativeDialect,
    model: &ModelRequest,
    cleaned: &str,
    relations: &SessionSchema,
) -> Option<ShareKey> {
    let names: Vec<&str> = model
        .lineage_references
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();
    let distinct: HashSet<&str> = names.iter().copied().collect();
    if names.is_empty()
        || distinct.len() != names.len()
        // A leading letter makes a quoted lower-case name and an unquoted upper-cased one compare
        // the same way whatever follows, so the two name orders in the key cover a model that
        // quotes only some references.
        || !names.iter().all(|name| is_lowercase_identifier(name))
        || !cleaned.is_ascii()
        || cleaned.to_ascii_lowercase().contains(RELATION_STUB)
    {
        return None;
    }
    let width: usize = names.len().to_string().len();
    let stubs: HashMap<String, String> = names
        .iter()
        .enumerate()
        .map(|(position, name)| {
            let stub: String = format!("{RELATION_STUB}{position:0width$}");
            ((*name).to_owned(), stub)
        })
        .collect();
    let sql: String = native_sql(dialect, model, &stubs).ok()?;
    let unstubbed: String = without_stubs(&sql.to_ascii_lowercase());
    if !stubs_stand_alone(&sql, width) || names.iter().any(|name| unstubbed.contains(name)) {
        return None;
    }
    let mut folded: HashSet<String> = HashSet::new();
    let mut shaped: Vec<(String, bool, &[SchemaColumn])> = Vec::new();
    for (open, table) in [(false, &relations.relations), (true, &relations.open)] {
        for (name, columns) in table {
            let lowered: String = name.to_ascii_lowercase();
            if lowered.contains(RELATION_STUB) || !folded.insert(lowered) {
                return None;
            }
            let renamed: String = stubs.get(name).cloned().unwrap_or_else(|| name.clone());
            shaped.push((renamed, open, columns.as_slice()));
        }
    }
    shaped.sort_by(|left, right| (&left.0, left.1).cmp(&(&right.0, right.1)));
    let name_orders: [Vec<usize>; 2] = [
        name_order(&names, |name| name.to_owned()),
        name_order(&names, str::to_ascii_uppercase),
    ];
    let mut hasher: Sha256 = Sha256::new();
    hasher.update(sql.len().to_le_bytes());
    hasher.update(sql.as_bytes());
    hasher.update(format!("{:?}", (shaped, names.len(), name_orders)).as_bytes());
    Some(hasher.finalize().into())
}

/// The positions of `names` sorted by each name as `spelled`.
fn name_order(names: &[&str], spelled: impl Fn(&str) -> String) -> Vec<usize> {
    let mut order: Vec<usize> = (0..names.len()).collect();
    order.sort_by_cached_key(|position| spelled(names[*position]));
    order
}

/// Whether every relation stub in `sql`, with its `width` position digits, is a whole name: no
/// identifier character touches it, so a reference glued into a longer name (`__ref("x")_split`)
/// never shares.
fn stubs_stand_alone(sql: &str, width: usize) -> bool {
    let identifier = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let bytes: &[u8] = sql.as_bytes();
    sql.match_indices(RELATION_STUB).all(|(start, _)| {
        let end: usize = start + RELATION_STUB.len() + width;
        let before: bool = start > 0 && identifier(bytes[start - 1]);
        let after: bool = bytes.get(end).is_some_and(|c| identifier(*c));
        !before && !after
    })
}

/// `folded_sql` with every relation stub blanked, leaving only text the model spells itself.
fn without_stubs(folded_sql: &str) -> String {
    let mut text: String = String::with_capacity(folded_sql.len());
    let mut rest: &str = folded_sql;
    while let Some(start) = rest.find(RELATION_STUB) {
        text.push_str(&rest[..start]);
        text.push(' ');
        rest = rest[start + RELATION_STUB.len()..].trim_start_matches(|c: char| c.is_ascii_digit());
    }
    text.push_str(rest);
    text
}
