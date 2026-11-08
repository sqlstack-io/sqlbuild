import type { components } from '$control/schema.d.ts';
import type { Location, LocationHealth, WarehouseSync } from './types';
import type { ConnectionTest, LocationSummary } from './types/platform';

type Wire = components['schemas'];

/** Locations report every 30 seconds; the server treats two missed reports as down. */
const LIVE_WINDOW_MS = 90_000;

function latestOf<K extends Wire['LocationHealth']['kind']>(observations: Wire['LocationHealthObservation'][], kind: K) {
	return observations.find((o) => o.report.health.kind === kind) as (Wire['LocationHealthObservation'] & { report: { health: Extract<Wire['LocationHealth'], { kind: K }> } }) | undefined;
}

/** Health from the newest observations: down without a recent check-in, then load failures, then connection failures. */
export function healthOf(observations: Wire['LocationHealthObservation'][], now: number): { health: LocationHealth; liveInstances: number; lastSeenAt: string | null } {
	const live = new Set(observations.filter((o) => now - Date.parse(o.received_at) <= LIVE_WINDOW_MS).map((o) => o.report.session_id));
	const lastSeenAt = observations.reduce<string | null>((latest, o) => (!latest || o.received_at > latest ? o.received_at : latest), null);
	const failed = latestOf(observations, 'compile_failed');
	const connection = latestOf(observations, 'connection');
	const health: LocationHealth = !live.size
		? 'broken'
		: failed && (!connection || failed.report.health.generation >= connection.report.health.generation)
			? 'falling_back'
			: connection && !connection.report.health.healthy
				? 'degraded'
				: 'healthy';
	return { health, liveInstances: live.size, lastSeenAt };
}

/** The connection check a location ran after publishing; the server keeps no message or duration. */
export function connectionTestOf(observations: Wire['LocationHealthObservation'][]): ConnectionTest | null {
	const latest = latestOf(observations, 'connection');
	if (!latest) return null;
	const ok = latest.report.health.healthy;
	return { outcome: ok ? 'ok' : 'failed', at: latest.received_at, durationMs: 0, trigger: 'publish', by: null, message: ok ? 'Connected after publishing' : 'Could not connect after publishing' };
}

/** `observations` is null when the person may not list the whole target. */
export function toLocationSummary(location: Location, observations: Wire['LocationHealthObservation'][] | null, published: Wire['PublishedSnapshot'] | null, now: number): LocationSummary {
	const known = observations ? healthOf(observations, now) : { health: null, liveInstances: 0, lastSeenAt: null };
	return {
		id: location.id,
		name: location.name,
		target: location.target,
		...known,
		brokenDeployPolicy: null,
		failedDeploy: null,
		connectionTest: observations ? connectionTestOf(observations) : null,
		activeProblems: 0,
		snapshot: published
			? {
					id: published.snapshot_id,
					generation: published.generation,
					gitRevision: published.git_revision ?? '',
					gitDirty: published.git_dirty,
					gitAuthor: published.git_author ?? '',
					publishedAt: published.published_at,
					sqbVersion: published.sqb_version
				}
			: null
	};
}

const SYNC_STAGE: Record<Wire['WarehouseSyncFailure'], string> = {
	watermark: 'reading where the last sync stopped',
	read: 'reading the warehouse',
	delivery: 'sending results to the server'
};

/** When a table last synced cleanly, and which tables' newest sync failed; null without any sync report. */
export function warehouseSyncOf(observations: Wire['LocationHealthObservation'][]): WarehouseSync | null {
	type Sync = Wire['LocationHealthObservation'] & { report: { health: Extract<Wire['LocationHealth'], { kind: 'warehouse_sync' }> } };
	const syncs = observations.filter((o): o is Sync => o.report.health.kind === 'warehouse_sync').sort((a, b) => b.received_at.localeCompare(a.received_at));
	if (!syncs.length) return null;
	const newest = new Map<string, Sync>();
	for (const sync of syncs) if (!newest.has(sync.report.health.table)) newest.set(sync.report.health.table, sync);
	const failing = [...newest.values()].flatMap((s) => (s.report.health.failure ? [`${s.report.health.table}: failed while ${SYNC_STAGE[s.report.health.failure]}`] : []));
	return {
		lastSyncedAt: syncs.find((s) => !s.report.health.failure)?.received_at ?? null,
		lastError: failing.length ? failing.sort().join('\n') : null
	};
}
