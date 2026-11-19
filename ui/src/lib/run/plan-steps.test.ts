import { describe, expect, it } from 'vitest';
import type { PlanModel } from '$lib/api';
import { planSummary, planPage } from '$lib/api/mock/large-plan';
import { summaryPlanDocument } from '$lib/api/mock/plan-summary-mockup';
import { columnStep, migrationStep, migrationSteps, renamedRow, retentionStep, stepRows, stepRowsOf, tableTypeStep } from './plan-steps';

const model = (name: string, extra: Partial<PlanModel> = {}): PlanModel => ({ name, materialization_type: 'table', action: 'create_table', reason: 'renamed', backfill: { action: 'forward', duration: null }, ...extra });

describe('renamedRow', () => {
	it('names the old name and what stays there, from the row alone', () => {
		const row = renamedRow(model('orders', { origin_model: 'orders_v1', query_changed: true, old_name: { action: 'archive_and_view', expires_at: '2026-12-18T00:00:00Z' } }));
		expect(row).toEqual({ from: 'orders_v1', to: 'orders', detail: ['query changed too', 'old table archived; a view at the old name until 2026-12-18'] });
	});

	it('says only its names when nothing stays at the old name; the usual rebuild goes unsaid', () => {
		expect(renamedRow(model('orders', { origin_model: 'orders_v1', old_name: null }))).toEqual({ from: 'orders_v1', to: 'orders', detail: [] });
	});
});

describe('migration steps', () => {
	it('say what each step does to the node it acts on', () => {
		expect(migrationStep({ model: 'totals', origin_model: 'totals_legacy', decision: 'forced_replace', completed_at: null, is_rename: false })).toMatchObject({ node: 'totals', what: 'replaced by the data of totals_legacy (forced)' });
		// Only an unusual outcome is said; the heading says the rest.
		expect(columnStep({ model: 'orders', origin_column: 'amt', destination_column: 'amount', decision: 'rename' })).toEqual({ node: 'orders', what: 'amt → amount', detail: [], warning: null });
		expect(columnStep({ model: 'orders', origin_column: 'amt', destination_column: 'amount', decision: 'record' }).detail).toEqual(['already renamed; recorded']);
		expect(tableTypeStep({ model: 'events', actual_type: 'permanent', desired_type: 'transient', downgrade: true, irreversible_warning: 'History is lost.' })).toEqual({ node: 'events', what: 'permanent → transient', detail: [], warning: 'History is lost.' });
	});

	it('name a retention change by its models, or its schema', () => {
		expect(retentionStep({ scope: 'table', name: 'analytics.orders', models: ['orders'], actual_days: 1, desired_days: 14, direction: 'increase' })).toMatchObject({ node: 'orders', what: 'raised: 1 day → 14 days' });
		expect(retentionStep({ scope: 'schema', name: 'archive', models: ['a', 'b'], actual_days: 7, desired_days: 30, direction: 'increase' })).toMatchObject({ node: 'schema archive', detail: ['for 2 models'] });
	});

	it('read the mockup’s steps by kind, a rename’s own migration not among them', () => {
		const doc = summaryPlanDocument(true);
		const summary = planSummary(doc);
		expect(migrationSteps(summary)).toBe(63);
		expect(migrationSteps(doc)).toBe(63);
		const page = planPage(doc, 'renamed', 0, 1);
		expect((page.rows[0] as PlanModel).origin_model).toMatch(/_old$/);
		expect(stepRows({ ...summary, migrations: doc.migrations }, 'migrations')).toHaveLength(2);
		expect(stepRowsOf('migrations', planPage(doc, 'migrations', 0, 100).rows)).toHaveLength(2);
		expect(stepRows({ ...summary, retention: doc.retention }, 'retention')).toHaveLength(8);
	});
});
