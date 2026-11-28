import type { components } from '$control/schema.d.ts';
import { formatClock, formatStamp } from '$lib/format/time';
import { locationKey as keyOf } from './http-client';
import type { PlatformEvent } from './types';
import type { PlatformEventRecord, PlatformHealth } from './types/platform';

type Wire = components['schemas'];

const placeOf = (name: string, target: string) => `${name} · ${target}`;
/** Dropped calendar times a message names before `and N more`. */
const DROPPED_TIMES_NAMED = 10;

/** What failed, by state, for a failing location's banner. */
const FAILED: Record<string, string> = {
	falling_back: 'latest code failed to load',
	change_plan_failed: 'planning what changed failed',
	discovery_failed: 'a discover function failed',
	degraded: 'the warehouse connection check failed'
};

/** `place: what failed: error. Serving generation N, published at T.`, or what it serves before any publication. */
function failingMessage(place: string, location: Wire['FailingLocation']): string {
	const error = location.error ? `: ${location.error}` : '';
	const serving = location.serving
		? `Serving generation ${location.serving.generation}, published ${formatStamp(location.serving.published_at)}.`
		: 'No snapshot published yet.';
	return `${place}: ${FAILED[location.state] ?? location.state}${error}. ${serving}`;
}

/** Live problems as active banner events: a stopped scheduler, each broken location, and late schedules, low calendars, skipped triggers, sensors checked again from scratch, calendars' dropped times and skipped stored events per location. */
export function toPlatformEvents(problems: Wire['PlatformProblems'], now: number): PlatformEvent[] {
	const events: PlatformEvent[] = [];
	if (!problems.scheduler_leader) {
		events.push({
			id: 'scheduler',
			kind: 'scheduler_no_leader',
			severity: 'critical',
			occurredAt: problems.scheduler_lease_expires_at ?? new Date(now).toISOString(),
			// The lease that ran out tells one leaderless outage from the next.
			occurrence: problems.scheduler_lease_expires_at ?? undefined,
			active: true,
			locationId: null,
			subject: null,
			message: 'No server is running the scheduler, so schedules and sensors aren’t starting runs.'
		});
	}
	for (const location of problems.broken_locations) {
		events.push({
			id: `broken:${keyOf(location.location_id, location.target)}`,
			kind: 'location_broken',
			severity: 'critical',
			occurredAt: location.last_seen_at,
			// Its last check-in tells one outage from the next.
			occurrence: location.last_seen_at,
			active: true,
			locationId: keyOf(location.location_id, location.target),
			subject: placeOf(location.location_name, location.target),
			message: `${placeOf(location.location_name, location.target)} stopped checking in, so its scheduled runs aren’t starting.`
		});
	}
	for (const location of problems.failing_locations ?? []) {
		const place = placeOf(location.location_name, location.target);
		events.push({
			id: `failing:${keyOf(location.location_id, location.target)}`,
			kind: 'location_failing',
			severity: location.state === 'degraded' ? 'warning' : 'critical',
			occurredAt: location.failed_at,
			// The failure's report time tells one failure from the next.
			occurrence: location.failed_at,
			active: true,
			locationId: keyOf(location.location_id, location.target),
			subject: place,
			message: failingMessage(place, location)
		});
	}
	for (const sync of problems.failing_syncs ?? []) {
		const place = placeOf(sync.location_name, sync.target);
		events.push({
			id: `sync:${keyOf(sync.location_id, sync.target)}`,
			kind: 'warehouse_sync_failed',
			severity: 'warning',
			occurredAt: sync.since,
			// The first failure in a row tells one failing streak from the next.
			occurrence: sync.since,
			active: true,
			locationId: keyOf(sync.location_id, sync.target),
			subject: place,
			message: syncMessage(place, sync, now),
			sync: { halted: sync.halted, unreachable: sync.unreachable, retrySeconds: sync.retry_seconds ?? null, retryAt: sync.retry_at ?? null, lastSuccessAt: sync.last_success_at ?? null }
		});
	}
	for (const sink of problems.failing_sinks ?? []) {
		const place = placeOf(sink.location_name, sink.target);
		events.push({
			id: `sink:${keyOf(sink.location_id, sink.target)}:${sink.sink}`,
			kind: 'event_sink_failed',
			severity: 'warning',
			occurredAt: sink.since,
			// The first lossy run in a row tells one failing streak from the next.
			occurrence: sink.since,
			active: true,
			locationId: keyOf(sink.location_id, sink.target),
			subject: sink.sink,
			message: `${place}: event sink ${sink.sink} losing events since ${formatClock(sink.since, now)}: ${sink.reason}.`
		});
	}
	for (const forwarding of problems.failing_forwarding ?? []) {
		events.push({
			id: `forwarding:${forwarding.name}`,
			kind: 'event_forwarding_failed',
			severity: 'warning',
			occurredAt: forwarding.since,
			occurrence: forwarding.since,
			active: true,
			locationId: null,
			subject: forwarding.name,
			message: `${forwarding.name} failing since ${formatClock(forwarding.since, now)}: ${forwarding.reason}.`
		});
	}
	const late = new Map<string, Wire['OverdueTick'][]>();
	for (const tick of problems.overdue_ticks) {
		const key = keyOf(tick.location_id, tick.target);
		late.set(key, [...(late.get(key) ?? []), tick]);
	}
	for (const [key, ticks] of late) {
		const oldest = ticks.reduce((a, b) => (a.scheduled_for <= b.scheduled_for ? a : b));
		const place = placeOf(oldest.location_name, oldest.target);
		events.push({
			id: `late:${key}`,
			kind: 'tick_overdue',
			severity: 'warning',
			occurredAt: oldest.scheduled_for,
			members: [...new Set(ticks.map((t) => t.trigger_id))].sort(),
			active: true,
			locationId: key,
			subject: place,
			message: ticks.length === 1 ? `${oldest.name} in ${place} is late.` : `${ticks.length} schedules in ${place} are late, starting with ${oldest.name}.`
		});
	}
	const low = new Map<string, Wire['LowCalendar'][]>();
	for (const calendar of problems.low_calendars ?? []) {
		const key = keyOf(calendar.location_id, calendar.target);
		low.set(key, [...(low.get(key) ?? []), calendar]);
	}
	for (const [key, calendars] of low) {
		const first = calendars[0];
		const place = placeOf(first.location_name, first.target);
		const ends = (c: Wire['LowCalendar']) => c.horizon_end ?? c.refresh_at;
		const soonest = calendars.reduce((a, b) => (ends(a) <= ends(b) ? a : b));
		events.push({
			id: `calendar:${key}`,
			kind: 'calendar_horizon_low',
			severity: 'warning',
			occurredAt: ends(soonest),
			members: [...new Set(calendars.map((c) => c.trigger_id))].sort(),
			active: true,
			locationId: key,
			subject: place,
			message:
				calendars.length === 1
					? `${soonest.name} in ${place} ${soonest.horizon_end ? 'is running out of dates' : 'has no dates left'}; the location works out more at its next refresh.`
					: `${calendars.length} calendars in ${place} are running out of dates, starting with ${soonest.name}.`
		});
	}
	const skipped = new Map<string, Wire['SkippedTrigger'][]>();
	for (const trigger of problems.skipped_triggers ?? []) {
		const key = keyOf(trigger.location_id, trigger.target);
		skipped.set(key, [...(skipped.get(key) ?? []), trigger]);
	}
	for (const [key, triggers] of skipped) {
		const first = triggers.reduce((a, b) => (a.skipped_at <= b.skipped_at ? a : b));
		const place = placeOf(first.location_name, first.target);
		events.push({
			id: `skipped:${key}`,
			kind: 'trigger_skipped',
			severity: 'warning',
			occurredAt: first.skipped_at,
			members: [...new Set(triggers.map((t) => t.trigger_id))].sort(),
			active: true,
			locationId: key,
			subject: place,
			message:
				triggers.length === 1
					? `${first.name} in ${place}: ${first.reason}.`
					: `${triggers.length} triggers in ${place} need attention, starting with ${first.name}: ${first.reason}.`
		});
	}
	const rechecked = new Map<string, Wire['RecheckedSensor'][]>();
	for (const sensor of problems.rechecked_sensors ?? []) {
		const key = keyOf(sensor.location_id, sensor.target);
		rechecked.set(key, [...(rechecked.get(key) ?? []), sensor]);
	}
	for (const [key, sensors] of rechecked) {
		const first = sensors.reduce((a, b) => (a.rechecked_at <= b.rechecked_at ? a : b));
		const place = placeOf(first.location_name, first.target);
		events.push({
			id: `rechecked:${key}`,
			kind: 'sensor_rechecked',
			severity: 'info',
			occurredAt: first.rechecked_at,
			members: [...new Set(sensors.map((s) => s.trigger_id))].sort(),
			active: true,
			locationId: key,
			subject: place,
			message:
				sensors.length === 1
					? `${first.name} in ${place} is being checked again from scratch: its last check was recorded by an earlier SQLBuild release and can’t be read.`
					: `${sensors.length} sensors in ${place} are being checked again from scratch, starting with ${first.name}: their last checks were recorded by an earlier SQLBuild release and can’t be read.`
		});
	}
	const dropped = new Map<string, Wire['DroppedCalendarTimes'][]>();
	for (const calendar of problems.dropped_calendar_times ?? []) {
		const key = keyOf(calendar.location_id, calendar.target);
		dropped.set(key, [...(dropped.get(key) ?? []), calendar]);
	}
	for (const [key, calendars] of dropped) {
		const first = calendars.reduce((a, b) => (a.dropped_at <= b.dropped_at ? a : b));
		const place = placeOf(first.location_name, first.target);
		events.push({
			id: `dropped:${key}`,
			kind: 'calendar_times_dropped',
			severity: 'warning',
			occurredAt: first.dropped_at,
			members: [...new Set(calendars.map((c) => c.trigger_id))].sort(),
			active: true,
			locationId: key,
			subject: place,
			message:
				calendars.length === 1
					? `${first.name} in ${place} won’t tick ${droppedTimes(first.times)}: its previous definition was published by an earlier SQLBuild release and can’t be read.`
					: `${calendars.length} calendars in ${place} dropped pending times, starting with ${first.name}, which won’t tick ${droppedTimes(first.times)}.`
		});
	}
	const unread = new Map<string, Wire['SkippedEvent'][]>();
	for (const event of problems.skipped_events ?? []) {
		const key = event.location_id && event.target ? keyOf(event.location_id, event.target) : '';
		unread.set(key, [...(unread.get(key) ?? []), event]);
	}
	for (const [key, items] of unread) {
		const first = items.reduce((a, b) => (a.skipped_at <= b.skipped_at ? a : b));
		const place = first.location_name && first.target ? placeOf(first.location_name, first.target) : null;
		const what = items.length === 1 ? `A stored ${first.event_type} event` : `${items.length} stored events, starting with a ${first.event_type} event,`;
		events.push({
			id: `skipped:${key || 'server'}`,
			kind: 'unreadable_event_skipped',
			severity: 'warning',
			occurredAt: first.skipped_at,
			members: [...new Set(items.map((e) => String(e.storage_id)))].sort(),
			active: true,
			locationId: key || null,
			subject: place,
			message: `${what}${place ? ` in ${place}` : ''} can’t be read by this SQLBuild release, so webhooks and platform sensors skipped ${items.length === 1 ? 'it' : 'them'} and carried on: ${first.reason}.`
		});
	}
	return events;
}

