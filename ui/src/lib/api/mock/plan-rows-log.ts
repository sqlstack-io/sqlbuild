// The mock's record of the plan pages and diffs the page asked for, for the end-to-end tests: the mock
// makes no network requests, so this stands in for the server's plan reads. Read as
// `window.__sqbPlanRows` and `window.__sqbPlanDiffs`.

import type { PlanPageSection } from '../types';

export type PlanRowsCall = { section: PlanPageSection; offset: number; limit: number | undefined; name?: string };

type Log = { __sqbPlanRows?: PlanRowsCall[]; __sqbPlanDiffs?: string[] };
const log = () => (typeof window === 'undefined' ? null : (window as unknown as Log));

export function recordPlanRows(section: PlanPageSection, offset: number, limit: number | undefined, name?: string): void {
	const w = log();
	if (w) (w.__sqbPlanRows ??= []).push({ section, offset, limit, ...(name ? { name } : {}) });
}

export function recordPlanDiff(model: string): void {
	const w = log();
	if (w) (w.__sqbPlanDiffs ??= []).push(model);
}
