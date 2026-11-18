import { describe, expect, it } from 'vitest';
import type { PlanDocument, PlanTimings } from '$lib/api';
import { timingFootnote, timingRows } from './plan-timings';

const timings: PlanTimings = {
	total_seconds: 10,
	steps: [
		{ name: 'Writing the plan', seconds: 0.8 },
		{ name: 'Other', seconds: 0.4 },
		{ name: 'Reading warehouse state', seconds: 3.1 },
		{ name: 'Detecting changes', seconds: 1.75 }
	],
	planned_from: 'synced_state'
};

describe('plan timings', () => {
	it('lists the steps longest first, Other last, each with its share of the total', () => {
		expect(timingRows(timings).map((r) => [r.name, r.seconds, r.percent])).toEqual([
			['Reading warehouse state', '3.1', '31%'],
			['Detecting changes', '1.8', '18%'],
			['Writing the plan', '0.8', '8%'],
			['Other', '0.4', '4%']
		]);
	});

	it('says where the plan ran and how many nodes it planned', () => {
		const plan = { selected_count: 2418 } as PlanDocument;
		expect(timingFootnote(plan, timings)).toBe('On the location, from the synced warehouse state · 2,418 nodes');
		expect(timingFootnote(plan, { ...timings, planned_from: 'live' })).toBe('On the location, live from the warehouse · 2,418 nodes');
	});
});
