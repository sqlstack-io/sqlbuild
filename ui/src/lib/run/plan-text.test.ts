import { describe, expect, it } from 'vitest';
import type { PlanDocument, PlanModel } from '$lib/api';
import { actionText, planSections, planSentence } from './plan-text';

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
		expect(planSentence(p)).toBe('5 models build: orders changed since its last build, 2 incremental models take new rows and the other 2 are recreated as usual. 1 other model is up to date.');
	});

	it('counts many changes, says first builds apart, and says when nothing changed', () => {
		const changed = ['a', 'b', 'c'].map((n) => model(n, { reason: 'config_changed' }));
		expect(planSentence(plan([...changed, model('new', { reason: 'first_run' })]))).toBe('4 models build: 3 models changed since their last build and new builds for the first time.');
		expect(planSentence(plan([model('a'), model('b')], { source_loads: [{ name: 'raw', kind: 'source', is_reload: false }] }))).toBe('2 models build: none changed since their last build. 1 source loads first.');
		expect(planSentence(plan([model('a', { action: 'skip' })]))).toBe('Nothing to build: 1 model is up to date.');
	});
});
