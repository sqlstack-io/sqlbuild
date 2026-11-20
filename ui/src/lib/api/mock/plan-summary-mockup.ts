// The plan summary's mock plan (CHI-1563, `?mock&summary`): a `sqb build --changed` plan
// shaped like a large real one: 134 changed, 4 new, 58 renamed, 79 loads, 418 Python, 2,184 unchanged,
// 97 skipped and 83 warnings, and 63 migration steps on models counted in those: 2 model migrations into
// models that exist, 41 column renames, 12 table type changes and 8 retention changes. Each renamed
// model has its own rename migration, as a real plan's does. Pure.

import type { PlanDocument, PlanModel } from '../types';
import { DOMAINS, HEAD, incremental, model, name, queryDiff } from './large-plan';

/** The mockup's plan; `changed` adds the `--changed` decision (97 skipped). */
export function summaryPlanDocument(changed: boolean): PlanDocument {
	let i = 0;
	const models: PlanModel[] = [];
	// 134 changed: 120 query, 8 config, 4 through a changed function, 2 schema.
	for (let k = 0; k < 120; k++) models.push(model(i++, 'query_changed', { backfill: { action: k % 3 ? 'forward' : 'full', duration: null }, query_diff: queryDiff(i, 1 + ((k * 7) % 60), (k * 5) % 40) }));
	for (let k = 0; k < 8; k++) models.push(model(i++, 'config_changed', { config_diff: [...HEAD, '@@ -1,3 +1,3 @@', '   "materialized": "table",', '-  "full_refresh": true,', '+  "full_refresh": false,'], config_change: 'full_refresh true → false' }));
	for (let k = 0; k < 4; k++) models.push(model(i++, 'function_changed', { changed_functions: ['parse_order_codes'] }));
	for (let k = 0; k < 2; k++) models.push(model(i++, 'schema_changed', { schema_findings: [{ kind: 'column_added', column: 'region_code', expected_type: 'varchar', actual_type: null }] }));
	const changedFrom = 0;
	const changedTo = i;
	// 4 first runs.
	for (let k = 0; k < 4; k++) models.push(model(i++, 'first_run', k % 2 ? incremental() : {}));
	// 58 renamed models, each with its rename migration below.
	const renamedFrom = i;
	/** What stays at the `k`th renamed model's old name: a view there until a date, for two in three. */
	const oldName = (k: number) => (k % 3 ? { action: 'archive_and_view', view: `analytics.${name(renamedFrom + k)}_old`, reads: `analytics.${name(renamedFrom + k)}`, expires_at: '2026-12-18T00:00:00Z' } : null);
	for (let k = 0; k < 58; k++, i++) models.push(model(i, 'renamed', { origin_model: `${name(i)}_old`, old_name: oldName(k) }));
	// 2,170 models with no change of their own that still build: incremental runs and tables downstream of a change.
	for (let k = 0; k < 2170; k++) models.push(model(i++, k % 4 === 0 ? 'normal_incremental' : 'no_change', k % 4 === 0 ? incremental({ cursor: null }) : {}));
	const keys = models.map((m) => `model:${m.name}`);
	// The models --changed skips are planned like the others; only the decision leaves them unbuilt.
	const skipped = changed ? Array.from({ length: 97 }, (_, k) => model(8000 + k, 'no_change')) : [];
	models.push(...skipped);
	const seeds = Array.from({ length: 14 }, (_, k) => ({ name: `reference_codes_${k}`, reason: 'no_change' }));
	return {
		// As sqb plan counts it: models, seeds and functions; loads and Python nodes are counted apart.
		selected_count: models.length + seeds.length,
		models,
		seeds,
		source_loads: [
			...Array.from({ length: 70 }, (_, k) => ({ name: `${DOMAINS[k % DOMAINS.length]}_feed_${k}`, kind: 'source', write_strategy: 'append', cursor_column: 'loaded_at', is_reload: false })),
			...Array.from({ length: 9 }, (_, k) => ({ name: `${DOMAINS[k % DOMAINS.length]}_api_${k}`, kind: 'loader', write_strategy: 'merge', cursor_column: 'updated_at', is_reload: false }))
		],
		functions: [],
		python_nodes: Array.from({ length: 418 }, (_, k) => ({ name: `${DOMAINS[k % DOMAINS.length]}_export_${k}`, kind: 'asset', phase: 'after_models', identity_status: 'known' })),
		warnings: [
			...Array.from({ length: 71 }, (_, k) => ({ severity: 'warning', message: `Scenario 'test_${DOMAINS[k % 12]}_${k % 24}' provides fixture for unknown model '${name(k + 9000)}'.` })),
			...Array.from({ length: 9 }, (_, k) => ({ severity: 'warning', message: `Model '${name(changedFrom + k)}' reads column 'legacy_code_${k}', which is deprecated.` })),
			...Array.from({ length: 3 }, (_, k) => ({ severity: 'warning', message: `Scenario 'test_returns_${k}' requires source 'raw.returns_${k}', but no fixture was provided. Add a CTE named __source__raw__returns_${k} AS (...).` }))
		],
		migrations: [
			...Array.from({ length: 58 }, (_, k) => ({
				model: name(renamedFrom + k),
				origin_model: `${name(renamedFrom + k)}_old`,
				decision: 'renamed',
				completed_at: null,
				is_rename: true,
				compatibility: 'not_checked',
				transfer: 'rebuild',
				old_name: oldName(k)
			})),
			{ model: name(changedTo + 10), origin_model: 'orders__mart__totals_legacy', decision: 'forced_replace', completed_at: null, is_rename: false, compatibility: 'compatible', transfer: null, old_name: null },
			{ model: name(changedTo + 11), origin_model: 'customers__mart__summary_v1', decision: 'superseded_replace', completed_at: null, is_rename: false, compatibility: 'compatible', transfer: null, old_name: null }
		],
		column_migrations: Array.from({ length: 41 }, (_, k) => ({ model: name(k % 7 === 0 ? changedFrom + k : renamedFrom + 60 + k), origin_column: ['cust_id', 'amt', 'created', 'region', 'sku_code'][k % 5], destination_column: ['customer_id', 'amount', 'created_at', 'region_code', 'product_sku'][k % 5], decision: k % 9 === 4 ? 'record' : 'rename' })),
		table_type_conversions: Array.from({ length: 12 }, (_, k) => ({ model: name(changedTo + 100 + k * 3), actual_type: k < 9 ? 'permanent' : 'transient', desired_type: k < 9 ? 'transient' : 'permanent', downgrade: k < 9, irreversible_warning: k < 9 ? 'Time travel beyond 1 day is lost when the table becomes transient.' : null })),
		retention: Array.from({ length: 8 }, (_, k) =>
			k === 7
				? { scope: 'schema', name: 'analytics_archive', models: [name(changedTo + 300), name(changedTo + 301), name(changedTo + 302)], desired_days: 30, actual_days: 7, direction: 'increase', phase: 'post', irreversible_warning: null }
				: { scope: 'table', name: `analytics.${name(changedTo + 200 + k)}`, models: [name(changedTo + 200 + k)], desired_days: k < 5 ? 14 : 1, actual_days: k < 5 ? 1 : 7, direction: k < 5 ? 'increase' : 'decrease', phase: k < 5 ? 'post' : 'pre', irreversible_warning: k < 5 ? null : 'Lowering retention drops history older than 1 day; it cannot be restored.' }
		),
		synced_copy: null,
		changed: changed
			? {
					counts: ['definition'],
					reach: 'all',
					runs: [...keys, ...seeds.map((s) => `seed:${s.name}`)].map((node, k) => ({ node, reason: k < changedTo ? 'definition changed' : 'no completed record', downstream_of: null })),
					held_back: [],
					unchanged: skipped.map((m) => `model:${m.name}`)
				}
			: null
	};
}
