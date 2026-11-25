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

function notebookMarks(available: number): NodeOutputIndex['marks'] {
	const marks: NodeOutputIndex['marks'] = [];
	for (let c = 0; c < CELLS; c++) {
		const base = c * CELL + 1;
		const w = base + warnAt(c);
		if (w <= available) marks.push({ line: w, level: 'warn', text: bigLine(w).text });
		if (ERROR_CELLS.has(c) && base + errorAt(c) <= available) marks.push({ line: base + errorAt(c), level: 'error', text: bigLine(base + errorAt(c)).text });
	}
	return marks.sort((a, b) => a.line - b.line);
}

// ---- output sources: a node's printed lines, computed by line number ----

/** A node's whole output, as the output store and its index would serve it. */
export type OutputSource = {
	total: number;
	/** Lines a second while it runs. */
	rate: number;
	line(n: number): NodeOutputLine;
	/** How many similar lines run from `n` in `step`'s direction. */
	similarRun(n: number, step: 1 | -1, available: number): number;
	/** Error and warning lines, from the index. */
	marks(available: number): NodeOutputIndex['marks'];
	/** Every stderr line's number. */
	stderrLines(available: number): number[];
};

const NOTEBOOK: OutputSource = { total: BIG_TOTAL, rate: BIG_RATE, line: bigLine, similarRun, marks: notebookMarks, stderrLines: (available) => notebookMarks(available).map((m) => m.line) };

/** Lines that read the same but for their numbers: one shape. */
const shapeOf = (l: NodeOutputLine) => `${l.stream}|${l.text.replace(/\d+/g, '#')}`;

/** A small output's own source: similar runs and marks found by reading it, which is cheap at its size. */
function smallSource(lines: (n: number) => NodeOutputLine, total: number, rate: number): OutputSource {
	return {
		total,
		rate,
		line: lines,
		similarRun(n, step, available) {
			const shape = shapeOf(lines(n));
			let count = 1;
			while (n + step * count >= 1 && n + step * count <= available && shapeOf(lines(n + step * count)) === shape) count++;
			return count;
		},
		marks(available) {
			const out: NodeOutputIndex['marks'] = [];
			for (let n = 1; n <= available; n++) {
				const l = lines(n);
				if (l.level !== 'info') out.push({ line: n, level: l.level, text: l.text });
			}
			return out;
		},
		stderrLines(available) {
			const out: number[] = [];
			for (let n = 1; n <= available; n++) if (lines(n).stream === 'stderr') out.push(n);
			return out;
		}
	};
}

// A Python task that prints a few thousand lines, then fails with a chained pandas KeyError (CHI-1591).
export const V2_NODE = 'score_customers_v2';
const V2_PRINTED = 2_960;
export const V2_TRACEBACK = [
	'Traceback (most recent call last):',
	'  File "/app/.venv/lib/python3.12/site-packages/pandas/core/indexes/base.py", line 3805, in get_loc',
	'    return self._engine.get_loc(casted_key)',
	'           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^',
	'  File "index.pyx", line 167, in pandas._libs.index.IndexEngine.get_loc',
	'  File "index.pyx", line 196, in pandas._libs.index.IndexEngine.get_loc',
	'  File "pandas/_libs/hashtable_class_helper.pxi", line 7081, in pandas._libs.hashtable.PyObjectHashTable.get_item',
	'  File "pandas/_libs/hashtable_class_helper.pxi", line 7089, in pandas._libs.hashtable.PyObjectHashTable.get_item',
	"KeyError: 'customer_segment'",
	'',
	'The above exception was the direct cause of the following exception:',
	'',
	'Traceback (most recent call last):',
	'  File "/app/.venv/lib/python3.12/site-packages/sqlbuild/executor/task.py", line 142, in run_task',
	'    result = self._call(node, ctx)',
	'             ^^^^^^^^^^^^^^^^^^^^^',
	'  File "/app/.venv/lib/python3.12/site-packages/sqlbuild/executor/task.py", line 201, in _call',
	'    return node.fn(ctx)',
	'           ^^^^^^^^^^^^',
	'  File "/app/python/nodes/ml/score_customers_v2.py", line 41, in score_customers_v2',
	'    features = build_features(customers, segments)',
	'               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^',
	'  File "/app/python/nodes/ml/score_customers_v2.py", line 26, in build_features',
	'    joined["segment_weight"] = joined["customer_segment"].map(SEGMENT_WEIGHTS)',
	'                               ~~~~~~^^^^^^^^^^^^^^^^^^^^',
	'  File "/app/.venv/lib/python3.12/site-packages/pandas/core/frame.py", line 4102, in __getitem__',
	'    indexer = self.columns.get_loc(key)',
	'              ^^^^^^^^^^^^^^^^^^^^^^^^^',
	'  File "/app/.venv/lib/python3.12/site-packages/pandas/core/indexes/base.py", line 3812, in get_loc',
	'    raise KeyError(key) from err',
	"KeyError: 'customer_segment'"
];
export const V2_TOTAL = V2_PRINTED + V2_TRACEBACK.length;
function v2Line(n: number): NodeOutputLine {
	const out = (text: string, stream: 'stdout' | 'stderr' = 'stdout', level: NodeOutputLevel = 'info'): NodeOutputLine => ({ n, stream, text, level });
	if (n > V2_PRINTED) {
		const text = V2_TRACEBACK[n - V2_PRINTED - 1] ?? '';
		return out(text, 'stderr', text.startsWith('KeyError') ? 'error' : 'info');
	}
	if (n === 1) return out('Loading customers from analytics.dim_customers');
	if (n === 2) return out('Loaded 48,120 customers, 31 columns');
	if (n === 3) return out('Loading segments from analytics.customer_segments');
	if (n === V2_PRINTED) return out('Building features for 48,120 customers');
	if (n === V2_PRINTED - 1) return out('Scored 48,120 customers with model churn_v2 (rev 14)');
	return out(`  scoring batch ${n - 3}/${V2_PRINTED - 5}  customers=${16 + ((n * 7) % 9)}  ${2 + ((n * 13) % 30)}ms`);
}

