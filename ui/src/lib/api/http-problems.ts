import type { components } from '$control/schema.d.ts';
import { locationKey as keyOf } from './http-client';
import type { PlatformEvent } from './types';
import type { PlatformEventRecord, PlatformHealth } from './types/platform';

type Wire = components['schemas'];

const placeOf = (name: string, target: string) => `${name} · ${target}`;

/** Live problems as active banner events: a stopped scheduler, each broken location, and late schedules and low calendars per location. */
export function toPlatformEvents(problems: Wire['PlatformProblems'], now: number): PlatformEvent[] {
	const events: PlatformEvent[] = [];
	if (!problems.scheduler_leader) {
		events.push({
			id: 'scheduler',
			kind: 'scheduler_no_leader',
			severity: 'critical',
			occurredAt: problems.scheduler_lease_expires_at ?? new Date(now).toISOString(),
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
			active: true,
			locationId: keyOf(location.location_id, location.target),
			subject: placeOf(location.location_name, location.target),
			message: `${placeOf(location.location_name, location.target)} stopped checking in, so its scheduled runs aren’t starting.`
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
			active: true,
			locationId: key,
			subject: place,
			message:
				calendars.length === 1
					? `${soonest.name} in ${place} ${soonest.horizon_end ? 'is running out of dates' : 'has no dates left'}; the location works out more at its next refresh.`
					: `${calendars.length} calendars in ${place} are running out of dates, starting with ${soonest.name}.`
		});
	}
	return events;
}

/** The scheduler from the problems read: whether a lease is live and when it runs out. Replicas, heartbeat and forwarding aren't reported. */
export function toPlatformHealth(problems: Wire['PlatformProblems']): PlatformHealth {
	const expiresAt = problems.scheduler_lease_expires_at;
	return {
		scheduler: { lease: problems.scheduler_leader && expiresAt ? { holder: null, acquiredAt: null, renewedAt: null, expiresAt } : null, instances: null },
		heartbeat: null
	};
}

/** Stored events newest first; an open problem marks the newest stored event of its kind and place as ongoing, or comes first when none was stored. */
export function toPlatformEventRecords(open: PlatformEvent[], stored: Wire['PersonPlatformEvent'][], names: Map<string, string>): PlatformEventRecord[] {
	const records = stored.flatMap((wire) => storedRecord(wire, names) ?? []);
	const unmatched: PlatformEventRecord[] = [];
	for (const event of open) {
		const latest = records.find((r) => !r.active && r.kind === event.kind && r.locationId === event.locationId);
		if (latest) latest.active = true;
		else unmatched.push({ ...event, resolvedAt: null, links: [], reactions: [], payload: {} });
	}
	return [...unmatched, ...records];
}

/** Null for `run_failed`, which only the platform webhook receives; person reads never return it. */
function storedRecord(wire: Wire['PersonPlatformEvent'], names: Map<string, string>): PlatformEventRecord | null {
	const key = wire.location_id && wire.target ? keyOf(wire.location_id, wire.target) : null;
	const place = key && wire.target ? placeOf(names.get(key) ?? 'A location', wire.target) : null;
	const base = { id: wire.event_id, occurredAt: wire.occurred_at, active: false, locationId: key, subject: place, resolvedAt: null, reactions: [] };
	const event = wire.event;
	switch (event.kind) {
		case 'scheduler_no_leader':
			return { ...base, kind: event.kind, severity: 'critical', message: 'No server held the scheduler lease, so schedules and sensors stopped starting runs.', links: [], payload: {} };
		case 'location_broken':
			return { ...base, kind: event.kind, severity: 'critical', message: `${place} stopped checking in.`, links: key ? [{ kind: 'location', id: key }] : [], payload: { last_seen_at: event.last_seen_at } };
		case 'tick_overdue':
			return { ...base, kind: event.kind, severity: 'warning', message: `A schedule in ${place ?? 'a location'} was late.`, links: key ? [{ kind: 'trigger', id: event.trigger_id, name: 'schedule', locationId: key }] : [], payload: { scheduled_for: event.scheduled_for } };
		case 'calendar_horizon_low':
			return { ...base, kind: event.kind, severity: 'warning', message: `A calendar in ${place ?? 'a location'} was running out of dates.`, links: key ? [{ kind: 'trigger', id: event.trigger_id, name: 'calendar', locationId: key }] : [], payload: { refresh_at: event.refresh_at, ...(event.horizon_end ? { horizon_end: event.horizon_end } : {}) } };
		case 'launch_failed':
			return { ...base, kind: event.kind, severity: 'critical', message: `A run in ${place ?? 'a location'} couldn't start (${event.code}).`, links: key ? [{ kind: 'run', id: event.run_id, locationId: key }] : [], payload: { code: event.code } };
		case 'heartbeat':
			return { ...base, kind: event.kind, severity: 'info', message: 'A server checked in.', links: [], payload: {} };
		case 'run_failed':
			return null;
	}
}
