// Smooth lines through daily points: monotone cubic interpolation (Fritsch–Carlson), which passes
// through every point and never overshoots between them, so a day is never drawn below zero or
// above its neighbours. Pure.

/** An SVG path through `points`, sorted by x. */
export function monotonePath(points: readonly [number, number][]): string {
	const n = points.length;
	if (n === 0) return '';
	if (n === 1) return `M${points[0][0]},${points[0][1]}`;
	const dx: number[] = [];
	const slope: number[] = [];
	for (let i = 0; i < n - 1; i++) {
		dx.push(points[i + 1][0] - points[i][0]);
		slope.push(dx[i] === 0 ? 0 : (points[i + 1][1] - points[i][1]) / dx[i]);
	}
	const t: number[] = [slope[0]];
	for (let i = 1; i < n - 1; i++) t.push(slope[i - 1] * slope[i] <= 0 ? 0 : (slope[i - 1] + slope[i]) / 2);
	t.push(slope[n - 2]);
	for (let i = 0; i < n - 1; i++) {
		if (slope[i] === 0) {
			t[i] = 0;
			t[i + 1] = 0;
			continue;
		}
		const a = t[i] / slope[i];
		const b = t[i + 1] / slope[i];
		const s = a * a + b * b;
		if (s > 9) {
			const tau = 3 / Math.sqrt(s);
			t[i] = tau * a * slope[i];
			t[i + 1] = tau * b * slope[i];
		}
	}
	let d = `M${points[0][0]},${points[0][1]}`;
	for (let i = 0; i < n - 1; i++) {
		const [x0, y0] = points[i];
		const [x1, y1] = points[i + 1];
		const h = dx[i] / 3;
		d += ` C${x0 + h},${y0 + t[i] * h} ${x1 - h},${y1 - t[i + 1] * h} ${x1},${y1}`;
	}
	return d;
}

/** Points for `values` across a `width` × `height` box, the largest at the top less `pad`. */
export function plot(values: readonly number[], width: number, height: number, pad = 1): [number, number][] {
	const max = Math.max(...values, 0) || 1;
	const step = values.length > 1 ? width / (values.length - 1) : 0;
	return values.map((v, i) => [values.length > 1 ? i * step : width / 2, height - (v / max) * (height - pad * 2) - pad / 2]);
}
