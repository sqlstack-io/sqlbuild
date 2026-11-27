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
