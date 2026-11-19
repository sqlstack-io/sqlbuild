// The plan page's list as an index and a pane: the plan's sections (plan-text.ts) gathered into a few
// groups, one shown at a time, each row one line of name · action · detail. Pure.

import type { PlanDiff, PlanDocument, PlanPageSection, PlanSectionCount, PlanWarning, PlanWarningGroup } from '$lib/api';
import { plural } from '$lib/format/plural';
import { changedSections, loadedRows, looseWarnings, planSections, sectionSize, type PlanEntry, type PlanSection } from './plan-text';

/** The plan's groups in plan order; the index sorts them under Will run and Won't run. */
export type PlanGroupKey = 'full_refresh' | 'changed' | 'renames' | 'first_run' | 'loads' | 'python' | 'held_back' | 'warnings' | 'unchanged' | 'skipped';
const ORDER: PlanGroupKey[] = ['full_refresh', 'changed', 'renames', 'first_run', 'loads', 'python', 'held_back', 'warnings', 'unchanged', 'skipped'];
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
/** Each server section's group in the index. */
const GROUP_OF: Record<PlanPageSection, PlanGroupKey> = {
	full_refresh: 'full_refresh',
	query_changed: 'changed',
	function_changed: 'changed',
	config_changed: 'changed',
	schema_changed: 'changed',
	changed_functions: 'changed',
	renamed: 'renames',
	migrations: 'renames',
	column_migrations: 'renames',
	table_type_conversions: 'renames',
	retention: 'renames',
	first_run: 'first_run',
	loader_reloads: 'loads',
	loader_loads: 'loads',
	source_reloads: 'loads',
	source_loads: 'loads',
	python_nodes: 'python',
	held_back: 'held_back',
	warnings: 'warnings',
	routine_models: 'unchanged',
	functions: 'unchanged',
	first_run_seeds: 'first_run',
	changed_seeds: 'changed',
	unchanged_seeds: 'unchanged',
	unchanged: 'skipped'
};
/** The kind of change a Changed section counts as, for its breakdown. */
const CHANGE_KIND: Partial<Record<PlanPageSection, ChangeKind>> = {
	query_changed: 'query',
	config_changed: 'config',
	function_changed: 'function',
	changed_functions: 'function',
	schema_changed: 'schema'
};
/** A section's rows in words, for the breakdown of a group of several sections. */
const SECTION_WORD: Partial<Record<PlanPageSection, string>> = {
	routine_models: 'models',
	functions: 'functions',
	first_run_seeds: 'seeds',
	changed_seeds: 'seeds',
	unchanged_seeds: 'seeds',
	renamed: 'renamed',
	migrations: 'migrations',
	column_migrations: 'column migrations',
	table_type_conversions: 'table type changes',
	retention: 'retention',
	loader_reloads: 'loader reloads',
	loader_loads: 'loader loads',
	source_reloads: 'source reloads',
	source_loads: 'source loads'
};
const sectionWord = (section: PlanPageSection) => SECTION_WORD[section] ?? section.replace(/_/g, ' ');
/** A count in a breakdown, such as "87 query" or "1 seed": seeds say one or many. */
const counted = (n: number, word: string) => (word === 'seeds' ? plural(n, 'seed') : `${n.toLocaleString()} ${word}`);
/** Sections whose rows are catalog models, so they link to the catalog. */
const MODEL_ROWS = new Set<PlanPageSection>(['query_changed', 'function_changed', 'config_changed', 'schema_changed', 'first_run', 'renamed', 'routine_models', 'migrations', 'column_migrations', 'table_type_conversions', 'retention', 'held_back', 'unchanged']);
/** A whole plan's full refresh, which plan-text sums by materialization rather than listing as a section of models. */
const TITLE_SECTION: Record<string, PlanPageSection> = { 'Full refresh': 'full_refresh' };
/** The notes a section's subtitle can say once for its rows: why `--changed` runs them. */
const ROW_NOTES = new Set(['run_reason', 'reads']);

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
	/** A changed row's kinds of change. */
	kinds?: string[];
	/** The row's diff is read on demand ('diff'), or the server has none to give ('unavailable'). */
	fetch: 'diff' | 'unavailable' | null;
};

