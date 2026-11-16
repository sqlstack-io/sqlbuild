// Summaries of change-feed entries. Pure; no side effects.

import type { ChangeOp, NodeChange, SnapshotChange } from '$lib/api';
import { nodeNoun, plural } from '$lib/format/plural';


/** e.g. "3 nodes · 1 trigger · 1 role · dependencies". */
export function summarizeChange(change: SnapshotChange): string {
	const parts: string[] = [];
	if (change.nodes.length) parts.push(plural(change.nodes.length, 'node'));
	const orchestration = change.orchestration.length;
	if (orchestration) parts.push(plural(orchestration, 'job or trigger', 'jobs and triggers'));
	if (change.roles.length) parts.push(plural(change.roles.length, 'role'));
	if (change.environment) parts.push('environment');
	return parts.length ? parts.join(' · ') : 'No definition changes';
}

/** A deploy lists its node changes one by one up to this many; more are counted by kind. */
export const NODE_LIST_LIMIT = 20;

export type NodeChangeGroup = { op: ChangeOp; prefix: string; count: number; noun: string };

/** A deploy's node changes counted by change and kind, largest first, e.g. 252 added models. */
export function nodeChangeGroups(nodes: readonly NodeChange[]): NodeChangeGroup[] {
	const groups = new Map<string, { op: ChangeOp; prefix: string; count: number }>();
	for (const n of nodes) {
		const prefix = n.key.includes(':') ? n.key.slice(0, n.key.indexOf(':')) : 'model';
		const id = `${n.op}\u0000${prefix}`;
		const group = groups.get(id) ?? { op: n.op, prefix, count: 0 };
		group.count += 1;
		groups.set(id, group);
	}
	return [...groups.values()]
		.sort((a, b) => b.count - a.count || a.prefix.localeCompare(b.prefix))
		.map(({ op, prefix, count }) => {
			return { op, prefix, count, noun: nodeNoun(prefix, count) };
		});
}

/** One line on a deploy's node changes: the first few keys, or the largest groups when there are many. */
export function nodeChangeLine(nodes: readonly NodeChange[]): string {
	if (!nodes.length) return 'No node changes';
	if (nodes.length <= 3) return nodes.map((n) => `${n.op} ${n.key}`).join(' · ');
	const groups = nodeChangeGroups(nodes);
	const shown = groups.slice(0, 3).map((g) => `${g.op} ${g.count.toLocaleString('en')} ${g.noun}`);
	return groups.length > 3 ? `${shown.join(' · ')} · ${groups.length - 3} more kinds` : shown.join(' · ');
}
