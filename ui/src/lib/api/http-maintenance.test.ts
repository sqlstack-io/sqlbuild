import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { formatRelation } from '$lib/platform/labels';
import { oldNameStatus } from '$lib/platform/maintenance';
import { toMaintenanceHistory } from './http-maintenance';

type W = components['schemas'];

function migration(eventId: string, originSchema: string | null, createdAt: string): W['WarehouseMigrationRecord'] {
	return {
		state_schema: 'sqlbuild_state',
		event_id: eventId,
		target_name: 'development',
		origin_model: null,
		origin_schema: originSchema,
		origin_name: `${eventId}_old`,
		destination_model: `model:${eventId}`,
		destination_schema: 'analytics',
		destination_name: eventId,
		origin_version_hash: 'v1',
		discovery: 'manual',
		decision: 'renamed',
		run_id: 'run-1',
		created_at: createdAt
	};
}

function step(migrationEventId: string, eventType: string, createdAt: string, facts: Partial<W['MaintenanceOldNameView']> = {}): W['MaintenanceOldNameView'] {
	return {
		state_schema: 'sqlbuild_state',
		event_id: `${migrationEventId}-${eventType}`,
		event_type: eventType,
		migration_event_id: migrationEventId,
		destination_model: `model:${migrationEventId}`,
		old_schema: 'analytics',
		old_name: `${migrationEventId}_old`,
		new_schema: 'analytics',
		new_name: migrationEventId,
		run_id: `run-${eventType}`,
		created_at: createdAt,
		...facts
	};
}

describe('maintenance history mapping', () => {
	// Moves arrive newest first; old-name steps arrive oldest first, interleaved across moves.
	const history = toMaintenanceHistory({
		migrations: [migration('customers', 'analytics', '2026-11-08T10:00:00Z'), migration('orders', null, '2026-11-07T10:00:00Z')],
		old_name_views: [
			step('orders', 'required', '2026-11-07T10:00:00Z', { view_retention: '30d' }),
			step('customers', 'required', '2026-11-08T10:00:00Z', { view_retention: 'false' }),
			step('orders', 'view_created', '2026-11-07T10:01:00Z', { expires_at: '2026-12-07T10:01:00Z', column_aliases: '{"amount":"total","id":"order_id"}', grants_copied: 3 }),
			step('orders', 'view_dropped', '2026-11-08T11:00:00Z', { drop_reason: 'Removed by hand' })
		],
		janitor_events: []
	});

	it('keeps one view per move, in the order of the moves, with its steps oldest first', () => {
		expect(history.oldNameViews.map((v) => [v.migrationEventId, v.steps.map((s) => s.eventType)])).toEqual([
			['customers', ['required']],
			['orders', ['required', 'view_created', 'view_dropped']]
		]);
	});

	it('takes each fact from the step that recorded it', () => {
		const orders = history.oldNameViews[1];
		expect([orders.retention, orders.expiresAt, orders.dropReason, orders.grantsCopied]).toEqual(['30d', '2026-12-07T10:01:00Z', 'Removed by hand', 3]);
		expect(orders.columnAliases).toEqual([
			{ old: 'amount', new: 'total' },
			{ old: 'id', new: 'order_id' }
		]);
		expect(oldNameStatus(orders, Date.parse('2026-11-09T00:00:00Z'))).toBe('dropped');
		expect(oldNameStatus(history.oldNameViews[0], Date.parse('2026-11-09T00:00:00Z'))).toBe('not_kept');
	});

	it('shows a relation recorded without its schema by its bare name', () => {
		expect(history.migrations.map((m) => formatRelation(m.origin))).toEqual(['analytics.customers_old', 'orders_old']);
	});
});
