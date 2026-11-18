// Edges routed through the gaps between cards instead of straight across them. Pure.
//
// Layouts place cards in columns with gaps between them. A long edge crosses columns on its way: it
// runs level through each column, at a height where no card is (keeping its height when it can,
// otherwise taking the gap nearest it), and bends only in the empty gaps between columns, so no part
// of it crosses a card. Edges into the same card share their heights column by column, so they run
// as one bundle, then merge into the trunk just before the card (trunk-path.ts). An edge with no card
// in its way is the ordinary trunk curve.

import type { Node } from '@xyflow/svelte';
import { getContext, setContext } from 'svelte';
import { TRUNK } from './trunk-path';

const KEY = Symbol('edge obstacles');
/** Shares the canvas's obstacles with its edges. */
export const shareObstacles = (get: () => Obstacles) => setContext(KEY, get);
/** The canvas's obstacles, for an edge to route around; undefined outside a canvas. */
export const sharedObstacles = (): (() => Obstacles) | undefined => getContext(KEY);

export type Box = { id: string; left: number; top: number; right: number; bottom: number };

/** Every drawn node's box in the flow, for routing edges around; built once per layout. */
export type Obstacles = {
	boxes: Box[];
	/** Each node's enclosing boxes, innermost first. */
	ancestors: Map<string, string[]>;
	/** Per target, the height each column's run was given, so edges into it bundle. */
	bundles: Map<string, Map<number, number>>;
	/** While a card is dragged, edges aren't routed: the layout is changing under them. */
	moving: boolean;
};

/** Space kept between an edge and a card it passes. */
export const CLEARANCE = 8;
/** Columns closer than this are one column: there is no room to bend between them. */
const MIN_GAP = 12;

/** The boxes of `nodes` in flow coordinates; positions are relative to a node's parent. */
export function obstacles(nodes: Node[]): Obstacles {
	const byId = new Map(nodes.map((n) => [n.id, n]));
	const origin = new Map<string, { x: number; y: number }>();
	const at = (n: Node): { x: number; y: number } => {
		const known = origin.get(n.id);
		if (known) return known;
		const parent = n.parentId ? byId.get(n.parentId) : undefined;
		const base = parent ? at(parent) : { x: 0, y: 0 };
		const p = { x: base.x + n.position.x, y: base.y + n.position.y };
		origin.set(n.id, p);
		return p;
	};
	const boxes: Box[] = [];
	const ancestors = new Map<string, string[]>();
	for (const n of nodes) {
		const chain: string[] = [];
		for (let p = n.parentId; p; p = byId.get(p)?.parentId) chain.push(p);
		ancestors.set(n.id, chain);
		const width = n.measured?.width ?? n.width;
		const height = n.measured?.height ?? n.height;
		if (n.hidden || !width || !height) continue;
		const { x, y } = at(n);
		boxes.push({ id: n.id, left: x, top: y, right: x + width, bottom: y + height });
	}
	boxes.sort((a, b) => a.left - b.left);
	return { boxes, ancestors, bundles: new Map(), moving: nodes.some((n) => n.dragging) };
}

type Run = { x1: number; x2: number; y: number };

/**
 * The path of the edge from (sx, sy) to (tx, ty) between `source` and `target`, around every other
 * box; null when nothing is in its way, or it runs backwards, and the plain curve does.
 */
export function routedPath(o: Obstacles, source: string, target: string, sx: number, sy: number, tx: number, ty: number): string | null {
	const jx = tx - TRUNK;
	if (o.moving || jx - sx < TRUNK) return null;
	const own = new Set([source, target, ...(o.ancestors.get(source) ?? []), ...(o.ancestors.get(target) ?? [])]);
	const columns: { left: number; right: number; boxes: Box[] }[] = [];
	for (const b of o.boxes) {
		if (b.left >= jx - 1) break;
		if (b.right <= sx + 1 || own.has(b.id)) continue;
		const last = columns.at(-1);
		if (last && b.left < last.right + MIN_GAP) {
			last.right = Math.max(last.right, b.right);
			last.boxes.push(b);
		} else columns.push({ left: b.left, right: b.right, boxes: [b] });
	}
	if (!columns.length) return null;
	let bundle = o.bundles.get(target);
	if (!bundle) o.bundles.set(target, (bundle = new Map()));
	const runs: Run[] = [];
	let y = sy;
	for (const column of columns) {
		const blocked = merge(column.boxes.map((b) => [b.top - CLEARANCE, b.bottom + CLEARANCE] as const));
		const free = (v: number) => !blocked.some(([a, b]) => v > a && v < b);
		const key = Math.round(column.left);
		const shared = bundle.get(key);
		const next = shared !== undefined && free(shared) ? shared : free(y) ? y : nearestFree(blocked, y, ty);
		if (shared === undefined) bundle.set(key, next);
		runs.push({ x1: Math.max(column.left, sx), x2: Math.min(column.right, jx), y: next });
		y = next;
	}
	return pathThrough(sx, sy, runs, jx, tx, ty);
}

/** Overlapping intervals merged, in order. */
function merge(spans: (readonly [number, number])[]): [number, number][] {
	const out: [number, number][] = [];
	for (const [a, b] of [...spans].sort((p, q) => p[0] - q[0])) {
		const last = out.at(-1);
		if (last && a <= last[1]) last[1] = Math.max(last[1], b);
		else out.push([a, b]);
	}
	return out;
}

/** A channel narrower than this between two cards is avoided when a roomier one is near. */
const ROOMY = 24;

/**
 * The free height nearest `y`, leaning towards the target's height `ty`: the middle of a channel
 * between cards, or just past the cards above or below them all, preferring a roomy channel.
 */
function nearestFree(blocked: [number, number][], y: number, ty: number): number {
	const gaps: [number, number][] = [];
	let from = -Infinity;
	for (const [a, b] of blocked) {
		gaps.push([from, a]);
		from = b;
	}
	gaps.push([from, Infinity]);
	let best = y;
	let cost = Infinity;
	for (const [a, b] of gaps) {
		const room = b - a;
		const v = Number.isFinite(room) ? (a + b) / 2 : Math.min(Math.max(y, a), b);
		const c = Math.abs(v - y) + 0.5 * Math.abs(v - ty) + Math.max(0, ROOMY - room) * 8;
		if (c < cost) [best, cost] = [v, c];
	}
	return best;
}

/** Level through each run, bending between them; then into the trunk, level, `TRUNK` before the target. */
function pathThrough(sx: number, sy: number, runs: Run[], jx: number, tx: number, ty: number): string {
	let d = `M${sx},${sy}`;
	let [x, y] = [sx, sy];
	const bendTo = (x2: number, y2: number) => {
		if (Math.abs(y2 - y) < 0.5) d += ` L${x2},${y2}`;
		else {
			const h = (x2 - x) / 2;
			d += ` C${x + h},${y} ${x2 - h},${y2} ${x2},${y2}`;
		}
		[x, y] = [x2, y2];
	};
	for (const run of runs) {
		bendTo(run.x1, run.y);
		d += ` L${run.x2},${run.y}`;
		x = run.x2;
	}
	bendTo(jx, ty);
	return `${d} L${tx},${ty}`;
}
