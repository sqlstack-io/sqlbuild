// A node that printed about 2 million lines (prototype, CHI-1597), and the server's paging over it.
// Line N is computed from N, never stored, so the mock pages, searches and indexes the way the output
// store would without holding the output anywhere: the browser only ever sees a page.

import type { NodeOutputIndex, NodeOutputLevel, NodeOutputLine, NodeOutputPage, NodeOutputRow, NodeOutputSearch, RunNode } from '../types';

export const BIG_NODE = 'notebook_runner';
export const BIG_TOTAL = 2_031_847;
/** Lines a second while it runs, for a node still printing. */
export const BIG_RATE = 9_000;
const CELLS = 48;
const CELL = Math.floor(BIG_TOTAL / CELLS);
const NAMES = ['load_inputs', 'clean_orders', 'join_customers', 'feature_window', 'train_split', 'score_batch'];

const cellOf = (n: number) => Math.min(CELLS - 1, Math.floor((n - 1) / CELL));
const cellLen = (c: number) => (c === CELLS - 1 ? BIG_TOTAL - CELL * (CELLS - 1) : CELL);
/** Where each cell's warning sits, and the two cells with an error. */
const warnAt = (c: number) => 1_200 + ((c * 7_919) % 30_000);
const ERROR_CELLS = new Set([17, 31]);
const errorAt = (c: number) => 20_000 + c * 13;

/** Line `n` (1-based) of the node's output. */
export function bigLine(n: number): NodeOutputLine {
	const c = cellOf(n);
	const k = n - 1 - c * CELL;
	const len = cellLen(c);
	const name = `${NAMES[c % NAMES.length]}_${String(c + 1).padStart(2, '0')}`;
	const out = (text: string, stream: 'stdout' | 'stderr' = 'stdout', level: NodeOutputLevel = 'info'): NodeOutputLine => ({ n, stream, text, level });
	if (k === 0) return out(`[cell ${c + 1}/${CELLS}] ▶ ${name}`);
	if (k === 1) return out(`  reading partition date=2026-11-${String(1 + (c % 20)).padStart(2, '0')} (${len - 5} batches)`);
	if (k === warnAt(c)) return out(`WARNING: cell ${c + 1}: ${c % 2 ? 'skew detected in batch' : 'slow batch'} ${k - 1} (${2 + (c % 5)}.${c % 10}s)`, 'stderr', 'warn');
	if (ERROR_CELLS.has(c) && k === errorAt(c)) return out(`ERROR: cell ${c + 1}: worker 3 lost during batch ${k - 1}; retrying from checkpoint`, 'stderr', 'error');
	if (k === len - 3) return out(`  ✓ cell ${c + 1} done in ${40 + (c % 17)}s`);
	if (k === len - 2) return out(`  wrote ${1000 + ((c * 37) % 9000)} rows to analytics.notebook_${name}`);
	if (k === len - 1) return out('');
	return out(`  batch ${k - 1}/${len - 5}  rows=${(k * 17) % 5000}  ${3 + ((k * 7) % 40)}ms`);
}

/** A cell's lines that are not progress: its header, warning, error and closing lines, as offsets. */
function specials(c: number): number[] {
	const len = cellLen(c);
	return [0, 1, warnAt(c), ...(ERROR_CELLS.has(c) ? [errorAt(c)] : []), len - 3, len - 2, len - 1].sort((a, b) => a - b);
}

/**
 * How many similar lines run from line `n` in `step`'s direction: the progress lines between two of a
 * cell's other lines read the same but for their numbers. Worked out, not scanned, as an index would.
 */
function similarRun(n: number, step: 1 | -1, available: number): number {
	const c = cellOf(n);
	const k = n - 1 - c * CELL;
	const sp = specials(c);
	if (sp.includes(k)) return 1;
	const end = step > 0 ? sp.find((x) => x > k)! : [...sp].reverse().find((x) => x < k)!;
	const count = Math.abs(end - k);
	return step > 0 ? Math.min(count, available - n + 1) : Math.min(count, n);
}

export function bigIndex(available: number, live: boolean): NodeOutputIndex {
	const marks: NodeOutputIndex['marks'] = [];
	for (let c = 0; c < CELLS; c++) {
		const base = c * CELL + 1;
		const w = base + warnAt(c);
		if (w <= available) marks.push({ line: w, level: 'warn', text: bigLine(w).text });
		if (ERROR_CELLS.has(c) && base + errorAt(c) <= available) marks.push({ line: base + errorAt(c), level: 'error', text: bigLine(base + errorAt(c)).text });
	}
	marks.sort((a, b) => a.line - b.line);
	const stderr = marks.length;
	return { total: available, stdout: available - stderr, stderr, bytes: Math.round(available * 46.3), marks, live };
}

