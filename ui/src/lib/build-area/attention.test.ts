import { describe, expect, it } from 'vitest';
import type { AttentionItem, AttentionRun, PlatformEvent } from '$lib/api';
import { attentionViews, staleSentence } from './attention';
import { CHANGED_PLAN_HREF, planHref } from '$lib/run/command';

const now = Date.parse('2026-01-01T12:00:00Z');
const ago = (minutes: number) => new Date(now - minutes * 60_000).toISOString();
const event = (id: string, kind: PlatformEvent['kind'], locationId: string | null, over: Partial<PlatformEvent> = {}): AttentionItem => ({
	kind: 'platform_event',
	event: { id, kind, severity: 'warning', occurredAt: ago(5), active: true, locationId, subject: null, message: `${kind} message`, ...over }
});
const run = (id: string, jobKey: string | null): AttentionRun => ({ id, jobKey, kind: 'job', status: 'unresponsive', statusReason: null, nodeCounts: { success: 0, warn: 0, failed: 0, skipped: 0, running: 1, pending: 0 }, startedAt: ago(30), endedAt: null });
const unresponsive = (id: string, jobKey: string | null, heardMinutesAgo: number): AttentionItem => ({ kind: 'run_unresponsive', run: run(id, jobKey), lastHeartbeatAt: ago(heardMinutesAgo), acknowledged: null });
const queued = (runId: string, jobKey: string | null, waits: Extract<AttentionItem, { kind: 'queue_blocked' }>['waits']): AttentionItem => ({ kind: 'queue_blocked', runId, jobKey, queuedAt: ago(20), waits });

describe('overview alerts', () => {
	it("lists an overdue schedule under its own location's outage, else under the stopped scheduler, a location that can't run first", () => {
		const views = attentionViews(
			[
				event('broken', 'location_broken', 'shop'),
				event('late-shop', 'tick_overdue', 'shop', { message: 'hourly is late' }),
				event('late-other', 'tick_overdue', 'marketing', { message: 'daily is late' }),
				event('stopped', 'scheduler_no_leader', null, { severity: 'critical' })
			],
			now
		);
		expect(views.map(({ view }) => [view.key, view.consequences.map((c) => c.text)])).toEqual([
			['event:broken', ['hourly is late']],
			['event:stopped', ['daily is late']]
		]);
	});

	it('keeps an overdue schedule as its own alert when its outage is acknowledged, so it isn’t hidden with it', () => {
		const broken = event('broken', 'location_broken', 'shop');
		const acknowledged = { ...broken, condition: { key: 'event:location_broken:shop:', shape: {}, acknowledged: { by: 'ada', at: ago(1), until: ago(-600) }, lapsed: null } } as AttentionItem;
		const views = attentionViews([acknowledged, event('late-shop', 'tick_overdue', 'shop', { message: 'hourly is late' })], now);
		expect(views.map(({ view }) => [view.key, view.consequences.length])).toEqual([
			['event:broken', 0],
			['event:late-shop', 0]
		]);
	});

	it('keeps an overdue schedule as its own alert when nothing explains it, after a location that can’t run', () => {
		const views = attentionViews([event('late', 'tick_overdue', 'shop'), event('broken', 'location_broken', 'marketing')], now);
		expect(views.map(({ view }) => [view.key, view.tone])).toEqual([
			['event:broken', 'error'],
			['event:late', 'warning']
		]);
	});

	it('lists a queued run under the unresponsive run of its job or the one it waits for, and the cause takes the more urgent tone', () => {
		const views = attentionViews(
			[
				unresponsive('r1', 'nightly', 1),
				queued('q1', 'nightly', []),
				unresponsive('r2', null, 1),
				queued('q2', 'exports', [{ kind: 'write_claim', runId: 'r2', nodeKey: 'model:orders', otherNodes: 0 }]),
				queued('q3', 'other', [])
			],
			now
		);
		expect(views.map(({ view }) => [view.key, view.tone, view.consequences.length])).toEqual([
			['unresponsive:r1', 'warning', 1],
			['unresponsive:r2', 'warning', 1],
			['queue:q3', 'warning', 0]
		]);
	});

	it('keeps a queued run as its own alert when the unresponsive run that holds it up is acknowledged', () => {
		const handled: AttentionItem = { ...unresponsive('r1', 'nightly', 1), acknowledged: { by: 'sam', at: ago(1) } } as AttentionItem;
		const views = attentionViews([handled, queued('q1', 'nightly', [])], now);
		expect(views.map(({ view }) => [view.key, view.consequences.length])).toEqual([
			['queue:q1', 0],
			['unresponsive:r1', 0]
		]);
	});

	it('words a short heartbeat gap as a pause and a long one as an outage', () => {
		const [outage, pause] = attentionViews([unresponsive('short', 'a', 1), unresponsive('long', 'b', 6)], now).sort((x, y) => x.view.key.localeCompare(y.view.key));
		expect([outage.view.title, outage.view.tone]).toEqual(['b is unresponsive', 'warning']);
		expect([pause.view.title, pause.view.tone]).toEqual(['a missed a heartbeat', 'info']);
	});

	it('offers one build: what changed when anything changed, else a rebuild of the nodes behind', () => {
		const build = (changed: string[], behind: string[]) => attentionViews([{ kind: 'stale_models', changed, behind }], now)[0].view.build;
		expect(build(['model:a'], ['model:a', 'model:b'])).toEqual({
			label: 'Build 1 changed',
			extra: null,
			tooltip: 'Plan a build of the 1 changed node and the downstream nodes it reaches. You review the plan before anything runs.',
			href: CHANGED_PLAN_HREF,
			variant: 'stale'
		});
		// `--changed` would build nothing when nodes are only behind, so the rebuild names them.
		expect(build([], ['model:b', 'model:c'])).toEqual({ label: 'Rebuild 2', extra: null, tooltip: null, href: planHref(['model:b', 'model:c']), variant: 'stale-outline' });
	});

	it('labels the build with the location\'s plan of what changed and how much it reaches downstream', () => {
		const [{ view }] = attentionViews([{ kind: 'stale_models', changed: ['model:a'], behind: ['model:b'] }], now, { generation: 2, changed: ['model:a', 'model:c'], downstream: ['model:b'], failing: null });
		expect([view.build?.label, view.build?.extra]).toEqual(['Build 2 changed', ' (+1 downstream)']);
	});

	it('states how many nodes are out of date, counting a node both changed and behind once', () => {
		const view = (changed: string[], behind: string[]) => attentionViews([{ kind: 'stale_models', changed, behind }], now)[0].view;
		expect(view(['model:a', 'model:b'], ['model:b']).title).toBe('2 nodes out of date');
		expect(view(['model:a', 'model:b'], ['model:b', 'model:c']).detail).toBe('2 changed since their last build, and 1 more is behind an upstream rebuild: a, b and c.');
	});
});

