// Presentation of the overview's attention items. Pure; no side effects.

import type { AttentionItem, QueueWait } from '$lib/api';
import { describeWait, outcomeCounts } from '$lib/operate/labels';
import { eventKindLabel, problemHref } from '$lib/platform/labels';
import { formatAgo } from '$lib/format/time';
import { runStatusLabel } from '$lib/status';
import { shortName } from '$lib/build-area/labels';

export type AttentionTone = 'error' | 'warning' | 'stale' | 'info';

/** Something an alert's cause also brought about, listed under it instead of as an alert of its own. */
export type Consequence = { text: string; href: string };

export type AttentionView = {
	key: string;
	tone: AttentionTone;
	title: string;
	detail: string;
	/** When it started, as a time ago; empty when the alert has no start of its own. */
	since: string;
	href: string;
	action: string;
	consequences: Consequence[];
};

/** A heartbeat gap shorter than this reads as a pause, not an outage: the run carries on when its worker reports again. */
export const UNRESPONSIVE_OUTAGE_MS = 5 * 60_000;

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const minutes = (seconds: number | null) => (seconds === null ? '—' : seconds % 3600 === 0 ? `${seconds / 3600}h` : `${Math.round(seconds / 60)}m`);
const waitText = (waits: QueueWait[]) => waits.map((w) => describeWait(w).map((p) => p.text).join('')).join(' · ');

export function attentionView(item: AttentionItem, now: number): AttentionView {
	switch (item.kind) {
		case 'run_problem':
			return {
				key: `run:${item.run.id}`,
				tone: 'error',
				title: `${item.run.jobKey ?? 'Ad-hoc run'} ${runStatusLabel[item.run.status].toLowerCase()}`,
				// The node counts say what went wrong; the worker's reason only says how it stopped.
				detail: outcomeCounts(item.run.nodeCounts) || item.run.statusReason || item.run.id,
				since: formatAgo(item.run.endedAt, now),
				href: `/runs/${item.run.id}`,
				action: 'View run',
				consequences: []
			};
		case 'run_unresponsive': {
			const heard = item.lastHeartbeatAt ?? item.run.startedAt;
			const gap = heard ? now - Date.parse(heard) : Infinity;
			const name = item.run.jobKey ?? item.run.id;
			const outage = gap >= UNRESPONSIVE_OUTAGE_MS;
			return {
				key: `unresponsive:${item.run.id}`,
				tone: outage ? 'warning' : 'info',
				title: outage ? `${name} is unresponsive` : `${name} missed a heartbeat`,
				detail: item.lastHeartbeatAt
					? `Its worker last reported ${formatAgo(item.lastHeartbeatAt, now)}.${outage ? '' : ' The run carries on when the worker reports again.'}`
					: (item.run.statusReason ?? 'Its worker hasn’t reported since the run started.'),
				since: formatAgo(item.lastHeartbeatAt ?? item.run.startedAt, now),
				href: `/runs/${item.run.id}`,
				action: 'View run',
				consequences: []
			};
		}
		case 'stale_models': {
			const changed = new Set(item.changed);
			const both = item.behind.filter((key) => changed.has(key)).length;
			const all = [...new Set([...item.changed, ...item.behind])];
			const only = item.behind.length - both;
			const alsoBehind = both ? `, ${item.changed.length === 1 ? 'also' : `${both} of them also`} behind an upstream rebuild` : '';
			const onlyBehind = only ? `${both ? `, and ${only} more` : ` and ${only}`} behind an upstream rebuild` : '';
			const counts = item.changed.length
				? `${item.changed.length} changed since ${item.changed.length === 1 ? 'its' : 'their'} last build${alsoBehind}${onlyBehind}`
				: `${only} behind an upstream rebuild`;
			return {
				key: 'stale',
				tone: 'stale',
				title: `${plural(all.length, 'node')} out of date`,
				detail: `${counts}: ${all.slice(0, 4).map(shortName).join(', ')}${all.length > 4 ? ` and ${all.length - 4} more` : ''}`,
				since: '',
				href: '/catalog?stale=1',
				action: 'View in catalog',
				consequences: []
			};
		}
		case 'stale_sources':
			return {
				key: 'sources',
				tone: item.sources.some((s) => s.status === 'error') ? 'error' : 'warning',
				title: `${plural(item.sources.length, 'source')} past the freshness threshold`,
				detail: item.sources
					.map((s) => `${s.key} loaded ${formatAgo(s.loadedAt, now)} (${s.status} after ${minutes(s.status === 'error' ? s.errorAfterSeconds : s.warnAfterSeconds)})`)
					.join(' · '),
				since: '',
				href: '/catalog?kind=source',
				action: 'View sources',
				consequences: []
			};
		case 'platform_event':
			return {
				key: `event:${item.event.id}`,
				tone: item.event.severity === 'critical' ? 'error' : item.event.severity === 'warning' ? 'warning' : 'info',
				title: `${eventKindLabel[item.event.kind]}${item.event.subject ? ` · ${item.event.subject}` : ''}`,
				detail: item.event.message,
				since: formatAgo(item.event.occurredAt, now),
				href: problemHref(item.event),
				action: item.event.locationId ? 'View location' : 'View server status',
				consequences: []
			};
		case 'queue_blocked':
			return {
				key: `queue:${item.runId}`,
				tone: 'warning',
				title: `${item.jobKey ?? item.runId} is waiting in the queue`,
				detail: waitText(item.waits),
				since: formatAgo(item.queuedAt, now),
				href: '/runs?view=queue',
				action: 'View queue',
				consequences: []
			};
		case 'calendar_low':
			return {
				key: `calendar:${item.triggerId}`,
				tone: 'warning',
				title: `${item.triggerName}: calendar horizon is low`,
				detail: item.detail,
				since: '',
				href: `/triggers/${item.triggerId}`,
				action: 'View trigger',
				consequences: []
			};
	}
}

