// A plan the size of a real project's `sqb build --changed`, for the plan page with `?mock&large`: about
// 2,400 nodes, with changed models carrying diffs (some whitespace-only, some changing keyword case),
// first runs, renames, loads, Python nodes, repeated warnings and 1,507 routine models. Served as the
// server does: a summary, with routine models, held-back and skipped nodes a page at a time. Pure.

import { groupWarnings } from '$lib/run/plan-index';
import { seedSection } from '$lib/run/plan-text';
import type { PlanDiff, PlanDiffStats, PlanDocument, PlanModel, PlanPage, PlanPageSection, PlanSectionCount, PlanSectionNote, PlanWarning } from '../types';

const DOMAINS = ['orders', 'customers', 'inventory', 'payments', 'shipping', 'returns', 'pricing', 'suppliers', 'regions', 'finance', 'marketing', 'support'];
const LAYERS = ['stg', 'int', 'mart'];
const NOUNS = ['daily_rollup', 'events', 'snapshot', 'curated', 'enriched', 'mapped', 'totals', 'history', 'latest', 'summary'];
/** A neutral model name, distinct for each `i`. */
const name = (i: number) => `${DOMAINS[i % DOMAINS.length]}__${LAYERS[Math.floor(i / DOMAINS.length) % LAYERS.length]}__${NOUNS[Math.floor(i / (DOMAINS.length * LAYERS.length)) % NOUNS.length]}_${i}`;

const HEAD = ['--- previous', '+++ current'];
/** A query diff of `added` and `removed` lines. */
function queryDiff(i: number, added: number, removed: number): string[] {
	return [...HEAD, `@@ -1,${removed + 2} +1,${added + 2} @@`, ' select', ...Array.from({ length: removed }, (_, k) => `-    old_column_${k},`), ...Array.from({ length: added }, (_, k) => `+    new_column_${k}_${i},`), ` from ${name(i + 1)}`];
}
const model = (i: number, reason: string, extra: Partial<PlanModel> = {}): PlanModel => ({
	name: name(i),
	materialization_type: 'table',
	action: 'create_table',
	reason,
	backfill: { action: 'forward', duration: null },
	qualified_name: `analytics.${name(i)}`,
	...extra
});
const incremental = (extra: Partial<PlanModel> = {}): Partial<PlanModel> => ({
	materialization_type: 'incremental',
	action: 'incremental_merge',
	incremental_strategy: 'merge',
	cursor_column: 'updated_at',
	cursor_type: 'timestamp',
	cursor: { requested_bounds: { start: null, end: null }, resolved_bounds: { start: '__SQB_CURSOR_START__', end: '__SQB_CURSOR_END__' } },
	...extra
});

