// Node build state apart from the node listing: the listing is read once per snapshot, and the
// states that change as runs finish are applied to it. Pure; no side effects.

import { shortName } from '$lib/build-area/labels';
import type { CatalogNode, NodeState, NodeStatusDelta, StaleReason } from './types/build';

/** The state part of a listed node. */
export function stateOf(node: CatalogNode): NodeState {
	return {
		lastBuild: node.lastBuild,
		lastOutcome: node.lastOutcome,
		definitionChanged: !!node.stale?.some((r) => r.reason === 'changed'),
		checks: node.checks,
		status: node.status,
		statusAt: node.statusAt
	};
}

/** The reason a node whose definition changed since its last build is stale, keeping a fuller one already held. */
function changedReason(node: CatalogNode, state: NodeState): StaleReason[] {
	if (!state.definitionChanged || !state.lastBuild) return [];
	const held = node.stale?.find((r) => r.reason === 'changed');
	return [held ?? { reason: 'changed', change: 'definition', detail: 'The definition changed since it was last built here.', since: state.lastBuild.builtAt }];
}

/** A node with `state` in place of its own; behind is left to `withBehind`, which needs every node. */
function withState(node: CatalogNode, state: NodeState): CatalogNode {
	const kept = (node.stale ?? []).filter((r) => r.reason !== 'changed');
	return { ...node, lastBuild: state.lastBuild, lastOutcome: state.lastOutcome, checks: state.checks, status: state.status, statusAt: state.statusAt, stale: [...changedReason(node, state), ...kept] };
}

/**
 * When the data a node reads last changed: its own last build, or for a view, which reads its
 * upstream when queried, the newest data behind it. Null when nothing behind it is built.
 */
function dataTimes(listed: Map<string, CatalogNode>): (key: string) => number | null {
	const memo = new Map<string, number | null>();
	const at = (key: string): number | null => {
		if (memo.has(key)) return memo.get(key) ?? null;
		memo.set(key, null);
		const node = listed.get(key);
		let time = node?.lastBuild ? Date.parse(node.lastBuild.builtAt) : null;
		if (node?.kind === 'view') {
			const upstream = node.upstream.flatMap((k) => at(k) ?? []);
			time = upstream.length ? Math.max(...upstream) : null;
		}
		memo.set(key, time);
		return time;
	};
	return at;
}

/**
 * Upstream nodes with newer data than this node's own last build. Views read their upstream when
 * queried, so they are never behind, and an unbuilt node is not built yet rather than behind.
 */
function behindOf(node: CatalogNode, dataAt: (key: string) => number | null): StaleReason[] {
	const builtAt = node.lastBuild?.builtAt;
	if (!builtAt || node.kind === 'view') return [];
	const newer = node.upstream.flatMap((key) => {
		const at = dataAt(key);
		return at !== null && at > Date.parse(builtAt) ? [{ key, at }] : [];
	});
	if (!newer.length) return [];
	const since = new Date(Math.min(...newer.map((n) => n.at))).toISOString();
	const names = newer.map((n) => shortName(n.key));
	const detail = `${names.length === 1 ? names[0] : `${names.length} upstream nodes`} ${names.length === 1 ? 'has' : 'have'} newer data than this build.`;
	return [{ reason: 'behind', upstream: newer.map((n) => n.key), detail, since }];
}

const behindKey = (reasons: StaleReason[]) => JSON.stringify(reasons.filter((r) => r.reason === 'behind'));

/**
 * Nodes with the "behind" reason worked out again from upstream build times, for its explanation
 * (which upstream, since when); whether a node is behind is the server's `status`. A node whose behind reasons
 * did not change is returned as it was, so views keyed on it do not redraw.
 */
export function withBehind(nodes: CatalogNode[]): CatalogNode[] {
	const dataAt = dataTimes(new Map(nodes.map((node) => [node.key, node])));
	return nodes.map((node) => {
		const behind = behindOf(node, dataAt);
		const own = node.stale ?? [];
		if (behindKey(own) === JSON.stringify(behind)) return node;
		return { ...node, stale: [...own.filter((r) => r.reason !== 'behind'), ...behind] };
	});
}

/** The listing with `states` applied; nodes without a state keep theirs, and states of unlisted nodes are ignored. */
export function applyNodeStates(nodes: CatalogNode[], states: Record<string, NodeState>): CatalogNode[] {
	let changed = false;
	const applied = nodes.map((node) => {
		const state = states[node.key];
		if (!state) return node;
		changed = true;
		return withState(node, state);
	});
	return changed ? withBehind(applied) : nodes;
}

/**
 * What to do with a `node_status` change given the version held: apply it, ignore one already
 * covered, or read every state again after a gap, a `full` change or before any version is held.
 */
export function deltaAction(held: number | null, delta: NodeStatusDelta): 'apply' | 'ignore' | 'refetch' {
	if (held !== null && delta.version <= held) return 'ignore';
	if (held === null || delta.full || delta.previousVersion !== held) return 'refetch';
	return 'apply';
}
