// The mock's record of the plan pages the page asked for, for the end-to-end tests: the mock makes no
// network requests, so this stands in for the server's plan rows reads. Read as `window.__sqbPlanRows`.

import type { PlanPageSection } from '../types';

export type PlanRowsCall = { section: PlanPageSection; offset: number; limit: number | undefined };

export function recordPlanRows(section: PlanPageSection, offset: number, limit: number | undefined): void {
	if (typeof window === 'undefined') return;
	const w = window as unknown as { __sqbPlanRows?: PlanRowsCall[] };
	(w.__sqbPlanRows ??= []).push({ section, offset, limit });
}
