import type { components } from '$control/schema.d.ts';
import type { CatalogNode } from './types/build';
import type { NodeKind } from './types/operate';

type Wire = components['schemas'];

/** The catalog shows models, seeds and sources; audits, functions, macros and hooks are not catalog nodes. */
function kindOf(wire: Wire['CatalogNodeRecord']): NodeKind | null {
	if (wire.kind === 'source' || wire.kind === 'seed') return wire.kind;
	if (wire.kind !== 'model') return null;
	if (wire.materialization === 'view') return 'view';
	return wire.materialization?.startsWith('incremental') ? 'incremental' : 'table';
}

/**
 * One node of the current snapshot. The server reports declared metadata and the newest
 * fingerprint, not run outcomes, staleness, check counts, owners or groups, so those stay null
 * or empty and the pages hide them. The fingerprint's run is a `sqb` run, not a scheduler run.
 */
export function toCatalogNode(wire: Wire['CatalogNodeRecord']): CatalogNode | null {
	const kind = kindOf(wire);
	if (!kind) return null;
	return {
		key: wire.node_key,
		kind,
		materialization: kind === 'source' || kind === 'seed' ? null : (wire.materialization ?? null),
		group: null,
		tags: wire.tags,
		owner: [],
		description: wire.description ?? '',
		path: wire.path ?? '',
		upstream: wire.dependencies,
		relation: wire.relation ? { database: wire.relation.database ?? null, schema: wire.relation.schema, name: wire.relation.name } : null,
		columnCount: wire.column_count ?? 0,
		lastBuild: wire.last_build
			? { fingerprintRunId: wire.last_build.run_id, runId: null, jobKey: null, builtAt: wire.last_build.observed_at, fingerprint: wire.last_build.version_hash }
			: null,
		lastOutcome: null,
		stale: null,
		sourceFreshness: null,
		checks: null
	};
}

/** Catalog nodes in listing order, keeping only dependencies that are themselves listed. */
export function toCatalogNodes(wire: Wire['CatalogNodeRecord'][]): CatalogNode[] {
	const nodes = wire.flatMap((node) => toCatalogNode(node) ?? []);
	const listed = new Set(nodes.map((node) => node.key));
	return nodes.map((node) => ({ ...node, upstream: node.upstream.filter((key) => listed.has(key)) }));
}
