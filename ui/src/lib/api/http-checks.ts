import type { components } from '$control/schema.d.ts';
import type { Check, CheckAttachment, CheckDetail, CheckEvaluation, CheckEvidence, CheckOutcome, CheckSeverity, CheckSql, CheckThreshold, EvaluationMode, RunScopePhase } from './types/build';

type Wire = components['schemas'];

type Bound = { operator: CheckThreshold['operator']; limit?: number; lower?: number; upper?: number };

/** `thresholds_json` holds `{warn?, error?}` bounds, each with a `limit` or, for `outside`, a `lower` and `upper`. */
export function thresholdsOf(json: string | null | undefined): CheckThreshold[] {
	if (!json) return [];
	const levels = JSON.parse(json) as Partial<Record<CheckThreshold['level'], Bound>>;
	return (['warn', 'error'] as const).flatMap((level) => {
		const bound = levels[level];
		if (!bound) return [];
		const value: CheckThreshold['value'] = bound.operator === 'outside' ? [bound.lower ?? 0, bound.upper ?? 0] : (bound.limit ?? 0);
		return [{ level, operator: bound.operator, value }];
	});
}

/**
 * One stored result. Its `run_id` is the `sqb` run that evaluated it; the server does not
 * report which scheduler run, if any, that was, so `runId` stays null.
 */
export function toEvaluation(wire: Wire['CheckResult']): CheckEvaluation {
	return {
		id: wire.event_id,
		runId: null,
		fingerprintRunId: wire.run_id,
		evaluatedAt: wire.occurred_at,
		outcome: wire.outcome as CheckOutcome,
		violationCount: wire.violation_count ?? null,
		measuredValue: wire.measured_value == null ? null : Number(wire.measured_value),
		evidenceCount: wire.evidence_count ?? null,
		evidenceTruncated: wire.evidence_truncated === 'true'
	};
}

/** A check from its newest result. The server stores no check kind or recent outcomes with the listing. */
export function toCheck(wire: Wire['CheckResult']): Check {
	return {
		id: wire.binding_key,
		name: wire.audit_name,
		definition: wire.audit_definition_name,
		kind: null,
		attachment: wire.attachment_kind as CheckAttachment,
		node: wire.attached_target_name ?? null,
		column: wire.attached_column_name ?? null,
		severity: wire.severity as CheckSeverity,
		runScopePhase: wire.run_scope_phase as RunScopePhase,
		evaluationMode: wire.evaluation_mode as EvaluationMode,
		thresholds: thresholdsOf(wire.thresholds_json),
		description: wire.audit_description ?? null,
		latest: toEvaluation(wire),
		recentOutcomes: null
	};
}

/** A check's detail from its history, newest first; no history means no such check. */
export function checkDetailOf(history: Wire['CheckResult'][]): CheckDetail | null {
	if (!history.length) return null;
	const evaluations = history.map(toEvaluation);
	return { ...toCheck(history[0]), recentOutcomes: [...evaluations].reverse().map((e) => e.outcome), history: evaluations };
}

export function toCheckSql(wire: Wire['CheckSql']): CheckSql {
	return {
		evaluationId: wire.event_id,
		executedSql: wire.executed_sql ?? null,
		measurementSql: wire.measurement_sql ?? null,
		evidenceSql: wire.evidence_sql ?? null,
		executionError: wire.execution_error ?? null,
		evidenceError: wire.evidence_error ?? null
	};
}

export function toCheckEvidence(wire: Wire['CheckEvidence']): CheckEvidence {
	return {
		evaluationId: wire.event_id,
		columns: wire.columns,
		rows: wire.rows as unknown as CheckEvidence['rows'],
		truncated: wire.truncated === 'true'
	};
}
