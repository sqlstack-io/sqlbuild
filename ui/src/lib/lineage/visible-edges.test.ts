import { describe, expect, it } from 'vitest';
import type { CatalogNode } from '$lib/api';
import { folderGraph, folderTree, standInOf } from './folder-graph';
import { edgeStyle, edgeWidth, edgesTouching, visibleEdges, type VisibleEdgeData } from './visible-edges';

function node(key: string, path: string, upstream: string[] = []): CatalogNode {
	return { key, kind: 'table', path, upstream, group: null, tags: [], stale: [], lastOutcome: null, lastBuild: null } as unknown as CatalogNode;
}

// Three staging models feed four marts models; inventory reads one mart.
const nodes = [
	node('model:stg_orders', 'sql/models/staging/stg_orders.sql'),
	node('model:stg_customers', 'sql/models/staging/stg_customers.sql'),
	node('model:stg_items', 'sql/models/staging/stg_items.sql'),
	node('model:orders', 'sql/models/marts/orders.sql', ['model:stg_orders', 'model:stg_items']),
	node('model:customers', 'sql/models/marts/customers.sql', ['model:stg_customers']),
	node('model:order_totals', 'sql/models/marts/order_totals.sql', ['model:orders', 'model:stg_orders']),
	node('model:inventory', 'sql/models/inventory/inventory.sql', ['model:orders'])
];
const tree = folderTree(nodes);

describe('edges at the visible level', () => {
	it('draws one edge per distinct visible pair when folders are closed, counting the node links', () => {
		const graph = folderGraph(nodes, new Set(), { tree });
		const standIn = standInOf(tree, new Set());
		const pairs = new Set(nodes.flatMap((n) => n.upstream.map((up) => `${standIn(up)}>${standIn(n.key)}`)).filter((p) => p.split('>')[0] !== p.split('>')[1]));
		expect(graph.edges).toHaveLength(pairs.size);
		const toMarts = graph.edges.find((e) => e.source === 'group:staging' && e.target === 'group:marts')!;
		expect((toMarts.data as VisibleEdgeData).count).toBe(4);
		expect((toMarts.data as VisibleEdgeData).title).toBe('4 lineage links from staging to marts');
		expect(String(toMarts.style)).toContain(`stroke-width: ${edgeWidth(4)}`);
	});

	it('ends an open folder’s edges at its visible children, never at a node inside a closed folder', () => {
		const graph = folderGraph(nodes, new Set(['marts']), { tree });
		const ends = new Set(graph.edges.flatMap((e) => [e.source, e.target]));
		expect([...ends].sort()).toEqual(['group:inventory', 'group:staging', 'model:customers', 'model:order_totals', 'model:orders']);
		const drawn = new Set(graph.nodes.map((n) => n.id));
		expect([...ends].every((id) => drawn.has(id))).toBe(true);
	});

	it('leaves out links inside one closed box', () => {
		expect(visibleEdges([{ from: 'a', to: 'b' }], () => 'box')).toEqual([]);
	});
});

describe('highlighting', () => {
	it('lights the edges of a card, or of a box and everything inside it', () => {
		const graph = folderGraph(nodes, new Set(['marts']), { tree });
		const lit = edgesTouching('folder:marts', graph.edges, graph.nodes);
		expect(lit.size).toBe(graph.edges.length);
		expect([...edgesTouching('model:customers', graph.edges, graph.nodes)]).toEqual(['group:staging-model:customers']);
	});

	it('keeps an edge’s thickness while lit or dimmed', () => {
		expect(edgeStyle(8, 'dim')).toContain(`stroke-width: ${edgeWidth(8)}`);
		expect(edgeStyle(8, 'on')).toContain('var(--primary)');
	});
});
