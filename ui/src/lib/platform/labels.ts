// Labels, tones and pure helpers shared by the Platform and Admin pages.

import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
import type { SyncNowState } from './sync-now';

import type {
	AccountSource,
	AssignmentScope,
	ConnectionTest,
	RepublishResult,
	LocationHealth,
	Location,
	LocationSummary,
	LocationTarget,
	PlatformEventKind,
	PlatformEventLink,
	PlatformEventReaction,
	PlatformEventReactionOutcome,
	PlatformEventSeverity,
	RelationName,
	TestOutcome,
	TokenState
} from '$lib/api';

export const healthTone: Record<LocationHealth, string> = {
	starting: 'var(--sb-running)',
	healthy: 'var(--sb-success)',
	degraded: 'var(--sb-error)',
	missing_secrets: 'var(--sb-error)',
	change_plan_failed: 'var(--sb-error)',
	discovery_failed: 'var(--sb-error)',
	falling_back: 'var(--sb-error)',
	broken: 'var(--sb-error)'
};

export const healthLabel: Record<LocationHealth, string> = {
	starting: 'Starting',
	healthy: 'Healthy',
	degraded: 'Needs attention',
	missing_secrets: 'Missing provider secrets',
	change_plan_failed: 'Change plan failed',
	discovery_failed: 'Discovery failed',
	falling_back: 'Running previous version',
	broken: 'Broken'
};

/** What each health means, for tooltips. */
export const healthHelp: Record<LocationHealth, string> = {
	starting: 'The newest instance is loading the project; earlier instances keep serving',
	healthy: 'Loaded and starting scheduled runs',
	degraded: 'Running, with open problems',
	missing_secrets: 'A provider setting is missing from the location environment',
	change_plan_failed: 'Planning what changed failed on the current snapshot, so what needs building is unknown',
	discovery_failed: 'A discover function failed at the last publish; its nodes keep their last good items',
	falling_back: 'The last deploy failed to load; the previous version keeps running',
	broken: 'Not starting scheduled runs'
};

/** What a location does when a deploy fails to load, as a short fact; `brokenDeployPolicyHelp` explains it. */
export const brokenDeployPolicyLabel = { fallback: 'Keep previous', hold: 'Hold runs' } as const;
export const brokenDeployPolicyHelp = { fallback: 'Keep running the previous version', hold: 'Hold scheduled runs until a deploy works' } as const;

/** A location's health in words, e.g. "Deploy failed — running previous version 17". */
export function healthWords(location: Pick<LocationSummary, 'health' | 'failedDeploy' | 'brokenDeployPolicy' | 'snapshot'>): string {
	const version = location.snapshot?.generation;
	if (location.failedDeploy) {
		if (location.brokenDeployPolicy === 'fallback') return `Deploy failed · on version${version != null ? ` ${version}` : ' before'}`;
		return 'Deploy failed · runs held';
	}
	return location.health ? healthLabel[location.health] : 'Unknown';
}

export const eventKindLabel: Record<PlatformEventKind, string> = {
	tick_overdue: 'Schedule overdue',
	calendar_horizon_low: 'Calendar running out of dates',
	trigger_skipped: 'Trigger skipped',
	sensor_rechecked: 'Sensor checked again from scratch',
	calendar_times_dropped: 'Calendar times dropped',
	warehouse_sync_stalled: 'Warehouse sync stalled',
	scheduler_no_leader: 'Scheduler stopped',
	location_broken: 'Location not checking in',
	location_failing: 'Location failing',
	location_missing_secrets: 'Provider secrets missing',
	launch_failed: 'Couldn’t start runs',
	connection_test_failed: 'Connection test failed',
	warehouse_sync_failed: 'Warehouse sync failing',
	event_sink_failed: 'Event sink losing events',
	event_forwarding_failed: 'Event forwarding failing',
	heartbeat: 'Server heartbeat'
};

/** Where a problem is explained: its location's problem history, or the server status on Locations. */
export function problemHref(event: { locationId: string | null }): string {
	return event.locationId ? `/locations/${event.locationId}#problems` : '/locations#server';
}

export const severityTone: Record<PlatformEventSeverity, string> = {
	critical: 'var(--sb-error)',
	warning: 'var(--sb-warning)',
	info: 'var(--sb-text-faint)'
};

export const reactionTone: Record<PlatformEventReactionOutcome, string> = {
	consumed: 'var(--sb-success)',
	failed: 'var(--sb-error)',
	pending: 'var(--sb-text-faint)'
};

