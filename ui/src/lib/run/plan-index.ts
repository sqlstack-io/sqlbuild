// The plan page's list as an index and a pane: the plan's sections (plan-text.ts) gathered into a few
// groups, one shown at a time, each row one line of name · action · detail. Pure.

import type { PlanDocument, PlanPageSection, PlanWarning } from '$lib/api';
import { plural } from '$lib/format/plural';
import { changedSections, looseWarnings, planSections, sectionSize, type PlanEntry, type PlanSection } from './plan-text';

/** The groups in the index, in its order; the ones after the divider are the bulky, quiet ones. */
export type PlanGroupKey = 'full_refresh' | 'changed' | 'renames' | 'first_run' | 'loads' | 'python' | 'held_back' | 'warnings' | 'unchanged' | 'skipped';
const ORDER: PlanGroupKey[] = ['full_refresh', 'changed', 'renames', 'first_run', 'loads', 'python', 'held_back', 'warnings', 'unchanged', 'skipped'];
/** The first group below the index's divider. */
export const AFTER_DIVIDER: PlanGroupKey = 'warnings';
const TITLES: Record<PlanGroupKey, string> = {
	full_refresh: 'Full refresh',
	changed: 'Changed',
	renames: 'Renames and migrations',
	first_run: 'First run',
	loads: 'Sources to load',
	python: 'Python',
	held_back: 'Held back',
	warnings: 'Warnings',
	unchanged: 'Unchanged',
	skipped: 'Skipped by --changed'
};
/** Each plan-text section's group, with the kind of change it is for the Changed group. */
const SECTION_GROUP: Record<string, [PlanGroupKey, ChangeKind?]> = {
	'Full refresh': ['full_refresh'],
	'Query changed': ['changed', 'query'],
	'Function changed': ['changed', 'function'],
	'Config changed': ['changed', 'config'],
	'Schema changed': ['changed', 'schema'],
	'Changed functions': ['changed', 'function'],
	Migrations: ['renames'],
	'Column migrations': ['renames'],
	'Table type changes': ['renames'],
	Retention: ['renames'],
	Renamed: ['renames'],
	'First run': ['first_run'],
	'Loaders to reload': ['loads'],
	'Loaders to load': ['loads'],
	'Sources to reload': ['loads'],
	'Sources to load': ['loads'],
	Python: ['python'],
	'Held back': ['held_back'],
	Models: ['unchanged'],
	Functions: ['unchanged'],
	Seeds: ['unchanged'],
	Unchanged: ['skipped']
};

export type ChangeKind = 'query' | 'config' | 'function' | 'schema';

/** A line of a diff the row can open: `cls` colours it. */
export type DiffLine = { text: string; cls: 'add' | 'del' | 'hunk' | '' };

export type PlanRow = {
	name: string;
	/** What happens to it: an action, a materialization, a decision. */
	action: string;
	/** The one-line detail: for a change, its kind with line counts or the config change. */
	detail: string;
	/** More detail lines, such as a cursor range. */
	notes: string[];
	/** The diff the row opens, when it carries one: config, schema and query parts. */
	diff: DiffLine[] | null;
	/** Which part of the diff the "diff" link opens: the query, or the config when it isn't inline. */
	diffOf: 'query' | 'config' | 'schema' | null;
	warnings: PlanWarning[];
	/** Rows of plan-text's catalog-linked sections link to the catalog. */
	linked: boolean;
	/** A changed row's kinds of change, for its group's breakdown. */
	kinds?: string[];
};

export type PlanGroup = {
	key: PlanGroupKey;
	title: string;
	/** Rows in the group, loaded or not. */
	count: number;
	/** The breakdown under the title, such as "87 query · 6 config · 1 function", and a note every row shares. */
	subtitle: string;
	rows: PlanRow[];
	/** The server pages still to load, in the order their rows follow the loaded ones. */
	pages: { section: PlanPageSection; loaded: number; total: number }[];
	/** The Warnings group's warnings, grouped by message. */
	warningGroups?: WarningGroup[];
};

/** A warnings group: one message template and the warnings that fill it in. */
export type WarningGroup = { template: string; warnings: PlanWarning[]; label: string };

const DIFF_HEAD = /^(---|\+\+\+|@@)/;
const lineClass = (line: string): DiffLine['cls'] => (DIFF_HEAD.test(line) ? 'hunk' : line.startsWith('+') ? 'add' : line.startsWith('-') ? 'del' : '');
const changedLines = (diff: string[], sign: '+' | '-') => diff.filter((l) => l.startsWith(sign) && !DIFF_HEAD.test(l));

/** A unified diff's changed lines: `+N −M`. */
export function diffStats(diff: string[]): { added: number; removed: number } {
	return { added: changedLines(diff, '+').length, removed: changedLines(diff, '-').length };
}

