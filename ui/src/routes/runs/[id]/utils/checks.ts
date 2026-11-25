// A node's checks from their results in the run, problems first, and the short form a node's one log line uses. Pure.

import type { RunEvent } from '$lib/api';

export type CheckOutcome = 'failed' | 'warned' | 'passed' | 'skipped' | 'not-run';
export type CheckResult = {
	id: string;
	/** As declared, such as `not_null(payment_id)`. */
	name: string;
	outcome: CheckOutcome;
	/** Rows it found, and what they are, such as `14` and `duplicate rows`; null when it passed. */
	count: number | null;
	noun: string | null;
	/** The check's own query, for its SQL, query ID and failing rows. */
	statementId: string | null;
	/** The check's ID and this result's, for the rows it found. */
	bindingKey: string | null;
	resultId: string | null;
	/** Why it was not run, such as `reads payments, which doesn't exist in the target`; null otherwise. */
	reason: string | null;
};

const RANK: Record<CheckOutcome, number> = { failed: 0, warned: 1, passed: 2, skipped: 3, 'not-run': 4 };

/** A node's checks from its `audit_completed` events, problems first (failed, then warned), then passed, skipped and not run. */
export function checksOf(events: readonly RunEvent[]): CheckResult[] {
	return events
		.filter((e) => e.type === 'audit_completed')
		.map((e): CheckResult => {
			const check = e.check ?? { name: e.message || 'check', outcome: e.level === 'error' ? 'failed' : e.level === 'warn' ? 'warned' : 'passed', rows: null, bindingKey: null, resultId: null };
			const problem = check.outcome === 'failed' || check.outcome === 'warned';
			return {
				id: e.id,
				name: check.name,
				outcome: check.outcome,
				count: problem ? check.rows : null,
				noun: problem ? (check.name.startsWith('unique') ? 'duplicate rows' : 'failing rows') : null,
				statementId: e.statementId,
				bindingKey: check.bindingKey,
				resultId: check.resultId,
				reason: check.outcome === 'not-run' ? e.message || null : null
			};
		})
		.sort((a, b) => RANK[a.outcome] - RANK[b.outcome]);
}

/** The worst outcome among the checks, or null without any. */
export function worstCheck(checks: readonly CheckResult[]): CheckOutcome | null {
	return checks.length ? checks.reduce((w, c) => (RANK[c.outcome] < RANK[w] ? c.outcome : w), 'passed' as CheckOutcome) : null;
}

const rows = (c: CheckResult) => `${(c.count ?? 0).toLocaleString('en-GB')} ${c.count === 1 ? 'row' : 'rows'}`;

/**
 * The checks on a node's one line: all passing is counted ("5 checks passed"); one or two problems are
 * named ("not_null(payment_id) warned: 2 rows", "2 checks warned (a, b)"); more are counted
 * ("1 failed · 3 warned"); passing checks beside problems are only counted ("4 passed").
 */
export function checksText(checks: readonly CheckResult[]): string | null {
	if (!checks.length) return null;
	const failed = checks.filter((c) => c.outcome === 'failed');
	const warned = checks.filter((c) => c.outcome === 'warned');
	const passed = checks.filter((c) => c.outcome === 'passed').length;
	const problems = failed.length + warned.length;
	if (!problems) return `${passed} ${passed === 1 ? 'check' : 'checks'} passed`;
	const parts: string[] = [];
	if (problems === 1) {
		const c = failed[0] ?? warned[0];
		parts.push(`${c.name} ${c.outcome}: ${rows(c)}`);
	} else if (problems === 2 && (failed.length === 2 || warned.length === 2)) {
		const same = failed.length === 2 ? failed : warned;
		parts.push(`2 checks ${same[0].outcome} (${same.map((c) => c.name).join(', ')})`);
	} else if (problems === 2) {
		parts.push(`${failed[0].name} failed: ${rows(failed[0])}`, `${warned[0].name} warned: ${rows(warned[0])}`);
	} else {
		if (failed.length) parts.push(`${failed.length} failed`);
		if (warned.length) parts.push(`${warned.length} warned`);
	}
	if (passed) parts.push(`${passed} passed`);
	return parts.join(' · ');
}
