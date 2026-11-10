import { describe, expect, it } from 'vitest';
import type { PlanDocument, PlanModel } from '$lib/api';
import { actionText, planSections, readySummary } from './plan-text';

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

	it('collapses a plan that is all full refresh into counts by materialization', () => {
		const full = plan([model('a', { reason: 'full_refresh' }), model('b', { reason: 'full_refresh' }), model('v', { reason: 'full_refresh', materialization_type: 'view' })]);
		expect(planSections(full)).toEqual([{ title: 'Full refresh', changed: true, entries: [{ name: '2', value: 'table', detail: [] }, { name: '1', value: 'view', detail: [] }] }]);
		expect(readySummary(full)).toBe('full refresh, 3 selected');
	});
});
