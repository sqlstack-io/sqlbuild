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
	failing: { since: '2026-11-26T03:10:00Z', reason: 'IP is not allowed to access the warehouse', halted: false, unreachable: true, retrySeconds: 60, retryAt: '2026-11-27T03:51:00Z' },
	stalled: false
};
/** The sync completing once the warehouse lets the location in again. */
const recovered: WarehouseSync = {
	lastSyncedAt: '2026-11-27T03:54:44Z',
	lastError: null,
	lastReportedAt: '2026-11-27T03:54:44Z',
	syncing: null,
	completed: { at: '2026-11-27T03:54:44Z', kinds: 14, rows: 2446, elapsedMs: 23_200 },
	failing: null,
	stalled: false
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

	it('shows a running sync with its progress, and as stalled only when the server says so, however old its last report', () => {
		const running: WarehouseSync = { ...recovered, lastReportedAt: '2026-11-27T03:54:00Z', syncing: { position: 7, kinds: 14, table: 'run_results', rows: 1240 } };
		expect(syncNowState(running, null, NOW + 60 * 60_000).kind).toBe('running');
		expect(syncNowState({ ...running, stalled: true }, null, NOW)).toMatchObject({ kind: 'stalled', progressAt: '2026-11-27T03:54:00Z' });
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
	it("names the server's verdict: red when runs stop, amber when it needs a look, Healthy only when it says so", async () => {
		const { locationVerdict } = await import('./labels');
		expect(locationVerdict({ fact: 'not_checking_in', stopsRuns: true })).toEqual({ tone: 'failed', label: 'Not checking in' });
		expect(locationVerdict({ fact: 'cant_connect', stopsRuns: true })).toEqual({ tone: 'failed', label: 'Can’t connect' });
		expect(locationVerdict({ fact: 'deploy_failed', stopsRuns: true })).toEqual({ tone: 'failed', label: 'Deploy failed' });
		expect(locationVerdict({ fact: 'deploy_failed_on_previous', stopsRuns: false })).toEqual({ tone: 'warning', label: 'Running previous version' });
		expect(locationVerdict({ fact: 'sync_failing', stopsRuns: false })).toEqual({ tone: 'warning', label: 'Sync failing' });
		expect(locationVerdict({ fact: 'sync_stalled', stopsRuns: false })).toEqual({ tone: 'warning', label: 'Sync stalled' });
		expect(locationVerdict({ fact: 'not_tested', stopsRuns: false })).toEqual({ tone: 'warning', label: 'Not tested' });
		expect(locationVerdict({ fact: 'starting', stopsRuns: false })).toEqual({ tone: 'warning', label: 'Starting' });
		expect(locationVerdict({ fact: 'healthy', stopsRuns: false })).toEqual({ tone: 'ok', label: 'Healthy' });
	});

	it('says nothing it cannot know when the person may not read the health', async () => {
		const { locationVerdict } = await import('./labels');
		expect(locationVerdict(null)).toEqual({ tone: 'unknown', label: 'Unknown' });
	});
});

describe('stopsRuns', () => {
	it("is the server's word on a location problem, whatever its kind or severity, and never a server-wide one", async () => {
		const { stopsRuns } = await import('./labels');
		expect(stopsRuns({ locationId: 'l1', stopsRuns: true })).toBe(true);
		expect(stopsRuns({ locationId: 'l1', stopsRuns: false })).toBe(false);
		expect(stopsRuns({ locationId: null, stopsRuns: true })).toBe(false);
	});
});
