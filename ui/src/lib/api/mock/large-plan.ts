// A plan the size of a real project's `sqb build --changed`, for the plan page with `?mock&large`: about
// 2,400 nodes, with changed models carrying diffs (some whitespace-only, some changing keyword case),
// first runs, renames, loads, Python nodes, repeated warnings and 1,507 routine models. Served as the
// server does: a summary, with routine models, held-back and skipped nodes a page at a time. Pure.

import type { PlanDocument, PlanModel, PlanPage, PlanPageSection } from '../types';

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
	for (let k = 0; k < 4; k++) models.push(model(i++, 'config_changed', { config_diff: [...HEAD, '@@ -1,3 +1,3 @@', '   "materialized": "table",', '-  "full_refresh": true,', '+  "full_refresh": false,'] }));
	models.push(model(i++, 'config_changed', { ...incremental(), config_diff: ['-  "cursor": "observed_at",', '+  "cursor": "event_date",'], query_diff: queryDiff(i, 3, 2) }));
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
		seeds: Array.from({ length: 10 }, (_, k) => ({ name: `reference_codes_${k}`, reason: 'no_change' })),
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

const ROUTINE = new Set(['no_change', 'normal_incremental']);

/** The server's summary of `doc`: everything but its large sections, which it counts. */
export function planSummary(doc: PlanDocument): PlanDocument {
	const routine = doc.models.filter((m) => ROUTINE.has(m.reason));
	const routineKeys = new Set(routine.map((m) => `model:${m.name}`));
	return {
		...doc,
		models: doc.models.filter((m) => !ROUTINE.has(m.reason)),
		built_model_names: doc.models.map((m) => m.name),
		pages: { routine_models: routine.length, ...(doc.changed ? { held_back: doc.changed.held_back.length, unchanged: doc.changed.unchanged.length } : {}) },
		changed: doc.changed && { ...doc.changed, runs: doc.changed.runs.filter((r) => !routineKeys.has(r.node)), held_back: [], unchanged: [] }
	};
}

/** One page of a large section, as the server's plan rows read returns it. */
export function planPage(doc: PlanDocument, section: PlanPageSection, offset: number, limit = 50): PlanPage {
	if (section === 'routine_models') {
		const routine = doc.models.filter((m) => ROUTINE.has(m.reason));
		const rows = routine.slice(offset, offset + limit);
		const keys = new Set(rows.map((m) => `model:${m.name}`));
		return { total: routine.length, rows, runs: (doc.changed?.runs ?? []).filter((r) => keys.has(r.node)) };
	}
	const all: unknown[] = section === 'held_back' ? (doc.changed?.held_back ?? []) : (doc.changed?.unchanged ?? []);
	return { total: all.length, rows: all.slice(offset, offset + limit), runs: [] };
}
