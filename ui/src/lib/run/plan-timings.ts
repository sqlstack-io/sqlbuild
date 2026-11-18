// Where a plan's time went, for the plan page's "Planned in 10.6 s" popover. Pure.

import type { PlanDocument, PlanTimings } from '$lib/api';
import { plural } from '$lib/format/plural';

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

/** Where the plan ran and from what, with the nodes it planned. */
export function timingFootnote(plan: PlanDocument, timings: PlanTimings): string {
	const from = timings.planned_from === 'live' ? 'On the location, live from the warehouse' : 'On the location, from the synced warehouse state';
	return `${from} · ${plural(plan.selected_count, 'node')}`;
}
