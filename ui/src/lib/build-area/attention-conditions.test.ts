import { describe, expect, it } from 'vitest';
import type { AttentionAcknowledgement, AttentionItem, CatalogNode, PlatformEvent, StaleReason } from '$lib/api';
import { overviewOf } from '$lib/api/http-overview';
import { acknowledgementOf, clearedWithdrawalId, conditionOf, judgedRead, snoozeWords, withConditions, withdrawalsDue } from './attention-conditions';
import { toPlatformEvents } from '$lib/api/http-problems';

const now = Date.parse('2026-11-18T12:00:00Z');
const iso = (hoursFromNow: number) => new Date(now + hoursFromNow * 3_600_000).toISOString();
const event = (extra: Partial<PlatformEvent> = {}): PlatformEvent => ({
	id: 'ev-1',
	kind: 'tick_overdue',
	severity: 'warning',
	occurredAt: iso(-1),
	active: true,
	locationId: 'loc-1',
	subject: 'segments_hourly',
	message: 'The segments_hourly schedule should have started a run 12 minutes ago.',
	...extra
});
const stale = (changed: number, behind: number): AttentionItem => ({
	kind: 'stale_models',
	changed: Array.from({ length: changed }, (_, i) => `model:c${i}`),
	behind: Array.from({ length: behind }, (_, i) => `model:b${i}`)
});
const source = (key: string, status: 'warn' | 'error') => ({ key, status, loadedAt: iso(-2), checkedAt: iso(0), warnAfterSeconds: 1800, errorAfterSeconds: 7200 });
/** The acknowledgement of `item` as it looks now, made `hoursAgo` ago. */
const acknowledged = (item: AttentionItem, hoursAgo = 1): AttentionAcknowledgement => {
	const { key, shape } = conditionOf(item)!;
	return { eventId: `ack-${key}-${hoursAgo}`, key, shape, by: 'ada', at: iso(-hoursAgo), until: iso(24 - hoursAgo) };
};
const stateOf = (item: AttentionItem, acks: AttentionAcknowledgement[], at = now) => {
	const [shown] = withConditions([item], acks, at);
	const condition = 'condition' in shown ? shown.condition : undefined;
	return { acknowledged: !!acknowledgementOf(shown), lapsed: condition?.lapsed?.reason ?? null };
};

describe('acknowledging a condition until it changes', () => {
	it('keys a condition by its kind and subject, and leaves runs to their own acknowledgement', () => {
		expect(conditionOf(stale(2, 3))).toEqual({ key: 'stale_models', noun: 'node', shape: { counts: { changed: 2, behind: 3 } } });
		expect(conditionOf({ kind: 'platform_event', event: event() })?.key).toBe('event:tick_overdue:loc-1:segments_hourly');
		expect(conditionOf({ kind: 'run_problem', run: { id: 'r' } as never, acknowledged: null })).toBeNull();
	});

	it('holds while the condition is the same or better', () => {
		const ack = acknowledged(stale(2, 3));
		expect(stateOf(stale(2, 3), [ack])).toEqual({ acknowledged: true, lapsed: null });
		expect(stateOf(stale(1, 0), [ack])).toEqual({ acknowledged: true, lapsed: null });
	});

	it('comes back, saying why, when it gets worse', () => {
		expect(stateOf(stale(2, 5), [acknowledged(stale(2, 3))])).toEqual({ acknowledged: false, lapsed: '2 more nodes behind since it was acknowledged' });
		const sources: AttentionItem = { kind: 'stale_sources', sources: [source('raw__orders', 'warn')] };
		const escalated: AttentionItem = { kind: 'stale_sources', sources: [source('raw__orders', 'error')] };
		expect(stateOf(escalated, [acknowledged(sources)]).lapsed).toBe('1 more source past their error threshold since it was acknowledged');
		expect(stateOf({ kind: 'platform_event', event: event({ severity: 'critical' }) }, [acknowledged({ kind: 'platform_event', event: event() })]).lapsed).toBe('it became more severe');
	});

	it('ignores a changed message, which can hold relative times', () => {
		const before: AttentionItem = { kind: 'platform_event', event: event() };
		const later: AttentionItem = { kind: 'platform_event', event: event({ message: 'The segments_hourly schedule should have started a run 40 minutes ago.' }) };
		expect(stateOf(later, [acknowledged(before)])).toEqual({ acknowledged: true, lapsed: null });
	});

	it('comes back when it resolved and recurred, or after 24 hours', () => {
		const before: AttentionItem = { kind: 'platform_event', event: event() };
		expect(stateOf({ kind: 'platform_event', event: event({ id: 'ev-2' }) }, [acknowledged(before)]).lapsed).toBe('it resolved and came back');
		expect(stateOf(stale(2, 3), [acknowledged(stale(2, 3), 25)]).lapsed).toBe('the time it was acknowledged for ended');
	});

	it('keeps large shapes small: many sources compare by their counts', () => {
		const many: AttentionItem = { kind: 'stale_sources', sources: Array.from({ length: 80 }, (_, i) => source(`raw__s${i}`, 'warn')) };
		expect(conditionOf(many)?.shape).toEqual({ counts: { warn: 80, error: 0 } });
		expect(JSON.stringify(conditionOf(stale(10_000, 10_000))!.shape).length).toBeLessThan(100);
	});
});

