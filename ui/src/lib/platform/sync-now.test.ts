import { describe, expect, it } from 'vitest';
import type { WarehouseSync } from '$lib/api/types';
import { SYNC_REQUEST_PENDING_MS, syncNowState, syncReportKey } from './sync-now';

const NOW = Date.parse('2026-11-27T03:55:00Z');
/** A day of failed reads: the location can't reach the warehouse. */
const failing: WarehouseSync = {
	lastSyncedAt: '2026-11-26T03:00:00Z',
	lastError: 'catalog: failed while reading: IP is not allowed to access the warehouse',
	lastReportedAt: '2026-11-27T03:50:00Z',
	syncing: null,
	completed: null,
	failing: { since: '2026-11-26T03:10:00Z', reason: 'IP is not allowed to access the warehouse', halted: false, unreachable: true, retrySeconds: 60, retryAt: '2026-11-27T03:51:00Z' }
};
/** The sync completing once the warehouse lets the location in again. */
const recovered: WarehouseSync = {
	lastSyncedAt: '2026-11-27T03:54:44Z',
	lastError: null,
	lastReportedAt: '2026-11-27T03:54:44Z',
	syncing: null,
	completed: { at: '2026-11-27T03:54:44Z', kinds: 14, rows: 2446, elapsedMs: 23_200 },
	failing: null
};

describe('syncNowState', () => {
	it('goes from failing to waiting on Sync now, and to synced once the successful report arrives', () => {
		expect(syncNowState(failing, null, NOW)).toEqual({ kind: 'failing', error: failing.lastError, lastSyncedAt: '2026-11-26T03:00:00Z', failing: failing.failing });
		const requested = { at: NOW, seen: syncReportKey(failing) };
		expect(syncNowState(failing, requested, NOW + 30_000)).toEqual({ kind: 'requested', at: NOW, by: null });
		expect(syncNowState(recovered, requested, NOW + 31_000)).toEqual({ kind: 'synced', at: '2026-11-27T03:54:44Z' });
	});

	it('stops waiting when the request expires without a newer report, showing the report it has', () => {
		const requested = { at: NOW, seen: syncReportKey(failing) };
		expect(syncNowState(failing, requested, NOW + SYNC_REQUEST_PENDING_MS).kind).toBe('failing');
	});

	it('ends the wait on a newer failed report too', () => {
		const requested = { at: NOW, seen: syncReportKey(failing) };
		expect(syncNowState({ ...failing, lastReportedAt: '2026-11-27T03:56:00Z' }, requested, NOW + 60_000).kind).toBe('failing');
	});

	it('shows a running sync with its progress, and as stalled once no progress arrives for ten minutes', () => {
		const running: WarehouseSync = { ...recovered, lastReportedAt: '2026-11-27T03:54:00Z', syncing: { position: 7, kinds: 14, table: 'run_results', rows: 1240 } };
		expect(syncNowState(running, null, NOW).kind).toBe('running');
		expect(syncNowState(running, null, Date.parse('2026-11-27T04:04:00Z'))).toMatchObject({ kind: 'stalled', progressAt: '2026-11-27T03:54:00Z' });
	});

	it('waits on a request the server reports, even one made on another page', () => {
		expect(syncNowState({ ...recovered, requested: { at: '2026-11-27T03:54:50Z', by: 'ada' } }, null, NOW)).toEqual({ kind: 'requested', at: Date.parse('2026-11-27T03:54:50Z'), by: 'ada' });
	});
});

describe('messageParts', () => {
	it('keeps the error out of the summary, after the first clause of three words or more', async () => {
		const { messageParts } = await import('./labels');
		expect(messageParts("Run history can't reach the warehouse: 390422 (08001): IP 203.0.113.7 is not allowed")).toEqual({
			summary: "Run history can't reach the warehouse",
			error: '390422 (08001): IP 203.0.113.7 is not allowed'
		});
		expect(messageParts('orders · prod: the warehouse connection check failed: IP is not allowed.')).toEqual({
			summary: 'orders · prod: the warehouse connection check failed',
			error: 'IP is not allowed.'
		});
		expect(messageParts('stock_levels in inventory · development is late.')).toEqual({ summary: 'stock_levels in inventory · development is late.', error: null });
	});
});

describe('locationVerdict', () => {
	it('names the worst open fact: red when runs stop, amber when it needs a look, Healthy only when nothing is open', async () => {
		const { locationVerdict } = await import('./labels');
		const ok = { outcome: 'ok' as const, at: '2026-11-27T03:00:00Z', durationMs: 800, trigger: 'manual' as const, by: null, message: 'Connected' };
		const base = { health: 'healthy' as const, live: true, test: ok, deployedAt: '2026-11-27T02:00:00Z', failedDeploy: false, fallback: false, events: [] };
		const event = (kind: 'warehouse_sync_failed' | 'tick_overdue' | 'location_broken', severity: 'warning' | 'critical' = 'warning') => ({ kind, severity, locationId: 'l1', active: true });
		expect(locationVerdict(base)).toEqual({ tone: 'ok', label: 'Healthy' });
		expect(locationVerdict({ ...base, events: [event('tick_overdue')] })).toEqual({ tone: 'warning', label: 'Schedule overdue' });
		expect(locationVerdict({ ...base, events: [event('tick_overdue'), event('warehouse_sync_failed')] })).toEqual({ tone: 'warning', label: 'Sync failing' });
		expect(locationVerdict({ ...base, test: { ...ok, outcome: 'failed' }, events: [event('warehouse_sync_failed')] })).toEqual({ tone: 'failed', label: 'Can’t connect' });
		expect(locationVerdict({ ...base, live: false })).toEqual({ tone: 'failed', label: 'Not checking in' });
		expect(locationVerdict({ ...base, failedDeploy: true, fallback: true })).toEqual({ tone: 'warning', label: 'Deploy failed · running previous' });
		expect(locationVerdict({ ...base, failedDeploy: true, fallback: false })).toEqual({ tone: 'failed', label: 'Deploy failed' });
		expect(locationVerdict({ ...base, test: null })).toEqual({ tone: 'warning', label: 'Not tested' });
	});
});
