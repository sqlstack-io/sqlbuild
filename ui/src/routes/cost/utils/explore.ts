// Rows of the explore view for each grouping, and the slices a selected row drills into. Pure.
import type { CostModel, CostPage, CostRow, CostRun } from '../types';

export type GroupBy = 'job' | 'folder' | 'model' | 'size' | 'run';
export type SortBy = 'cost' | 'change' | 'runs' | 'perRun';
export type ExploreRow = CostRow & { sub?: string };

export function exploreRows(cost: CostPage, by: GroupBy): ExploreRow[] {
	switch (by) {
		case 'job':
			return cost.jobs;
		case 'folder':
			return cost.folders;
		case 'size':
			return cost.sizes;
		case 'model':
			return cost.models.map((m) => ({ ...m, sub: m.folder }));
		case 'run':
			return cost.runs.map((r) => ({ key: r.id, sub: r.job ?? 'Manual or CLI', credits: r.credits, previous: 0, seconds: r.seconds, runs: 1, nodes: r.nodes }));
	}
}

export function sortRows(rows: ExploreRow[], by: SortBy): ExploreRow[] {
	const value = (r: ExploreRow) => (by === 'cost' ? r.credits : by === 'runs' ? r.runs : by === 'perRun' ? r.credits / Math.max(1, r.runs) : r.previous > 0 ? r.credits - r.previous : -Infinity);
	return [...rows].sort((a, b) => value(b) - value(a));
}

/** Sums of models per key, most expensive first. */
export function rollUp(models: CostModel[], key: (m: CostModel) => string): CostRow[] {
	const out = new Map<string, CostRow>();
	for (const m of models) {
		const k = key(m);
		const r = out.get(k) ?? { key: k, credits: 0, previous: 0, seconds: 0, runs: 0, nodes: 0 };
		r.credits += m.credits;
		r.previous += m.previous;
		r.seconds += m.seconds;
		r.runs = Math.max(r.runs, m.runs);
		r.nodes += 1;
		out.set(k, r);
	}
	return [...out.values()].sort((a, b) => b.credits - a.credits);
}

/** Runs rolled up by job, for a warehouse size's drill-down. */
export function runsByJob(runs: CostRun[]): CostRow[] {
	const out = new Map<string, CostRow>();
	for (const r of runs) {
		const k = r.job ?? 'Manual or CLI';
		const g = out.get(k) ?? { key: k, credits: 0, previous: 0, seconds: 0, runs: 0, nodes: r.nodes };
		g.credits += r.credits;
		g.seconds += r.seconds;
		g.runs += 1;
		out.set(k, g);
	}
	return [...out.values()].sort((a, b) => b.credits - a.credits);
}
