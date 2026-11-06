import { COL_STEP, NODE_H, NODE_W, ROW_STEP, type GraphLayout, type GraphNode } from './graph-layout';

// Far-zoom rendering for large graphs: when the lineage page is zoomed out
// beyond the point where cards are legible, cards are hidden and this canvas
// draws every model as a status bar plus every edge, in the flow's coordinates.

export type View = { x: number; y: number; zoom: number };

export type DrawState = {
	layout: GraphLayout;
	view: View;
	width: number;
	height: number;
	dpr: number;
	/** the page's lit set (selection lineage); everything else is dimmed */
	lit: Set<string> | null;
	selected: Set<string>;
};

export function hitTest(sx: number, sy: number, layout: GraphLayout, view: View): GraphNode | null {
	const wx = (sx - view.x) / view.zoom;
	const wy = (sy - view.y) / view.zoom;
	const d = Math.floor(wx / COL_STEP);
	if (d < 0 || d >= layout.columns.length || wx - d * COL_STEP > NODE_W) return null;
	const i = Math.floor((wy - layout.colTop[d]) / ROW_STEP);
	const n = layout.columns[d][i];
	return n && wy >= n.y && wy - n.y <= NODE_H ? n : null;
}

function css(name: string): string {
	return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#888';
}

export function drawFar(ctx: CanvasRenderingContext2D, s: DrawState): void {
	const { layout, view } = s;
	const k = view.zoom;
	const c = {
		edge: css('--sb-edge'),
		primary: css('--primary'),
		ok: css('--sb-success'),
		stale: css('--sb-stale'),
		fail: css('--sb-error'),
		src: css('--sb-layer-source')
	};
	const statusColour = (st: string) => (st === 'fail' ? c.fail : st === 'stale' ? c.stale : st === 'src' ? c.src : c.ok);

	ctx.setTransform(s.dpr, 0, 0, s.dpr, 0, 0);
	ctx.clearRect(0, 0, s.width, s.height);
	ctx.setTransform(s.dpr * k, 0, 0, s.dpr * k, s.dpr * view.x, s.dpr * view.y);

	const left = -view.x / k;
	const right = (s.width - view.x) / k;
	const top = -view.y / k;
	const bottom = (s.height - view.y) / k;
	const lit = s.lit;

	const dim = new Path2D();
	const hot = new Path2D();
	for (const [a, b] of layout.edges) {
		const p = layout.byId.get(a)!;
		const q = layout.byId.get(b)!;
		const x1 = p.x + NODE_W;
		const x2 = q.x;
		if (x2 < left || x1 > right) continue;
		const y1 = p.y + NODE_H / 2;
		const y2 = q.y + NODE_H / 2;
		if (Math.max(y1, y2) < top || Math.min(y1, y2) > bottom) continue;
		const path = lit && lit.has(a) && lit.has(b) ? hot : dim;
		const mx = (x1 + x2) / 2;
		path.moveTo(x1, y1);
		path.bezierCurveTo(mx, y1, mx, y2, x2, y2);
	}
	ctx.lineWidth = 1 / k;
	ctx.strokeStyle = c.edge;
	ctx.globalAlpha = lit ? 0.12 : 0.5;
	ctx.stroke(dim);
	ctx.globalAlpha = 1;
	ctx.strokeStyle = c.primary;
	ctx.lineWidth = 1.5 / k;
	ctx.stroke(hot);

	for (const n of layout.nodes) {
		if (n.x > right || n.x + NODE_W < left || n.y > bottom || n.y + NODE_H < top) continue;
		ctx.globalAlpha = lit && !lit.has(n.id) ? 0.15 : 1;
		ctx.fillStyle = statusColour(n.status);
		ctx.fillRect(n.x, n.y, NODE_W, NODE_H);
	}
	ctx.globalAlpha = 1;

	ctx.strokeStyle = c.primary;
	ctx.lineWidth = 3 / k;
	for (const id of s.selected) {
		const n = layout.byId.get(id);
		if (n) ctx.strokeRect(n.x - 8 / k, n.y - 8 / k, NODE_W + 16 / k, NODE_H + 16 / k);
	}
}