describe('server problems, whose event IDs are fixed per problem', () => {
	type Problems = Parameters<typeof toPlatformEvents>[0];
	const place = { location_id: '0190a000-0000-7000-8000-000000000001', location_name: 'shop', target: 'prod' };
	const tick = (trigger: string) => ({ ...place, trigger_id: trigger, name: trigger, scheduled_for: iso(-1) });
	const problems = (over: Partial<Problems>): Problems => ({ scheduler_leader: true, scheduler_lease_expires_at: null, broken_locations: [], overdue_ticks: [], low_calendars: [], ...over }) as Problems;
	const item = (over: Partial<Problems>): AttentionItem => ({ kind: 'platform_event', event: toPlatformEvents(problems(over), now)[0] });

	it('comes back when another schedule is late, though the event ID stays the same', () => {
		const one = item({ overdue_ticks: [tick('t-hourly')] });
		const two = item({ overdue_ticks: [tick('t-hourly'), tick('t-daily')] });
		expect((one as { event: PlatformEvent }).event.id).toBe((two as { event: PlatformEvent }).event.id);
		expect(stateOf(one, [acknowledged(one)])).toEqual({ acknowledged: true, lapsed: null });
		expect(stateOf(two, [acknowledged(one)]).lapsed).toBe('1 more schedule late since it was acknowledged');
		// The same schedules late again, at a later time: still the same condition.
		expect(stateOf(item({ overdue_ticks: [{ ...tick('t-hourly'), scheduled_for: iso(0) }] }), [acknowledged(one)]).acknowledged).toBe(true);
	});

	it('comes back when another calendar runs low', () => {
		const cal = (trigger: string) => ({ ...place, trigger_id: trigger, name: trigger, horizon_end: iso(5), refresh_at: iso(1) });
		const one = item({ low_calendars: [cal('c-1')] });
		expect(stateOf(item({ low_calendars: [cal('c-1'), cal('c-2')] }), [acknowledged(one)]).lapsed).toBe('1 more calendar running out of dates since it was acknowledged');
	});

	it('comes back when another stored event is skipped', () => {
		const skip = (storage_id: number) => ({ ...place, storage_id, event_id: `e${storage_id}`, event_type: 'platform_run_failed', skipped_at: iso(-1), reason: 'missing field `trigger`' });
		const one = item({ skipped_events: [skip(3)] });
		expect(stateOf(item({ skipped_events: [skip(3)] }), [acknowledged(one)]).acknowledged).toBe(true);
		expect(stateOf(item({ skipped_events: [skip(3), skip(5)] }), [acknowledged(one)]).lapsed).toBe('1 more stored event skipped since it was acknowledged');
	});

	it('tells one scheduler or location outage from the next', () => {
		const outage = item({ scheduler_leader: false, scheduler_lease_expires_at: iso(-3) });
		expect(stateOf(item({ scheduler_leader: false, scheduler_lease_expires_at: iso(-3) }), [acknowledged(outage)]).acknowledged).toBe(true);
		expect(stateOf(item({ scheduler_leader: false, scheduler_lease_expires_at: iso(-1) }), [acknowledged(outage)]).lapsed).toBe('it resolved and came back');
		const broken = (seen: number) => item({ broken_locations: [{ ...place, last_seen_at: iso(seen) }] as Problems['broken_locations'] });
		expect(stateOf(broken(-2), [acknowledged(broken(-2))]).acknowledged).toBe(true);
		expect(stateOf(broken(-1), [acknowledged(broken(-2))]).lapsed).toBe('it resolved and came back');
	});
});