/** The first few dropped calendar times, then how many more. */
function droppedTimes(times: string[]): string {
	const named = times.slice(0, DROPPED_TIMES_NAMED).join(', ');
	return times.length > DROPPED_TIMES_NAMED ? `${named} and ${times.length - DROPPED_TIMES_NAMED} more` : named;
}

/** The scheduler from the problems read: whether a lease is live and when it runs out. Replicas, heartbeat and forwarding aren't reported. */
export function toPlatformHealth(problems: Wire['PlatformProblems']): PlatformHealth {
	const expiresAt = problems.scheduler_lease_expires_at;
	return {
		scheduler: { lease: problems.scheduler_leader && expiresAt ? { holder: null, acquiredAt: null, renewedAt: null, expiresAt } : null, instances: null },
		heartbeat: null
	};
}

/** Stored events newest first; an open problem marks the newest stored event of its kind and place as ongoing, or comes first when none was stored. An event that can't be read is a row of its own, so it never hides the others. */
export function toPlatformEventRecords(
	open: PlatformEvent[],
	stored: Wire['PersonPlatformEvent'][],
	names: Map<string, string>,
	unreadable: Wire['UnreadablePlatformEvent'][] = []
): PlatformEventRecord[] {
	const rows: { storage: number; records: () => PlatformEventRecord[] }[] = [
		...stored.map((wire) => ({ storage: wire.storage_id, records: () => readStored(wire, names) })),
		...unreadable.map((item) => ({ storage: item.storage_id, records: () => [unreadableRecord(item, names)] }))
	];
	const records = rows.sort((a, b) => b.storage - a.storage).flatMap((row) => row.records());
	const unmatched: PlatformEventRecord[] = [];
	for (const event of open) {
		const latest = records.find((r) => !r.active && r.kind === event.kind && r.locationId === event.locationId);
		if (latest) latest.active = true;
		else unmatched.push({ ...event, resolvedAt: null, links: [], reactions: [], payload: {} });
	}
	return [...unmatched, ...records];
}

