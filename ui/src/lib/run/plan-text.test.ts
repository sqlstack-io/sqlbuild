import { describe, expect, it } from 'vitest';
import type { PlanDocument, PlanModel } from '$lib/api';
import { actionText, changedPolicyLine, changedSections, jobRefreshText, planSections, planSentence } from './plan-text';

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

	it('collapses a plan that is all full refresh into counts by materialization', () => {
		const full = plan([model('a', { reason: 'full_refresh' }), model('b', { reason: 'full_refresh' }), model('v', { reason: 'full_refresh', materialization_type: 'view' })]);
		expect(planSections(full)).toEqual([{ title: 'Full refresh', changed: true, entries: [{ name: 'table', value: '2 models', detail: [] }, { name: 'view', value: '1 model', detail: [] }] }]);
		expect(planSentence(full)).toBe('3 models build from scratch, replacing what was built.');
	});
});

describe('plan sentence', () => {
	const incremental = { reason: 'normal_incremental', materialization_type: 'incremental' } as const;

	it('names a changed model, counts the ones taking new rows, and calls the rest usual', () => {
		const p = plan([model('orders', { reason: 'query_changed' }), model('events', incremental), model('visits', incremental), model('a'), model('b'), model('c', { action: 'skip' })]);
		expect(planSentence(p)).toBe('5 models build: orders changed since its last build, 2 incremental models take new rows and the other 2 are recreated as usual. 1 disabled model is skipped.');
	});

	it('counts many changes, says first builds apart, and says when nothing changed', () => {
		const changed = ['a', 'b', 'c'].map((n) => model(n, { reason: 'config_changed' }));
		expect(planSentence(plan([...changed, model('new', { reason: 'first_run' })]))).toBe('4 models build: 3 models changed since their last build and new builds for the first time.');
		expect(planSentence(plan([model('a'), model('b')], { source_loads: [{ name: 'raw', kind: 'source', is_reload: false }] }))).toBe('2 models build: none changed since their last build. 1 source loads first.');
		expect(planSentence(plan([model('a', { action: 'skip', reason: 'disabled' })]))).toBe('Nothing to build. 1 disabled model is skipped.');
	});

	it('says what runs when no model builds', () => {
		const seeds = [{ name: 'carriers', reason: 'first_run' }, { name: 'regions', reason: 'no_change' }];
		expect(planSentence(plan([], { seeds }))).toBe('No models build. 1 seed loads.');
		const functions = [{ name: 'margin', language: 'sql', reason: 'definition_changed' }, { name: 'tax', language: 'sql', reason: 'no_change' }];
		expect(planSentence(plan([], { functions }))).toBe('No models build. 1 changed function is recreated.');
	});

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

	it('under --changed lists only what runs, then what waits for its job and what is skipped, as sqb plan does', () => {
		const sections = [...planSections(reach), ...changedSections(reach.changed!, (job) => `job ${job}`)];
		expect(sections.map((s) => [s.title, s.entries.map((e) => `${e.name}: ${e.value}${e.detail.length ? ` (${e.detail.join(', ')})` : ''}`)])).toEqual([
			['Query changed', ['stg_orders: continue forward']],
			['Models', ['customer_orders: table (reads stg_orders)', 'int_orders: table (reads stg_orders)', 'orders_export: table (reads stg_orders)']],
			['Held back', ['fct_revenue: job finance_daily', 'rpt_pnl: job finance_daily', 'summary: reads fct_revenue, which is held back']],
			['Unchanged', ['stg_customers: skipped by --changed']]
		]);
		expect(changedPolicyLine(reach.changed!)).toBe('Counting: definition (source data not counted) · reach: job');
		expect(changedPolicyLine({ ...reach.changed!, counts: ['definition', 'source_data'], reach: 'all' })).toBe('Counting: definition, source data · reach: all');
		expect(planSentence(reach)).toBe(
			'4 models build: stg_orders changed since its last build and the other 3 models read a change upstream. 3 models wait for their job\'s own run. 1 unchanged node is skipped by --changed.'
		);
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
		expect(planSentence(first)).toBe('2 models build: orders and order_count run for their own reason.');
		expect(planSections(first).find((s) => s.title === 'Models')?.entries.map((e) => e.detail.at(-1))).toEqual(['no completed record', 'no completed record']);
	});

	it("says what refreshes a held-back node: the named job's next run, its sensor, or that it is paused", () => {
		const now = Date.parse('2026-01-01T00:00:00Z');
		type Kind = 'schedule' | 'sensor';
		const trigger = (jobKey: string, state: 'running' | 'paused', nextDueAt: string | null, type: Kind = 'schedule') => ({ jobKey, type, state, nextDueAt });
		// A group's schedule says nothing about a job whose own trigger is paused; the plan names the group itself when it matters.
		expect(jobRefreshText('finance_daily', [trigger('finance_daily', 'paused', '2026-01-01T05:00:00Z'), trigger('nightly', 'running', '2026-01-01T02:00:00Z')], now)).toBe('job finance_daily is paused');
		expect(jobRefreshText('nightly', [trigger('nightly', 'running', '2026-01-01T02:00:00Z'), trigger('nightly', 'running', '2026-01-01T05:00:00Z')], now)).toBe('job nightly · next run in 2h');
		expect(jobRefreshText('finance_daily', [trigger('finance_daily', 'paused', null), trigger('finance_daily', 'running', null, 'sensor')], now)).toBe('job finance_daily · when its sensor fires');
		// A running schedule with no next run, such as a finished calendar, is not a sensor.
		expect(jobRefreshText('finance_daily', [trigger('finance_daily', 'running', null)], now)).toBe('job finance_daily');
		expect(jobRefreshText('crm', [trigger('finance_daily', 'running', '2026-01-01T05:00:00Z')], now)).toBe('job crm');
	});
});