describe('the out-of-date sentence', () => {
	const keys = (prefix: string, n: number) => Array.from({ length: n }, (_, i) => `model:${prefix}${i + 1}`);

	it('reads as plain English, with counts that add up to the total and at most three names', () => {
		// 30 changed, 9 of them also behind, and 82 only behind: 112 in all.
		const changed = keys('orders_', 30);
		const behind = [...changed.slice(0, 9), ...keys('customers_', 82)];
		expect(staleSentence(changed, behind)).toBe(
			'30 changed since their last build, and 82 more are behind an upstream rebuild. Including orders_1, orders_2, orders_3 and 109 more.'
		);
	});

	it('drops the overlap: nodes changed and also behind count only as changed', () => {
		expect(staleSentence(['model:orders', 'model:customers'], ['model:customers'])).toBe('2 changed since their last build: orders and customers.');
		expect(staleSentence(['model:orders'], ['model:orders'])).toBe('1 changed since its last build: orders.');
	});

	it('handles singular and plural for each part', () => {
		expect(staleSentence(['model:orders'], ['model:customers'])).toBe('1 changed since its last build, and 1 more is behind an upstream rebuild: orders and customers.');
		expect(staleSentence(keys('orders_', 2), keys('customers_', 2))).toBe(
			'2 changed since their last build, and 2 more are behind an upstream rebuild. Including orders_1, orders_2, customers_1 and 1 more.'
		);
	});

	it('words only-changed and only-behind nodes naturally', () => {
		expect(staleSentence(keys('orders_', 5), [])).toBe('5 changed since their last build. Including orders_1, orders_2, orders_3 and 2 more.');
		expect(staleSentence([], ['model:inventory'])).toBe('1 is behind an upstream rebuild: inventory.');
		expect(staleSentence([], ['model:inventory', 'model:orders', 'model:customers'])).toBe('3 are behind an upstream rebuild: inventory, orders and customers.');
		expect(staleSentence([], keys('inventory_', 4))).toBe('4 are behind an upstream rebuild. Including inventory_1, inventory_2, inventory_3 and 1 more.');
	});
	it("names why a sensor fails when its location didn't answer, before the stuck event and the count", () => {
		const failing = { reason: "the location didn't answer within 60s", since: ago(30), failedTicks: 2, eventId: 'run:r-1', eventSummary: null };
		const [{ view }] = attentionViews([{ kind: 'sensor_failing', triggerId: 't-1', triggerName: 'orders_sensor', failing }], now);
		expect(view.detail).toBe("the location didn't answer within 60s · Stuck on run:r-1 · 2 failed ticks");
	});
});
