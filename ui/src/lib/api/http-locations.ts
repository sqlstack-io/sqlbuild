import type { components } from '$control/schema.d.ts';
import type { Location, LocationHealth, Snapshot, WarehouseSync } from './types';
import type { ConnectionDetails, ConnectionTest, LocationDetail, LocationInstance, LocationSummary, WarehouseConnection, WarehouseTableSync } from './types/platform';

type Wire = components['schemas'];

/** Locations report every 30 seconds; the server treats two missed reports as down. */
const LIVE_WINDOW_MS = 90_000;

function latestOf<K extends Wire['LocationHealth']['kind']>(observations: Wire['LocationHealthObservation'][], kind: K) {
	return observations.find((o) => o.report.health.kind === kind) as (Wire['LocationHealthObservation'] & { report: { health: Extract<Wire['LocationHealth'], { kind: K }> } }) | undefined;
}

/** Health from the newest observations: down without a recent check-in, then load failures, then missing provider secrets, then connection failures. */
export function healthOf(observations: Wire['LocationHealthObservation'][], now: number): { health: LocationHealth; liveInstances: number; lastSeenAt: string | null } {
	const live = new Set(observations.filter((o) => now - Date.parse(o.received_at) <= LIVE_WINDOW_MS).map((o) => o.report.session_id));
	const lastSeenAt = observations.reduce<string | null>((latest, o) => (!latest || o.received_at > latest ? o.received_at : latest), null);
	const failed = latestOf(observations, 'compile_failed');
	const connection = latestOf(observations, 'connection');
	const secrets = latestOf(observations, 'provider_secrets');
	const health: LocationHealth = !live.size
		? 'broken'
		: failed && (!connection || failed.report.health.generation >= connection.report.health.generation)
			? 'falling_back'
			: secrets && secrets.report.health.missing > 0
				? 'missing_secrets'
				: connection && !connection.report.health.healthy
					? 'degraded'
					: 'healthy';
	return { health, liveInstances: live.size, lastSeenAt };
}

/**
 * The newest connection check a location ran on its own: on starting, or after publishing. Older
 * locations report neither why nor how long, so their checks count as after publishing, with no duration.
 */
export function connectionTestOf(observations: Wire['LocationHealthObservation'][]): ConnectionTest | null {
	const latest = latestOf(observations, 'connection');
	if (!latest) return null;
	const { healthy: ok, trigger, elapsed_millis } = latest.report.health;
	const when = trigger === 'startup' ? 'on startup' : 'after publishing';
	return {
		outcome: ok ? 'ok' : 'failed',
		at: latest.received_at,
		durationMs: elapsed_millis ?? 0,
		trigger: trigger ?? 'publish',
		by: null,
		message: ok ? `Connected ${when}` : `Could not connect ${when}`
	};
}

/**
 * A finished manual connection test, or null while the location hasn't answered. The location
 * reports only whether it connected; `sqb debug` in the location shows why it couldn't.
 */
export function manualConnectionTest(record: Wire['LocationWorkRecord'] | null): ConnectionTest | null {
	const outcome = record?.result_json?.outcome;
	if (!record?.completed_at || !outcome) return null;
	if (outcome.kind !== 'connection') throw new Error('A newer request replaced this connection test; test again');
	return {
		outcome: outcome.healthy ? 'ok' : 'failed',
		at: record.completed_at,
		durationMs: Date.parse(record.completed_at) - Date.parse(record.requested_at),
		trigger: 'manual',
		by: null,
		message: outcome.healthy ? 'Connected to the warehouse' : 'Could not connect to the warehouse; run sqb debug in the location to see why'
	};
}

export function snapshotOf(published: Wire['PublishedSnapshot'] | null): Snapshot | null {
	return published
		? {
				id: published.snapshot_id,
				generation: published.generation,
				gitRevision: published.git_revision ?? '',
				gitDirty: published.git_dirty,
				gitAuthor: published.git_author ?? '',
				publishedAt: published.published_at,
				sqbVersion: published.sqb_version
			}
		: null;
}

/** `observations` is null when the person may not list the whole target; `activeProblems` counts the problems read's events. */
export function toLocationSummary(
	location: Location,
	observations: Wire['LocationHealthObservation'][] | null,
	published: Wire['PublishedSnapshot'] | null,
	activeProblems: number,
	now: number
): LocationSummary {
	const known = observations ? healthOf(observations, now) : { health: null, liveInstances: 0, lastSeenAt: null };
	return {
		id: location.id,
		name: location.name,
		target: location.target,
		...known,
		brokenDeployPolicy: null,
		failedDeploy: null,
		connectionTest: observations ? connectionTestOf(observations) : null,
		activeProblems,
		snapshot: snapshotOf(published)
	};
}

/** One process per reporting session, newest check-in first; the server keeps no image, commit or start time for them. */
export function instancesOf(observations: Wire['LocationHealthObservation'][], now: number): LocationInstance[] {
	const lastSeen = new Map<string, string>();
	for (const o of observations) {
		const seen = lastSeen.get(o.report.session_id);
		if (!seen || o.received_at > seen) lastSeen.set(o.report.session_id, o.received_at);
	}
	return [...lastSeen]
		.sort(([, a], [, b]) => b.localeCompare(a))
		.map(([id, lastSeenAt]) => ({ id, imageId: null, gitRevision: null, startedAt: null, lastSeenAt, state: now - Date.parse(lastSeenAt) <= LIVE_WINDOW_MS ? 'live' : 'stale' }));
}