/**
 * Whether a query diff changes only whitespace: its removed and added lines read the same with all
 * whitespace taken out. A display hint only; the planner decides what rebuilds. A change of keyword
 * case is a change.
 */
export function whitespaceOnly(diff: string[]): boolean {
	const side = (sign: '+' | '-') =>
		changedLines(diff, sign)
			.map((l) => l.slice(1))
			.join('')
			.replace(/\s+/g, '');
	const [removed, added] = [changedLines(diff, '-'), changedLines(diff, '+')];
	return (removed.length > 0 || added.length > 0) && side('-') === side('+');
}

/**
 * A config diff's changes in one line, such as `full_refresh true → false`, when it changes at most
 * `most` top-level keys with plain values; null otherwise, and the row links to the diff instead.
 */
export function configChange(diff: string[], most = 2): string | null {
	const values = new Map<string, { old?: string; new?: string }>();
	for (const line of diff) {
		if (DIFF_HEAD.test(line) || !(line.startsWith('+') || line.startsWith('-'))) continue;
		const m = /^[-+]\s*"([^"]+)":\s*(.*?),?\s*$/.exec(line);
		// A nested value, or a line that isn't one key's value, can't be read in one line.
		if (!m || /[{[]$/.test(m[2])) return null;
		const entry = values.get(m[1]) ?? {};
		entry[line.startsWith('-') ? 'old' : 'new'] = m[2].replace(/^"(.*)"$/, '$1');
		values.set(m[1], entry);
	}
	if (!values.size || values.size > most) return null;
	return [...values].map(([key, v]) => `${key} ${v.old ?? '(unset)'} → ${v.new ?? '(unset)'}`).join('; ');
}

/** The executor's placeholders for a microbatch's bounds, which a preview reports as the range. */
const CURSOR_START = '__SQB_CURSOR_START__';
const CURSOR_END = '__SQB_CURSOR_END__';
/** A cursor range's placeholders in words: "from the last run → the latest data when the run starts". */
export function cursorWords(line: string): string {
	return line.replace(new RegExp(`${CURSOR_START}`, 'g'), 'from the last run').replace(new RegExp(`${CURSOR_END}`, 'g'), 'the latest data when the run starts');
}

/** The order a row's kinds of change read in, as "config + query". */
const KIND_ORDER: ChangeKind[] = ['config', 'query', 'function', 'schema'];

/** A changed model's row: the kinds of change, line counts or the config change, and the diff it opens. */
function changedRow(e: PlanEntry, kind: ChangeKind | undefined, linked: boolean): PlanRow {
	const kinds = new Set<ChangeKind>();
	if (kind) kinds.add(kind);
	if (e.queryDiff?.length) kinds.add('query');
	if (e.configDiff?.length) kinds.add('config');
	if (e.schema?.length) kinds.add('schema');
	const cause = e.detail.find((d) => d.startsWith('cause: function '));
	if (cause) kinds.add('function');
	const parts: string[] = [];
	const config = e.configDiff?.length ? configChange(e.configDiff) : null;
	const query = e.queryDiff?.length ? e.queryDiff : null;
	const quiet = query ? whitespaceOnly(query) : false;
	const label = KIND_ORDER.filter((k) => kinds.has(k)).join(' + ');
	if (quiet && kinds.size === 1) parts.push('whitespace only');
	else {
		const stats = query && !quiet ? diffStats(query) : null;
		parts.push(stats && kinds.size === 1 ? `${label} +${stats.added} −${stats.removed}` : label);
		if (config) parts.push(config);
		if (stats && kinds.size > 1) parts.push(`+${stats.added} −${stats.removed}`);
		if (quiet) parts.push('query whitespace only');
	}
	if (cause) parts.push(cause.slice('cause: function '.length));
	if (e.schema?.length) parts.push(plural(e.schema.length, 'column'));
	// The diff the link opens: the query, else a config too long to show inline, else the schema.
	const diffOf = query ? 'query' : e.configDiff?.length && !config ? 'config' : e.schema?.length ? 'schema' : null;
	const diff: DiffLine[] | null = diffOf
		? [
				...(e.configDiff?.length && !config ? e.configDiff.map((text) => ({ text, cls: lineClass(text) })) : []),
				...(e.schema ?? []).map((f) => ({ text: `${f.sign} ${f.text}`, cls: (f.sign === '+' ? 'add' : f.sign === '-' ? 'del' : 'hunk') as DiffLine['cls'] })),
				...(query ?? []).map((text) => ({ text, cls: lineClass(text) }))
			]
		: null;
	return {
		name: e.name,
		action: e.value,
		detail: parts.filter(Boolean).join(' · '),
		notes: e.detail.filter((d) => d !== cause).map(cursorWords),
		diff,
		diffOf,
		warnings: e.warnings ?? [],
		linked,
		// Its plan reason's kind first: the group's breakdown counts the row by it.
		kinds: [...kinds]
	};
}

