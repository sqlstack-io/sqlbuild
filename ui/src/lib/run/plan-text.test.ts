import { describe, expect, it } from 'vitest';
import type { PlanDocument, PlanModel } from '$lib/api';
import { actionText, changedSections, cloneCommands, jobRefreshText, loadedRows, looseWarnings, planSections, sectionSize, warningText } from './plan-text';

const model = (name: string, over: Partial<PlanModel> = {}): PlanModel => ({
	name,
	materialization_type: 'table',
	action: 'create_table',
	reason: 'no_change',
	backfill: { action: 'forward', duration: null },
	...over
});
const plan = (models: PlanModel[], over: Partial<PlanDocument> = {}): PlanDocument => ({
	selected_count: models.length,
	models,
	seeds: [],
	source_loads: [],
	functions: [],
	python_nodes: [],
	warnings: [],
	migrations: [],
	column_migrations: [],
	table_type_conversions: [],
	retention: [],
	synced_copy: null,
	changed: null,
	...over
});

describe('plan text, as sqb plan prints it', () => {
	it('words a changed model by its backfill, as the CLI does', () => {
		expect(actionText(model('a', { action: 'create_view', backfill: { action: 'full', duration: null } }))).toBe('recreate view');
		expect(actionText(model('a', { backfill: { action: 'bounded', duration: '7d' } }))).toBe('rebuild last 7d');
		expect(actionText(model('a', { reason: 'query_changed', backfill: { action: 'full', duration: null } }))).toBe('full rebuild');
		expect(actionText(model('a', { reason: 'first_run', backfill: { action: 'full', duration: null } }))).toBe('continue forward');
	});

	it('words maintained models as the CLI does, with no replay policy', () => {
		const recreated = model('a', { action: 'create_maintained', reason: 'query_changed', backfill: { action: 'full', duration: null } });
		expect(actionText(recreated)).toBe('recreate, full recompute');
		expect(actionText(model('a', { action: 'create_maintained', reason: 'first_run' }))).toBe('create');
		expect(actionText(model('a', { action: 'alter_maintained', reason: 'config_changed' }))).toBe('change refresh settings in place');
		const [section] = planSections(plan([recreated]));
		expect(section.entries[0].detail).toEqual([]);
	});

	it('says a kept maintained model recomputes in full when an upstream it reads is rebuilt', () => {
		const kept = model('daily_totals', { materialization_type: 'maintained', action: 'keep_maintained', reinitialised_by: 'orders' });
		const [routine] = planSections(plan([kept]));
		expect(routine.entries).toEqual([{ name: 'daily_totals', value: 'maintained', detail: ['recomputes in full: orders is rebuilt'] }]);
	});

	it('says a kept maintained model is paused when the warehouse lists it as suspended', () => {
		const kept = model('daily_totals', { materialization_type: 'maintained', action: 'keep_maintained', paused: true });
		const [routine] = planSections(plan([kept]));
		expect(routine.entries).toEqual([{ name: 'daily_totals', value: 'maintained', detail: ['paused: scheduled refreshes are suspended'] }]);
	});

	it('puts each change in its own section in reason order, then the routine models, and leaves skipped ones out', () => {
		const sections = planSections(
			plan([
				model('fresh', { reason: 'first_run' }),
				model('orders', { reason: 'query_changed', backfill: { action: 'full', duration: null }, query_diff: ['--- previous', '+++ current', '-SELECT 1', '+SELECT 2'] }),
				model('lines', { materialization_type: 'incremental', incremental_strategy: 'merge', cursor_type: 'timestamp', cursor_column: 'placed_at', action: 'incremental_merge', reason: 'normal_incremental' }),
				model('customers'),
				model('retired', { action: 'skip', reason: 'disabled' })
			])
		);
		expect(sections.map((s) => [s.title, s.entries.map((e) => e.name)])).toEqual([
			['Query changed', ['orders']],
			['First run', ['fresh']],
			['Models', ['lines', 'customers']]
		]);
		const [changed, , routine] = sections;
		expect(changed.entries[0]).toMatchObject({ value: 'full rebuild', detail: ['policy: replay_on_change=full'], queryDiff: ['--- previous', '+++ current', '-SELECT 1', '+SELECT 2'] });
		// Unchanged tables still build: they are listed with their materialization, not as skipped.
		expect(routine.entries).toEqual([
			{ name: 'lines', value: 'merge (timestamp)', detail: ['cursor: placed_at (timestamp)'] },
			{ name: 'customers', value: 'table', detail: [] }
		]);
	});

	it('shows a preview window from the synced cursor to the run, and keeps a range the plan resolved', () => {
		const incremental = { materialization_type: 'incremental', incremental_strategy: 'append', cursor_type: 'timestamp', cursor_column: 'placed_at', action: 'incremental_append', reason: 'normal_incremental' } as const;
		const resolved = { start: '2026-01-02 00:00:00', end: '2026-01-03 00:00:00' };
		const preview = plan([model('lines', incremental), model('events', { ...incremental, cursor_bounds: resolved })], {
			synced_copy: { synced_at: '2026-01-03T00:05:00Z', cursor_window_starts: { lines: '2026-01-01 01:00:00', events: '2025-12-31 00:00:00' }, unknown_until_run: ['retention'] }
		});
		expect(planSections(preview)[0].entries.map((e) => e.detail)).toEqual([
			['cursor: placed_at (timestamp)', 'range: 2026-01-01 01:00:00 → latest data when the run starts'],
			['cursor: placed_at (timestamp)', 'range: 2026-01-02 00:00:00 → 2026-01-03 00:00:00']
		]);
	});

	it("shows a model's warning with the changed model it names, keeps the rest for the warnings list, and leaves out the forward-only advice", () => {
		const forward = "events: the model's SQL changed, so already-built dates keep their old results - only new dates will use the new query.";
		const p = plan(
			[model('events', { reason: 'query_changed', materialization_type: 'incremental' }), model('orders', { reason: 'query_changed' }), model('event_counts')],
			{ warnings: [{ severity: 'warning', message: forward }, { severity: 'warning', message: 'lag', model_name: 'orders' }, { severity: 'warning', message: 'event_counts: routine' }, { severity: 'error', message: 'no model named here' }] }
		);
		const sections = planSections(p);
		expect(sections[0].entries.map((e) => [e.name, e.warnings?.map(warningText)])).toEqual([
			['events', undefined],
			['orders', ['Lag']]
		]);
		// Only changed models carry warnings; one about a routine model, or about no model, is listed on its own.
		expect(looseWarnings(p, sections).map((w) => w.message)).toEqual(['event_counts: routine', 'no model named here']);
	});

	it('shows a warning once, under the first entry for its model, when the model is in more than one changed section', () => {
		const lag = { severity: 'warning', message: 'lag', model_name: 'orders' };
		const p = plan([model('orders', { reason: 'query_changed' })], { warnings: [lag], retention: [{ model: 'orders', action: 'drop' }] });
		expect(planSections(p).flatMap((s) => s.entries.map((e) => [s.title, e.warnings?.length ?? 0]))).toEqual([
			['Retention', 1],
			['Query changed', 0]
		]);
	});

	it('collapses a plan that is all full refresh into counts by materialization', () => {
		const full = plan([model('a', { reason: 'full_refresh' }), model('b', { reason: 'full_refresh' }), model('v', { reason: 'full_refresh', materialization_type: 'view' })]);
		expect(planSections(full)).toEqual([{ title: 'Full refresh', changed: true, entries: [{ name: 'table', value: '2 models', detail: [] }, { name: 'view', value: '1 model', detail: [] }] }]);
	});
});