describe('the server’s limits', () => {
	it('cuts a long key between characters, never inside one', () => {
		const key = conditionOf({ kind: 'platform_event', event: event({ subject: '🦆'.repeat(300) }) })!.key;
		expect(Array.from(key).length).toBeLessThanOrEqual(256);
		expect(key).not.toMatch(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/);
	});

	it('keeps keys to 256 characters, distinct for distinct long subjects', () => {
		const long = (name: string) => conditionOf({ kind: 'platform_event', event: event({ subject: `${name.repeat(300)} · prod` }) })!.key;
		expect(long('a').length).toBeLessThanOrEqual(256);
		expect(long('a')).not.toBe(long('b'));
		expect(conditionOf({ kind: 'platform_event', event: event() })!.key).toBe('event:tick_overdue:loc-1:segments_hourly');
	});

	it('keeps shapes under 8 KiB, comparing by counts when members would not fit', () => {
		const sources: AttentionItem = { kind: 'stale_sources', sources: Array.from({ length: 50 }, (_, i) => source(`raw__${'x'.repeat(240)}_${i}`, 'warn')) };
		const shape = conditionOf(sources)!.shape;
		expect(shape).toEqual({ counts: { warn: 50, error: 0 } });
		expect(JSON.stringify(shape).length).toBeLessThan(8192);
	});
});