/** The whole plan, as `sqb plan --json` prints it. */
export function largePlanDocument(changed: boolean): PlanDocument {
	let i = 0;
	const models: PlanModel[] = [];
	// 94 changed: 80 query changes of every size, 5 whitespace-only, 2 changing keyword case, 6 config, 1 function.
	for (let k = 0; k < 80; k++) models.push(model(i++, 'query_changed', { action: 'create_table', backfill: { action: k % 3 ? 'forward' : 'full', duration: null }, query_diff: queryDiff(i, 1 + ((k * 7) % 90), (k * 5) % 60) }));
	for (let k = 0; k < 5; k++) models.push(model(i++, 'query_changed', { query_diff: [...HEAD, '@@ -1,2 +1,3 @@', '-select id,  amount', '-from orders', '+select id,', '+       amount', '+from orders'] }));
	for (let k = 0; k < 2; k++) models.push(model(i++, 'query_changed', { query_diff: [...HEAD, '@@ -1,2 +1,2 @@', '-select id from orders', '+SELECT id FROM orders'] }));
	for (let k = 0; k < 4; k++) models.push(model(i++, 'config_changed', { config_diff: [...HEAD, '@@ -1,3 +1,3 @@', '   "materialized": "table",', '-  "full_refresh": true,', '+  "full_refresh": false,'], config_change: 'full_refresh true → false' }));
	models.push(model(i++, 'config_changed', { ...incremental(), config_diff: ['-  "cursor": "observed_at",', '+  "cursor": "event_date",'], config_change: 'cursor observed_at → event_date', query_diff: queryDiff(i, 3, 2) }));
	models.push(model(i++, 'config_changed', { config_diff: ['-  "grain": "hour",', '+  "grain": "day",', '-  "lookback": 2,', '+  "lookback": 3,', '-  "partition_by": "event_date",', '+  "partition_by": "event_hour",'] }));
	models.push(model(i++, 'function_changed', { changed_functions: ['parse_orders_html'] }));
	// 672 first runs; a third are incremental, their range the run's own bounds.
	for (let k = 0; k < 672; k++) models.push(model(i++, 'first_run', k % 3 === 0 ? incremental() : {}));
	// 1,507 routine models.
	const routineFrom = i;
	for (let k = 0; k < 1507; k++) models.push(model(i++, k % 4 === 0 ? 'normal_incremental' : 'no_change', k % 4 === 0 ? incremental({ cursor: null }) : {}));
	const keys = models.map((m) => `model:${m.name}`);
	return {
		selected_count: 2400,
		models,
		seeds: Array.from({ length: 10 }, (_, k) => ({ name: `reference_codes_${k}`, reason: k < 2 ? 'first_run' : k < 3 ? 'config_changed' : 'no_change' })),
		source_loads: Array.from({ length: 64 }, (_, k) => ({ name: `${DOMAINS[k % DOMAINS.length]}_feed_${k}`, kind: 'source', write_strategy: 'append', cursor_column: 'loaded_at', is_reload: false })),
		functions: Array.from({ length: 12 }, (_, k) => ({ name: `format_${DOMAINS[k % DOMAINS.length]}_code`, language: 'sql', reason: 'no_change' })),
		python_nodes: Array.from({ length: 433 }, (_, k) => ({ name: `${DOMAINS[k % DOMAINS.length]}_export_${k}`, kind: 'asset', phase: 'after_models', identity_status: 'known' })),
		warnings: [
			// 71 alike, about 24 scenarios' unused mocks, and 3 of another kind.
			...Array.from({ length: 71 }, (_, k) => ({ severity: 'warning', message: `Scenario 'test_${DOMAINS[k % 12]}_${k % 24}' provides fixture for unknown model '${name(k + 3000)}'.` })),
			...Array.from({ length: 3 }, (_, k) => ({ severity: 'warning', message: `Scenario 'test_returns_${k}' requires source 'raw.returns_${k}', but no fixture was provided. Add a CTE named __source__raw__returns_${k} AS (...).` }))
		],
		migrations: Array.from({ length: 121 }, (_, k) => ({ model: name(5000 + k), origin_model: name(6000 + k), decision: 'renamed', completed_at: null })),
		column_migrations: [],
		table_type_conversions: [],
		retention: [],
		synced_copy: null,
		changed: changed
			? {
					counts: ['definition'],
					reach: 'all',
					runs: keys.map((node, k) => ({ node, reason: k >= routineFrom ? 'no completed record' : 'definition changed', downstream_of: null })),
					held_back: Array.from({ length: 40 }, (_, k) => ({ node: `model:${name(7000 + k)}`, refreshed_by: ['nightly_all'] })),
					unchanged: Array.from({ length: 300 }, (_, k) => `model:${name(8000 + k)}`)
				}
			: null
	};
}

