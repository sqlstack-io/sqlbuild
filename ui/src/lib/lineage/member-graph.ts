// A job group's member jobs as a graph: one card per member, an edge where a member waits for
// another, laid out left to right in wait order. Pure; no side effects.

import type { Edge, Node } from '@xyflow/svelte';
import type { JobGroupMember, RunRef } from '$lib/api';
import { maxColumnCards } from './job-graph';

export const MEMBER_WIDTH = 236;
export const MEMBER_HEIGHT = 62;
/** Space between columns, and between cards stacked in one column. */
const COLUMN_GAP = 64;
const ROW_GAP = 20;
const MARGIN = 24;

/** What the jobs list reports for a member; null where it isn't known. */
export type MemberInfo = { nodeCount: number | null; lastRun: RunRef | null };

export type MemberCardData = {
	jobKey: string;
	nodeCount: number | null;
	lastRun: RunRef | null;
	/** How many members this one waits for. */
	waitsFor: number;
};

const edgeStyle = 'stroke: var(--sb-edge); stroke-width: 1.5;';

export const memberNodeId = (jobKey: string) => `member:${jobKey}`;

/**
 * The members in columns, left to right: a member's column follows the latest member it waits for,
 * as its stage says. A stage taller than the canvas suits wraps into several columns. Within a
 * column, members sit near the members they wait for (the mean row of those), then in the server's
 * order, which keeps edges short and crossings few.
 */
export function memberColumns(members: readonly JobGroupMember[]): string[][] {
	const known = new Set(members.map((m) => m.jobKey));
	const order = new Map(members.map((m, i) => [m.jobKey, i]));
	const stages = Map.groupBy(members, (m) => m.stage);
	const max = maxColumnCards(members.length);
	const row = new Map<string, number>();
	const columns: string[][] = [];
	for (const stage of [...stages.keys()].sort((a, b) => a - b)) {
		const weight = (m: JobGroupMember) => {
			const rows = m.upstream.filter((u) => known.has(u.jobKey) && row.has(u.jobKey)).map((u) => row.get(u.jobKey)!);
			return rows.length ? rows.reduce((a, b) => a + b, 0) / rows.length : Infinity;
		};
		const sorted = [...stages.get(stage)!]
			.map((m) => ({ key: m.jobKey, weight: weight(m) }))
			.sort((a, b) => (a.weight === b.weight ? 0 : a.weight - b.weight) || order.get(a.key)! - order.get(b.key)!)
			.map((m) => m.key);
		// Spread evenly over as few columns as fit, as Lineage wraps a tall column.
		const per = Math.ceil(sorted.length / Math.ceil(sorted.length / max));
		for (let i = 0; i < sorted.length; i += per) {
			const column = sorted.slice(i, i + per);
			column.forEach((key, r) => row.set(key, r));
			columns.push(column);
		}
	}
	return columns;
}

/**
 * Cards for the members, placed in their columns with each column centred on the tallest, and an
 * edge from each member to every member that waits for it. Waits on jobs outside the group are not
 * drawn. Cards have a fixed size, so the graph is laid out without measuring them.
 */
export function memberGraph(members: readonly JobGroupMember[], info: ReadonlyMap<string, MemberInfo>): { nodes: Node[]; edges: Edge[] } {
	const known = new Set(members.map((m) => m.jobKey));
	const columns = memberColumns(members);
	const tallest = Math.max(0, ...columns.map((c) => c.length));
	const step = MEMBER_HEIGHT + ROW_GAP;
	const byKey = new Map(members.map((m) => [m.jobKey, m]));
	const nodes: Node[] = columns.flatMap((column, c) => {
		const top = MARGIN + ((tallest - column.length) * step) / 2;
		return column.map((jobKey, r) => {
			const member = byKey.get(jobKey)!;
			const data: MemberCardData = {
				jobKey,
				nodeCount: info.get(jobKey)?.nodeCount ?? null,
				lastRun: info.get(jobKey)?.lastRun ?? null,
				waitsFor: member.upstream.filter((u) => known.has(u.jobKey)).length
			};
			return {
				id: memberNodeId(jobKey),
				type: 'member',
				position: { x: MARGIN + c * (MEMBER_WIDTH + COLUMN_GAP), y: top + r * step },
				width: MEMBER_WIDTH,
				height: MEMBER_HEIGHT,
				selectable: false,
				data
			};
		});
	});
	const seen = new Set<string>();
	const edges: Edge[] = members.flatMap((m) =>
		m.upstream.flatMap((u) => {
			const [source, target] = [memberNodeId(u.jobKey), memberNodeId(m.jobKey)];
			const id = `${source}-${target}`;
			if (!known.has(u.jobKey) || u.jobKey === m.jobKey || seen.has(id)) return [];
			seen.add(id);
			return [{ id, source, target, type: 'bezier', style: edgeStyle }];
		})
	);
	return { nodes, edges };
}
