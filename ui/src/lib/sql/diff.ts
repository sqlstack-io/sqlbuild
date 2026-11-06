// Pure line diff (LCS) with context folding for the SQL drift view.

export type DiffOp = 'same' | 'add' | 'del';
export type DiffLine = { op: DiffOp; a?: number; b?: number; text: string };
export type DiffRow = DiffLine | { op: 'fold'; count: number; key: string };

export function diffLines(before: string, after: string): DiffLine[] {
	const a = before.split('\n');
	const b = after.split('\n');
	const n = a.length;
	const m = b.length;
	const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
	for (let i = n - 1; i >= 0; i--) {
		for (let j = m - 1; j >= 0; j--) {
			lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
		}
	}
	const out: DiffLine[] = [];
	let i = 0;
	let j = 0;
	while (i < n && j < m) {
		if (a[i] === b[j]) {
			out.push({ op: 'same', a: i + 1, b: j + 1, text: b[j] });
			i++;
			j++;
		} else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
			out.push({ op: 'del', a: i + 1, text: a[i] });
			i++;
		} else {
			out.push({ op: 'add', b: j + 1, text: b[j] });
			j++;
		}
	}
	while (i < n) out.push({ op: 'del', a: ++i, text: a[i - 1] });
	while (j < m) out.push({ op: 'add', b: ++j, text: b[j - 1] });
	return out;
}

/** Collapse runs of unchanged lines, keeping `context` lines around each change. */
export function foldDiff(lines: DiffLine[], context = 3): DiffRow[] {
	const keep = lines.map(() => false);
	lines.forEach((l, idx) => {
		if (l.op === 'same') return;
		for (let k = Math.max(0, idx - context); k <= Math.min(lines.length - 1, idx + context); k++) keep[k] = true;
	});
	const rows: DiffRow[] = [];
	let run = 0;
	lines.forEach((l, idx) => {
		if (keep[idx]) {
			if (run) rows.push({ op: 'fold', count: run, key: `f${idx}` });
			run = 0;
			rows.push(l);
		} else run++;
	});
	if (run) rows.push({ op: 'fold', count: run, key: 'fend' });
	return rows;
}

export function diffStats(lines: DiffLine[]) {
	return {
		added: lines.filter((l) => l.op === 'add').length,
		removed: lines.filter((l) => l.op === 'del').length
	};
}