const MODEL_SECTIONS: PlanPageSection[] = ['full_refresh', 'query_changed', 'function_changed', 'config_changed', 'schema_changed', 'first_run', 'renamed'];
/** Every section in plan order, as the server counts them. */
const SECTIONS: PlanPageSection[] = [...MODEL_SECTIONS, 'routine_models', 'migrations', 'column_migrations', 'table_type_conversions', 'retention', 'loader_reloads', 'loader_loads', 'source_reloads', 'source_loads', 'changed_functions', 'functions', 'first_run_seeds', 'changed_seeds', 'unchanged_seeds', 'python_nodes', 'held_back', 'unchanged', 'warnings'];
const ROW_LISTS = ['models', 'seeds', 'source_loads', 'functions', 'python_nodes', 'warnings', 'migrations', 'column_migrations', 'table_type_conversions', 'retention'] as const;
const keyName = (key: string) => key.slice(key.indexOf(':') + 1);

/** The section's rows in plan order, as the server's plan rows read finds them. */
function sectionRows(doc: PlanDocument, section: PlanPageSection): { rows: unknown[]; name: (row: unknown) => string } {
	const runs = doc.changed && new Set(doc.changed.runs.map((r) => r.node));
	const runsNode = (key: string) => !runs || runs.has(key);
	const named = (rows: { name: string }[]) => ({ rows, name: (row: unknown) => (row as { name: string }).name });
	const byModel = (rows: { model: string }[]) => ({ rows, name: (row: unknown) => (row as { model: string }).model });
	if (MODEL_SECTIONS.includes(section) || section === 'routine_models') {
		const built = doc.models.filter((m) => m.action !== 'skip' && runsNode(`model:${m.name}`));
		return named(built.filter((m) => (MODEL_SECTIONS.includes(m.reason as PlanPageSection) ? m.reason === section : section === 'routine_models')));
	}
	const loads = (kind: string, reload: boolean) => named(doc.source_loads.filter((l) => l.kind === kind && l.is_reload === reload));
	const changedFunction = (f: PlanDocument['functions'][number]) => !!f.reason && f.reason !== 'no_change';
	switch (section) {
		case 'migrations':
			return byModel(doc.migrations);
		case 'column_migrations':
			return byModel(doc.column_migrations);
		case 'table_type_conversions':
			return byModel(doc.table_type_conversions);
		case 'retention':
			return byModel(doc.retention);
		case 'loader_reloads':
			return loads('loader', true);
		case 'loader_loads':
			return loads('loader', false);
		case 'source_reloads':
			return loads('source', true);
		case 'source_loads':
			return loads('source', false);
		case 'changed_functions':
			return named(doc.functions.filter(changedFunction));
		case 'functions':
			return named(doc.functions.filter((f) => !changedFunction(f)));
		case 'first_run_seeds':
		case 'changed_seeds':
		case 'unchanged_seeds':
			return named(doc.seeds.filter((s) => runsNode(`seed:${s.name}`) && seedSection(s.reason) === section));
		case 'python_nodes':
			return named(doc.python_nodes);
		case 'held_back':
			return { rows: doc.changed?.held_back ?? [], name: (row) => keyName((row as { node: string }).node) };
		case 'unchanged':
			return { rows: doc.changed?.unchanged ?? [], name: (row) => keyName(row as string) };
		default:
			return { rows: groupWarnings(doc.warnings).flatMap((g) => g.warnings), name: (row) => `${(row as PlanWarning).model_name ?? ''} ${(row as PlanWarning).message}` };
	}
}

/** The notes of a section's model rows the mock counts: why `--changed` runs each. */
function runNotes(doc: PlanDocument, rows: unknown[]): PlanSectionNote[] {
	const runs = new Map((doc.changed?.runs ?? []).map((r) => [r.node, r]));
	const counts = new Map<string, PlanSectionNote>();
	for (const row of rows) {
		const run = runs.get(`model:${(row as { name: string }).name}`);
		if (!run) continue;
		const [kind, value] = run.downstream_of ? ['reads', keyName(run.downstream_of)] : ['run_reason', run.reason];
		const note = counts.get(`${kind}\u0000${value}`) ?? { kind, value, count: 0 };
		note.count++;
		counts.set(`${kind}\u0000${value}`, note);
	}
	return [...counts.values()].sort((a, b) => b.count - a.count);
}

