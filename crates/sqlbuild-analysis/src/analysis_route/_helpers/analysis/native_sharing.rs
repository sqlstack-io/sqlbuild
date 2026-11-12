//! One native analysis shared by the models of a wave whose analyses differ only in names.

use std::collections::{HashMap, HashSet};

use rayon::ThreadPool;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use sha2::{Digest, Sha256};
use sqlbuild_core::identifier_text::main::is_lowercase_identifier::is_lowercase_identifier;
use sqlquack_semantics::lineage::models::SchemaColumn;

use crate::analysis_route::_helpers::analysis::native_outcome::{
    analysed_sql, native_outcome, native_sql,
};
use crate::analysis_route::models::{NativeAnalysis, ShareCounts};
use crate::analysis_route::types::NativeDialect;
use crate::analysis_route::types::RelationsOf;
use crate::assembly::analysis_session::models::{
    LineageRow, ModelOutcome, ModelRequest, SessionSchema,
};
use crate::assembly::analysis_session::types::Shapes;

/// Spelling of a referenced relation in a share key's SQL, followed by its position; never parsed.
const RELATION_STUB: &str = "__sqlbuild_shared_relation_";
/// Opens the resource name the shared analysis gives a reference's position in its lineage rows.
const REFERENCE_TOKEN: char = '\u{0}';
/// Letter the name probe spells its same-length stand-in with.
const PROBE_LETTER: char = 'q';

/// Digest of a model's stubbed analysis SQL, renamed relation shapes and reference name orders.
type ShareKey = [u8; 32];

/// One model of a native wave, prepared for analysis.
pub(crate) struct NativeMember<'a> {
    pub(crate) model: &'a ModelRequest,
    pub(crate) schema: Shapes,
    pub(crate) cleaned: Result<String, String>,
    pub(crate) key: Option<ShareKey>,
}

/// `model` prepared over `schema`, with its share key when one can be built exactly.
pub(crate) fn native_member<'a>(
    dialect: &dyn NativeDialect,
    model: &'a ModelRequest,
    schema: Shapes,
    relations_of: RelationsOf<'_>,
) -> NativeMember<'a> {
    let cleaned: Result<String, String> = native_sql(dialect, model, &HashMap::new());
    let key: Option<ShareKey> = match &cleaned {
        Ok(sql) => share_key(dialect, model, sql, &relations_of(&schema)),
        Err(_) => None,
    };
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
        if matches!((&own, &analysed), (Err(_), Ok(_))) {
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
            Some(Ok(mut analysis)) => {
                rebase_ranges(
                    &mut analysis,
                    members[*leader].cleaned.as_deref().unwrap_or_default(),
                    members[index].cleaned.as_deref().unwrap_or_default(),
                );
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

/// `template` with positional resources renamed to `references`; empty `Err` when one is unnamed.
fn mapped(
    template: &Result<NativeAnalysis, String>,
    references: &[(String, String, String)],
) -> Result<NativeAnalysis, String> {
    let mut analysis: NativeAnalysis = template.clone()?;
    analysis.row_sources = Vec::new();
    for row in &mut analysis.lineage {
        let mut sources: Vec<(String, String, String)> = Vec::with_capacity(row.sources.len());
        for (_, token, column) in &row.sources {
            let reference: Option<&(String, String, String)> = reference_at(token, references);
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

/// Each expression range moved onto `member`'s relation names, or dropped where it cannot move.
pub(crate) fn rebase_ranges(analysis: &mut NativeAnalysis, leader: &str, member: &str) {
    if leader == member {
        return;
    }
    let renamed: Option<Vec<(usize, usize, usize)>> = renamed_runs(leader, member);
    for row in &mut analysis.lineage {
        row.expression_range = match (&renamed, row.expression_range) {
            (Some(runs), Some((start, end))) => rebased(runs, start).zip(rebased(runs, end)),
            _ => None,
        };
    }
}

/// `(leader start, leader end, member end)` of each differing identifier run, if only those differ.
fn renamed_runs(leader: &str, member: &str) -> Option<Vec<(usize, usize, usize)>> {
    let identifier = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let (left, right): (&[u8], &[u8]) = (leader.as_bytes(), member.as_bytes());
    let (mut i, mut j): (usize, usize) = (0, 0);
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    while i < left.len() || j < right.len() {
        if i < left.len() && j < right.len() && left[i] == right[j] {
            i += 1;
            j += 1;
            continue;
        }
        let mut back: usize = 0;
        while back < i && identifier(left[i - back - 1]) {
            back += 1;
        }
        let (mut left_end, mut right_end): (usize, usize) = (i, j);
        while left_end < left.len() && identifier(left[left_end]) {
            left_end += 1;
        }
        while right_end < right.len() && identifier(right[right_end]) {
            right_end += 1;
        }
        if left_end == i && right_end == j {
            return None;
        }
        runs.push((i - back, left_end, right_end));
        (i, j) = (left_end, right_end);
    }
    Some(runs)
}

/// The member offset of leader `offset`, or `None` strictly inside a renamed run.
fn rebased(runs: &[(usize, usize, usize)], offset: usize) -> Option<usize> {
    let mut moved: usize = offset;
    for &(left_start, left_end, right_end) in runs {
        if offset <= left_start {
            break;
        }
        if offset < left_end {
            return None;
        }
        moved = offset - left_end + right_end;
    }
    Some(moved)
}

/// Whether `leader`'s `template` succeeded, found nothing and spells none of its relation names.
fn exact_for_others(template: &Result<NativeAnalysis, String>, leader: &ModelRequest) -> bool {
    let Ok(analysis) = template else {
        return false;
    };
    let names: Vec<&str> = leader
        .lineage_references
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();
    let spells = |text: &str| spells_any(&names, text);
    analysis.diagnostics.is_empty()
        && !analysis
            .columns
            .iter()
            .any(|column| spells(&column.name) || column.data_type.as_deref().is_some_and(spells))
        && !analysis
            .lineage
            .iter()
            .any(|row| row_spells_any(&names, row))
}

/// Whether `text`, case-folded, contains any of `names`.
fn spells_any(names: &[&str], text: &str) -> bool {
    let folded: String = text.to_ascii_lowercase();
    names.iter().any(|name| folded.contains(name))
}

/// Whether `row`'s output column or any of its source columns contains any of `names`.
fn row_spells_any(names: &[&str], row: &LineageRow) -> bool {
    spells_any(names, &row.output_column)
        || row
            .sources
            .iter()
            .any(|(_, _, column)| spells_any(names, column))
}

/// The reference a positional resource `token` names, when it names one.
fn reference_at<'a>(
    token: &str,
    references: &'a [(String, String, String)],
) -> Option<&'a (String, String, String)> {
    let position: &str = token.strip_prefix(REFERENCE_TOKEN)?;
    match position.parse::<usize>() {
        Ok(position) => references.get(position),
        Err(_) => None,
    }
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

/// Forget the key of each member referencing a relation the parser does not read as a plain table.
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

/// Whether the parser reads `FROM name` as it reads `FROM` a same-length plain stand-in.
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

/// The key of `model` with cleaned SQL `cleaned`, or `None` where renaming could change its analysis.
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
    let Ok(sql) = native_sql(dialect, model, &stubs) else {
        return None;
    };
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

/// Whether no identifier character touches any relation stub in `sql` with its `width` digits.
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
