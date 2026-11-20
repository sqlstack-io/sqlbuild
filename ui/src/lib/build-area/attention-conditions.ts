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
			return { key: 'stale_sources', noun: 'source', shape: bounded({ counts, members }) };
		}
		case 'platform_event': {
			const e = item.event;
			const severity = SEVERITY[e.severity] ?? 0;
			const key = boundedKey(`event:${e.kind}:${e.locationId ?? ''}:${e.subject ?? ''}`);
			// A grouped event (late schedules, low calendars) gets worse as it holds more; its ID is fixed.
			if (e.members) return { key, noun: NOUNS[e.kind] ?? 'event', shape: bounded({ severity, counts: { [e.kind]: e.members.length }, members: e.members }) };
			// A fixed ID (a stopped scheduler, a broken location) comes with what tells its occurrences apart.
			return { key, noun: 'event', shape: { severity, ...(e.occurrence ?? e.id ? { occurrence: e.occurrence ?? e.id } : {}) } };
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

const COUNT_LABEL: Record<string, string> = { changed: 'changed', behind: 'behind', warn: 'late', error: 'past their error threshold', tick_overdue: 'late', calendar_horizon_low: 'running out of dates', trigger_skipped: 'skipped' };
const NOUNS: Record<string, string> = { tick_overdue: 'schedule', calendar_horizon_low: 'calendar', trigger_skipped: 'trigger' };

/** The server keeps keys to 256 characters; a longer one is cut, with a hash of the whole to keep it distinct. */
export const MAX_KEY_CHARS = 256;
export function boundedKey(key: string): string {
	// The server counts characters (code points), so a key is measured and cut by them, never mid-pair.
	const points = Array.from(key);
	if (points.length <= MAX_KEY_CHARS) return key;
	// FNV-1a, 32 bits, twice with different seeds: deterministic and enough to tell keys of one target apart.
	const fnv = (seed: number) => {
		let h = seed;
		for (let i = 0; i < key.length; i++) h = Math.imul(h ^ key.charCodeAt(i), 0x01000193) >>> 0;
		return h.toString(16).padStart(8, '0');
	};
	return `${points.slice(0, MAX_KEY_CHARS - 18).join('')}#${fnv(0x811c9dc5)}${fnv(0x050c5d1f)}`;
}

/** The server keeps shapes to 8 KiB; members are left out beyond MAX_MEMBERS or this many bytes, comparing by counts. */
const MAX_SHAPE_BYTES = 6 * 1024;
function bounded(shape: ConditionShape): ConditionShape {
	if (!shape.members) return shape;
	const fits = shape.members.length <= MAX_MEMBERS && new TextEncoder().encode(JSON.stringify(shape)).length <= MAX_SHAPE_BYTES;
	if (fits) return shape;
	const { members: _, ...rest } = shape;
	return rest;
}

/** Why an acknowledgement no longer holds at `now`, or null while it does. */
export function lapseOf(stored: ConditionShape, current: ConditionShape, until: string | null, now: number, noun: string): string | null {
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
	if (until !== null && now >= Date.parse(until)) return 'the time it was acknowledged for ended';
	return null;
}

/** A snooze in words, such as `24 hours` or `1 day and 12 hours`. */
export function snoozeWords(seconds: number): string {
	const parts = [
		[Math.floor(seconds / 86_400), 'day'],
		[Math.floor((seconds % 86_400) / 3_600), 'hour'],
		[Math.floor((seconds % 3_600) / 60), 'minute'],
		[seconds % 60, 'second']
	] as const;
	const words = parts.filter(([n]) => n > 0).map(([n, unit]) => plural(n, unit));
	return words.length > 1 ? `${words.slice(0, -1).join(', ')} and ${words.at(-1)}` : (words[0] ?? '0 seconds');
}

/**
 * Acknowledged conditions that `items` no longer hold, among those this read can `see`; a condition it
 * can't see is never judged gone. Runs are acknowledged for good and have no condition, so never appear.
 */
export function clearedAcknowledgements(items: readonly AttentionItem[], acknowledgements: readonly AttentionAcknowledgement[], see: (key: string) => boolean): AttentionAcknowledgement[] {
	const present = new Set(items.flatMap((item) => conditionOf(item)?.key ?? []));
	return acknowledgements.filter((a) => see(a.key) && !present.has(a.key));
}

/**
 * The cleared acknowledgements to withdraw now: none after a failed read, and nodes out of date only once
 * node state has settled (no sync running, the last one succeeded), when their absence can be trusted.
 */
export function withdrawalsDue(cleared: readonly AttentionAcknowledgement[] | undefined, read: { failed: boolean; syncing: boolean }): AttentionAcknowledgement[] {
	if (read.failed) return [];
	return (cleared ?? []).filter((a) => a.key !== 'stale_models' || !read.syncing);
}

/**
 * An Overview read as the page made it: its own ID (the page holds data in a proxy, so the read is told
 * apart by ID, not identity), for which scope, and whether node state had settled when it began.
 */
export type JudgedRead<T> = { overview: T; readId: string; scopeKey: string; nodesSettled: boolean };

/**
 * The read whose cleared acknowledgements may be withdrawn, or null: only the page's own latest read, while
 * it is what is shown, for the scope now selected, and not after a failed reload. Shown data that came from
 * elsewhere (remembered from an earlier visit, or still the previous target's) is never judged.
 */
export function judgedRead<T>(read: JudgedRead<T> | null, now: { shown: { readId?: string } | null; scopeKey: string | null; failed: boolean }): JudgedRead<T> | null {
	if (!read || now.failed || now.shown?.readId !== read.readId || now.scopeKey !== read.scopeKey) return null;
	return read;
}

/** What the audit log says of a withdrawal made because the condition cleared. */
export const CLEARED_REASON = 'automatic: the condition cleared';

/**
 * Whether an event's condition is one every viewer of `scopeKey` (a `<location>:<target>` key) sees in full:
 * the server-wide scheduler, which everyone sees, or this target's own problems. Others' problems are
 * listed only to people who may view their target, so their absence here says nothing.
 */
export function visibleEventKey(key: string, scopeKey: string): boolean {
	if (key === boundedKey('event:scheduler_no_leader::')) return true;
	return ['location_broken', 'tick_overdue', 'calendar_horizon_low', 'trigger_skipped'].some((kind) => key.startsWith(`event:${kind}:${scopeKey}:`));
}

/**
 * The event ID of withdrawing `ack` because its condition cleared: the same for every viewer, so the
 * server applies it once however many Overviews notice. A new acknowledgement gets a new one.
 */
export function clearedWithdrawalId(ack: Pick<AttentionAcknowledgement, 'eventId'>): string {
	const text = `${ack.eventId}\ncleared`;
	// FNV-1a, 32 bits, with four seeds: 128 deterministic bits, shaped as a UUID (version 8, variant 10).
	const hash = (seed: number) => {
		let h = seed;
		for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 0x01000193) >>> 0;
		return h.toString(16).padStart(8, '0');
	};
	const hex = [0x811c9dc5, 0x050c5d1f, 0x2d358dcc, 0x7a1f3e91].map(hash).join('');
	const variant = ((parseInt(hex[16], 16) & 0x3) | 0x8).toString(16);
	return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-8${hex.slice(13, 16)}-${variant}${hex.slice(17, 20)}-${hex.slice(20, 32)}`;
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
/** `until` is absent for a run, acknowledged for good, and null for a condition without a time limit. */
export function acknowledgementOf(item: AttentionItem): { by: string; at: string; until?: string | null } | null {
	if ('acknowledged' in item) return item.acknowledged;
	return ('condition' in item && item.condition?.acknowledged) || null;
}