describe('plan under --changed', () => {
	// The reach fixture's `sqb plan --changed --json` after stg_orders is redefined: finance_daily holds two models back.
	const reach = plan(
		[
			model('stg_customers'),
			model('stg_orders', { reason: 'query_changed' }),
			model('customer_orders'),
			model('int_orders'),
			model('fct_revenue'),
			model('orders_export'),
			model('rpt_pnl'),
			model('summary')
		],
		{
			changed: {
				counts: ['definition'],
				reach: 'job',
				runs: [
					{ node: 'model:customer_orders', reason: 'upstream model stg_orders runs', downstream_of: 'model:stg_orders' },
					{ node: 'model:int_orders', reason: 'upstream model stg_orders runs', downstream_of: 'model:stg_orders' },
					{ node: 'model:orders_export', reason: 'upstream model stg_orders runs', downstream_of: 'model:stg_orders' },
					{ node: 'model:stg_orders', reason: 'planner change (query_changed)', downstream_of: null }
				],
				held_back: [
					{ node: 'model:fct_revenue', refreshed_by: ['finance_daily'] },
					{ node: 'model:rpt_pnl', refreshed_by: ['finance_daily'] },
					{ node: 'model:summary', reads: 'model:fct_revenue' }
				],
				unchanged: ['model:stg_customers']
			}
		}
	);

	it('lists only what runs, then what waits for its job and, collapsed, what is skipped, as sqb plan does', () => {
		const sections = [...planSections(reach), ...changedSections(reach.changed!, (job) => `job ${job}`)];
		expect(sections.map((s) => [s.title, s.entries.map((e) => `${e.name}: ${e.value}${e.detail.length ? ` (${e.detail.join(', ')})` : ''}`)])).toEqual([
			['Query changed', ['stg_orders: continue forward']],
			['Models', ['customer_orders: table (reads stg_orders)', 'int_orders: table (reads stg_orders)', 'orders_export: table (reads stg_orders)']],
			['Held back', ['fct_revenue: job finance_daily', 'rpt_pnl: job finance_daily', 'summary: reads fct_revenue, which is held back']],
			['Unchanged', ['stg_customers: skipped by --changed']]
		]);
		expect(changedSections(reach.changed!, (job) => job).map((s) => [s.title, !!s.collapsed])).toEqual([
			['Held back', false],
			['Unchanged', true]
		]);
	});

	it('names the models that run under --changed for their own reason, not as reading a change', () => {
		const first = plan([model('orders'), model('order_count', { action: 'incremental_append', reason: 'normal_incremental' })], {
			changed: {
				counts: ['definition'],
				reach: 'job',
				runs: [
					{ node: 'model:order_count', reason: 'no completed record', downstream_of: null },
					{ node: 'model:orders', reason: 'no completed record', downstream_of: null }
				],
				held_back: [],
				unchanged: []
			}
		});
		expect(planSections(first).find((s) => s.title === 'Models')?.entries.map((e) => e.detail.at(-1))).toEqual(['no completed record', 'no completed record']);
	});

	it("says what refreshes a held-back node: the named job's next run, its sensor, or that it is paused", () => {
		const now = Date.parse('2026-01-01T00:00:00Z');
		type Kind = 'schedule' | 'sensor';
		const trigger = (jobKey: string, state: 'running' | 'paused', nextDueAt: string | null, type: Kind = 'schedule') => ({ jobKey, type, state, nextDueAt });
		// A group's schedule says nothing about a job whose own trigger is paused; the plan names the group itself when it matters.
		expect(jobRefreshText('finance_daily', [trigger('finance_daily', 'paused', '2026-01-01T05:00:00Z'), trigger('nightly', 'running', '2026-01-01T02:00:00Z')], now)).toBe('job finance_daily is paused');
		expect(jobRefreshText('nightly', [trigger('nightly', 'running', '2026-01-01T02:00:00Z'), trigger('nightly', 'running', '2026-01-01T05:00:00Z')], now)).toBe('job nightly · next run in 2h');
		// While no server runs the scheduler, neither a schedule nor a sensor starts the job, so it shows no countdown.
		const stopped = true;
		expect(jobRefreshText('nightly', [trigger('nightly', 'running', '2026-01-01T02:00:00Z')], now, stopped)).toBe('job nightly · scheduler stopped');
		expect(jobRefreshText('finance_daily', [trigger('finance_daily', 'running', null, 'sensor')], now, stopped)).toBe('job finance_daily · scheduler stopped');
		// A paused job stays paused whether or not the scheduler runs.
		expect(jobRefreshText('finance_daily', [trigger('finance_daily', 'paused', '2026-01-01T05:00:00Z')], now, stopped)).toBe('job finance_daily is paused');
		expect(jobRefreshText('finance_daily', [trigger('finance_daily', 'paused', null), trigger('finance_daily', 'running', null, 'sensor')], now)).toBe('job finance_daily · on sensor');
		// A running schedule with no next run, such as a finished calendar, is not a sensor.
		expect(jobRefreshText('finance_daily', [trigger('finance_daily', 'running', null)], now)).toBe('job finance_daily');
		expect(jobRefreshText('crm', [trigger('finance_daily', 'running', '2026-01-01T05:00:00Z')], now)).toBe('job crm');
	});
});