/** The run a wait is held up by, when it names one. */
const waitRun = (wait: QueueWait): string | null => ('runId' in wait ? wait.runId : null);

/**
 * The cause an item follows from, by the key of the alert that names that cause, or null when it stands alone:
 * a schedule is overdue because its location stopped checking in, or else because no server runs the scheduler;
 * a run waits in the queue because an unresponsive run of its job, or one it waits for, holds it up.
 */
function causeOf(item: AttentionItem, items: AttentionItem[]): string | null {
	if (item.kind === 'platform_event' && item.event.kind === 'tick_overdue') {
		const events = items.flatMap((i) => (i.kind === 'platform_event' ? [i.event] : []));
		const location = events.find((e) => e.kind === 'location_broken' && e.locationId === item.event.locationId);
		const scheduler = events.find((e) => e.kind === 'scheduler_no_leader');
		const cause = location ?? scheduler;
		return cause ? `event:${cause.id}` : null;
	}
	if (item.kind === 'queue_blocked') {
		const held = new Set(item.waits.map(waitRun));
		const stuck = items.find((i) => i.kind === 'run_unresponsive' && ((item.jobKey !== null && i.run.jobKey === item.jobKey) || held.has(i.run.id)));
		return stuck && stuck.kind === 'run_unresponsive' ? `unresponsive:${stuck.run.id}` : null;
	}
	return null;
}

function consequenceOf(item: AttentionItem, view: AttentionView): Consequence {
	if (item.kind === 'queue_blocked') return { text: `${view.title}, queued ${view.since}. ${view.detail}`, href: view.href };
	if (item.kind === 'platform_event') return { text: item.event.message, href: view.href };
	return { text: `${view.title}: ${view.detail}`, href: view.href };
}

/** The items as alerts, errors first: an item that follows from another alert's cause is listed under it, not on its own. */
export function attentionViews(items: AttentionItem[], now: number): { item: AttentionItem; view: AttentionView }[] {
	const viewed = items.map((item) => ({ item, view: attentionView(item, now) }));
	const byKey = new Map(viewed.map((v) => [v.view.key, v]));
	const shown = viewed.filter(({ item, view }) => {
		const cause = byKey.get(causeOf(item, items) ?? '');
		if (!cause) return true;
		cause.view.consequences.push(consequenceOf(item, view));
		// The cause is at least as urgent as anything it brings about.
		if (attentionRank(view.tone) < attentionRank(cause.view.tone)) cause.view.tone = view.tone;
		return false;
	});
	return shown.sort((a, b) => attentionRank(a.view.tone) - attentionRank(b.view.tone));
}

/** Error first, then warnings, stale and info. */
export function attentionRank(tone: AttentionTone): number {
	return { error: 0, warning: 1, stale: 2, info: 3 }[tone];
}
