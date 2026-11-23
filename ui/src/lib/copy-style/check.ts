// The copy check: finds the UI's copy sites in source text and the rules each breaks. Pure; the test
// in copy-style.test.ts reads the files. See ui/COPY.md for the rules in words.

/** What a piece of copy is, which decides its rules. */
export type CopyKind = 'label' | 'value' | 'note' | 'empty';

export type CopySite = { file: string; line: number; kind: CopyKind; text: string; title?: string };

export type Finding = CopySite & { rule: string };

/** Most words each kind may have, outside `{…}` tokens. */
export const MAX_WORDS: Record<CopyKind, number> = { label: 4, value: 5, note: 6, empty: 8 };

const BANNED: [RegExp, string][] = [
	[/\bwill\b/i, 'says "will"'],
	[/\b(is|are|gets?) (recorded|kept|stored|written|audited|logged)\b/i, 'narrates what is recorded'],
	[/\bfails? if\b/i, 'says "fails if"'],
	[/\b(after|before) (writing|it)\b/i, 'says when in a sentence'],
	[/\bso (that|it)\b/i, 'explains with "so"'],
	[/\bthis means\b/i, 'says "this means"'],
	[/\byou(r)?\b/i, 'addresses the reader'],
	[/\bwhen (it|its)\b/i, 'says "when it"']
];
const CONSEQUENCE = /\b(replace|delete|lost|can't|can’t|irreversible)/i;

/** Copy as the rules see it: each `{…}` or `${…}` collapsed to one token. */
export const collapse = (text: string) => text.replace(/\$?\{[^{}]*\}/g, '#').replace(/\s+/g, ' ').trim();

const words = (text: string) => collapse(text).split(' ').filter((w) => w && w !== '#' && w !== '·' && /[a-z]/i.test(w));

const STOP = new Set(['the', 'a', 'an', 'of', 'in', 'on', 'to', 'and', 'or', 'for', 'by', 'it', 'its', 'is']);
const contentWords = (text: string) => new Set(words(text).map((w) => w.toLowerCase().replace(/[^a-z]/g, '')).filter((w) => w && !STOP.has(w)));

/** The rules a copy site breaks; empty when it follows them. */
export function check(site: CopySite): string[] {
	const text = collapse(site.text);
	if (!/[a-z]/i.test(text)) return [];
	const broken: string[] = [];
	if (site.kind === 'empty') {
		const sentences = text.split(/(?<=[.!?])\s+/).filter(Boolean);
		if (words(sentences[0] ?? '').length > MAX_WORDS.empty) broken.push(`empty state over ${MAX_WORDS.empty} words`);
		if (sentences.length > 1 && !CONSEQUENCE.test(sentences.slice(1).join(' '))) broken.push('second sentence without a consequence');
		return broken;
	}
	if (words(text).length > MAX_WORDS[site.kind]) broken.push(`${site.kind} over ${MAX_WORDS[site.kind]} words`);
	for (const [pattern, rule] of BANNED) if (pattern.test(text)) broken.push(rule);
	if (text.includes(';')) broken.push('has a ";" clause');
	if (/: [a-z]/.test(text)) broken.push('has a ": clause"');
	if (text.split(' · ').length > 3) broken.push('more than 3 " · " segments');
	if (site.kind === 'note' && site.title) {
		const title = contentWords(site.title);
		if ([...contentWords(text)].filter((w) => title.has(w)).length >= 2) broken.push('note restates the title');
	}
	return broken;
}

/** A `copy-allow: reason` marker on the line or the line before; a bare `copy-allow` has no reason. */
export function allowed(lines: readonly string[], index: number): 'allowed' | 'bare' | null {
	for (const line of [lines[index] ?? '', lines[index - 1] ?? '']) {
		const m = /copy-allow\b(:\s*(\S.*?))?\s*(-->|\*\/|$)/.exec(line);
		if (m) return m[2] ? 'allowed' : 'bare';
	}
	return null;
}

const lineOf = (source: string, index: number) => source.slice(0, index).split('\n').length;

/** Label maps checked: string values of `Record<…, string>` maps, except explanations and tones. */
const MAP = /export const (\w+)\s*:\s*Record<[^>]+,\s*string>\s*=\s*\{([\s\S]*?)\n\};/g;
const SKIP_MAP = /(Help|Hint|Technical|Tone)$/;
const STRING = /(['"`])((?:\\.|(?!\1)[^\\\n])*)\1/g;

/** The copy sites in one file. `labelFile` turns on label-map scanning for labels.ts-style files. */
export function findCopy(file: string, source: string, labelFile = false): CopySite[] {
	const sites: CopySite[] = [];
	const add = (index: number, kind: CopyKind, text: string, title?: string) => sites.push({ file, line: lineOf(source, index), kind, text, title });
	if (labelFile) {
		for (const m of source.matchAll(MAP)) {
			if (SKIP_MAP.test(m[1])) continue;
			const bodyStart = m.index + m[0].indexOf(m[2]);
			for (const v of m[2].matchAll(/:\s*(['"`])((?:\\.|(?!\1)[^\\\n])*)\1/g)) add(bodyStart + v.index, 'value', v[2]);
		}
	}
	if (!file.endsWith('.svelte')) return sites;
	// Panel titles and notes; the note is compared with the title on the same tag.
	for (const m of source.matchAll(/<Panel\b((?:[^>{]|\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\})*?)>/g)) {
		const tag = m[1];
		const title = /\btitle="([^"]*)"/.exec(tag)?.[1];
		if (title) add(m.index, 'label', title);
		const note = /\bnote=(?:"([^"]*)"|\{((?:[^{}]|\{[^{}]*\})*)\})/.exec(tag);
		if (note?.[1] !== undefined) add(m.index, 'note', note[1], title);
		else if (note) for (const s of note[2].matchAll(STRING)) add(m.index, 'note', s[2], title);
	}
	// `empty="…"`, or every string in `empty={a ? '…' : '…'}`.
	for (const m of source.matchAll(/\bempty=(?:"([^"]*)"|\{([^{}]*)\})/g)) {
		if (m[1] !== undefined) add(m.index, 'empty', m[1]);
		else for (const s of m[2].matchAll(STRING)) add(m.index, 'empty', s[2]);
	}
	for (const m of source.matchAll(/<StatusLabel\b[^>]*?\blabel="([^"]*)"/g)) add(m.index, 'value', m[1]);
	// Column headers, keys and short values written into the markup.
	for (const m of source.matchAll(/<(th|dt)\b[^>]*>([^<{]*[a-z][^<{]*)<\/\1>/gi)) add(m.index, 'label', m[2]);
	for (const m of source.matchAll(/<dd\b[^>]*>([^<{]*[a-z][^<{]*)<\/dd>/gi)) add(m.index, 'value', m[1]);
	// Key-value rows, table columns and filter options: `label: '…'` and `value: '…'` literals.
	for (const m of source.matchAll(/\blabel:\s*(['"`])((?:(?!\1)[^\\\n])*)\1/g)) add(m.index, 'label', m[2]);
	for (const m of source.matchAll(/\{\s*label:[^{}]*?\bvalue:\s*(['"`])((?:(?!\1)[^\\\n])*)\1/g)) add(m.index + m[0].lastIndexOf('value:'), 'value', m[2]);
	// Empty states and messages, read as all the text of their element, nested elements included.
	for (const m of source.matchAll(/<([a-z]+)\b[^>]*\bclass="(?:[^"]*\s)?(?:empty|msg)(?:\s[^"]*)?"[^>]*>/gi)) {
		for (const text of blockTexts(source, m[1], m.index + m[0].length)) if (/[a-z]/i.test(text)) add(m.index, 'empty', text);
	}
	return sites;
}

/** The texts of the element whose content starts at `start`, one per `{#if}` branch, without buttons, help tips or code. */
function blockTexts(source: string, tag: string, start: number): string[] {
	const open = new RegExp(`<${tag}\\b|</${tag}>`, 'gi');
	open.lastIndex = start;
	let depth = 1;
	let end = source.length;
	for (let m = open.exec(source); m; m = open.exec(source)) {
		depth += m[0].startsWith('</') ? -1 : 1;
		if (!depth) {
			end = m.index;
			break;
		}
	}
	let text = source.slice(start, end).replace(/<(Button|button|HelpTip|code)\b[\s\S]*?<\/\1>/g, ' ');
	// Template expressions, innermost first, become one token; block tags split the branches.
	for (let before = ''; before !== text; ) {
		before = text;
		text = text.replace(/\{([#:/@]?)[^{}]*\}/g, (_, block: string) => (block ? '\u0000' : '#'));
	}
	return text
		.replace(/<[^>]*>/g, ' ')
		.split('\u0000')
		.map((part) => part.replace(/\s+/g, ' ').trim())
		.filter(Boolean);
}

/** The findings in one file, with `copy-allow` markers honoured; a bare marker is itself a finding. */
export function findings(file: string, source: string, labelFile = false): { findings: Finding[]; allowed: number } {
	const lines = source.split('\n');
	const out: Finding[] = [];
	let allowedCount = 0;
	for (const site of findCopy(file, source, labelFile)) {
		const broken = check(site);
		if (!broken.length) continue;
		const marker = allowed(lines, site.line - 1);
		if (marker === 'allowed') {
			allowedCount++;
			continue;
		}
		if (marker === 'bare') out.push({ ...site, rule: 'copy-allow without a reason' });
		for (const rule of broken) out.push({ ...site, rule });
	}
	return { findings: out, allowed: allowedCount };
}

/** A finding's baseline entry: its file, kind, rule and text, which survive the lines moving. */
export const baselineKey = (f: Finding) => `${f.file} | ${f.kind} | ${f.rule} | ${collapse(f.text)}`;
