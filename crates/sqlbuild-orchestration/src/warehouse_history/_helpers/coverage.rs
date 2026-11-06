//! Microbatch coverage projected from the mirrored completion and replay-requirement facts.

use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::{DateTime, Datelike, NaiveDateTime, Timelike};
use sqlbuild_cursor::algebra::main::normalize_timestamp_text::normalize_timestamp_text;
use sqlbuild_cursor::algebra::main::parse_integer_text::parse_integer_text;
use sqlbuild_cursor::algebra::models::{CivilDate, LocalDateTime, SortKey};
use sqlbuild_model_config::model_validation::main::parse_iso_timestamp::parse_iso_timestamp;
use sqlbuild_state::microbatches::main::project_coverage::project_coverage;
use sqlbuild_state::microbatches::models::{
    BoundInterval, CompletionFact, CoverageProjection, CursorBound, TextInterval,
};
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_history::_helpers::maintenance::records;
use crate::warehouse_history::models::{BatchState, CoverageBatch, MicrobatchCoverage};
use crate::warehouse_metadata::models::WarehouseMicrobatchRecord;

const ORDINARY_COMPLETION: &str = "partition_completion";
const SYNTHETIC_COMPLETION: &str = "synthetic_completion";
const REPLAY_REQUIREMENT: &str = "replay_requirement";
const UNKNOWN_FINGERPRINT: &str = "unknown";
const INTEGER_CURSOR: &str = "integer";
/// The `datetime.fromisoformat` rules cursor bounds are validated with.
const ISOFORMAT_PYTHON_MINOR: u8 = 12;

/// The key a cursor bound sorts by: the integer, or the wall time of a naive value and the UTC instant of an aware one.
fn sort_key(text: &str, cursor_type: &str) -> Option<SortKey> {
    if cursor_type == INTEGER_CURSOR {
        return parse_integer_text(text).map(SortKey::Integer);
    }
    let (micros, _) = parse_iso_timestamp(&normalize_timestamp_text(text), ISOFORMAT_PYTHON_MINOR)?;
    let wall: NaiveDateTime = DateTime::from_timestamp_micros(micros)?.naive_utc();
    Some(SortKey::Moment(LocalDateTime {
        date: CivilDate {
            year: i64::from(wall.year()),
            month: wall.month(),
            day: wall.day(),
        },
        hour: wall.hour(),
        minute: wall.minute(),
        second: wall.second(),
        microsecond: wall.nanosecond() / 1000,
    }))
}

fn order(left: &CursorBound, right: &CursorBound) -> Ordering {
    match (&left.key, &right.key) {
        (SortKey::Integer(left), SortKey::Integer(right)) => left.cmp(right),
        (SortKey::Moment(left), SortKey::Moment(right)) => left.cmp(right),
        _ => Ordering::Equal,
    }
}

/// Bound texts of one cursor type, each parsed once.
struct Bounds<'a> {
    cursor_type: &'a str,
    keys: HashMap<String, SortKey>,
}

impl Bounds<'_> {
    fn bound(&mut self, text: &str) -> Result<CursorBound, ControlError> {
        if let Some(key) = self.keys.get(text) {
            return Ok(CursorBound {
                text: text.into(),
                key: *key,
            });
        }
        let key: SortKey = sort_key(text, self.cursor_type).ok_or_else(|| {
            ControlError::Schema(format!(
                "stored microbatch bound is not a {} cursor value",
                self.cursor_type
            ))
        })?;
        self.keys.insert(text.into(), key);
        Ok(CursorBound {
            text: text.into(),
            key,
        })
    }

    fn interval(&mut self, interval: &TextInterval) -> Result<BoundInterval, ControlError> {
        Ok(BoundInterval {
            start: self.bound(&interval.start)?,
            end: self.bound(&interval.end)?,
        })
    }
}

fn newest<'a>(
    records: impl Iterator<Item = &'a WarehouseMicrobatchRecord>,
) -> Option<&'a WarehouseMicrobatchRecord> {
    records.max_by(|left, right| {
        (left.created_at, &left.event_id).cmp(&(right.created_at, &right.event_id))
    })
}