/** One stored event's record, or a row saying why it can't be shown: a kind this page doesn't know, or a shape it can't read. */
function readStored(wire: Wire['PersonPlatformEvent'], names: Map<string, string>): PlatformEventRecord[] {
	const kind = (wire.event as { kind?: unknown } | null)?.kind;
	try {
		const record = storedRecord(wire, names);
		if (record === undefined) return [unreadableRecord({ ...wire, event_type: String(kind), error: `this page doesn’t know events of kind ${String(kind)}` }, names)];
		return record ? [record] : [];
	} catch (error) {
		return [unreadableRecord({ ...wire, event_type: String(kind), error: error instanceof Error ? error.message : String(error) }, names)];
	}
}

/** A stored event the server or this page can't read, listed with why, so the rest of the list still loads. */
function unreadableRecord(item: Wire['UnreadablePlatformEvent'], names: Map<string, string>): PlatformEventRecord {
	const key = item.location_id && item.target ? keyOf(item.location_id, item.target) : null;
	const place = key && item.target ? placeOf(names.get(key) ?? 'A location', item.target) : null;
	return {
		id: item.event_id,
		kind: 'unreadable_event',
		severity: 'warning',
		occurredAt: item.occurred_at,
		active: false,
		locationId: key,
		subject: place,
		message: `Couldn’t read this ${item.event_type} event${place ? ` of ${place}` : ''}: ${item.error}.`,
		resolvedAt: null,
		links: key ? [{ kind: 'location', id: key }] : [],
		reactions: [],
		payload: { event_type: item.event_type, error: item.error }
	};
}