/** The things a group's label counts, "71 in 24 scenarios", as the server's `subject`. */
function subjectOf(label: string): { noun: string; count: number } | null {
	const found = / in (\d+) (\w+?)s?$/.exec(label);
	return found ? { noun: found[2], count: Number(found[1]) } : null;
}

/** The server's summary of `doc`: its facts, every row list empty, and each section counted. */
export function planSummary(doc: PlanDocument): PlanDocument {
	const sections: PlanSectionCount[] = SECTIONS.flatMap((section) => {
		const { rows } = sectionRows(doc, section);
		const models = MODEL_SECTIONS.includes(section) || section === 'routine_models';
		return rows.length ? [{ section, total: rows.length, notes: models ? runNotes(doc, rows) : [] }] : [];
	});
	const empty = Object.fromEntries(ROW_LISTS.map((list) => [list, []]));
	return {
		...doc,
		...empty,
		sections,
		pages: Object.fromEntries(sections.map((s) => [s.section, s.total])),
		warning_groups: groupWarnings(doc.warnings).map((g) => ({ template: g.template, count: g.count, subject: subjectOf(g.label) })),
		changed: doc.changed && { ...doc.changed, runs: [], held_back: [], unchanged: [] },
		synced_copy: doc.synced_copy && { ...doc.synced_copy, cursor_window_starts: {} }
	};
}

/** One page of a section's rows whose name contains `name`, as the server's plan rows read returns it. */
export function planPage(doc: PlanDocument, section: PlanPageSection, offset: number, limit = 50, name?: string): PlanPage {
	const { rows: all, name: nameOf } = sectionRows(doc, section);
	const wanted = name?.toLowerCase();
	const matching = wanted ? all.filter((row) => nameOf(row).toLowerCase().includes(wanted)) : all;
	const rows = matching.slice(offset, offset + limit);
	const models = MODEL_SECTIONS.includes(section) || section === 'routine_models';
	const names = new Set(models ? rows.map((row) => (row as { name: string }).name) : []);
	const starts = doc.synced_copy?.cursor_window_starts ?? {};
	return {
		total: matching.length,
		section_total: all.length,
		rows: models ? (rows as PlanModel[]).map(withDiffStats) : rows,
		runs: (doc.changed?.runs ?? []).filter((r) => names.has(keyName(r.node))),
		warnings: doc.warnings.filter((w) => names.has(w.model_name ?? w.message.split(':', 1)[0])),
		cursor_window_starts: Object.fromEntries(Object.entries(starts).filter(([model]) => names.has(model)))
	};
}

/** A diff's changed lines, and whether they differ only in whitespace, as the server counts them. */
function stats(diff: string[]): PlanDiffStats {
	const side = (sign: string) => diff.filter((l) => l.startsWith(sign) && !/^(---|\+\+\+)/.test(l));
	const text = (lines: string[]) => lines.map((l) => l.slice(1)).join('').replace(/\s+/g, '');
	const [added, removed] = [side('+'), side('-')];
	return { added: added.length, removed: removed.length, whitespace_only: text(added) === text(removed) };
}

/** A model row as a page carries it: its diff counted, the text left for the diff read. */
function withDiffStats({ query_diff, config_diff, ...m }: PlanModel): PlanModel {
	if (!query_diff && !config_diff) return m;
	return { ...m, diff_stats: { ...(query_diff ? { query: stats(query_diff) } : {}), ...(config_diff ? { config: stats(config_diff) } : {}) } };
}

/** A model whose diff the server won't give, as for a user without permission to read its SQL. */
export const DIFF_WITHHELD = name(79);

/** A model's diff text from the whole document, as the server's plan diff read returns it. */
export function planModelDiff(doc: PlanDocument, model: string): PlanDiff | null {
	const found = doc.models.find((m) => m.name === model);
	if (!found || model === DIFF_WITHHELD || !(found.query_diff || found.config_diff)) return null;
	return { model, ...(found.query_diff ? { query_diff: found.query_diff } : {}), ...(found.config_diff ? { config_diff: found.config_diff } : {}) };
}