/// Completion facts of the current relation and the records they came from, in the same order.
fn completions<'a>(
    scope: &[&'a WarehouseMicrobatchRecord],
    bounds: &mut Bounds<'_>,
) -> Result<(Vec<CompletionFact>, Vec<&'a WarehouseMicrobatchRecord>), ControlError> {
    let mut facts: Vec<CompletionFact> = Vec::new();
    let mut sources: Vec<&WarehouseMicrobatchRecord> = Vec::new();
    for record in scope {
        let (Some(start), Some(end)) = (&record.partition_start, &record.partition_end) else {
            continue;
        };
        if ![ORDINARY_COMPLETION, SYNTHETIC_COMPLETION].contains(&record.record_type.as_str()) {
            continue;
        }
        facts.push(CompletionFact {
            interval: BoundInterval {
                start: bounds.bound(start)?,
                end: bounds.bound(end)?,
            },
            ordinary: record.record_type == ORDINARY_COMPLETION,
            unknown_fingerprint: record.fingerprint_status == UNKNOWN_FINGERPRINT,
            created: (record.created_at - DateTime::UNIX_EPOCH).as_seconds_f64(),
            event_id: record.event_id.clone(),
        });
        sources.push(record);
    }
    Ok((facts, sources))
}

/// The newest replay requirement's range and required version, unless a newer completion at a third version ended it, as the executor decides.
fn replay_range(
    scope: &[&WarehouseMicrobatchRecord],
    bounds: &mut Bounds<'_>,
) -> Result<Option<(BoundInterval, Option<String>)>, ControlError> {
    let Some(requirement) = newest(
        scope
            .iter()
            .copied()
            .filter(|record| record.record_type == REPLAY_REQUIREMENT),
    ) else {
        return Ok(None);
    };
    let superseded: bool = scope.iter().any(|record| {
        record.record_type == ORDINARY_COMPLETION
            && (record.created_at, &record.event_id)
                > (requirement.created_at, &requirement.event_id)
            && record.model_version_hash.as_ref().is_some_and(|version| {
                Some(version) != requirement.required_model_version_hash.as_ref()
            })
    });
    if superseded {
        return Ok(None);
    }
    Ok(Some((
        bounds.interval(&TextInterval {
            start: requirement.run_start.clone(),
            end: requirement.run_end.clone(),
        })?,
        requirement.required_model_version_hash.clone(),
    )))
}

fn overlaps(left: &BoundInterval, right: &BoundInterval) -> bool {
    order(&left.start, &right.end).is_lt() && order(&right.start, &left.end).is_lt()
}

/// One stretch of the cursor and the completion that accounts for it, if any.
struct Piece {
    interval: BoundInterval,
    state: BatchState,
    completion: Option<usize>,
}

/// Adjacent stretches of one completion, or of one hole, read as one batch; empty stretches are dropped.
fn merged(mut pieces: Vec<Piece>) -> Vec<Piece> {
    pieces.sort_by(|left, right| order(&left.interval.start, &right.interval.start));
    let mut kept: Vec<Piece> = Vec::with_capacity(pieces.len());
    for piece in pieces {
        if order(&piece.interval.start, &piece.interval.end).is_ge() {
            continue;
        }
        match kept.last_mut() {
            Some(last)
                if last.completion == piece.completion
                    && last.state == piece.state
                    && order(&last.interval.end, &piece.interval.start).is_eq() =>
            {
                last.interval.end = piece.interval.end;
            }
            _ => kept.push(piece),
        }
    }
    kept
}

fn batch(piece: &Piece, sources: &[&WarehouseMicrobatchRecord]) -> CoverageBatch {
    CoverageBatch {
        start: piece.interval.start.text.clone(),
        end: piece.interval.end.text.clone(),
        state: piece.state,
        run_id: piece
            .completion
            .map(|completion| sources[completion].execution_run_id.clone())
            .filter(|run| !run.is_empty()),
    }
}