export type PageAt = { from: number; rows: number; direction: 'forward' | 'backward'; collapse: boolean; stream?: 'all' | 'stdout' | 'stderr'; around?: number };

/**
 * Up to `rows` rows from line `from`, forwards, or backwards ending just before `from`; similar lines
 * folded when `collapse`, except the block holding `around` (a jump's target), which opens around it.
 * `stream` keeps only stdout or stderr lines, keeping their line numbers.
 */
export function bigPage(available: number, at: PageAt): NodeOutputPage {
	const step = at.direction === 'forward' ? 1 : -1;
	const rows: NodeOutputRow[] = [];
	if (at.stream === 'stderr') {
		// stderr lines are the index's marks: read them from the index, not by walking the output.
		const lines = bigIndex(available, false).marks.map((m) => m.line);
		const picked = step > 0 ? lines.filter((l) => l >= at.from).slice(0, at.rows) : lines.filter((l) => l < at.from).slice(-at.rows);
		for (const l of picked) rows.push({ kind: 'line', line: bigLine(l) });
	} else {
		let n = step > 0 ? Math.max(1, at.from) : Math.min(available, at.from - 1);
		while (rows.length < at.rows && n >= 1 && n <= available) {
			const first = bigLine(n);
			if (at.stream === 'stdout' && first.stream === 'stderr') {
				n += step;
				continue;
			}
			let count = at.collapse ? similarRun(n, step, available) : 1;
			const lo = step > 0 ? n : n - count + 1;
			if (count >= 4 && at.around != null && at.around >= lo && at.around < lo + count) {
				// The jump's target is inside this block: fold what comes before it, then show its lines.
				const before = step > 0 ? at.around - 3 - n : n - (at.around + 3);
				if (before >= 4) {
					rows.push({ kind: 'similar', from: step > 0 ? n : at.around + 4, count: before, sample: bigLine(step > 0 ? n : at.around + 4) });
					n += step * before;
				}
				const near = Math.min(7, Math.abs(n - (step > 0 ? at.around + 4 : at.around - 4)));
				for (let i = 0; i < near && n >= 1 && n <= available; i++, n += step) rows.push({ kind: 'line', line: bigLine(n) });
				continue;
			}
			if (count >= 4) rows.push({ kind: 'similar', from: lo, count, sample: bigLine(lo) });
			else {
				count = 1;
				rows.push({ kind: 'line', line: first });
			}
			n += step * count;
		}
	}
	if (step < 0) rows.reverse();
	const firstLine = (r: NodeOutputRow) => (r.kind === 'line' ? r.line.n : r.from);
	const lastLine = (r: NodeOutputRow) => (r.kind === 'line' ? r.line.n : r.from + r.count - 1);
	return { total: available, from: rows.length ? firstLine(rows[0]) : at.from, to: rows.length ? lastLine(rows[rows.length - 1]) + 1 : at.from, rows };
}

// A search is a scan over every line; its hits are kept for the next and previous match, as the server
// would keep a search's result while someone steps through it.
const searches = new Map<string, number[]>();
export function bigSearch(available: number, query: string, from: number, direction: 'forward' | 'backward'): NodeOutputSearch {
	const q = query.toLowerCase();
	if (!q) return { count: 0, line: null, rank: null };
	const key = `${available}|${q}`;
	let hits = searches.get(key);
	if (!hits) {
		hits = [];
		for (let n = 1; n <= available; n++) if (bigLine(n).text.toLowerCase().includes(q)) hits.push(n);
		searches.clear();
		searches.set(key, hits);
	}
	const line = direction === 'forward' ? (hits.find((h) => h >= from) ?? hits[0] ?? null) : ([...hits].reverse().find((h) => h <= from) ?? hits[hits.length - 1] ?? null);
	return { count: hits.length, line, rank: line == null ? null : hits.indexOf(line) + 1 };
}

/**
 * Where the server streams the whole output as a file. The mock has no file server, so its link opens a
 * short note in place of the file.
 */
export function bigDownloadUrl(runId: string, nodeKey: string, available: number): string {
	const note = `Mock: the server streams ${runId}'s ${nodeKey} output here, ${available.toLocaleString('en-GB')} lines, as a file.\n`;
	return URL.createObjectURL(new Blob([note], { type: 'text/plain' }));
}

/** How many lines the node has printed by `now`: all of them once it finished, else so far. */
export function bigAvailable(n: RunNode, now: number): { available: number; live: boolean } | null {
	if (n.nodeKey !== BIG_NODE || !n.startedAt) return null;
	if (n.status !== 'running') return { available: BIG_TOTAL, live: false };
	return { available: Math.max(1, Math.min(BIG_TOTAL - 1, Math.floor(((now - Date.parse(n.startedAt)) / 1000) * BIG_RATE))), live: true };
}