export type PlanGroup = {
	key: PlanGroupKey;
	title: string;
	/** Rows in the group, loaded or not. */
	count: number;
	/** The breakdown under the title, such as "87 query · 6 config · 1 function", and a note every row shares. */
	subtitle: string;
	rows: PlanRow[];
	/** Its server sections' rows loaded so far, in the order their rows read. */
	pages: { section: PlanPageSection; loaded: number; total: number }[];
	/** The Warnings group's warnings, grouped by message. */
	warningGroups?: WarningGroup[];
	/** The server sections the group gathers, in plan order. */
	sections: PlanPageSection[];
};

/**
 * A warnings group: one message template, how many warnings fill it in and those read so far. A server
 * summary's group starts empty; its warnings are rows `offset` onwards of the warnings section.
 */
export type WarningGroup = { template: string; count: number; offset: number; warnings: PlanWarning[]; label: string };

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

/** The executor's placeholders for a microbatch's bounds, which a preview reports as the range. */
const CURSOR_START = '__SQB_CURSOR_START__';
const CURSOR_END = '__SQB_CURSOR_END__';
/** A cursor range's placeholders in words: "from the last run → the latest data when the run starts". */
export function cursorWords(line: string): string {
	return line.replace(new RegExp(`${CURSOR_START}`, 'g'), 'from the last run').replace(new RegExp(`${CURSOR_END}`, 'g'), 'the latest data when the run starts');
}

/** The order a row's kinds of change read in, as "config + query". */
const KIND_ORDER: ChangeKind[] = ['config', 'query', 'function', 'schema'];

type Stats = { added: number; removed: number; whitespace_only: boolean };

/**
 * A changed model's row: the kinds of change, line counts (from the diff text when the row has it,
 * else the server's counts) or the config change, and the diff it opens. `fetched` is the model's diff
 * read on demand, null when the server has none to give.
 */
function changedRow(e: PlanEntry, kind: ChangeKind | undefined, linked: boolean, fetched?: PlanDiff | null): PlanRow {
	const queryText = e.queryDiff ?? fetched?.query_diff;
	const configText = e.configDiff ?? fetched?.config_diff;
	const query: Stats | undefined = queryText?.length ? { ...diffStats(queryText), whitespace_only: whitespaceOnly(queryText) } : e.diffStats?.query;
	const configStats: Stats | undefined = configText?.length ? { ...diffStats(configText), whitespace_only: false } : e.diffStats?.config;
	const kinds = new Set<ChangeKind>();
	if (kind) kinds.add(kind);
	if (query) kinds.add('query');
	if (configStats) kinds.add('config');
	if (e.schema?.length) kinds.add('schema');
	const cause = e.detail.find((d) => d.startsWith('cause: function '));
	if (cause) kinds.add('function');
	const config = e.configChange ?? null;
	const quiet = !!query?.whitespace_only;
	const label = KIND_ORDER.filter((k) => kinds.has(k)).join(' + ');
	const counts = query && !quiet ? `+${query.added} −${query.removed}` : null;
	const parts: string[] = [];
	if (quiet && kinds.size === 1) parts.push('whitespace only');
	else {
		parts.push(counts && kinds.size === 1 ? `${label} ${counts}` : label);
		if (config) parts.push(config);
		if (counts && kinds.size > 1) parts.push(counts);
		if (quiet) parts.push('query whitespace only');
	}
	if (cause) parts.push(cause.slice('cause: function '.length));
	if (e.schema?.length) parts.push(plural(e.schema.length, 'column'));
	// The diff the link opens: the query, else a config too long to read inline, else the schema.
	const diffOf = query ? 'query' : configStats && !config ? 'config' : e.schema?.length ? 'schema' : null;
	// Whether the row holds the text of the diff its link opens; if not, opening it reads it.
	const held = diffOf === 'query' ? !!queryText : diffOf === 'config' ? !!configText : true;
	const diff: DiffLine[] | null =
		diffOf && held
			? [
					...(configText?.length && !config ? configText.map((text) => ({ text, cls: lineClass(text) })) : []),
					...(e.schema ?? []).map((f) => ({ text: `${f.sign} ${f.text}`, cls: (f.sign === '+' ? 'add' : f.sign === '-' ? 'del' : 'hunk') as DiffLine['cls'] })),
					...(queryText ?? []).map((text) => ({ text, cls: lineClass(text) }))
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
		kinds: [...kinds],
		fetch: diffOf && !held ? (fetched === undefined ? 'diff' : 'unavailable') : null
	};
}

