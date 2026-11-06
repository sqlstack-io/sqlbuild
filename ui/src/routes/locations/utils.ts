// Server status in plain words for the Locations page. Pure.

import type { HeartbeatStatus, SchedulerStatus, SinkStatus } from '$lib/api';
import { formatAgo, formatIn, formatStamp } from '$lib/format/time';

export type StatusWords = { text: string; tone: string; title: string; checkIn?: string };

/** "running (server-2)" while a replica holds the leader lease; the lease details go in the tooltip. */
export function schedulerWords(scheduler: SchedulerStatus, now: number): StatusWords {
	const lease = scheduler.lease;
	if (!lease) {
		return {
			text: 'stopped',
			tone: 'var(--sb-error)',
			title: 'No server holds the scheduler lease, so schedules and sensors aren’t starting runs. Another server takes over automatically when one is available.'
		};
	}
	return {
		text: `running (${lease.holder})`,
		tone: 'var(--sb-success)',
		checkIn: formatAgo(lease.renewedAt, now),
		title: `Leader lease held by ${lease.holder} since ${formatStamp(lease.acquiredAt)}; renewed ${formatAgo(lease.renewedAt, now)}, expires ${formatIn(lease.expiresAt, now)}`
	};
}

export function sinkWords(sink: SinkStatus | null, now: number): StatusWords {
	if (!sink) return { text: 'not set up', tone: 'var(--sb-text-faint)', title: 'No event forwarding in the server config' };
	if (sink.lastError) {
		return {
			text: `failing · ${sink.pendingEvents} waiting`,
			tone: 'var(--sb-error)',
			title: `${sink.consecutiveFailures} failed attempts; last delivered ${formatAgo(sink.lastDeliveredAt, now)}. ${sink.lastError}`
		};
	}
	return { text: 'ok', tone: 'var(--sb-success)', title: `Delivered ${formatAgo(sink.lastDeliveredAt, now)} via ${sink.kind} (${sink.destinationSource})` };
}

/** Late once two intervals pass without a heartbeat. */
export function heartbeatLate(heartbeat: HeartbeatStatus, now: number): boolean {
	if (!heartbeat.lastEmittedAt) return true;
	return now - Date.parse(heartbeat.lastEmittedAt) > 2 * heartbeat.intervalSeconds * 1000;
}

/** A server is live while it was seen within the last minute. */
export function replicaLive(lastSeenAt: string, now: number): boolean {
	return now - Date.parse(lastSeenAt) < 60_000;
}
