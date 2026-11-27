import { shortName } from '$lib/build-area/labels';
import type { components } from '$control/schema.d.ts';
import type { Location, LocationHealth, PlatformEvent, Snapshot, WarehouseSync } from './types';
import type { ConnectionDetails, ConnectionTest, LocationDetail, LocationVerdictFact, MissingProviderSecrets, PoolCapWarning, LocationInstance, LocationSummary, WarehouseConnection, WarehouseTableSync } from './types/platform';

type Wire = components['schemas'];

/** Locations report every 30 seconds; the server treats two missed reports as down. */
const LIVE_WINDOW_MS = 90_000;

function latestOf<K extends Wire['LocationHealth']['kind']>(observations: Wire['LocationHealthObservation'][], kind: K) {
	return observations.find((o) => o.report.health.kind === kind) as (Wire['LocationHealthObservation'] & { report: { health: Extract<Wire['LocationHealth'], { kind: K }> } }) | undefined;
}

/**
 * How many instances checked in within the live window, and the newest check-in. The server's own
 * pool-cap warning and failed-plan record are not check-ins. The target's health is the server's
 * `state`; it is never worked out here.
 */
export function checkInsOf(observations: Wire['LocationHealthObservation'][], now: number): { liveInstances: number; lastSeenAt: string | null } {
	const reported = observations.filter((o) => o.report.health.kind !== 'pool_capped' && o.report.health.kind !== 'change_plan_failed');
	const live = new Set(reported.filter((o) => now - Date.parse(o.received_at) <= LIVE_WINDOW_MS).map((o) => o.report.session_id));
	const lastSeenAt = reported.reduce<string | null>((latest, o) => (!latest || o.received_at > latest ? o.received_at : latest), null);
	return { liveInstances: live.size, lastSeenAt };
}

/** The providers the newest provider check found missing secrets, problems before those the target allows. */
export function missingSecretsOf(observations: Wire['LocationHealthObservation'][]): MissingProviderSecrets[] {
	const latest = latestOf(observations, 'provider_secrets');
	const providers = (latest?.report.health.providers ?? []).map((p) => ({ ...p, settings: p.settings.map((s) => ({ ...s, line: s.line ?? null })) }));
	return [...providers.filter((p) => !p.allowed), ...providers.filter((p) => p.allowed)];
}

/** How many nodes the missing-secrets problem names before "and N more". */
const SECRET_NODES_SHOWN = 3;

/** "a", "a and b", "a, b and c". */
const andList = (parts: string[]) => (parts.length < 2 ? parts.join('') : `${parts.slice(0, -1).join(', ')} and ${parts.at(-1)}`);

/**
 * The Overview's problem for a location whose providers miss secrets its target doesn't allow, or null.
 * Its members are the unset variables, so a newly missing one makes it worse.
 */
