// Test fixtures: a hand-built node with the status the server would give its fields, unless it
// already has one. Only tests import this; pages never work a status out.

import type { CatalogNode, NodeBuildStatus } from '$lib/api';

/** `node` with the server's status for its outcome, stale reasons and build. */
export function withServerStatus(node: CatalogNode): CatalogNode {
	if (node.status) return node;
	const stale = node.stale ?? [];
	const status: NodeBuildStatus =
		node.kind === 'source'
			? 'source'
			: node.lastOutcome === 'failed'
				? 'failed'
				: stale.some((r) => r.reason === 'changed')
					? 'changed'
					: stale.some((r) => r.reason === 'behind')
						? 'behind'
						: node.lastBuild || node.lastOutcome
							? 'current'
							: 'not_built';
	return { ...node, status, statusAt: node.lastBuild?.builtAt ?? null };
}