/// Completed segments, spelled as their completion recorded them where they share its bounds, and the holes between them, oldest first; and the holes alone.
fn batches(
    projection: &CoverageProjection,
    sources: &[&WarehouseMicrobatchRecord],
    (bounds, replay): (&mut Bounds<'_>, Option<&(BoundInterval, Option<String>)>),
) -> Result<(Vec<CoverageBatch>, Vec<CoverageBatch>), ControlError> {
    let mut completed: Vec<Piece> = Vec::new();
    for segment in &projection.segments {
        let mut interval: BoundInterval = bounds.interval(&segment.interval)?;
        let source: &WarehouseMicrobatchRecord = sources[segment.completion];
        for (recorded, bound) in [
            (&source.partition_start, &mut interval.start),
            (&source.partition_end, &mut interval.end),
        ] {
            let Some(recorded) = recorded else { continue };
            let recorded: CursorBound = bounds.bound(recorded)?;
            if order(&recorded, bound).is_eq() {
                *bound = recorded;
            }
        }
        let stale: bool = replay.is_some_and(|(range, required)| {
            overlaps(&interval, range)
                && source.fingerprint_status != UNKNOWN_FINGERPRINT
                && source.model_version_hash != *required
        });
        completed.push(Piece {
            interval,
            state: if stale {
                BatchState::ReplayRequired
            } else {
                BatchState::Complete
            },
            completion: Some(segment.completion),
        });
    }
    let mut holes: Vec<Piece> = Vec::new();
    for hole in projection
        .known_missing
        .iter()
        .chain(&projection.unaccounted)
    {
        holes.push(Piece {
            interval: bounds.interval(hole)?,
            state: BatchState::Gap,
            completion: None,
        });
    }
    let holes: Vec<Piece> = merged(holes);
    let gaps: Vec<CoverageBatch> = holes.iter().map(|piece| batch(piece, sources)).collect();
    let mut every: Vec<Piece> = merged(completed);
    every.extend(holes);
    every.sort_by(|left, right| order(&left.interval.start, &right.interval.start));
    Ok((
        every.iter().map(|piece| batch(piece, sources)).collect(),
        gaps,
    ))
}

/// Coverage of the model's newest recorded relation and generation, or `None` when none is tracked.
pub(crate) async fn microbatch_coverage(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
    (model, limit): (&str, u32),
) -> Result<(ControlTransaction, Option<MicrobatchCoverage>), ControlError> {
    let (transaction, history): (ControlTransaction, Vec<WarehouseMicrobatchRecord>) = records(transaction,
        "SELECT record_json AS record FROM sqb_warehouse_microbatches WHERE location_id=$1 AND target=$2 AND model_name=$3",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Text(model.into())]).await?;
    let Some(current) = newest(history.iter()) else {
        return Ok((transaction, None));
    };
    let scope: Vec<&WarehouseMicrobatchRecord> = history
        .iter()
        .filter(|record| {
            record.scope_key == current.scope_key
                && record.physical_generation_id == current.physical_generation_id
        })
        .collect();
    let mut bounds = Bounds {
        cursor_type: &current.cursor_type,
        keys: HashMap::new(),
    };
    let (facts, sources) = completions(&scope, &mut bounds)?;
    let (Some(earliest), Some(latest)) = (
        facts
            .iter()
            .map(|fact| &fact.interval.start)
            .min_by(|left, right| order(left, right)),
        facts
            .iter()
            .map(|fact| &fact.interval.end)
            .max_by(|left, right| order(left, right)),
    ) else {
        return Ok((transaction, None));
    };
    let projection: CoverageProjection = project_coverage(
        &facts,
        &[BoundInterval {
            start: earliest.clone(),
            end: latest.clone(),
        }],
    );
    let replay: Option<(BoundInterval, Option<String>)> = replay_range(&scope, &mut bounds)?;
    let (mut recent, known_gaps) = batches(&projection, &sources, (&mut bounds, replay.as_ref()))?;
    let keep: usize = usize::try_from(limit.clamp(1, 1000)).unwrap_or(usize::MAX);
    let dropped: usize = recent.len().saturating_sub(keep);
    recent.drain(..dropped);
    Ok((
        transaction,
        Some(MicrobatchCoverage {
            model: model.into(),
            target_name: current.target_name.clone(),
            physical_generation_id: current.physical_generation_id.clone(),
            cursor_column: current.cursor_column.clone(),
            cursor_type: current.cursor_type.clone(),
            batch_size: current.batch_size.clone(),
            earliest: earliest.text.clone(),
            contiguous_frontier: projection.contiguous_frontier.clone(),
            known_gaps,
            batches: recent,
            batches_truncated: dropped > 0,
        }),
    ))
}