const plainRow = (e: PlanEntry, linked: boolean): PlanRow => {
	const [first = '', ...rest] = e.detail.map(cursorWords);
	return { name: e.name, action: e.value, detail: first, notes: rest, diff: null, diffOf: null, warnings: e.warnings ?? [], linked, fetch: null };
};

/** A note every loaded row shares, such as "no completed record": for a whole plan, which comes without counted notes. */
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
 * The note most of a section's rows share, from the server's counts: its text on a row, and as the
 * subtitle says it, "no completed record (all 1,507)" or "(1,200 of 1,507)". Said only when more than
 * half the rows carry it; `all` when every row does, so the rows can leave it off.
 */
export function sectionNote(count: PlanSectionCount): { row: string; text: string; all: boolean } | null {
	const top = count.notes.filter((n) => ROW_NOTES.has(n.kind) && n.value).sort((a, b) => b.count - a.count)[0];
	if (!top || top.count * 2 <= count.total) return null;
	const row = top.kind === 'reads' ? `reads ${top.value}` : top.value!;
	const all = top.count >= count.total;
	return { row, all, text: `${row} (${all ? `all ${count.total.toLocaleString()}` : `${top.count.toLocaleString()} of ${count.total.toLocaleString()}`})` };
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
	const sorted = [...groups].sort((a, b) => b[1].length - a[1].length);
	let offset = 0;
	return sorted.map(([template, list]) => {
		const noun = /^(\w+) …/.exec(template)?.[1];
		const firsts = new Set(list.map((w) => /'([^']*)'|`([^`]*)`|"([^"]*)"/.exec(w.message)?.slice(1).find(Boolean) ?? ''));
		const subject = noun && list.length > 1 && firsts.size < list.length ? { noun: noun.toLowerCase(), count: firsts.size } : null;
		const group = { template, count: list.length, offset, warnings: list, label: warningGroupLabel(list.length, subject) };
		offset += list.length;
		return group;
	});
}

/** A warning group's size, and how many named things it covers when they repeat: "71 in 24 scenarios". */
function warningGroupLabel(count: number, subject: PlanWarningGroup['subject']): string {
	return subject ? `${count} in ${plural(subject.count, subject.noun)}` : plural(count, 'warning');
}

/** A server summary's warning groups, each starting where the one before ends in the warnings section. */
function serverWarningGroups(groups: PlanWarningGroup[], total: number): WarningGroup[] {
	let offset = 0;
	const listed: WarningGroup[] = groups.map((g) => {
		const group = { template: g.template, count: g.count, offset, warnings: [], label: warningGroupLabel(g.count, g.subject) };
		offset += g.count;
		return group;
	});
	// The summary lists only the largest groups; the warnings past them page as one rest.
	const rest = total - offset;
	return rest > 0 ? [...listed, { template: REST_TEMPLATE, count: rest, offset, warnings: [], label: `${plural(rest, 'warning')} of smaller kinds` }] : listed;
}

/** The group holding the warnings past a server summary's largest groups. */
const REST_TEMPLATE = 'Other warnings';

/** The plan's sections with their sizes and notes: the server summary's, else counted from a whole plan's rows. */
export function sectionCounts(plan: PlanDocument, sections: PlanSection[]): PlanSectionCount[] {
	if (plan.sections) return plan.sections;
	const counts: PlanSectionCount[] = sections.flatMap((s) => {
		const section = s.section ?? TITLE_SECTION[s.title];
		return section ? [{ section, total: sectionSize(s), notes: [] }] : [];
	});
	const warnings = looseWarnings(plan, sections).length;
	return warnings ? [...counts, { section: 'warnings', total: warnings, notes: [] }] : counts;
}

/**
 * The plan's groups in index order, empty ones left out: counts and notes from the server's sections,
 * rows those loaded so far. `refresh` words what refreshes a held-back job's nodes; `diffs` holds the
 * diffs read on demand, by model (null: the server has none to give).
 */
