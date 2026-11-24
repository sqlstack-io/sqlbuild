// A selection's cost read from cost rows, as the server sums them: whole UTC days ending today, the
// period before, job by node, and daily series by job, top-level folder or any group. Pure.

import type { CostFact, CostSelection, CostSeriesGroup, SelectionProblem } from '../types';

/** One selected node's cost in one build. */
export type CostRowIn = { runId: string; job: string | null; nodeKey: string; folder: string; startedAt: number; pending: boolean; credits: number; usd: number | null; seconds: number; queries: number; bytes: number };

const DAY = 86_400_000;

/** The period's first day (ms), its dates, and the day index of a time (null before it). */
export function costPeriod(days: number, now: number) {
	const first = Math.floor(now / DAY) * DAY - (days - 1) * DAY;
	const dates = Array.from({ length: days }, (_, i) => new Date(first + i * DAY).toISOString().slice(0, 10));
	const day = (at: number) => (at < first ? null : Math.min(days - 1, Math.floor((at - first) / DAY)));
	return { first, previous: first - days * DAY, dates, day };
}

/** A node's folder from its published path: without `sql/models/` (or `models/`), other roots as published. */
export function costFolder(path: string | undefined): string {
	if (!path) return 'other';
	const dir = path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '';
	return dir.replace(/^(sql\/)?models(\/|$)/, '');
}

const addUsd = (a: number | null, b: number | null) => (a === null || b === null ? null : a + b);

export function costSelectionOf(rows: CostRowIn[], days: number, now: number, error: SelectionProblem | null = null): CostSelection {
	const period = costPeriod(days, now);
	const facts = new Map<string, CostFact & { runSet: Set<string> }>();
	const jobDaily = new Map<string | null, number[]>();
	const folderDaily = new Map<string, number[]>();
	const runs = new Set<string>();
	const pending = new Set<string>();
	let usd: number | null = 0;
	let previousUsd: number | null = 0;
	for (const r of rows) {
		if (r.startedAt < period.previous) continue;
		const id = `${r.job ?? ''}\u0000${r.nodeKey}`;
		let f = facts.get(id);
		if (!f) facts.set(id, (f = { job: r.job, nodeKey: r.nodeKey, folder: r.folder, credits: 0, previousCredits: 0, usd: 0, previousUsd: 0, seconds: 0, runs: 0, queries: 0, bytesScanned: 0, runSet: new Set() }));
		const day = period.day(r.startedAt);
		if (day === null) {
			f.previousCredits += r.credits;
			f.previousUsd = addUsd(f.previousUsd, r.usd);
			previousUsd = addUsd(previousUsd, r.usd);
			continue;
		}
		f.credits += r.credits;
		f.usd = addUsd(f.usd, r.usd);
		f.seconds += r.seconds;
		f.runSet.add(r.runId);
		f.queries += r.queries;
		f.bytesScanned += r.bytes;
		usd = addUsd(usd, r.usd);
		runs.add(r.runId);
		if (r.pending) pending.add(r.runId);
		const job = jobDaily.get(r.job) ?? jobDaily.set(r.job, Array(days).fill(0)).get(r.job)!;
		job[day] += r.credits;
		const top = r.folder.split('/')[0];
		const folder = folderDaily.get(top) ?? folderDaily.set(top, Array(days).fill(0)).get(top)!;
		folder[day] += r.credits;
	}
	const list: CostFact[] = [...facts.values()].map(({ runSet, ...f }) => ({ ...f, runs: runSet.size })).sort((a, b) => b.credits - a.credits);
	const sum = (pick: (f: CostFact) => number) => list.reduce((t, f) => t + pick(f), 0);
	const credits = sum((f) => f.credits);
	const priced = runs.size ? usd : null;
	return {
		snapshotId: 'mock-snapshot',
		since: new Date(period.first).toISOString(),
		days,
		dates: period.dates,
		error,
		price: priced !== null && credits > 0 ? { usdPerCredit: priced / credits, source: 'configured' } : null,
		credits,
		previousCredits: sum((f) => f.previousCredits),
		usd: priced,
		previousUsd,
		runs: runs.size,
		incompleteRuns: pending.size,
		queries: sum((f) => f.queries),
		seconds: sum((f) => f.seconds),
		bytesScanned: sum((f) => f.bytesScanned),
		facts: error ? [] : list,
		jobDaily: [...jobDaily].map(([key, credits]) => ({ key, credits })),
		folderDaily: [...folderDaily].map(([key, credits]) => ({ key, credits }))
	};
}

/** Whether a row is in a group: every field the group gives matches. */
export function inCostGroup(g: CostSeriesGroup, r: { job: string | null; folder: string; nodeKey: string }): boolean {
	const job = g.manual ? r.job === null : g.job === undefined || r.job === g.job;
	const folder = !g.folder || r.folder === g.folder || r.folder.startsWith(`${g.folder}/`);
	return job && folder && (g.nodeKey === undefined || r.nodeKey === g.nodeKey);
}

/** Each group's credits per day of the period. */
export function costSeriesOf(rows: CostRowIn[], days: number, now: number, groups: CostSeriesGroup[]): number[][] {
	const period = costPeriod(days, now);
	const out = groups.map(() => Array<number>(days).fill(0));
	for (const r of rows) {
		const day = period.day(r.startedAt);
		if (day === null) continue;
		groups.forEach((g, i) => {
			if (inCostGroup(g, r)) out[i][day] += r.credits;
		});
	}
	return out;
}
