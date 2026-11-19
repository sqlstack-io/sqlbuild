//! The plan action and reason of one model.

use crate::planner_strategy::_helpers::reasons::{backfill_reason, own_change_reason};
use crate::planner_strategy::constants::INCREMENTAL_STRATEGIES;
use crate::planner_strategy::errors::StrategyError;
use crate::planner_strategy::models::{
    ChangeFacts, MaterializationType, ModelStrategy, PlanAction, PlanChangeKind, PlanReason,
};

/// Python's `resolve_model_plan_action`: disabled models skip, then materialization decides.
pub fn model_plan_action(
    model: &ModelStrategy,
    change: &ChangeFacts,
    full_refresh: bool,
) -> Result<(PlanAction, PlanReason), StrategyError> {
    if model.disabled {
        return Ok((PlanAction::Skip, PlanReason::Disabled));
    }
    let refreshed = |fallback: PlanReason, renamed: bool| -> PlanReason {
        if full_refresh {
            return PlanReason::FullRefresh;
        }
        if renamed && change.kind == PlanChangeKind::Renamed {
            return PlanReason::Renamed;
        }
        own_change_reason(change.kind).unwrap_or(fallback)
    };
    match model.materialization {
        MaterializationType::Custom => {
            return Ok((PlanAction::Custom, refreshed(PlanReason::NoChange, false)));
        }
        MaterializationType::View => {
            return Ok((
                PlanAction::CreateView,
                refreshed(PlanReason::NoChange, true),
            ));
        }
        MaterializationType::Snapshot => {
            return Ok((
                PlanAction::Snapshot,
                refreshed(PlanReason::NormalIncremental, false),
            ));
        }
        MaterializationType::Maintained => {
            let reason: PlanReason = refreshed(PlanReason::NoChange, true);
            if reason == PlanReason::NoChange {
                return Ok((PlanAction::KeepMaintained, reason));
            }
            return Ok((PlanAction::CreateMaintained, reason));
        }
        MaterializationType::Table
        | MaterializationType::Incremental
        | MaterializationType::Seed => {}
    }
    if full_refresh {
        return Ok((PlanAction::CreateTable, PlanReason::FullRefresh));
    }
    if change.kind == PlanChangeKind::FirstRun {
        return Ok((PlanAction::CreateTable, PlanReason::FirstRun));
    }
    if change.kind == PlanChangeKind::Renamed {
        return Ok((PlanAction::CreateTable, PlanReason::Renamed));
    }
    if change.backfill_full {
        return Ok((PlanAction::CreateTable, backfill_reason(change)));
    }
    let edited: Option<PlanReason> = match change.kind {
        PlanChangeKind::QueryChanged => Some(PlanReason::QueryChanged),
        PlanChangeKind::FunctionChanged => Some(PlanReason::FunctionChanged),
        PlanChangeKind::ConfigChanged => Some(PlanReason::ConfigChanged),
        PlanChangeKind::SchemaChanged => Some(PlanReason::SchemaChanged),
        PlanChangeKind::FirstRun | PlanChangeKind::Renamed | PlanChangeKind::NoChange => None,
    };
    if model.materialization == MaterializationType::Table {
        return Ok((
            PlanAction::CreateTable,
            edited.unwrap_or(PlanReason::NoChange),
        ));
    }
    let Some(strategy) = model.incremental_strategy.as_deref() else {
        return Err(StrategyError::Missing {
            model: model.name.clone(),
        });
    };
    let reason: PlanReason = edited.unwrap_or(PlanReason::NormalIncremental);
    INCREMENTAL_STRATEGIES
        .iter()
        .find(|(name, _)| *name == strategy)
        .map(|(_, action)| (*action, reason))
        .ok_or_else(|| StrategyError::Unknown {
            model: model.name.clone(),
            strategy: strategy.to_owned(),
        })
}
