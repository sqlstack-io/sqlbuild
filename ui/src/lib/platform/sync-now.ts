import type { WarehouseSync } from '$lib/api/types';

/** How long a requested sync counts as pending without a newer report; the request expires on the server then too. */
export const SYNC_REQUEST_PENDING_MS = 5 * 60_000;

/** The sync report the page saw when it asked, compared by value rather than by clock, since browser and server clocks differ. */
export type SyncRequest = { at: number; seen: string };

/** One sync report as the request compares it: any newer report, failed or not, differs. */
export function syncReportKey(sync: WarehouseSync | null): string {
	return `${sync?.lastReportedAt ?? ''}|${sync?.lastSyncedAt ?? ''}|${sync?.lastError ?? ''}`;
}

type Running = NonNullable<WarehouseSync['syncing']>;

/** What the Sync now line shows: waiting for a requested sync, failing, running, stalled, synced, or not synced yet. */
export type SyncNowState =
	| { kind: 'requested'; at: number; by: string | null }
	| { kind: 'failing'; error: string; lastSyncedAt: string | null; failing: WarehouseSync['failing'] }
	| { kind: 'running'; running: Running; progressAt: string }
	| { kind: 'stalled'; running: Running; progressAt: string }
	| { kind: 'synced'; at: string }
	| { kind: 'never' };

/**
 * A request waits until any newer report arrives or it expires; then the newest report says how the sync
 * stands. A running sync has stalled when the server says so.
 */
export function syncNowState(sync: WarehouseSync | null, requested: SyncRequest | null, now: number): SyncNowState {
	if (requested && syncReportKey(sync) === requested.seen && now - requested.at < SYNC_REQUEST_PENDING_MS) return { kind: 'requested', at: requested.at, by: null };
	if (sync?.requested) return { kind: 'requested', at: Date.parse(sync.requested.at), by: sync.requested.by };
	if (sync?.lastError) return { kind: 'failing', error: sync.lastError, lastSyncedAt: sync.lastSyncedAt, failing: sync.failing ?? null };
	if (sync?.syncing && sync.lastReportedAt) {
		return { kind: sync.stalled ? 'stalled' : 'running', running: sync.syncing, progressAt: sync.lastReportedAt };
	}
	if (sync?.lastSyncedAt) return { kind: 'synced', at: sync.lastSyncedAt };
	return { kind: 'never' };
}
