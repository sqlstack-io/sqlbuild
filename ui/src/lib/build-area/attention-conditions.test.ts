import { describe, expect, it } from 'vitest';
import type { AttentionAcknowledgement, AttentionItem, PlatformEvent } from '$lib/api';
import { acknowledgementOf, conditionOf, withConditions } from './attention-conditions';
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
	return { key, shape, by: 'ada', at: iso(-hoursAgo), until: iso(24 - hoursAgo) };
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
		expect(stateOf(stale(2, 3), [acknowledged(stale(2, 3), 25)]).lapsed).toBe('its 24-hour acknowledgement ended');
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
