import { describe, expect, it } from 'vitest';
import type { JobGroupMember } from '$lib/api';
import { maxColumnCards } from './job-graph';
import { MEMBER_WIDTH, memberColumns, memberGraph, memberNodeId } from './member-graph';

const member = (jobKey: string, stage: number, waitsFor: string[] = []): JobGroupMember => ({
	jobKey,
	stage,
	upstream: waitsFor.map((k) => ({ jobKey: k, source: null }))
});

// customers and inventory load first; orders waits for customers; shipments waits for orders and inventory.
const members = [
	member('customers', 0),
	member('inventory', 0),
	member('orders', 1, ['customers']),
	member('shipments', 2, ['orders', 'inventory'])
];

describe('member columns', () => {
	it('puts each member in its wait-order stage, left to right', () => {
		expect(memberColumns(members)).toEqual([['customers', 'inventory'], ['orders'], ['shipments']]);
	});

	it('orders a column by the rows of the members it waits for, then by server order', () => {
		const crossed = [
			member('customers', 0),
			member('inventory', 0),
			member('stock_levels', 1, ['inventory']),
			member('orders', 1, ['customers']),
			member('audit', 1)
		];
		// orders waits for row 0, stock_levels for row 1; audit waits for nothing and goes last.
		expect(memberColumns(crossed)[1]).toEqual(['orders', 'stock_levels', 'audit']);
	});

	it('wraps a stage taller than the column limit into evenly filled columns', () => {
		const wide = Array.from({ length: 49 }, (_, i) => member(`orders_${String(i).padStart(2, '0')}`, 0));
		const columns = memberColumns(wide);
		const max = maxColumnCards(wide.length);
		expect(columns.length).toBe(Math.ceil(49 / max));
		expect(columns.every((c) => c.length <= max)).toBe(true);
		expect(columns.flat()).toEqual(wide.map((m) => m.jobKey));
	});
});

describe('member graph', () => {
	const info = new Map([
		['orders', { nodeCount: 12, lastRun: { id: 'run-1', status: 'success' as const, startedAt: '2026-01-01T00:00:00Z', endedAt: '2026-01-01T00:01:00Z' } }],
		['customers', { nodeCount: 3, lastRun: null }]
	]);

	it('draws one fixed-size card per member with what the jobs list knows', () => {
		const { nodes } = memberGraph(members, info);
		expect(nodes.map((n) => n.id)).toEqual(['customers', 'inventory', 'orders', 'shipments'].map(memberNodeId));
		const orders = nodes.find((n) => n.id === memberNodeId('orders'))!;
		expect(orders.data).toMatchObject({ jobKey: 'orders', nodeCount: 12, waitsFor: 1 });
		expect(orders.data.lastRun).toMatchObject({ status: 'success' });
		expect(nodes.find((n) => n.id === memberNodeId('inventory'))!.data).toMatchObject({ nodeCount: null, lastRun: null });
		expect(nodes.every((n) => n.width === MEMBER_WIDTH && n.height)).toBe(true);
	});

	it('places later stages further right and centres a short column on the tallest', () => {
		const { nodes } = memberGraph(members, info);
		const at = (k: string) => nodes.find((n) => n.id === memberNodeId(k))!.position;
		expect(at('customers').x).toBeLessThan(at('orders').x);
		expect(at('orders').x).toBeLessThan(at('shipments').x);
		expect(at('customers').x).toBe(at('inventory').x);
		expect(at('orders').y).toBe((at('customers').y + at('inventory').y) / 2);
	});

	it('draws an edge from each member to the members waiting for it, ignoring waits outside the group', () => {
		const outside = [...members, member('returns', 1, ['billing', 'orders', 'orders'])];
		const { edges } = memberGraph(outside, info);
		expect(edges.map((e) => `${e.source}>${e.target}`)).toEqual([
			`${memberNodeId('customers')}>${memberNodeId('orders')}`,
			`${memberNodeId('orders')}>${memberNodeId('shipments')}`,
			`${memberNodeId('inventory')}>${memberNodeId('shipments')}`,
			`${memberNodeId('orders')}>${memberNodeId('returns')}`
		]);
	});

	it('draws nothing for a group without members', () => {
		expect(memberGraph([], new Map())).toEqual({ nodes: [], edges: [] });
	});
});