/** A location's page from its health observations and newest snapshot; locations never send compile error text, so a failed reload shows only as its health. */
export function toLocationDetail(location: Location, observations: Wire['LocationHealthObservation'][], published: Wire['PublishedSnapshot'] | null, now: number): LocationDetail {
	const { health } = healthOf(observations, now);
	return {
		id: location.id,
		name: location.name,
		health,
		createdAt: null,
		brokenDeployPolicy: null,
		failedDeploy: null,
		lastLoadError: null,
		instances: instancesOf(observations, now),
		target: location.target,
		snapshot: snapshotOf(published),
		warehouseConnection: null,
		connectionTest: connectionTestOf(observations)
	};
}

const SYNC_STAGE: Record<Exclude<Wire['WarehouseSyncFailure'], 'busy'>, string> = {
	watermark: 'reading where the last sync stopped',
	read: 'reading the warehouse',
	delivery: 'sending results to the server'
};

/** What held up a table's newest sync; a build holding the warehouse file is a wait, not a failure. */
const syncProblemOf = (failure: Wire['WarehouseSyncFailure']) => (failure === 'busy' ? 'Waiting for a build that holds the warehouse file' : `Failed while ${SYNC_STAGE[failure]}`);

type Sync = Wire['LocationHealthObservation'] & { report: { health: Extract<Wire['LocationHealth'], { kind: 'warehouse_sync' }> } };

/** Sync reports newest first, and the newest of each table. */
function syncsOf(observations: Wire['LocationHealthObservation'][]): { syncs: Sync[]; newest: Map<string, Sync> } {
	const syncs = observations.filter((o): o is Sync => o.report.health.kind === 'warehouse_sync').sort((a, b) => b.received_at.localeCompare(a.received_at));
	const newest = new Map<string, Sync>();
	for (const sync of syncs) if (!newest.has(sync.report.health.table)) newest.set(sync.report.health.table, sync);
	return { syncs, newest };
}

/** The non-secret connection settings the location published with its snapshot; null when it published none. */
export function connectionDetailsOf(published: Wire['PublishedSnapshot'] | null): ConnectionDetails | null {
	const c = published?.connection;
	return c
		? { adapter: c.adapter, account: c.account ?? null, host: c.host ?? null, user: c.user ?? null, role: c.role ?? null, warehouse: c.warehouse ?? null, database: c.database ?? null, authMethod: c.auth_method ?? null }
		: null;
}

/**
 * A location target's connection from its health reports, newest snapshot and mirrored totals: the
 * settings it published, the newest connection check, where its state tables are read from, each
 * table's newest sync and how many records the server mirrors of it, and when the next sync is due.
 * `totals` is null when the server doesn't report them.
 */
export function toWarehouseConnection(
	location: Location,
	observations: Wire['LocationHealthObservation'][],
	published: Wire['PublishedSnapshot'] | null = null,
	totals: Wire['MirroredTotal'][] | null = null
): WarehouseConnection {
	const { syncs, newest } = syncsOf(observations);
	const tables: WarehouseTableSync[] = [...newest.values()]
		.map(({ received_at, report: { health } }) => ({
			table: health.table,
			at: received_at,
			recordsSent: health.records_sent,
			elapsedMs: health.elapsed_millis,
			failure: health.failure ? syncProblemOf(health.failure) : null,
			waiting: health.failure === 'busy',
			lastSyncedAt: syncs.find((s) => s.report.health.table === health.table && !s.report.health.failure)?.received_at ?? null,
			mirroredTotal: totals ? (totals.find((t) => t.table === health.table)?.records ?? 0) : null
		}))
		.sort((a, b) => a.table.localeCompare(b.table));
	const state = syncs[0]?.report.health.location;
	return {
		id: location.id,
		location: location.name,
		target: location.target.name,
		details: connectionDetailsOf(published),
		stateLocation: state ? { database: state.database ?? null, schema: state.schema } : null,
		lastTest: connectionTestOf(observations),
		// The report that completes a sync names the next safety sync; one still running names none.
		nextSyncAt: syncs[0]?.report.health.next_sync_at ?? null,
		tables
	};
}

/** When a table last synced cleanly, and which tables' newest sync failed; null without any sync report. */
export function warehouseSyncOf(observations: Wire['LocationHealthObservation'][]): WarehouseSync | null {
	const { syncs, newest } = syncsOf(observations);
	if (!syncs.length) return null;
	const failing = [...newest.values()].flatMap(({ report: { health } }) => (health.failure && health.failure !== 'busy' ? [`${health.table}: failed while ${SYNC_STAGE[health.failure]}`] : []));
	return {
		lastSyncedAt: syncs.find((s) => !s.report.health.failure)?.received_at ?? null,
		lastError: failing.length ? failing.sort().join('\n') : null,
		lastReportedAt: syncs[0].received_at
	};
}