export function missingSecretsEventOf(locationKey: string, observations: Wire['LocationHealthObservation'][]): PlatformEvent | null {
	const latest = latestOf(observations, 'provider_secrets');
	const problems = missingSecretsOf(observations).filter((p) => !p.allowed);
	if (!latest || !problems.length) return null;
	const variables = problems.flatMap((p) => p.settings.map((s) => s.variable));
	const nodes = [...new Set(problems.flatMap((p) => p.nodes))].map(shortName);
	const it = problems.length === 1 ? 'it' : 'them';
	const stopped = !nodes.length
		? `runs that use ${it}`
		: nodes.length <= SECRET_NODES_SHOWN
			? `runs of ${andList(nodes)}`
			: `runs of ${nodes.slice(0, SECRET_NODES_SHOWN).join(', ')} and ${nodes.length - SECRET_NODES_SHOWN} more`;
	const providers = problems.map((p) => p.provider).join(', ');
	return {
		id: `secrets:${locationKey}`,
		kind: 'location_missing_secrets',
		severity: 'warning',
		occurredAt: latest.received_at,
		members: [...new Set(variables)].sort(),
		active: true,
		locationId: locationKey,
		subject: null,
		message: `${providers} ${problems.length === 1 ? 'is' : 'are'} missing ${variables.join(', ')} in the location's environment, so ${stopped} stop before the warehouse.`
	};
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

/** `read` is null when the person may not list the whole target; `activeProblems` counts the problems read's events. */
export function toLocationSummary(
	location: Location,
	read: Wire['PersonLocationHealth'] | null,
	published: Wire['PublishedSnapshot'] | null,
	activeProblems: number,
	now: number
): LocationSummary {
	const observations = read?.observations ?? null;
	const known = read ? { health: read.state, ...checkInsOf(read.observations, now) } : { health: null, liveInstances: 0, lastSeenAt: null };
	return {
		id: location.id,
		name: location.name,
		target: location.target,
		...known,
		brokenDeployPolicy: null,
		failedDeploy: null,
		connectionTest: observations ? connectionTestOf(observations) : null,
		verdict: read ? verdictOf(read.verdict) : null,
		activeProblems,
		snapshot: snapshotOf(published)
	};
}

/** One process per reporting session, newest check-in first; the server keeps no image, commit or start time for them, and its own pool-cap warning is no process. */
export function instancesOf(observations: Wire['LocationHealthObservation'][], now: number): LocationInstance[] {
	const lastSeen = new Map<string, string>();
	for (const o of observations) {
		if (o.report.health.kind === 'pool_capped') continue;
		const seen = lastSeen.get(o.report.session_id);
		if (!seen || o.received_at > seen) lastSeen.set(o.report.session_id, o.received_at);
	}
	return [...lastSeen]
		.sort(([, a], [, b]) => b.localeCompare(a))
		.map(([id, lastSeenAt]) => ({ id, imageId: null, gitRevision: null, startedAt: null, lastSeenAt, state: now - Date.parse(lastSeenAt) <= LIVE_WINDOW_MS ? 'live' : 'stale' }));
}

/** The server's newest pool-cap warning: each job or group asking a pool for more than this server's cap. */
export function poolCapWarningsOf(observations: Wire['LocationHealthObservation'][]): PoolCapWarning[] {
	return (latestOf(observations, 'pool_capped')?.report.health.jobs ?? []).map((w) => ({ jobKey: w.job_key, kind: w.kind === 'group' ? 'group' : 'job', pool: w.pool, amount: w.amount, cap: w.cap, message: w.message }));
}

/** Why the newest load failed: the location's error, or its rule findings as `sqb compile` prints them; null when it isn't falling back. */
export function loadErrorOf(observations: Wire['LocationHealthObservation'][], health: LocationHealth): string | null {
	const failed = health === 'falling_back' ? latestOf(observations, 'compile_failed')?.report.health : undefined;
	const rules = failed?.rules;
	if (!rules) return failed?.error ?? null;
	const noun = rules.finding_count === 1 ? 'rule finding' : 'rule findings';
	const lines = rules.findings.map((f) => `${f.path ? `${f.path}${f.line ? `:${f.line}` : ''} ` : ''}[${f.code}] ${f.message}`);
	const more = rules.finding_count - rules.findings.length;
	return [`Compile failed: ${rules.finding_count} ${noun}`, ...lines, ...(more > 0 ? [`… and ${more} more`] : [])].join('\n');
}

export function toLocationDetail(location: Location, read: Wire['PersonLocationHealth'], published: Wire['PublishedSnapshot'] | null, now: number): LocationDetail {
	const { observations, state: health } = read;
	return {
		id: location.id,
		name: location.name,
		health,
		createdAt: null,
		brokenDeployPolicy: null,
		failedDeploy: null,
		lastLoadError: loadErrorOf(observations, health),
		poolCapWarnings: poolCapWarningsOf(observations),
		missingSecrets: missingSecretsOf(observations),
		instances: instancesOf(observations, now),
		target: location.target,
		snapshot: snapshotOf(published),
		warehouseConnection: null,
		connectionTest: connectionTestOf(observations),
		verdict: verdictOf(read.verdict)
	};
}

/** The server's verdict on a target: its worst open fact, and whether it stops runs. */
export function verdictOf(verdict: Wire['LocationVerdict']): LocationVerdictFact {
	return { fact: verdict.fact, stopsRuns: verdict.tier === 'stops_runs' };
}

/** The sync from its reports, with what the server knows beyond them: a request waiting, when the running sync started, and whether it stalled. */
export function withServerSync(sync: WarehouseSync | null, server: Wire['WarehouseSyncState']): WarehouseSync | null {
	const requested = server.requested ? { at: server.requested.requested_at, by: server.requested.requested_by } : null;
	const running = server.running;
	if (!sync) return requested ? { lastSyncedAt: null, lastError: null, requested } : null;
	return {
		...sync,
		requested,
		syncing: running ? { position: running.position, kinds: running.kinds, table: running.table, rows: running.rows, startedAt: running.started_at } : null,
		stalled: running?.stalled ?? false
	};
}

const SYNC_STAGE: Record<Exclude<Wire['WarehouseSyncFailure'], 'busy'>, string> = {
	watermark: 'reading where the last sync stopped',
	read: 'reading the warehouse',
	delivery: 'sending results to the server'
};

type Sync = Wire['LocationHealthObservation'] & { report: { health: Extract<Wire['LocationHealth'], { kind: 'warehouse_sync' }> } };
type SyncHealth = Sync['report']['health'];

/** Why a read failed and on what page size, e.g. "no reply within 30 s, on a page of 100 relations"; empty for other failures. The warehouse's own error text, when reported, is shown beside it. */
function readCauseOf(health: SyncHealth): string {
	const cause = health.read_failure;
	if (!cause) return '';
	const reason =
		cause.kind === 'timed_out'
			? `no reply within ${cause.seconds} s`
			: cause.kind === 'unreachable'
				? "couldn't reach the warehouse, so no compute ran"
				: health.error
				? 'the read failed'
				: 'the read failed; the location’s log has why';
	const page = health.page_limit ? `, on a page of ${health.page_limit.toLocaleString('en-US')} ${health.table === 'statistics' ? 'relations' : 'rows'}` : '';
	return `: ${reason}${page}`;
}

/** What held up a table's newest sync; a build holding the warehouse file is a wait, not a failure. */
const syncProblemOf = (health: SyncHealth & { failure: Wire['WarehouseSyncFailure'] }) =>
	health.failure === 'busy' ? 'Waiting for a build that holds the warehouse file' : `Failed while ${SYNC_STAGE[health.failure]}${readCauseOf(health)}`;

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
 * A location target's connection from its health reports and newest snapshot: the settings it
 * published, the newest connection check, where its state tables are read from, each table's newest
 * sync and how many records the server mirrored of it as its last read finished, and when the next
 * sync is due.
 */
export function toWarehouseConnection(
	location: Location,
	observations: Wire['LocationHealthObservation'][],
	published: Wire['PublishedSnapshot'] | null = null
): WarehouseConnection {
	const { syncs, newest } = syncsOf(observations);
	const tables: WarehouseTableSync[] = [...newest.values()]
		.map(({ received_at, report: { health } }) => ({
			table: health.table,
			at: received_at,
			recordsSent: health.records_sent,
			elapsedMs: health.elapsed_millis,
			failure: health.failure ? syncProblemOf({ ...health, failure: health.failure }) : null,
			error: health.error ?? null,
			waiting: health.failure === 'busy',
			lastSyncedAt: syncs.find((s) => s.report.health.table === health.table && !s.report.health.failure)?.received_at ?? null,
			mirroredTotal: syncs.find((s) => s.report.health.table === health.table && s.report.health.mirrored_records != null)?.report.health.mirrored_records ?? null
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
		halted: syncs[0]?.report.health.progress?.halted ?? false,
		tables
	};
}

/** When a table last synced cleanly, and which tables' newest sync failed; null without any sync report. */
export function warehouseSyncOf(observations: Wire['LocationHealthObservation'][]): WarehouseSync | null {
	const { syncs, newest } = syncsOf(observations);
	if (!syncs.length) return null;
	const failing = [...newest.values()].flatMap(({ report: { health } }) =>
		health.failure && health.failure !== 'busy' ? [`${health.table}: failed while ${SYNC_STAGE[health.failure]}${readCauseOf(health)}${health.error ? `: ${health.error}` : ''}`] : []
	);
	const running = syncs[0].report.health;
	const done = syncs.find((s) => s.report.health.progress?.completed);
	return {
		lastSyncedAt: syncs.find((s) => !s.report.health.failure)?.received_at ?? null,
		lastError: failing.length ? failing.sort().join('\n') : null,
		lastReportedAt: syncs[0].received_at,
		syncing: running.progress && !running.progress.completed && !running.progress.halted ? { position: running.progress.position, kinds: running.progress.kinds, table: running.table, rows: running.progress.rows } : null,
		completed: done?.report.health.progress ? { at: done.received_at, kinds: done.report.health.progress.kinds, rows: done.report.health.progress.rows, elapsedMs: done.report.health.progress.elapsed_millis } : null,
		failing: failingSyncOf(syncs)
	};
}

/** The newest sync report's failure: since its first failure in a row, its reason, and how it retries. */
function failingSyncOf(syncs: Sync[]): WarehouseSync['failing'] {
	const newest = syncs[0]?.report.health;
	if (!newest?.failure || newest.failure === 'busy') return null;
	const end = syncs.findIndex((s) => !s.report.health.failure || s.report.health.failure === 'busy');
	const streak = end === -1 ? syncs : syncs.slice(0, end);
	const halted = newest.progress?.halted ?? false;
	return {
		since: newest.failing_since ?? streak[streak.length - 1].received_at,
		reason: newest.error ?? `failed while ${SYNC_STAGE[newest.failure]}`,
		halted,
		unreachable: newest.read_failure?.kind === 'unreachable',
		retrySeconds: halted ? null : (newest.retry_seconds ?? null),
		retryAt: halted ? null : (newest.retry_at ?? null)
	};
}

/** Older than this, a sync report that left a sync running no longer shows it running: the location has stopped reporting. */
export const SYNC_REPORT_STALE_MS = 10 * 60_000;

/** The running sync, unless its newest report is stale. */
export function liveSyncOf(sync: WarehouseSync | null | undefined, now: number) {
	if (!sync?.syncing || !sync.lastReportedAt) return null;
	return now - Date.parse(sync.lastReportedAt) < SYNC_REPORT_STALE_MS ? sync.syncing : null;
}

/** Plans, history and costs read the server's copy of the warehouse; until a first sync completes, it may be missing or partial. */
export function firstSyncPending(sync: WarehouseSync | null | undefined, now: number): boolean {
	return !!liveSyncOf(sync, now) && !sync?.completed;
}