export function planGroups(plan: PlanDocument, refresh: (job: string) => string = (job) => `job ${job}`, diffs: ReadonlyMap<string, PlanDiff | null> = new Map()): PlanGroup[] {
	const sections: PlanSection[] = [...planSections(plan), ...(plan.changed ? changedSections(plan.changed, refresh, plan.pages) : [])];
	const textOf = new Map(sections.map((s) => [s.section ?? TITLE_SECTION[s.title], s]));
	const counts = sectionCounts(plan, sections);
	const warnings = looseWarnings(plan, sections);
	const warningTotal = counts.find((c) => c.section === 'warnings')?.total ?? 0;
	const grouped = plan.warning_groups ? serverWarningGroups(plan.warning_groups, warningTotal) : warnings.length ? groupWarnings(warnings) : [];
	const warningKinds = plan.warning_groups ? plural(plan.warning_groups.length, 'kind') + (grouped.length > plan.warning_groups.length ? ' and more' : '') : plural(grouped.length, 'kind');
	// The default mock serves whole plans, with every row and no counted notes; the server always pages them.
	const whole = !plan.sections;
	return ORDER.flatMap((key) => {
		const mine = counts.filter((c) => GROUP_OF[c.section] === key && c.total > 0);
		if (!mine.length) return [];
		const count = mine.reduce((n, c) => n + c.total, 0);
		const notes: string[] = [];
		const rows: PlanRow[] = [];
		const pages: PlanGroup['pages'] = [];
		let complete = true;
		for (const c of mine) {
			const entries = textOf.get(c.section)?.entries ?? [];
			const loaded = key === 'warnings' ? loadedRows(plan, 'warnings') : entries.length;
			if (!whole) pages.push({ section: c.section, loaded, total: c.total });
			// A section's rows follow the one before only once that one is all loaded, so rows read in order.
			if (key === 'warnings' || !complete) continue;
			const linked = MODEL_ROWS.has(c.section);
			let sectionRows = entries.map((e) => (key === 'changed' ? changedRow(e, CHANGE_KIND[c.section], linked, diffs.get(e.name)) : plainRow(e, linked)));
			// A changed row's run reason is its change, which the row already says.
			const note = key === 'changed' ? null : whole ? sharedNote(sectionRows) : sectionNote(c);
			const said = typeof note === 'string' ? note : note?.text;
			if (said) notes.push(mine.length > 1 ? `${sectionWord(c.section)}: ${said}` : said);
			// A note every row carries is said once, in the subtitle, and left off the rows.
			const off = typeof note === 'string' ? note : note?.all ? note.row : null;
			if (off) sectionRows = sectionRows.map((r) => withoutNote(r, off));
			rows.push(...sectionRows);
			complete = whole || entries.length >= c.total;
		}
		const kinds = new Map<string, number>();
		if (key === 'changed') for (const c of mine) kinds.set(CHANGE_KIND[c.section] ?? sectionWord(c.section), (kinds.get(CHANGE_KIND[c.section] ?? sectionWord(c.section)) ?? 0) + c.total);
		const breakdown =
			key === 'changed'
				? [...kinds].sort((a, b) => b[1] - a[1]).map(([k, n]) => counted(n, k))
				: key === 'warnings'
					? grouped.length && (whole || plan.warning_groups || loadedRows(plan, 'warnings') >= count)
						? [warningKinds]
						: []
					: mine.length > 1
						? mine.map((c) => counted(c.total, sectionWord(c.section)))
						: [];
		return [{ key, title: TITLES[key], count, subtitle: [...breakdown, ...notes].join(' · '), rows, pages, sections: mine.map((c) => c.section), ...(key === 'warnings' ? { warningGroups: grouped } : {}) }];
	});
}

/** `plan` without its loaded rows: the start of a name-filtered view of it, whose pages hold only matching rows. */
export function withoutRows(plan: PlanDocument): PlanDocument {
	return {
		...plan,
		models: [],
		seeds: [],
		source_loads: [],
		functions: [],
		python_nodes: [],
		warnings: [],
		migrations: [],
		column_migrations: [],
		table_type_conversions: [],
		retention: [],
		loaded: {},
		changed: plan.changed && { ...plan.changed, runs: [], held_back: [], unchanged: [] },
		synced_copy: plan.synced_copy && { ...plan.synced_copy, cursor_window_starts: {} }
	};
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
