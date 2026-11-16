import { describe, expect, it } from 'vitest';
import type { Node } from '@xyflow/svelte';
import { CLEARANCE, obstacles, routedPath } from './edge-routes';

const card = (id: string, x: number, y: number, extra: Partial<Node> = {}): Node => ({ id, position: { x, y }, data: {}, width: 100, height: 40, ...extra });

/** The path's points: every coordinate pair in it. */
const points = (d: string) => [...d.matchAll(/(-?[\d.]+),(-?[\d.]+)/g)].map((m) => [Number(m[1]), Number(m[2])]);

describe('edge routes', () => {
	// Three columns 200 apart: a, then b in the way, then c; the edge a → c is long.
	const nodes = [card('a', 0, 0), card('b', 200, 0), card('b2', 200, 60), card('c', 400, 0)];

	it('draws the plain curve when no card is in the way', () => {
		const o = obstacles(nodes);
		expect(routedPath(o, 'a', 'b', 100, 20, 200, 20)).toBeNull();
	});

	it('runs level past a card in its way, clear of it, and bends only between columns', () => {
		const o = obstacles(nodes);
		const d = routedPath(o, 'a', 'c', 100, 20, 400, 20)!;
		const level = points(d).filter(([x]) => x >= 200 && x <= 300);
		expect(level.length).toBeGreaterThan(0);
		for (const [, y] of level) expect(y < 0 - CLEARANCE + 0.01 || (y > 40 + CLEARANCE - 0.01 && y < 60 - CLEARANCE + 0.01) || y > 100 + CLEARANCE - 0.01).toBe(true);
		// The b, b2 channel is too narrow to be worth it; above the column is the nearest roomy way.
		expect(level[0][1]).toBeLessThanOrEqual(-CLEARANCE);
	});

	it('bundles the edges into one card through the same heights', () => {
		const more = [...nodes, card('a2', 0, 120)];
		const o = obstacles(more);
		const first = points(routedPath(o, 'a', 'c', 100, 20, 400, 20)!).filter(([x]) => x > 200 && x < 300);
		const second = points(routedPath(o, 'a2', 'c', 100, 140, 400, 20)!).filter(([x]) => x > 200 && x < 300);
		expect(second.map(([, y]) => y)).toEqual(first.map(([, y]) => y));
	});

	it("doesn't route around the boxes an edge's own ends are in", () => {
		const boxed = [card('box', 150, -50, { width: 400, height: 200 }), card('inner', 20, 50, { parentId: 'box' }), card('a', 0, 0), card('c', 600, 0)];
		const o = obstacles(boxed);
		// a → inner enters the box: the box isn't in its way.
		expect(routedPath(o, 'a', 'inner', 100, 20, 170, 120)).toBeNull();
		// a → c passes the box, which is in its way.
		expect(routedPath(o, 'a', 'c', 100, 20, 600, 20)).not.toBeNull();
	});

	it('leaves edges unrouted while a card is dragged', () => {
		const o = obstacles(nodes.map((n) => (n.id === 'b' ? { ...n, dragging: true } : n)));
		expect(routedPath(o, 'a', 'c', 100, 20, 400, 20)).toBeNull();
	});
});
