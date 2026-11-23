// Why a skipped node was skipped, naming the chain of skips back to the failure. Pure.

import type { RunEdge, RunNode } from '$lib/api';
import { nodeLabel } from '$lib/operate/labels';

/**
 * Why a skipped node was skipped, on one line, naming the chain: a node under a failure says that node
 * failed; one under another skipped node says that node was skipped, and after which failure.
 */
export function skipCause(n: RunNode, nodes: readonly RunNode[], edges: readonly RunEdge[]): string | null {
	if (n.status !== 'skipped') return null;
	const why = n.skippedBecauseOf;
	if (!why) return 'skipped: an upstream node did not run';
	const across = why.runId !== n.runId ? ` in ${why.jobKey ?? why.runId}` : '';
	const byKey = new Map(nodes.map((x) => [x.nodeKey, x]));
	const parents = edges.filter((e) => e.to === n.nodeKey).map((e) => byKey.get(e.from));
	if (parents.some((p) => p?.nodeKey === why.nodeKey)) return `skipped: ${nodeLabel(why.nodeKey)} failed${across}`;
	const via = parents.find((p) => p?.status === 'skipped' && p.skippedBecauseOf?.nodeKey === why.nodeKey);
	if (via) return `skipped: ${nodeLabel(via.nodeKey)} was skipped (after ${nodeLabel(why.nodeKey)} failed${across})`;
	return `skipped: ${nodeLabel(why.nodeKey)} failed${across}`;
}