describe('acknowledgements of conditions that cleared', () => {
	const ack = (key: string, shape = {}): AttentionAcknowledgement => ({ eventId: `ack-${key}`, key, shape, by: 'ada', at: iso(-2), until: iso(22) });
	const built = (stale: StaleReason[] | null): CatalogNode => ({ key: 'model:orders', kind: 'table', path: 'models/orders.sql', group: null, upstream: [], stale, lastOutcome: 'success', lastBuild: { builtAt: iso(-1) } }) as unknown as CatalogNode;
	const changed: StaleReason = { reason: 'changed', change: 'definition', detail: '', since: iso(-3) };
	const read = (nodes: CatalogNode[], acks: AttentionAcknowledgement[]) =>
		overviewOf({ runs: [], checks: [], nodes, changes: [], events: [], triggers: [], acknowledgements: new Map(), conditions: { snoozeSeconds: 86_400, acknowledgements: acks }, now, scopeKey: 'loc-1:dev' });

	it('lists one whose condition the read can see and no longer finds', () => {
		const nodesOutOfDate = ack('stale_models', { counts: { changed: 2, behind: 0 } });
		expect(read([built([])], [nodesOutOfDate]).clearedAcknowledgements).toEqual([nodesOutOfDate]);
		expect(read([built([changed])], [nodesOutOfDate]).clearedAcknowledgements).toEqual([]);
	});

	it('never lists one the read cannot see: unknown build state, or late sources, which it does not check', () => {
		expect(read([built(null)], [ack('stale_models')]).clearedAcknowledgements).toEqual([]);
		expect(read([built([])], [ack('stale_sources')]).clearedAcknowledgements).toEqual([]);
		expect(read([built([])], [ack('event:tick_overdue:loc-1:dev:shop · dev')]).clearedAcknowledgements).toEqual([ack('event:tick_overdue:loc-1:dev:shop · dev')]);
	});

	it('judges another target’s problems never, as a viewer may not be allowed to see them; the scheduler always', () => {
		// The server lists a broken location, late schedules and low calendars only to people who may view
		// that target, so their absence from this read says nothing about them.
		const elsewhere = [ack('event:location_broken:loc-2:prod:billing · prod'), ack('event:tick_overdue:loc-1:prod:shop · prod'), ack('event:calendar_horizon_low:loc-2:dev:billing · dev')];
		expect(read([built([])], elsewhere).clearedAcknowledgements).toEqual([]);
		expect(read([built([])], [ack('event:scheduler_no_leader::')]).clearedAcknowledgements).toEqual([ack('event:scheduler_no_leader::')]);
		// Skipped stored events without a target are listed to everyone; another target's are not.
		expect(read([built([])], [ack('event:unreadable_event_skipped::')]).clearedAcknowledgements).toEqual([ack('event:unreadable_event_skipped::')]);
		expect(read([built([])], [ack('event:unreadable_event_skipped:loc-2:prod:billing · prod')]).clearedAcknowledgements).toEqual([]);
		expect(read([built([])], [ack('event:unreadable_event_skipped:loc-1:dev:shop · dev')]).clearedAcknowledgements).toHaveLength(1);
		expect(read([built([])], [ack('event:location_broken:loc-1:dev:shop · dev')]).clearedAcknowledgements).toHaveLength(1);
	});

	it('withdraws missing secrets only when the health read succeeded and shows none for this location', () => {
		const secrets = ack('event:location_missing_secrets:loc-1:dev:');
		const healthy = (acks: AttentionAcknowledgement[], events: PlatformEvent[] = []) =>
			overviewOf({ runs: [], checks: [], nodes: [built([])], changes: [], events, triggers: [], acknowledgements: new Map(), conditions: { snoozeSeconds: 86_400, acknowledgements: acks }, now, scopeKey: 'loc-1:dev', healthRead: true });
		const stillMissing: PlatformEvent = { id: 'secrets:loc-1:dev', kind: 'location_missing_secrets', severity: 'warning', occurredAt: iso(-1), active: true, locationId: 'loc-1:dev', subject: null, message: 'orders_api is missing ORDERS_API_KEY', members: ['ORDERS_API_KEY'] };
		expect(read([built([])], [secrets]).clearedAcknowledgements).toEqual([]);
		expect(healthy([secrets]).clearedAcknowledgements).toEqual([secrets]);
		expect(healthy([secrets], [stillMissing]).clearedAcknowledgements).toEqual([]);
		expect(healthy([ack('event:location_missing_secrets:loc-2:dev:')]).clearedAcknowledgements).toEqual([]);
	});

	it('judges only the page’s own latest read, for the target now selected, shown and not failed', () => {
		// The page shows its data through a proxy, so the shown read is a copy carrying the read's ID.
		const a = { ...read([built([])], [ack('stale_models')]), readId: 'read-a' };
		const b = { ...read([built([])], []), readId: 'read-b' };
		const readA = { overview: a, readId: 'read-a', scopeKey: 'loc-1:dev', nodesSettled: true };
		expect(judgedRead(readA, { shown: { ...a }, scopeKey: 'loc-1:dev', failed: false })).toBe(readA);
		expect(judgedRead(readA, { shown: a, scopeKey: 'loc-1:dev', failed: false })).toBe(readA);
		// Switched to target B: A's data is still shown until B's read lands, and A's withdrawal waits on it.
		expect(judgedRead(readA, { shown: a, scopeKey: 'loc-2:dev', failed: false })).toBeNull();
		// Back on A, its remembered data shows before a fresh read: never judged.
		expect(judgedRead({ overview: b, readId: 'read-b', scopeKey: 'loc-2:dev', nodesSettled: true }, { shown: a, scopeKey: 'loc-1:dev', failed: false })).toBeNull();
		// Remembered from an earlier visit: a read of the same target, but not this page's latest.
		expect(judgedRead({ ...readA, readId: 'read-a2' }, { shown: a, scopeKey: 'loc-1:dev', failed: false })).toBeNull();
		expect(judgedRead(readA, { shown: a, scopeKey: 'loc-1:dev', failed: true })).toBeNull();
		expect(judgedRead(null, { shown: a, scopeKey: 'loc-1:dev', failed: false })).toBeNull();
	});

	it('judges nodes out of date only from a read that began after node state settled', () => {
		const a = { ...read([built([])], [ack('stale_models'), ack('event:scheduler_no_leader::')]), readId: 'read-a' };
		// Read during a sync: even once the sync has finished, that read's nodes are not judged; a new read is needed.
		const duringSync = judgedRead({ overview: a, readId: 'read-a', scopeKey: 'loc-1:dev', nodesSettled: false }, { shown: a, scopeKey: 'loc-1:dev', failed: false })!;
		expect(withdrawalsDue(duringSync.overview.clearedAcknowledgements, { failed: false, syncing: !duringSync.nodesSettled }).map((x) => x.key)).toEqual(['event:scheduler_no_leader::']);
		const afterSync = judgedRead({ overview: a, readId: 'read-a', scopeKey: 'loc-1:dev', nodesSettled: true }, { shown: a, scopeKey: 'loc-1:dev', failed: false })!;
		expect(withdrawalsDue(afterSync.overview.clearedAcknowledgements, { failed: false, syncing: !afterSync.nodesSettled }).map((x) => x.key)).toEqual(['stale_models', 'event:scheduler_no_leader::']);
	});

	it('withdraws nothing after a failed read, and nodes out of date only once node state has settled', () => {
		const cleared = [ack('stale_models'), ack('event:location_broken:loc-1:')];
		expect(withdrawalsDue(cleared, { failed: true, syncing: false })).toEqual([]);
		expect(withdrawalsDue(cleared, { failed: false, syncing: true }).map((a) => a.key)).toEqual(['event:location_broken:loc-1:']);
		expect(withdrawalsDue(cleared, { failed: false, syncing: false })).toEqual(cleared);
	});

	it('withdraws each acknowledgement with one event ID every viewer agrees on, derived from that acknowledgement', () => {
		const id = clearedWithdrawalId(ack('stale_models'));
		expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-8[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
		expect(clearedWithdrawalId(ack('stale_models'))).toBe(id);
		expect(clearedWithdrawalId({ ...ack('stale_models'), eventId: 'a-later-acknowledgement' })).not.toBe(id);
	});

	it('shows a condition that comes back after it was withdrawn as new, even with a smaller count', () => {
		const before = stale(5, 0);
		const withdrawnThenBack = stale(2, 0);
		// While acknowledged, a smaller count stays hidden.
		expect(stateOf(withdrawnThenBack, [acknowledged(before)])).toEqual({ acknowledged: true, lapsed: null });
		// Once the cleared condition's acknowledgement is withdrawn, the listing no longer holds it.
		expect(stateOf(withdrawnThenBack, [])).toEqual({ acknowledged: false, lapsed: null });
	});
});

describe('the configured snooze', () => {
	it('never lapses on time without a time limit', () => {
		const ack = { ...acknowledged(stale(2, 3), 1000), until: null };
		expect(stateOf(stale(2, 3), [ack])).toEqual({ acknowledged: true, lapsed: null });
		expect(stateOf(stale(2, 4), [ack]).lapsed).toBe('1 more node behind since it was acknowledged');
	});

	it('lapses after the configured time', () => {
		expect(stateOf(stale(2, 3), [{ ...acknowledged(stale(2, 3), 2), until: iso(-1) }]).lapsed).toBe('the time it was acknowledged for ended');
	});

	it('is worded in plain units', () => {
		expect([86_400, 5_400, 129_600, 1, 172_800].map(snoozeWords)).toEqual(['1 day', '1 hour and 30 minutes', '1 day and 12 hours', '1 second', '2 days']);
	});
});