export const OUTPUT_SOURCES: Record<string, OutputSource> = {
	[BIG_NODE]: NOTEBOOK,
	[V2_NODE]: smallSource(v2Line, V2_TOTAL, 900)
};

export function bigIndex(nodeKey: string, available: number, live: boolean): NodeOutputIndex {
	const src = OUTPUT_SOURCES[nodeKey];
	const marks = src.marks(available);
	const stderr = src.stderrLines(available).length;
	return { total: available, stdout: available - stderr, stderr, bytes: Math.round(available * 46.3), marks, live };
}

export type PageAt = { from: number; rows: number; direction: 'forward' | 'backward'; collapse: boolean; stream?: 'all' | 'stdout' | 'stderr'; around?: number };

/**
 * Up to `rows` rows from line `from`, forwards, or backwards ending just before `from`; similar lines
 * folded when `collapse`, except the block holding `around` (a jump's target), which opens around it.
 * `stream` keeps only stdout or stderr lines, keeping their line numbers.
 */
export function bigPage(nodeKey: string, available: number, at: PageAt): NodeOutputPage {
	const src = OUTPUT_SOURCES[nodeKey];
	const step = at.direction === 'forward' ? 1 : -1;
	const rows: NodeOutputRow[] = [];
	if (at.stream === 'stderr') {
		// stderr lines are the index's marks: read them from the index, not by walking the output.
		const lines = src.stderrLines(available);
		const picked = step > 0 ? lines.filter((l) => l >= at.from).slice(0, at.rows) : lines.filter((l) => l < at.from).slice(-at.rows);
		for (const l of picked) rows.push({ kind: 'line', line: src.line(l) });
	} else {
		let n = step > 0 ? Math.max(1, at.from) : Math.min(available, at.from - 1);
		while (rows.length < at.rows && n >= 1 && n <= available) {
			const first = src.line(n);
			if (at.stream === 'stdout' && first.stream === 'stderr') {
				n += step;
				continue;
			}
			let count = at.collapse ? src.similarRun(n, step, available) : 1;
			const lo = step > 0 ? n : n - count + 1;
			if (count >= 4 && at.around != null && at.around >= lo && at.around < lo + count) {
				// The jump's target is inside this block: fold what comes before it, then show its lines.
				const before = step > 0 ? at.around - 3 - n : n - (at.around + 3);
				if (before >= 4) {
					rows.push({ kind: 'similar', from: step > 0 ? n : at.around + 4, count: before, sample: src.line(step > 0 ? n : at.around + 4) });
					n += step * before;
				}
				const near = Math.min(7, Math.abs(n - (step > 0 ? at.around + 4 : at.around - 4)));
				for (let i = 0; i < near && n >= 1 && n <= available; i++, n += step) rows.push({ kind: 'line', line: src.line(n) });
				continue;
			}
			if (count >= 4) rows.push({ kind: 'similar', from: lo, count, sample: src.line(lo) });
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
export function bigSearch(nodeKey: string, available: number, query: string, from: number, direction: 'forward' | 'backward'): NodeOutputSearch {
	const src = OUTPUT_SOURCES[nodeKey];
	const q = query.toLowerCase();
	if (!q) return { count: 0, line: null, rank: null };
	const key = `${nodeKey}|${available}|${q}`;
	let hits = searches.get(key);
	if (!hits) {
		hits = [];
		for (let n = 1; n <= available; n++) if (src.line(n).text.toLowerCase().includes(q)) hits.push(n);
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
	const src = OUTPUT_SOURCES[n.nodeKey];
	if (!src || !n.startedAt) return null;
	if (n.status !== 'running') return { available: src.total, live: false };
	return { available: Math.max(1, Math.min(src.total - 1, Math.floor(((now - Date.parse(n.startedAt)) / 1000) * src.rate))), live: true };
}