describe('plan from a server summary', () => {
	const summary = plan([model('orders', { reason: 'query_changed' }), model('customers')], {
		pages: { routine_models: 3, unchanged: 120 },
		loaded: { routine_models: 1, unchanged: 1 },
		changed: { counts: ['definition'], reach: 'all', runs: [{ node: 'model:orders', reason: 'definition changed', downstream_of: null }, { node: 'model:customers', reason: 'upstream changed', downstream_of: 'model:orders' }], held_back: [], unchanged: ['model:events'] }
	});
	it.each([
		{ description: 'routine models count the whole section and page from what is loaded', title: 'Models', expected: { size: 3, loaded: 1, page: 'routine_models' } },
		{ description: 'unchanged nodes count the whole section', title: 'Unchanged', expected: { size: 120, loaded: 1, page: 'unchanged' } }
	])('$description', ({ title, expected }) => {
		const section = [...planSections(summary), ...changedSections(summary.changed!, (job) => job, summary.pages)].find((s) => s.title === title)!;
		expect({ size: sectionSize(section), loaded: section.entries.length, page: section.page }).toEqual(expected);
		expect(loadedRows(summary, section.page!)).toBe(expected.loaded);
	});
});

describe('cloneCommands', () => {
	it('returns each origin\'s clone command with every missing name, and nothing for other errors', () => {
		const error = [
			"S301: 7 upstream models this build reads don't exist in target 'dev'",
			'  a',
			'  ... 2 more (sqb plan --json lists all)',
			'',
			'  If they exist in prod:',
			'    read them from it for this run:  --defer-to prod',
			'    copy them into this target:      sqb clone --from prod --to dev -s a b c d e f g',
			'  If it exists in staging:',
			'    copy it into this target:        sqb clone --from staging -s a',
			'  Or build them too:                 add + before your selection'
		].join('\n');
		expect(cloneCommands(error)).toEqual([
			{ origin: 'prod', command: 'sqb clone --from prod --to dev -s a b c d e f g' },
			{ origin: 'staging', command: 'sqb clone --from staging -s a' }
		]);
		expect(cloneCommands('S007: unknown selector sqb clone --from prod')).toEqual([]);
	});
});
