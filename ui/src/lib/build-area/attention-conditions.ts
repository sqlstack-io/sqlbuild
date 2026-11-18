// Ongoing conditions on the Overview's attention list, and whether an acknowledgement still holds.
// A run is acknowledged for good; a condition only until it changes: it gets worse, it resolves and
// comes back, or the acknowledgement's 24 hours end. The server stores each acknowledgement with the
// condition's shape when it was acknowledged; this module says what a condition's shape is and what
// counts as worse. Shapes hold kinds, subjects, severities and counts, never message text, which can
// hold relative times ("12 minutes ago"). Pure.

import type { AttentionAcknowledgement, AttentionCondition, AttentionItem, ConditionShape } from '$lib/api';
import { plural } from '$lib/format/plural';

/** Members kept in a shape; beyond this a condition is compared by its counts only, to keep the stored shape small. */
const MAX_MEMBERS = 50;
const SEVERITY: Record<string, number> = { info: 0, warning: 1, critical: 2, warn: 1, error: 2 };

/** A condition's key and shape, and what its members are called; null for runs, which are acknowledged for good. */
export function conditionOf(item: AttentionItem): { key: string; shape: ConditionShape; noun: string } | null {
	switch (item.kind) {
		case 'stale_models':
			return { key: 'stale_models', noun: 'node', shape: { counts: { changed: new Set(item.changed).size, behind: new Set(item.behind).size } } };
		case 'stale_sources': {
			const members = item.sources.map((s) => `${s.key}:${s.status}`).sort();
			const counts = { warn: item.sources.filter((s) => s.status === 'warn').length, error: item.sources.filter((s) => s.status === 'error').length };
			return { key: 'stale_sources', noun: 'source', shape: members.length <= MAX_MEMBERS ? { counts, members } : { counts } };
		}
		case 'platform_event': {
			const e = item.event;
			return { key: `event:${e.kind}:${e.locationId ?? ''}:${e.subject ?? ''}`, noun: 'event', shape: { severity: SEVERITY[e.severity] ?? 0, occurrence: e.id } };
		}
		case 'queue_blocked':
			return { key: `queue:${item.runId}`, noun: 'wait', shape: { members: [...new Set(item.waits.map((w) => w.kind))].sort() } };
		case 'calendar_low':
			return { key: `calendar:${item.triggerId}`, noun: 'warning', shape: {} };
		case 'run_problem':
		case 'run_unresponsive':
			return null;
	}
}

const COUNT_LABEL: Record<string, string> = { changed: 'changed', behind: 'behind', warn: 'late', error: 'past their error threshold' };

/** Why an acknowledgement no longer holds at `now`, or null while it does. */
export function lapseOf(stored: ConditionShape, current: ConditionShape, until: string, now: number, noun: string): string | null {
	if (stored.occurrence !== undefined && current.occurrence !== undefined && stored.occurrence !== current.occurrence) return 'it resolved and came back';
	if ((current.severity ?? 0) > (stored.severity ?? 0)) return 'it became more severe';
	for (const [name, count] of Object.entries(current.counts ?? {})) {
		const more = count - (stored.counts?.[name] ?? 0);
		if (more > 0) return `${plural(more, `more ${noun}`)} ${COUNT_LABEL[name] ?? name} since it was acknowledged`;
	}
	if (stored.members && current.members) {
		const added = current.members.filter((m) => !stored.members!.includes(m)).length;
		if (added) return `${plural(added, `new ${noun}`)} since it was acknowledged`;
	}
	if (now >= Date.parse(until)) return 'its 24-hour acknowledgement ended';
	return null;
}

/** The items with their conditions, each acknowledged while its acknowledgement holds and otherwise saying why it lapsed. */
export function withConditions(items: AttentionItem[], acknowledgements: readonly AttentionAcknowledgement[], now: number): AttentionItem[] {
	const byKey = new Map(acknowledgements.map((a) => [a.key, a]));
	return items.map((item) => {
		const found = conditionOf(item);
		if (!found) return item;
		const ack = byKey.get(found.key);
		const reason = ack ? lapseOf(ack.shape, found.shape, ack.until, now, found.noun) : null;
		const condition: AttentionCondition = {
			key: found.key,
			shape: found.shape,
			acknowledged: ack && !reason ? { by: ack.by, at: ack.at, until: ack.until } : null,
			lapsed: ack && reason ? { reason, by: ack.by, at: ack.at } : null
		};
		return { ...item, condition } as AttentionItem;
	});
}

/** Whether an item is acknowledged now: a run by its own mark, a condition while its acknowledgement holds. */
export function acknowledgementOf(item: AttentionItem): { by: string; at: string; until?: string } | null {
	if ('acknowledged' in item) return item.acknowledged;
	return ('condition' in item && item.condition?.acknowledged) || null;
}
