import { describe, expect, it } from 'vitest';
import type { CatalogNode, JobGroupMember, StaleReason } from '$lib/api';
import { layoutFolderGraph } from './folder-graph';
import { toggled, withOpen } from './member-expand';
import { edgeTitle, edgeWidth, innerId, memberGraph, memberLinks, memberNodeId, needsBuilding, pairKey, waitReasons, type MemberEdgeData, type MemberInfo } from './member-graph';

const member = (jobKey: string, stage: number, waitsFor: string[] = []): JobGroupMember => ({
	jobKey,
	stage,
	upstream: waitsFor.map((k) => ({ jobKey: k, source: null }))
});

function node(key: string, upstream: string[] = [], stale: StaleReason['reason'][] = []): CatalogNode {
	const reasons = stale.map((reason) => ({ reason, upstream: [], detail: '', since: '', change: 'definition' }) as unknown as StaleReason);
	return { key, kind: 'table', path: `models/${key.split(':')[1]}.sql`, upstream, group: null, tags: [], stale: reasons, lastOutcome: null, lastBuild: null } as unknown as CatalogNode;
}

// customers builds two staging models; orders reads both, through a source no member selects for one;
// shipments reads orders and stock; inventory has no lineage to shipments but the server orders it first.
const catalog = [
	node('model:stg_customers'),
	node('model:customer_dim', ['model:stg_customers'], ['changed']),
	node('source:raw_orders', ['model:customer_dim']),
	node('model:orders', ['model:stg_customers', 'source:raw_orders'], ['behind']),
	node('model:order_lines', ['model:orders', 'model:customer_dim'], ['changed']),
	node('model:shipments', ['model:order_lines'])
];
const members = [member('customers', 0), member('inventory', 0), member('orders', 1, ['customers']), member('shipments', 2, ['orders', 'inventory'])];
const nodesOf: Record<string, string[]> = {
	customers: ['model:stg_customers', 'model:customer_dim'],
	inventory: [],
	orders: ['model:orders', 'model:order_lines'],
	shipments: ['model:shipments']
};
const info = new Map<string, MemberInfo>(Object.entries(nodesOf).map(([k, nodes]) => [k, { nodeCount: nodes.length, lastRun: null, nodes }]));
const links = memberLinks(members, info, catalog);
const pairs = (edges: { source: string; target: string }[]) => edges.map((e) => `${e.source} > ${e.target}`);

describe('member links', () => {
	it('finds the node-level reasons for each wait, through nodes no member selects', () => {
		expect(links.get(pairKey('customers', 'orders'))).toEqual([
			{ from: 'model:stg_customers', to: 'model:orders' },
			{ from: 'model:customer_dim', to: 'model:orders' },
			{ from: 'model:customer_dim', to: 'model:order_lines' }
		]);
		expect(links.get(pairKey('orders', 'shipments'))).toEqual([{ from: 'model:order_lines', to: 'model:shipments' }]);
	});

	it('leaves a wait with no lineage between the members empty, and never counts lineage through another member', () => {
		expect(links.get(pairKey('inventory', 'shipments'))).toEqual([]);
		// shipments reads customer_dim only through orders' order_lines, so customers is not a reason for shipments.
		expect(links.has(pairKey('customers', 'shipments'))).toBe(false);
	});

	it('caps the reasons listed for a wait, counting the rest', () => {
		expect(waitReasons('orders', members, links, 2)).toEqual([{ upstream: 'customers', reasons: links.get(pairKey('customers', 'orders'))!.slice(0, 2), more: 1 }]);
	});
});

