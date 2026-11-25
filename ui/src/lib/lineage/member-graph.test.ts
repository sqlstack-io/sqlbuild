import { withServerStatus } from '$lib/build-area/status-fixtures';
import { describe, expect, it } from 'vitest';
import type { CatalogNode, JobGroupMember, StaleReason } from '$lib/api';
import { layoutFolderGraph } from './folder-graph';
import { toggled, withOpen } from './member-expand';
import { bigGroupFile, bigGroupShape } from '$lib/api/mock/big-group';
import { declarationFolder, memberFolderTree } from './member-folders';
import {
	directWaits,
	edgeTitle,
	edgeWidth,
	innerId,
	layoutMemberGraph,
	litEdges,
	memberFolderBoxId,
	memberFolderCardId,
	memberGraph,
	memberLinks,
	memberNodeId,
	publishedLinks,
	memberReach,
	needsBuilding,
	pairKey,
	transitiveReduction,
	waitReasons,
	type MemberEdgeData,
	type MemberInfo
} from './member-graph';

const member = (jobKey: string, stage: number, waitsFor: string[] = []): JobGroupMember => ({
	jobKey,
	stage,
	upstream: waitsFor.map((k) => ({ jobKey: k, source: null })),
	reasons: []
});

function node(key: string, upstream: string[] = [], stale: StaleReason['reason'][] = []): CatalogNode {
	const reasons = stale.map((reason) => ({ reason, upstream: [], detail: '', since: '', change: 'definition' }) as unknown as StaleReason);
	return withServerStatus({ key, kind: 'table', path: `sql/models/${key.split(':')[1]}.sql`, upstream, group: null, tags: [], stale: reasons, lastOutcome: null, lastBuild: null } as unknown as CatalogNode);
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

describe('published wait reasons', () => {
	// The server's reasons for the same members: shipments waits for inventory with none, an ordering.
	const published = members.map((m) =>
		m.jobKey === 'orders'
			? { ...m, reasons: [{ upstreamJob: 'customers', fromNode: 'model:customer_dim', toNode: 'model:orders' }] }
			: m.jobKey === 'shipments'
				? { ...m, reasons: [{ upstreamJob: 'orders', fromNode: 'model:order_lines', toNode: 'model:shipments' }] }
				: m
	);

	it("uses the server's reasons, each wait keyed as the lineage derivation keys it", () => {
		const links = publishedLinks(published)!;
		expect(links.get(pairKey('customers', 'orders'))).toEqual([{ from: 'model:customer_dim', to: 'model:orders' }]);
		expect(links.get(pairKey('orders', 'shipments'))).toEqual([{ from: 'model:order_lines', to: 'model:shipments' }]);
		expect(links.get(pairKey('inventory', 'shipments'))).toEqual([]);
	});

	it('draws a direct wait the server gives no reasons for dashed', () => {
		const { edges } = memberGraph(published, info, { catalog, links: publishedLinks(published)! });
		const style = (from: string, to: string) => String(edges.find((e) => e.source === memberNodeId(from) && e.target === memberNodeId(to))!.style);
		expect(style('inventory', 'shipments')).toContain('stroke-dasharray');
		expect(style('orders', 'shipments')).not.toContain('stroke-dasharray');
	});

	it('has none to give for a snapshot that predates them, so the page works them out from lineage', () => {
		expect(publishedLinks(members)).toBeNull();
	});
});

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
		// customers also feeds order_lines directly, but that edge follows from customers > orders >
		// order_lines, so only the first is drawn.
		expect(pairs(edges)).toEqual([
			`${memberNodeId('customers')} > ${innerId('orders', 'model:orders')}`,
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

describe('a large group whose waits list every member upstream', () => {
	const shape = bigGroupShape();
	const big = shape.members.map((m) => member(m, 0, shape.waits.get(m)));
	const closure = [...shape.waits.values()].reduce((n, w) => n + w.length, 0);
	const reduction = [...shape.direct.values()].reduce((n, w) => n + w.length, 0);
	const ends = (e: { data?: unknown }) => e.data as MemberEdgeData;

	it('draws only the direct waits: the transitive reduction, far fewer than the waits listed', () => {
		const { edges } = memberGraph(big, new Map());
		expect(shape.members.length).toBeGreaterThanOrEqual(40);
		expect(edges).toHaveLength(reduction);
		expect(closure).toBeGreaterThan(4 * reduction);
		expect(edges.length).toBeLessThanOrEqual(2 * shape.members.length);
		expect(new Set(edges.map((e) => `${ends(e).from}>${ends(e).to}`))).toEqual(new Set([...shape.direct].flatMap(([m, ups]) => ups.map((u) => `${u}>${m}`))));
		expect(directWaits(big).direct).toEqual(shape.direct);
	});

	it('lights a member’s whole upstream and downstream, implied waits included', () => {
		const { nodes, edges } = memberGraph(big, new Map());
		const reach = memberReach(big);
		const focus = 'join_c';
		const lit = litEdges(memberNodeId(focus), edges, nodes, reach);
		const related = new Set([focus, ...reach.up.get(focus)!, ...reach.down.get(focus)!]);
		expect(reach.up.get(focus)).toEqual(new Set(shape.waits.get(focus)));
		for (const e of edges.filter((e) => lit.has(e.id))) expect(related.has(ends(e).from) && related.has(ends(e).to)).toBe(true);
		// Every member upstream is reached through lit edges, and the rest of the graph is dimmed.
		const litFrom = new Set(edges.filter((e) => lit.has(e.id)).map((e) => ends(e).from));
		for (const u of reach.up.get(focus)!) expect(litFrom.has(u)).toBe(true);
		expect(lit.size).toBeLessThan(edges.length);
	});

	it('lists a member’s direct waits first, then each implied one with the wait it comes through', () => {
		const waits = waitReasons('clean_a', big, new Map());
		const direct = shape.direct.get('clean_a')!;
		expect(waits.filter((w) => w.through === undefined).map((w) => w.upstream)).toEqual(direct);
		const implied = waits.filter((w) => w.through !== undefined);
		expect(implied.map((w) => w.upstream).sort()).toEqual(shape.waits.get('clean_a')!.filter((u) => !direct.includes(u)).sort());
		for (const w of implied) expect(direct).toContain(w.through);
		expect(waits.findIndex((w) => w.through !== undefined)).toBe(direct.length);
	});

	it('puts members with no waits either way in a compact block under the graph', () => {
		const { nodes, edges } = memberGraph(big, new Map());
		const placed = layoutMemberGraph(nodes, edges);
		const at = (m: string) => placed.find((n) => n.id === memberNodeId(m))!.position;
		const graphBottom = Math.max(...shape.members.filter((m) => !shape.alone.includes(m)).map((m) => at(m).y));
		const block = shape.alone.map(at);
		for (const p of block) expect(p.y).toBeGreaterThan(graphBottom);
		expect(new Set(block.map((p) => p.y)).size).toBe(1);
	});
});

describe('transitive reduction', () => {
	const e = (source: string, target: string) => ({ source, target });
	it('drops an edge a longer path implies, keeps what only it joins, and survives a cycle', () => {
		expect(transitiveReduction([e('a', 'c'), e('a', 'b'), e('b', 'c')])).toEqual([e('a', 'b'), e('b', 'c')]);
		expect(transitiveReduction([e('a', 'b'), e('c', 'b')])).toHaveLength(2);
		expect(transitiveReduction([e('a', 'b'), e('b', 'a'), e('a', 'c'), e('b', 'c')])).toHaveLength(3);
		expect(transitiveReduction([e('a', 'c'), e('a', 'b'), e('b', 'c')], (x) => x.source !== 'a' || x.target !== 'c')).toHaveLength(3);
	});
});

describe('a large group drawn by the folders its members are declared in', () => {
	const shape = bigGroupShape();
	const big = shape.members.map((m) => member(m, 0, shape.waits.get(m)));
	const tree = memberFolderTree(shape.members, (m) => declarationFolder(bigGroupFile(m)));
	const stages = ['stage', 'clean', 'join', 'enrich', 'aggregate', 'publish', 'housekeeping'].map((s) => `jobs/warehouse/${s}`);

	it('draws each closed folder as one card, a lone member as its own card, and one line per pair of cards', () => {
		const { nodes, edges } = memberGraph(big, new Map(), { folders: { tree, open: new Set() } });
		expect(nodes.filter((n) => n.type === 'folder').map((n) => n.id).sort()).toEqual(stages.map(memberFolderCardId).sort());
		// load_a and publish_e are each alone in their folder, so they are drawn as themselves.
		expect(nodes.filter((n) => n.type === 'member').map((n) => n.id).sort()).toEqual([memberNodeId('load_a'), memberNodeId('publish_e')].sort());
		const pairs = edges.map((e) => `${e.source} > ${e.target}`).sort();
		const card = (s: string) => memberFolderCardId(`jobs/warehouse/${s}`);
		expect(pairs).toEqual(
			[
				`${memberNodeId('load_a')} > ${card('stage')}`,
				...['stage', 'clean', 'join', 'enrich', 'aggregate'].map((s, i, all) => `${card(s)} > ${card(all[i + 1] ?? 'publish')}`),
				`${card('aggregate')} > ${memberNodeId('publish_e')}`
			].sort()
		);
		// The line between two folders stands for every wait between their members, and lights with them.
		const stageToClean = edges.find((e) => e.source === card('stage') && e.target === card('clean'))!;
		expect((stageToClean.data as MemberEdgeData).pairs).toHaveLength(10);
	});

	it('draws an open folder as a box around its members, with the lines between members again', () => {
		const { nodes, edges } = memberGraph(big, new Map(), { folders: { tree, open: new Set(tree.folders.keys()) } });
		expect(nodes.filter((n) => n.parentId === memberFolderBoxId('jobs/warehouse/clean')).map((n) => n.id)).toEqual(['clean_a', 'clean_b', 'clean_c', 'clean_d', 'clean_e'].map(memberNodeId));
		expect(edges).toHaveLength([...shape.direct.values()].reduce((n, w) => n + w.length, 0));
	});
});
