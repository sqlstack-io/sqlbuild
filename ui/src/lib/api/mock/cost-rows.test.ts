import { describe, expect, it } from 'vitest';
import { costFolder, costSelectionOf, costSeriesOf, type CostRowIn } from './cost-rows';

const DAY = 86_400_000;
const now = Date.parse('2026-11-23T12:00:00Z');
const row = (runId: string, job: string | null, nodeKey: string, daysAgo: number, credits: number, usd: number | null = credits * 3): CostRowIn => ({
	runId,
	job,
	nodeKey,
	folder: costFolder(`sql/models/${nodeKey === 'orders' ? 'staging/' : ''}${nodeKey}.sql`),
	startedAt: now - daysAgo * DAY,
	pending: false,
	credits,
	usd,
	seconds: 60,
	queries: 2,
	bytes: 10
});

describe('the mock’s cost of a selection', () => {
	it('splits job by node over whole days ending today, with the period before', () => {
		const read = costSelectionOf([row('a', 'daily', 'orders', 0, 0.1), row('a', 'daily', 'customers', 0, 0.05), row('b', null, 'orders', 1, 0.04), row('c', 'daily', 'orders', 9, 0.2)], 7, now);
		expect(read.dates).toHaveLength(7);
		expect(read.dates.at(-1)).toBe('2026-11-23');
		expect(read.facts.map((f) => [f.job, f.nodeKey, f.folder, f.credits, f.previousCredits])).toEqual([
			['daily', 'orders', 'staging', 0.1, 0.2],
			['daily', 'customers', '', 0.05, 0],
			[null, 'orders', 'staging', 0.04, 0]
		]);
		expect([read.runs, read.price?.source, read.price?.usdPerCredit]).toEqual([2, 'configured', 3]);
		expect(read.jobDaily.find((s) => s.key === null)?.credits[5]).toBe(0.04);
	});

	it('has no price once any row has none', () => {
		expect(costSelectionOf([row('a', 'daily', 'orders', 0, 0.1, null)], 7, now).price).toBeNull();
	});

	it('gives each group its credits per day', () => {
		const rows = [row('a', 'daily', 'orders', 0, 0.1), row('b', null, 'orders', 0, 0.04)];
		expect(costSeriesOf(rows, 7, now, [{ job: 'daily' }, { manual: true }, { folder: 'staging' }]).map((s) => s[6])).toEqual([0.1, 0.04, 0.14]);
	});
});
