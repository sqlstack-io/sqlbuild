// Pure helpers for the Checks pages.

import type { Check, CheckEvaluation, CheckOutcome, EvaluationMode } from '$lib/api';

export function outcomeOf(check: Pick<Check, 'latest'>): CheckOutcome {
	return check.latest?.outcome ?? 'skipped';
}

/** Violations for violation checks, the measured value for measurement checks. */
export function checkValue(evaluation: CheckEvaluation | null, mode: EvaluationMode): string {
	if (!evaluation) return '—';
	if (evaluation.outcome === 'error') return 'error';
	if (mode === 'measurement') return evaluation.measuredValue === null ? '—' : String(evaluation.measuredValue);
	if (evaluation.violationCount === null) return '—';
	return evaluation.violationCount === 0 ? '0 rows' : `${evaluation.violationCount.toLocaleString('en-US')} ${evaluation.violationCount === 1 ? 'row' : 'rows'}`;
}
