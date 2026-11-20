import { describe, expect, it } from 'vitest';
import type { PlanDocument, PlanTimings } from '$lib/api';
import { timingFootnote, timingRows, timingTitle } from './plan-timings';

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
		const now = Date.parse('2026-01-02T03:10:05Z');
		expect(timingFootnote(plan, timings, now)).toBe('On the location, from the synced warehouse state · 2,418 nodes');
		expect(timingFootnote(plan, { ...timings, planned_from: 'live' }, now)).toBe('On the location, live from the warehouse · 2,418 nodes');
	});

	it("labels a kept plan by its age instead of a planning time", () => {
		const plan = { selected_count: 2418 } as PlanDocument;
		const now = Date.parse('2026-01-02T03:10:05Z');
		const reused = { ...timings, reused_from: '2026-01-02T03:04:05Z' };
		expect([timingTitle(timings, now), timingTitle(reused, now)]).toEqual(['Planned in 10.0 s', 'Reused plan from 6m ago']);
		expect(timingFootnote(plan, reused, now)).toBe("On the location, from the synced warehouse state · 2,418 nodes · the location's plan from 6m ago; Plan again for a new one");
	});
});
