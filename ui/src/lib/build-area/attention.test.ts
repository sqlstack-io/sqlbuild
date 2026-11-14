import { describe, expect, it } from 'vitest';
import type { AttentionItem, AttentionRun, PlatformEvent } from '$lib/api';
import { attentionViews } from './attention';

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
	it("lists an overdue schedule under its own location's outage, else under the stopped scheduler", () => {
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
			['event:stopped', ['daily is late']],
			['event:broken', ['hourly is late']]
		]);
	});

	it('keeps an overdue schedule as its own alert when nothing explains it', () => {
		const views = attentionViews([event('late', 'tick_overdue', 'shop'), event('broken', 'location_broken', 'marketing')], now);
		expect(views.map(({ view }) => view.key)).toEqual(['event:late', 'event:broken']);
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

	it('words a short heartbeat gap as a pause and a long one as an outage', () => {
		const [outage, pause] = attentionViews([unresponsive('short', 'a', 1), unresponsive('long', 'b', 6)], now).sort((x, y) => x.view.key.localeCompare(y.view.key));
		expect([outage.view.title, outage.view.tone]).toEqual(['b is unresponsive', 'warning']);
		expect([pause.view.title, pause.view.tone]).toEqual(['a missed a heartbeat', 'info']);
	});

	it('states how many nodes are out of date, counting a node both changed and behind once', () => {
		const detail = (changed: string[], behind: string[]) => attentionViews([{ kind: 'stale_models', changed, behind }], now)[0].view;
		expect(detail(['model:a', 'model:b'], ['model:b']).title).toBe('2 nodes out of date');
		expect(detail(['model:a', 'model:b'], ['model:b']).detail).toBe('2 changed since their last build, 1 of them also behind an upstream rebuild: a, b');
		expect(detail(['model:a', 'model:b'], ['model:b', 'model:c']).detail).toBe('2 changed since their last build, 1 of them also behind an upstream rebuild, and 1 more behind an upstream rebuild: a, b, c');
		expect(detail(['model:a'], ['model:c']).detail).toBe('1 changed since its last build and 1 behind an upstream rebuild: a, c');
		expect(detail(['model:a'], ['model:a']).detail).toBe('1 changed since its last build, also behind an upstream rebuild: a');
		expect(detail([], ['model:c', 'model:d']).detail).toBe('2 behind an upstream rebuild: c, d');
	});
});