/** Null for `run_failed`, which only the platform webhook receives; person reads never return it; undefined for a kind this page doesn't know. */
function storedRecord(wire: Wire['PersonPlatformEvent'], names: Map<string, string>): PlatformEventRecord | null | undefined {
	const key = wire.location_id && wire.target ? keyOf(wire.location_id, wire.target) : null;
	const place = key && wire.target ? placeOf(names.get(key) ?? 'A location', wire.target) : null;
	const base = { id: wire.event_id, occurredAt: wire.occurred_at, active: false, locationId: key, subject: place, resolvedAt: null, reactions: [] };
	const event = wire.event;
	switch (event.kind) {
		case 'scheduler_no_leader':
			return { ...base, kind: event.kind, severity: 'critical', message: 'No server held the scheduler lease, so schedules and sensors stopped starting runs.', links: [], payload: {} };
		case 'location_broken':
			return { ...base, kind: event.kind, severity: 'critical', message: `${place} stopped checking in.`, links: key ? [{ kind: 'location', id: key }] : [], payload: { last_seen_at: event.last_seen_at } };
		case 'location_failing':
			return {
				...base,
				kind: event.kind,
				severity: event.state === 'degraded' ? 'warning' : 'critical',
				message: `${place ?? 'A location'}: ${FAILED[event.state] ?? event.state}${event.error ? `: ${event.error}` : ''}.`,
				links: key ? [{ kind: 'location', id: key }] : [],
				payload: { state: event.state, ...(event.serving_generation != null ? { serving_generation: String(event.serving_generation) } : {}) }
			};
		case 'tick_overdue':
			return { ...base, kind: event.kind, severity: 'warning', message: `A schedule in ${place ?? 'a location'} was late.`, links: key ? [{ kind: 'trigger', id: event.trigger_id, name: 'schedule', locationId: key }] : [], payload: { scheduled_for: event.scheduled_for } };
		case 'calendar_horizon_low':
			return { ...base, kind: event.kind, severity: 'warning', message: `A calendar in ${place ?? 'a location'} was running out of dates.`, links: key ? [{ kind: 'trigger', id: event.trigger_id, name: 'calendar', locationId: key }] : [], payload: { refresh_at: event.refresh_at, ...(event.horizon_end ? { horizon_end: event.horizon_end } : {}) } };
		case 'trigger_skipped':
			return { ...base, kind: event.kind, severity: 'warning', message: `A trigger in ${place ?? 'a location'} was skipped: ${event.reason}.`, links: key ? [{ kind: 'trigger', id: event.trigger_id, name: 'trigger', locationId: key }] : [], payload: { snapshot_id: event.snapshot_id } };
		case 'calendar_times_dropped':
			return { ...base, kind: event.kind, severity: 'warning', message: `A calendar in ${place ?? 'a location'} dropped pending times ${droppedTimes(event.times)}: its previous definition couldn’t be read.`, links: key ? [{ kind: 'trigger', id: event.trigger_id, name: 'calendar', locationId: key }] : [], payload: { snapshot_id: event.snapshot_id, times: event.times.join(', ') } };
		case 'unreadable_event_skipped':
			return {
				...base,
				kind: event.kind,
				severity: 'warning',
				message: `A stored ${event.event_type} event${place ? ` in ${place}` : ''} couldn’t be read, so webhooks and platform sensors skipped it: ${event.reason}.`,
				links: key ? [{ kind: 'location', id: key }] : [],
				payload: { event_type: event.event_type, storage_id: String(event.storage_id) }
			};
		case 'sensor_rechecked':
			return { ...base, kind: event.kind, severity: 'info', message: `A sensor in ${place ?? 'a location'} was checked again from scratch: its last check was recorded by an earlier SQLBuild release and couldn’t be read.`, links: key ? [{ kind: 'trigger', id: event.trigger_id, name: 'sensor', locationId: key }] : [], payload: { snapshot_id: event.snapshot_id, evaluation_id: event.evaluation_id } };
		case 'launch_failed':
			return { ...base, kind: event.kind, severity: 'critical', message: `A run in ${place ?? 'a location'} couldn't start (${event.code}).`, links: key ? [{ kind: 'run', id: event.run_id, locationId: key }] : [], payload: { code: event.code } };
		case 'heartbeat':
			return { ...base, kind: event.kind, severity: 'info', message: 'A server checked in.', links: [], payload: {} };
		case 'control_session_cut_off':
			return {
				...base,
				kind: event.kind,
				severity: 'critical',
				message: event.idle
					? `The control database ended a server session left idle in a transaction past its limit: ${event.error}`
					: `The control database cancelled a server statement that ran past its limit: ${event.error}`,
				links: [],
				payload: { cut_at: event.cut_at }
			};
		case 'run_failed':
			return null;
		default:
			return undefined;
	}
}

/** A failing run-history sync in words: what stands, since when, why, and how it retries. */
export function syncMessage(place: string, sync: Wire['FailingSync'], now: number): string {
	const since = formatClock(sync.since, now);
	const retry = sync.retry_seconds && !sync.halted ? ` · retrying every ${sync.retry_seconds} s` : '';
	if (sync.halted) return `${place}: run history sync stopped after failed reads in a row since ${since}: ${sync.reason}. Resume it once the cause is fixed.`;
	if (sync.unreachable) return `${place}: run history can't reach the warehouse since ${since}${retry}: ${sync.reason}`;
	return `${place}: run history not syncing since ${since}${retry}: ${sync.reason}`;
}
