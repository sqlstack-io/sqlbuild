// Presentation of the overview's attention items. Pure; no side effects.

import type { AttentionItem } from '$lib/api';
import { describeWait } from '$lib/operate/labels';
import { eventKindLabel, problemHref } from '$lib/platform/labels';
import { formatAgo } from '$lib/format/time';
import { runStatusLabel } from '$lib/status';

export type AttentionTone = 'error' | 'warning' | 'stale' | 'info';

export type AttentionView = {
	key: string;
	tone: AttentionTone;
	title: string;
	detail: string;
	since: string;
	href: string;
	action: string;
};

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const minutes = (seconds: number | null) => (seconds === null ? '—' : seconds % 3600 === 0 ? `${seconds / 3600}h` : `${Math.round(seconds / 60)}m`);

export function attentionView(item: AttentionItem, now: number): AttentionView {
	switch (item.kind) {
		case 'run_problem':
			return {
				key: `run:${item.run.id}`,
				tone: 'error',
				title: `${item.run.jobKey ?? 'Ad-hoc run'} ${runStatusLabel[item.run.status].toLowerCase()}`,
				detail: item.run.statusReason ?? item.run.id,
				since: formatAgo(item.run.endedAt, now),
				href: `/runs/${item.run.id}`,
				action: 'View run'
			};
		case 'run_unresponsive':
			return {
				key: `unresponsive:${item.run.id}`,
				tone: 'warning',
				title: `${item.run.jobKey ?? item.run.id} is unresponsive`,
				detail: item.lastHeartbeatAt ? `Not heard from since ${formatAgo(item.lastHeartbeatAt, now)}` : 'Not heard from yet',
				since: formatAgo(item.run.startedAt, now),
				href: `/runs/${item.run.id}`,
				action: 'View run'
			};
		case 'stale_models': {
			const total = new Set([...item.changed, ...item.behind]).size;
			const parts = [item.changed.length && `${item.changed.length} changed`, item.behind.length && `${item.behind.length} behind`].filter(Boolean);
			return {
				key: 'stale',
				tone: 'stale',
				title: `${plural(total, 'node')} ${total === 1 ? 'is' : 'are'} stale`,
				detail: `${parts.join(' · ')} · ${[...new Set([...item.changed, ...item.behind])].slice(0, 4).join(', ')}`,
				since: 'since the last build',
				href: '/catalog?stale=1',
				action: 'View in catalog'
			};
		}
		case 'stale_sources': {
			const first = item.sources[0];
			return {
				key: 'sources',
				tone: item.sources.some((s) => s.status === 'error') ? 'error' : 'warning',
				title: `${plural(item.sources.length, 'source')} past the freshness threshold`,
				detail: item.sources.map((s) => `${s.key} loaded ${formatAgo(s.loadedAt, now)}`).join(' · '),
				since: `${first.status} after ${minutes(first.status === 'error' ? first.errorAfterSeconds : first.warnAfterSeconds)}`,
				href: '/catalog?kind=source',
				action: 'View sources'
			};
		}
		case 'platform_event':
			return {
				key: `event:${item.event.id}`,
				tone: item.event.severity === 'critical' ? 'error' : item.event.severity === 'warning' ? 'warning' : 'info',
				title: `${eventKindLabel[item.event.kind]}${item.event.subject ? ` · ${item.event.subject}` : ''}`,
				detail: item.event.message,
				since: formatAgo(item.event.occurredAt, now),
				href: problemHref(item.event),
				action: item.event.locationId ? 'View location' : 'View server status'
			};
		case 'queue_blocked':
			return {
				key: `queue:${item.runId}`,
				tone: 'warning',
				title: `${item.jobKey ?? item.runId} is waiting in the queue`,
				detail: item.waits.map((w) => describeWait(w).map((p) => p.text).join('')).join(' · '),
				since: `queued ${formatAgo(item.queuedAt, now)}`,
				href: '/runs?view=queue',
				action: 'View queue'
			};
		case 'calendar_low':
			return {
				key: `calendar:${item.triggerId}`,
				tone: 'warning',
				title: `${item.triggerName}: calendar horizon is low`,
				detail: item.detail,
				since: '',
				href: `/triggers/${item.triggerId}`,
				action: 'View trigger'
			};
	}
}

/** Error first, then warnings, stale and info. */
export function attentionRank(tone: AttentionTone): number {
	return { error: 0, warning: 1, stale: 2, info: 3 }[tone];
}