const plainRow = (e: PlanEntry, linked: boolean): PlanRow => {
	const [first = '', ...rest] = e.detail.map(cursorWords);
	return { name: e.name, action: e.value, detail: first, notes: rest, diff: null, diffOf: null, warnings: e.warnings ?? [], linked };
};

/** Sections whose rows are catalog models. */
const linkedSection = (title: string) => !/unction|^Sources|^Loaders|^Full refresh|^Python/.test(title);

/**
 * A note every row of a group shares, such as "no completed record", said once in its subtitle and
 * left off the rows. Only the loaded rows are known, so a paged group is judged by those.
 */
export function sharedNote(rows: PlanRow[]): string | null {
	if (rows.length < 2) return null;
	const lines = (r: PlanRow) => [r.detail, ...r.notes].filter(Boolean);
	const first = lines(rows[0]);
	return first.find((line) => rows.every((r) => lines(r).includes(line))) ?? null;
}

function withoutNote(row: PlanRow, note: string): PlanRow {
	if (row.detail === note) {
		const [next = '', ...rest] = row.notes;
		return { ...row, detail: next, notes: rest };
	}
	return { ...row, notes: row.notes.filter((n) => n !== note) };
}

/**
 * A warning's message as a template: the names it quotes, a leading model name, and names that look
 * generated (with a double underscore or a digit) taken out, so one message about many names reads once.
 */
export function warningTemplate(w: PlanWarning): string {
	let message = w.message;
	if (w.model_name && message.startsWith(`${w.model_name}:`)) message = message.slice(w.model_name.length + 1).trim();
	// A sentence ending on an elided name ends on the ellipsis, not "…." as well.
	return message
		.replace(/'[^']*'|`[^`]*`|"[^"]*"/g, '…')
		.replace(/\b\w*(?:__|\d)\w*\b/g, '…')
		.replace(/…\.$/, '…');
}

/**
 * Warnings grouped by template, largest first. The label counts them, and when the template opens with
 * a named thing, such as a scenario, how many of those: "71 in 24 scenarios".
 */
export function groupWarnings(warnings: PlanWarning[]): WarningGroup[] {
	const groups = new Map<string, PlanWarning[]>();
	for (const w of warnings) {
		const t = warningTemplate(w);
		groups.set(t, [...(groups.get(t) ?? []), w]);
	}
	return [...groups]
		.map(([template, list]) => {
			const noun = /^(\w+) …/.exec(template)?.[1];
			const firsts = new Set(list.map((w) => /'([^']*)'|`([^`]*)`|"([^"]*)"/.exec(w.message)?.slice(1).find(Boolean) ?? ''));
			const label = noun && list.length > 1 && firsts.size < list.length ? `${list.length} in ${plural(firsts.size, noun.toLowerCase())}` : plural(list.length, 'warning');
			return { template, warnings: list, label };
		})
		.sort((a, b) => b.warnings.length - a.warnings.length);
}

