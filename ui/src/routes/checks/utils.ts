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

/** What kind of check it is, e.g. `audit · not_null`; just the definition where the kind is unknown. */
export function kindLabel(check: Pick<Check, 'kind' | 'definition'>): string {
	if (check.kind === null) return check.definition ?? 'check';
	return check.kind === 'audit' ? `audit · ${check.definition ?? 'custom'}` : check.kind.replace('_', ' ');
}

/** An evidence cell as text: NULL, JSON for structured values, the value itself otherwise. */
export function cellText(value: unknown): string {
	if (value === null || value === undefined) return 'NULL';
	return typeof value === 'object' ? JSON.stringify(value) : String(value);
}