export const reactionLabel: Record<PlatformEventReactionOutcome, string> = {
	consumed: 'handled',
	failed: 'failing',
	pending: 'not yet'
};

/** Deep link to the sensor that reacted, scoped to its location. */
export function reactionHref(reaction: PlatformEventReaction): string {
	return `/triggers/${encodeURIComponent(reaction.triggerId)}?location=${encodeURIComponent(reaction.locationId)}`;
}

export const testTone: Record<TestOutcome, string> = { ok: 'var(--sb-success)', failed: 'var(--sb-error)' };
export const testLabel: Record<TestOutcome, string> = { ok: 'Passed', failed: 'Failed' };

export const tokenTone: Record<TokenState, string> = {
	active: 'var(--sb-success)',
	revoked: 'var(--sb-text-faint)',
	expired: 'var(--sb-text-faint)'
};
export const tokenLabel: Record<TokenState, string> = { active: 'Active', revoked: 'Revoked', expired: 'Expired' };

export const accountSourceLabel: Record<AccountSource, string> = {
	oidc: 'Single sign-on',
	trusted_proxy: 'Trusted proxy',
	password: 'Password'
};

/** `SCHEMA.NAME`, or `DB.SCHEMA.NAME` when `withDatabase`. */
export function formatRelation(relation: RelationName, withDatabase = false): string {
	const parts = withDatabase && relation.database ? [relation.database, relation.schema, relation.name] : [relation.schema, relation.name];
	// A relation recorded without its schema shows its bare name.
	return parts.filter(Boolean).join('.');
}

/** Short image ID, e.g. `sha256:4be1c07d9a52` → `4be1c07`. */
export function shortImage(imageId: string | null): string {
	if (!imageId) return '—';
	return imageId.replace(/^sha256:/, '').slice(0, 7);
}

export function locationName(locations: readonly Location[] | undefined, locationId: string | null): string {
	if (!locationId) return 'All locations';
	return locations?.find((l) => l.id === locationId)?.name ?? locationId;
}

/** e.g. `All locations` or `waffle-shop`. */
export function formatAssignmentScope(scope: AssignmentScope, locations: readonly Location[] | undefined): string {
	return locationName(locations, scope.locationId);
}

/** A location's one target in words, e.g. `prod · Snowflake`, or `prod` while the adapter is unknown. */
export function targetWords(target: Pick<LocationTarget, 'name' | 'adapter'>): string {
	return target.adapter ? `${target.name} · ${adapterLabel(target.adapter)}` : target.name;
}

const ADAPTER_LABELS: Record<string, string> = { snowflake: 'Snowflake', duckdb: 'DuckDB', postgres: 'Postgres', bigquery: 'BigQuery', databricks: 'Databricks' };
export function adapterLabel(adapter: string): string {
	return ADAPTER_LABELS[adapter] ?? adapter;
}

/** Deep link for an event link; scoped pages carry `?location=`. */
export function eventLinkHref(link: PlatformEventLink): string {
	if (link.kind === 'location') return `/locations/${link.id}`;
	const scope = `location=${encodeURIComponent(link.locationId)}`;
	if (link.kind === 'trigger') return `/triggers/${encodeURIComponent(link.id)}?${scope}`;
	if (link.kind === 'job') return `/jobs/${encodeURIComponent(link.key)}?${scope}`;
	return `/runs/${encodeURIComponent(link.id)}?${scope}`;
}

export function eventLinkLabel(link: PlatformEventLink, locations: readonly Location[] | undefined): string {
	if (link.kind === 'location') return `Location ${locationName(locations, link.id)}`;
	if (link.kind === 'trigger') return link.name;
	if (link.kind === 'job') return `Job ${link.key}`;
	return `Run ${link.id.replace(/^run-/, '')}`;
}

export function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** How a discovered set's newest fetch went (`DiscoveredSetStatus`). */
export const discoveredStatusLabel: Record<'ok' | 'failed' | 'never', string> = {
	ok: 'Fetched',
	failed: 'Fetch failed',
	never: 'Never fetched'
};
export const discoveredStatusTone: Record<'ok' | 'failed' | 'never', string> = {
	ok: 'var(--sb-success)',
	failed: 'var(--sb-warning)',
	never: 'var(--sb-text-faint)'
};

