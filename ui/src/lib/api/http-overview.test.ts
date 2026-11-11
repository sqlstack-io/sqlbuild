import { describe, expect, it } from 'vitest';
import { overviewOf } from './http-overview';
import type { CatalogNode, Check, NodeChange, PlatformEvent, Run, SnapshotChange } from './types';

const now = Date.parse('2026-11-09T12:00:00Z');
const hoursAgo = (h: number) => new Date(now - h * 3_600_000).toISOString();

function run(id: string, status: Run['status'], queuedHoursAgo: number, endedHoursAgo: number | null, jobKey: string | null = 'orders', parentRunId: string | null = null): Run {
	return { id, status, parentRunId, jobKey, kind: jobKey ? 'job' : 'adhoc', statusReason: null, queuedAt: hoursAgo(queuedHoursAgo), startedAt: hoursAgo(queuedHoursAgo), endedAt: endedHoursAgo === null ? null : hoursAgo(endedHoursAgo) } as Run;
}

const node = (key: string, kind: CatalogNode['kind']) => ({ key, kind }) as CatalogNode;
const check = (id: string, outcome: string | null) => ({ id, latest: outcome ? { outcome } : null }) as unknown as Check;
const change = (publishedHoursAgo: number, nodes: [NodeChange['op'], string][]) =>
	({ snapshotId: `s-${publishedHoursAgo}`, publishedAt: hoursAgo(publishedHoursAgo), nodes: nodes.map(([op, key]) => ({ key, op })) }) as SnapshotChange;
const event = (id: string, active: boolean) => ({ id, active }) as PlatformEvent;

describe('overview from live reads', () => {
	// Newest queued first, as the runs listing returns them.
	const runs = [
		run('active', 'running', 1, null),
		run('child-failed', 'failed', 2, 2, 'orders', 'group'),
		run('adhoc-new', 'failed', 2.2, 2.1, null),
		run('orders-new', 'failed', 3, 2.5),
		run('fulfilment-failed', 'failed', 4, 3.5, 'fulfilment'),
		run('catalogue-partial', 'partial', 5, 4.5, 'catalogue'),
		run('orders-old', 'failed', 6, 5.5),
		// Queued before the fulfilment failure but ended after it, so fulfilment has recovered.
		run('fulfilment-ok', 'success', 7, 0.5, 'fulfilment'),
		run('presumed', 'presumed_failed', 8, 1.5, 'inventory'),
		run('stuck', 'unresponsive', 8, null, 'returns'),
		run('adhoc-old', 'failed', 9, 8, null),
		run('warn', 'warn', 23.5, 23, 'support'),
		run('yesterday-failed', 'failed', 25, 24.5, 'archive')
	];
	const overview = overviewOf({
		runs,
		checks: [check('a', 'pass'), check('b', 'fail'), check('c', 'error'), check('d', 'warn'), check('e', null)],
		nodes: [node('model:orders', 'table'), node('model:totals', 'view'), node('model:events', 'incremental'), node('source:raw', 'source'), node('seed:regions', 'seed')],
		changes: [change(1, [['added', 'model:orders'], ['added', 'seed:regions'], ['removed', 'model:old']]), change(24 * 8, [['added', 'model:totals']])],
		events: [event('scheduler', true), event('cleared', false)],
		acknowledgements: new Map([['orders-new', { by: 'ada', at: hoursAgo(1) }]]),
		now
	});

	it('counts top-level runs queued in the last 24 hours', () => {
		expect(overview.metrics.runs24h).toEqual({ total: 11, success: 1, warn: 1, partial: 1, failed: 6, active: 2 });
	});

	it('lists each job’s latest run once when it ended today without succeeding, newest first, then unresponsive runs and active platform events', () => {
		// Acknowledging the orders failure keeps its place and never brings the older orders failure back.
		expect(overview.attention.map((i) => (i.kind === 'platform_event' ? `event:${i.event.id}` : 'run' in i ? `${i.kind}:${i.run.id}:${i.acknowledged?.by ?? '-'}` : i.kind))).toEqual([
			'run_problem:presumed:-',
			'run_problem:adhoc-new:-',
			'run_problem:orders-new:ada',
			'run_problem:catalogue-partial:-',
			'run_unresponsive:stuck:-',
			'event:scheduler'
		]);
	});

	it('counts models and this week’s model changes only', () => {
		expect(overview.metrics.models).toEqual({ total: 3, addedThisWeek: 1, removedThisWeek: 1 });
	});

	it('counts check outcomes, with errors as failing', () => {
		expect(overview.metrics.checks).toEqual({ total: 5, passing: 1, warning: 1, failing: 2 });
	});

	it('names the run that ended last, not the newest queued', () => {
		expect(overview.latestRun?.id).toBe('fulfilment-ok');
	});

	it('leaves parts without a server read empty instead of zero', () => {
		expect([overview.metrics.stale, overview.metrics.cost, overview.groups]).toEqual([null, null, null]);
	});
});
