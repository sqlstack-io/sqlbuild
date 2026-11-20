// Where a plan's time went, for the plan page's "Planned in 10.6 s" popover. Pure.

import type { PlanDocument, PlanTimings } from '$lib/api';
import { plural } from '$lib/format/plural';
import { formatAgo } from '$lib/format/time';

export type TimingRow = { name: string; seconds: string; share: number; percent: string };

/** Seconds as the popover shows them: one decimal. */
export const seconds = (s: number) => s.toFixed(1);

/** The steps, longest first with `Other` last, each with its share of the total. */
export function timingRows(timings: PlanTimings): TimingRow[] {
	const total = timings.total_seconds || timings.steps.reduce((n, s) => n + s.seconds, 0);
	const other = timings.steps.filter((s) => s.name === 'Other');
	const steps = [...timings.steps.filter((s) => s.name !== 'Other').sort((a, b) => b.seconds - a.seconds), ...other];
	return steps.map((s) => {
		const share = total > 0 ? Math.min(1, s.seconds / total) : 0;
		return { name: s.name, seconds: seconds(s.seconds), share, percent: `${Math.round(share * 100)}%` };
	});
}

/** The popover's label: how long planning took, or how old the kept plan the location answered with is. */
export function timingTitle(timings: PlanTimings, now: number): string {
	return timings.reused_from ? `Reused plan from ${formatAgo(timings.reused_from, now)}` : `Planned in ${seconds(timings.total_seconds)} s`;
}

/** Where the plan ran and from what, with the nodes it planned and, for a kept plan, when it was made. */
export function timingFootnote(plan: PlanDocument, timings: PlanTimings, now: number): string {
	const from = timings.planned_from === 'live' ? 'On the location, live from the warehouse' : 'On the location, from the synced warehouse state';
	const reused = timings.reused_from ? ` · the location's plan from ${formatAgo(timings.reused_from, now)}; Plan again for a new one` : '';
	return `${from} · ${plural(plan.selected_count, 'node')}${reused}`;
}
