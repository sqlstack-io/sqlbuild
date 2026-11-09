import type { components } from '$control/schema.d.ts';
import { locationKey as keyOf } from './http-client';
import type { PlatformEvent } from './types';

type Wire = components['schemas'];

const placeOf = (name: string, target: string) => `${name} · ${target}`;

/** Live problems as active banner events: a stopped scheduler, each broken location, and late schedules per location. */
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
	return events;
}