// ---- action status: one set of words for every status that changes on its own ----
//
// Each reads "what · progress · age" in lower case after its lead (e.g. "Warehouse ● connected 54s ago"),
// in one of five tones. "Updated" is page data, "synced" only the warehouse history sync, "tested" a
// connection and "published" a snapshot.

/** The five tones a status takes. */
export const actionTone = {
	ok: 'var(--sb-success)',
	running: 'var(--sb-running)',
	warning: 'var(--sb-warning)',
	failed: 'var(--sb-error)',
	never: 'var(--sb-text-faint)'
} as const;
export type ActionTone = keyof typeof actionTone;

/** One status as a label shows it, with what its tooltip adds. */
export type ActionWords = { tone: ActionTone; label: string; title: string };

/** How long since `from` (an ISO time or epoch milliseconds), short: `4s`, `12m`, `3h`, `2d`. */
export function elapsed(from: string | number, now: number): string {
	const seconds = Math.max(0, Math.round((now - (typeof from === 'number' ? from : Date.parse(from))) / 1000));
	if (seconds < 60) return `${seconds}s`;
	if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
	if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h`;
	return `${Math.floor(seconds / 86_400)}d`;
}

/** Where a connection test came from, for its tooltip. */
export function testOrigin(test: Pick<ConnectionTest, 'trigger' | 'by'>): string {
	if (test.trigger === 'manual') return test.by ? `by ${test.by.name}` : 'by hand';
	return test.trigger === 'startup' ? 'on startup' : 'on deploy';
}

/** What a connection test's tooltip says: the last test in full, or that there is none. */
export function testTitle(test: ConnectionTest | null, deployedAt: string | null, now: number): string {
	if (!test) return 'The location hasn’t tested its warehouse connection yet. Test connection runs the same check as sqb debug.';
	// A failure's message is the error, shown once where the failure is listed, not in every tooltip.
	const when = `Tested ${testOrigin(test)} · ${formatStamp(test.at)} · ${test.durationMs} ms${test.outcome === 'ok' ? `\n${test.message}` : ''}`;
	if (test.outcome === 'ok' && deployedAt && Date.parse(test.at) < Date.parse(deployedAt)) return `Last connected ${formatAgo(test.at, now)}, before the latest deploy.\n${when}`;
	return when;
}

/** The warehouse connection test: connected, couldn't connect, not tested since the last deploy, or never tested. */
export function testWords(test: ConnectionTest | null, deployedAt: string | null, now: number): ActionWords {
	const title = testTitle(test, deployedAt, now);
	if (!test) return { tone: 'never', label: 'not tested yet', title };
	if (test.outcome === 'failed') return { tone: 'failed', label: `couldn’t connect ${formatAgo(test.at, now)}`, title };
	if (deployedAt && Date.parse(test.at) < Date.parse(deployedAt)) return { tone: 'warning', label: 'not tested since deploy', title };
	return { tone: 'ok', label: `connected ${formatAgo(test.at, now)}`, title };
}

/** What a sync status's tooltip says: who asked and when, what it reads, why it fails, or when it last ran. */
export function syncTitle(state: SyncNowState, now: number): string {
	const what = "What's been built, checks and freshness, read back from the warehouse.";
	switch (state.kind) {
		case 'requested':
			return `Requested ${state.by ? `by ${state.by} ` : ''}at ${formatStamp(new Date(state.at).toISOString())}. The location usually takes it within seconds.`;
		case 'running':
			return `Reading ${state.running.table.replaceAll('_', ' ')}${state.running.startedAt ? `, running since ${formatStamp(state.running.startedAt)}` : ''}. Last progress ${formatAgo(state.progressAt, now)}.\n${what}`;
		case 'stalled':
			return `At ${state.running.position}/${state.running.kinds} (${state.running.table.replaceAll('_', ' ')}) since ${formatStamp(state.progressAt)}, with no report since. The location may have stopped, so check its output, then Sync now.`;
		case 'failing':
			return `${state.failing ? `Failing since ${formatStamp(state.failing.since)}. ` : ''}${state.lastSyncedAt ? `Last synced ${formatStamp(state.lastSyncedAt)}.` : 'Not synced yet.'}`;
		case 'synced':
			return `Synced ${formatStamp(state.at)}. A sync also follows every run that ends.\n${what}`;
		case 'never':
			return what;
	}
}

/** The warehouse history sync, from `syncNowState`. */
export function syncWords(state: SyncNowState, now: number): ActionWords {
	const title = syncTitle(state, now);
	switch (state.kind) {
		case 'requested':
			return { tone: 'running', label: requestedLabel(state.at, now), title };
		case 'running': {
			const r = state.running;
			return { tone: 'running', label: `syncing · ${r.position}/${r.kinds} kinds · ${r.rows.toLocaleString('en-US')} rows${r.startedAt ? ` · ${elapsed(r.startedAt, now)}` : ''}`, title };
		}
		case 'stalled':
			return { tone: 'warning', label: `sync stalled · ${state.running.position}/${state.running.kinds} · no progress for ${elapsed(state.progressAt, now)}`, title };
		case 'failing': {
			const f = state.failing;
			const since = f ? ` for ${elapsed(f.since, now)}` : '';
			const retry = f?.halted ? ' · stopped' : f?.retryAt ? ` · retrying ${formatIn(f.retryAt, now)}` : '';
			return { tone: 'failed', label: `failing${since}${retry}`, title };
		}
		case 'synced':
			return { tone: 'ok', label: `synced ${formatAgo(state.at, now)}`, title };
		case 'never':
			return { tone: 'never', label: 'not synced yet', title };
	}
}

/** A sync someone asked for that the location hasn't taken yet, with how long ago. */
export function requestedLabel(at: number, now: number): string {
	return `requested · waiting for the location · ${elapsed(at, now)}`;
}

/** What the deploy status's tooltip says. */
export function publishTitle(result: RepublishResult | null, publishing: number | null, snapshot: { publishedAt: string } | null): string {
	if (publishing !== null) return 'Compiling and publishing again, running the discover functions.';
	if (result?.state === 'failed') return result.error ?? 'The publish failed.';
	if (result?.state === 'expired') return 'The location didn’t take the request in time. Check it is running, then try again.';
	return snapshot ? `Published ${formatStamp(snapshot.publishedAt)}.` : 'Nothing is deployed to this location yet.';
}

/** The deployed version, or the latest Republish: publishing, published, failed or expired. */
export function publishWords(result: RepublishResult | null, publishing: number | null, snapshot: { generation: number; publishedAt: string } | null, now: number): ActionWords {
	const title = publishTitle(result, publishing, snapshot);
	if (publishing !== null) return { tone: 'running', label: `publishing · ${elapsed(publishing, now)}`, title };
	if (result?.state === 'failed') return { tone: 'failed', label: 'publish failed', title };
	if (result?.state === 'expired') return { tone: 'failed', label: 'request expired', title };
	if (snapshot) return { tone: 'ok', label: `version ${snapshot.generation} · ${formatAgo(snapshot.publishedAt, now)}`, title };
	return { tone: 'never', label: 'nothing published yet', title };
}
/** The location's one health tag: its health, or the worst fact that would contradict "Healthy". */
export function headlineWords(health: LocationHealth, facts: { live: boolean; test: ActionWords; sync: SyncNowState }): { tone: string; label: string } {
	if (!facts.live) return { tone: actionTone.failed, label: 'Not checking in' };
	if (health !== 'healthy') return { tone: healthTone[health], label: healthLabel[health] };
	if (facts.sync.kind === 'failing') return { tone: actionTone.failed, label: 'Sync failing' };
	if (facts.test.tone === 'failed') return { tone: actionTone.failed, label: 'Can’t connect' };
	if (facts.sync.kind === 'stalled') return { tone: actionTone.warning, label: 'Sync stalled' };
	if (facts.test.tone === 'warning' || facts.test.tone === 'never') return { tone: actionTone.warning, label: 'Not tested' };
	return { tone: healthTone[health], label: healthLabel[health] };
}

/**
 * A problem message as a short summary and the error behind it, so the error is shown once, behind
 * "View error": the text before the first `: ` that follows at least three words, e.g. "Run history
 * can't reach the warehouse" and "390422 (08001): IP … is not allowed". Short messages stay whole.
 */
export function messageParts(message: string): { summary: string; error: string | null } {
	for (let at = message.indexOf(': '); at > 0; at = message.indexOf(': ', at + 2)) {
		const head = message.slice(0, at);
		const words = head.split(/\s+/).filter((w) => /[a-z]/i.test(w)).length;
		if (words >= 3) {
			const rest = message.slice(at + 2).trim();
			return rest ? { summary: head, error: rest } : { summary: message, error: null };
		}
	}
	return { summary: message, error: null };
}