/** The plan's groups in index order, empty ones left out; `refresh` words what refreshes a held-back job's nodes. */
export function planGroups(plan: PlanDocument, refresh: (job: string) => string = (job) => `job ${job}`): PlanGroup[] {
	const sections: PlanSection[] = [...planSections(plan), ...(plan.changed ? changedSections(plan.changed, refresh, plan.pages) : [])];
	// A group gathers plan sections, each with its rows and size. Its rows read section by section; the
	// sections after a paged one wait until all its pages have loaded, so loaded rows read in order and
	// the next page follows them.
	type Part = { title: string; rows: PlanRow[]; size: number; paged: boolean; complete: boolean };
	const byKey = new Map<PlanGroupKey, { parts: Part[]; count: number; pages: PlanGroup['pages'] }>();
	const group = (key: PlanGroupKey) => {
		let g = byKey.get(key);
		if (!g) byKey.set(key, (g = { parts: [], count: 0, pages: [] }));
		return g;
	};
	const part = (key: PlanGroupKey, title: string) => {
		const g = group(key);
		let p = g.parts.find((x) => x.title === title);
		if (!p) g.parts.push((p = { title, rows: [], size: 0, paged: false, complete: true }));
		return p;
	};
	// Seeds that aren't current belong with the changes: a first run, or a changed seed.
	for (const s of sections) {
		const [key, kind] = SECTION_GROUP[s.title] ?? ['changed'];
		const linked = linkedSection(s.title);
		if (s.title === 'Seeds') {
			for (const e of s.entries) {
				const to: PlanGroupKey = e.value === 'first_run' ? 'first_run' : e.value === 'seed_changed' ? 'changed' : 'unchanged';
				const p = part(to, 'Seeds');
				p.rows.push(to === 'changed' ? { ...plainRow(e, false), detail: 'seed changed', kinds: ['seed'] } : plainRow(e, false));
				p.size++;
				group(to).count++;
			}
			continue;
		}
		const g = group(key);
		const p = part(key, s.title);
		p.rows.push(...s.entries.map((e) => (key === 'changed' ? changedRow(e, kind, linked) : plainRow(e, linked))));
		p.size += sectionSize(s);
		g.count += sectionSize(s);
		if (s.page && s.total !== undefined) {
			g.pages.push({ section: s.page, loaded: s.entries.length, total: s.total });
			p.paged = true;
			p.complete = s.entries.length >= s.total;
		}
	}
	const warnings = looseWarnings(plan, sections);
	const grouped = warnings.length ? groupWarnings(warnings) : [];
	if (warnings.length) group('warnings').count = warnings.length;
	return ORDER.flatMap((key) => {
		const g = byKey.get(key);
		if (!g || !g.count) return [];
		// The big paged section first (an Unchanged group's models), then the small ones once it is all loaded.
		const parts = [...g.parts].sort((a, b) => Number(b.paged) - Number(a.paged));
		const notes: string[] = [];
		const rows: PlanRow[] = [];
		for (const [i, p] of parts.entries()) {
			if (i > 0 && !parts[i - 1].complete) break;
			const note = key === 'warnings' ? null : sharedNote(p.rows);
			if (note) notes.push(parts.length > 1 ? `${p.title.toLowerCase()}: ${note}` : note);
			rows.push(...(note ? p.rows.map((r) => withoutNote(r, note)) : p.rows));
		}
		// A row with a config and a query change counts once, by its plan reason.
		const kinds = new Map<string, number>();
		for (const r of rows) if (r.kinds?.length) kinds.set(r.kinds[0], (kinds.get(r.kinds[0]) ?? 0) + 1);
		const breakdown =
			key === 'changed'
				? [...kinds].sort((a, b) => b[1] - a[1]).map(([k, n]) => `${n.toLocaleString()} ${k}`)
				: key === 'warnings'
					? [plural(grouped.length, 'kind')]
					: parts.length > 1
						? parts.map((p) => `${p.size.toLocaleString()} ${p.title.toLowerCase()}`)
						: [];
		return [{ key, title: TITLES[key], count: g.count, subtitle: [...breakdown, ...notes].join(' · '), rows, pages: g.pages, ...(key === 'warnings' ? { warningGroups: grouped } : {}) }];
	});
}

/** The index's one-line summary, such as "94 changed, 672 new, 121 renames, 64 loads, 433 Python · 1,507 unchanged". */
export function planSummaryLine(groups: PlanGroup[]): string {
	const n = (key: PlanGroupKey) => groups.find((g) => g.key === key)?.count ?? 0;
	const word: [PlanGroupKey, string][] = [
		['full_refresh', 'full refresh'],
		['changed', 'changed'],
		['first_run', 'new'],
		['renames', 'renames'],
		['loads', 'loads'],
		['python', 'Python'],
		['held_back', 'held back']
	];
	const main = word.filter(([key]) => n(key)).map(([key, w]) => `${n(key).toLocaleString()} ${w}`);
	const quiet = (['unchanged', 'skipped'] as PlanGroupKey[]).filter((key) => n(key)).map((key) => `${n(key).toLocaleString()} ${key === 'unchanged' ? 'unchanged' : 'skipped'}`);
	return [main.join(', '), quiet.join(', ')].filter(Boolean).join(' · ');
}

/** The group a `?section=` names, else the first in the index. */
export function selectedGroup(groups: PlanGroup[], wanted: string | null): PlanGroup | null {
	return groups.find((g) => g.key === wanted) ?? groups[0] ?? null;
}

/** How many changed lines an opened diff shows before "open the full diff". */
export const DIFF_PREVIEW = 12;

/** The start of a diff (its headers, context and first `most` changed lines), and how many lines it leaves out. */
export function diffPreview(diff: DiffLine[], most = DIFF_PREVIEW): { lines: DiffLine[]; cut: boolean; more: number } {
	let changed = 0;
	for (let i = 0; i < diff.length; i++) {
		if (diff[i].cls === 'add' || diff[i].cls === 'del') changed++;
		if (changed === most) return { lines: diff.slice(0, i + 1), cut: i + 1 < diff.length, more: diff.length - i - 1 };
	}
	return { lines: diff, cut: false, more: 0 };
}

/** How many warnings an opened warning group lists before "show all". */
export const WARNINGS_SHOWN = 20;
