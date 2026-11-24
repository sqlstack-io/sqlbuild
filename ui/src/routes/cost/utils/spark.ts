// A row's cost per day as a smooth filled line, its SVG paths worked out once per series. Pure apart
// from the cache, which holds each series' paths while the series is alive.

import { monotonePath, plot } from './smooth';

/** The trend's height in its viewBox; its width is 100. */
export const SPARK_H = 20;

const held = new WeakMap<readonly number[], { area: string; line: string }>();

/** The line through `values` and the area under it. */
export function sparkPaths(values: readonly number[]): { area: string; line: string } {
	let paths = held.get(values);
	if (!paths) {
		const line = monotonePath(plot(values, 100, SPARK_H, 2));
		paths = { line, area: values.length ? `${line} L100,${SPARK_H} L0,${SPARK_H} Z` : '' };
		held.set(values, paths);
	}
	return paths;
}
