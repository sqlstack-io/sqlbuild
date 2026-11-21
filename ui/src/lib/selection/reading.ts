// The "Reads as" line: a selection in plain words, grouped exactly as `sqb` resolves it. Each `+`
// belongs to the comma part it is written on and widens that part before the parts are
// intersected; tokens are unioned; the except row is subtracted from the whole.

import { isFolder, parseSelection, type Part, type Token } from './syntax';

/** One run of the line; `strong` runs name what is selected, the rest joins them. */
export type Segment = { text: string; strong?: boolean };

const KIND_PLURAL: Record<string, string> = {
	model: 'models',
	source: 'sources',
	seed: 'seeds',
	udf: 'UDFs',
	table_fn: 'table functions',
	function: 'functions',
	asset: 'assets',
	loader: 'loaders',
	check: 'checks'
};

const hasGlob = (value: string) => /[*?[]/.test(value);

/** What a part selects before `+`, and whether that reads as one node. */
function base(p: Part): { text: string; one: boolean } {
	if (p.between) return { text: `nodes on the paths from ${p.between[0]} to ${p.between[1]}`, one: false };
	if (isFolder(p)) return { text: `nodes in ${p.value}`, one: false };
	switch (p.type) {
		case null:
			return hasGlob(p.value) ? { text: `nodes named like ${p.value}`, one: false } : { text: p.value, one: true };
		case 'tag':
			return { text: `nodes tagged ${p.value}`, one: false };
		case 'path':
			return { text: `nodes in ${p.value}`, one: false };
		case 'job':
			return { text: `the nodes of job ${p.value}`, one: false };
		case 'kind':
			return { text: KIND_PLURAL[p.value] ?? `${p.value} nodes`, one: false };
		default:
			return { text: `${p.type} ${p.value}`, one: !hasGlob(p.value) };
	}
}

/** A part with its own `+`, e.g. "nodes tagged finance and everything upstream of them". */
export function readPart(p: Part): string {
	const { text, one } = base(p);
	const them = one ? 'it' : 'them';
	if (p.up && p.down) return `${text} and everything upstream and downstream of ${them}`;
	if (p.up) return `${text} and everything upstream of ${them}`;
	if (p.down) return `${text} and everything downstream of ${them}`;
	return text;
}

/** A later comma part, as what the earlier parts must also be. */
function also(p: Part): string {
	if (p.up || p.down || p.between) return `among ${readPart(p)}`;
	if (isFolder(p)) return `in ${p.value}`;
	switch (p.type) {
		case 'tag':
			return `tagged ${p.value}`;
		case 'path':
			return `in ${p.value}`;
		case 'job':
			return `in job ${p.value}`;
		default:
			return readPart(p);
	}
}

/** A token: its first part, narrowed by the others. */
export function readToken(t: Token): string {
	const [first, ...rest] = t.parts;
	if (!rest.length) return readPart(first);
	return `${readPart(first)}, that are also ${rest.map(also).join(' and ')}`;
}

/** The whole selection in plain words, as segments to draw. */
export function readSelection(select: string, exclude = ''): Segment[] {
	const chosen = parseSelection(select);
	const dropped = parseSelection(exclude);
	const out: Segment[] = [];
	if (!chosen.length) out.push({ text: 'every node', strong: true });
	chosen.forEach((t, i) => {
		if (i) out.push({ text: ', plus ' });
		out.push({ text: readToken(t), strong: true });
	});
	if (dropped.length) {
		out.push({ text: ', minus ' });
		dropped.forEach((t, i) => {
			if (i) out.push({ text: ' or ' });
			out.push({ text: readToken(t), strong: true });
		});
	}
	return out;
}

/** The line as plain text. */
export const readingText = (select: string, exclude = '') =>
	readSelection(select, exclude)
		.map((s) => s.text)
		.join('');