describe('member graph edges at the visible level', () => {
	it('draws one edge per pair of collapsed members, thicker and counted when many node links join them', () => {
		const { nodes, edges } = memberGraph(members, info, { catalog, links });
		expect(nodes.map((n) => n.type)).toEqual(['member', 'member', 'member', 'member']);
		expect(pairs(edges)).toEqual([
			`${memberNodeId('customers')} > ${memberNodeId('orders')}`,
			`${memberNodeId('orders')} > ${memberNodeId('shipments')}`,
			`${memberNodeId('inventory')} > ${memberNodeId('shipments')}`
		]);
		const [customers, , inventory] = edges.map((e) => e.data as MemberEdgeData);
		expect(customers.count).toBe(3);
		expect(String(edges[0].style)).toContain(`stroke-width: ${edgeWidth(3)}`);
		expect(edgeTitle(customers)).toBe('orders waits for customers: 3 node links');
		// A wait the lineage doesn't show stays a dashed edge between the cards.
		expect(inventory.count).toBe(0);
		expect(String(edges[2].style)).toContain('dasharray');
	});

	it('ends edges at the nodes inside an expanded member, still starting from collapsed members', () => {
		const { nodes, edges } = memberGraph(members, info, { catalog, links, expanded: new Set(['orders']) });
		const box = nodes.find((n) => n.id === memberNodeId('orders'))!;
		expect(box.type).toBe('memberBox');
		expect(nodes.filter((n) => n.parentId === box.id).map((n) => n.id)).toEqual([innerId('orders', 'model:orders'), innerId('orders', 'model:order_lines')]);
		expect(pairs(edges)).toEqual([
			`${memberNodeId('customers')} > ${innerId('orders', 'model:orders')}`,
			`${memberNodeId('customers')} > ${innerId('orders', 'model:order_lines')}`,
			`${innerId('orders', 'model:order_lines')} > ${memberNodeId('shipments')}`,
			`${memberNodeId('inventory')} > ${memberNodeId('shipments')}`,
			// Lineage inside the expanded member.
			`${innerId('orders', 'model:orders')} > ${innerId('orders', 'model:order_lines')}`
		]);
		expect((edges[0].data as MemberEdgeData).count).toBe(2);
	});

	it('draws node to node between two expanded members', () => {
		const { edges } = memberGraph(members, info, { catalog, links, expanded: new Set(['customers', 'orders']) });
		expect(pairs(edges).filter((p) => p.startsWith(`in:customers`) && p.includes('in:orders'))).toEqual([
			`${innerId('customers', 'model:stg_customers')} > ${innerId('orders', 'model:orders')}`,
			`${innerId('customers', 'model:customer_dim')} > ${innerId('orders', 'model:orders')}`,
			`${innerId('customers', 'model:customer_dim')} > ${innerId('orders', 'model:order_lines')}`
		]);
	});

	it('keeps a member with no known nodes as a card, even when asked to expand it', () => {
		const { nodes } = memberGraph(members, info, { catalog, links, expanded: new Set(['inventory']) });
		expect(nodes.find((n) => n.id === memberNodeId('inventory'))).toMatchObject({ type: 'member', data: { expandable: false, expanded: false } });
	});

	it('draws every wait between cards without links, and nothing for a group without members', () => {
		expect(pairs(memberGraph(members, info).edges)).toHaveLength(3);
		expect(memberGraph([], new Map())).toEqual({ nodes: [], edges: [] });
	});

	it('lays an expanded member out as a box around its nodes, upstream members to its left', () => {
		const { nodes, edges } = memberGraph(members, info, { catalog, links, expanded: new Set(['orders']) });
		const placed = layoutFolderGraph(nodes, edges);
		const at = (id: string) => placed.find((n) => n.id === id)!;
		const box = at(memberNodeId('orders'));
		const inner = at(innerId('orders', 'model:order_lines'));
		expect(box.width).toBeGreaterThanOrEqual(inner.position.x + 236);
		expect(box.height).toBeGreaterThan(inner.position.y);
		expect(at(memberNodeId('customers')).position.x).toBeLessThan(box.position.x);
		expect(at(memberNodeId('shipments')).position.x).toBeGreaterThan(box.position.x + box.width!);
	});
});

describe('member cards', () => {
	it('count the nodes needing a build: changed, and only behind', () => {
		const byKey = new Map(catalog.map((n) => [n.key, n]));
		expect(needsBuilding(nodesOf.orders, byKey)).toEqual({ changed: 1, behind: 1 });
		const { nodes } = memberGraph(members, info, { catalog, links });
		expect(nodes.find((n) => n.id === memberNodeId('customers'))!.data).toMatchObject({ jobKey: 'customers', nodeCount: 2, waitsFor: 0, needs: { changed: 1, behind: 0 } });
	});
});

describe('expand state', () => {
	it('opens a member, closes it again, and leaves others as they were', () => {
		const open = toggled(new Set(['customers']), 'orders');
		expect([...open]).toEqual(['customers', 'orders']);
		expect([...toggled(open, 'customers')]).toEqual(['orders']);
		expect([...withOpen(open, 'orders')]).toEqual(['customers', 'orders']);
	});
});
