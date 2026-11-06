// Labels, tones and pure helpers shared by the Platform and Admin pages.

import type {
	AccountSource,
	AssignmentScope,
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
	healthy: 'var(--sb-success)',
	degraded: 'var(--sb-warning)',
	falling_back: 'var(--sb-warning)',
	broken: 'var(--sb-error)'
};

export const healthLabel: Record<LocationHealth, string> = {
	healthy: 'Healthy',
	degraded: 'Needs attention',
	falling_back: 'Running previous version',
	broken: 'Not starting scheduled runs'
};

/** A location's health in words, e.g. "Deploy failed — running previous version 17". */
export function healthWords(location: Pick<LocationSummary, 'health' | 'failedDeploy' | 'brokenDeployPolicy' | 'snapshot'>): string {
	const version = location.snapshot?.generation;
	if (location.failedDeploy) {
		if (location.brokenDeployPolicy === 'fallback') return `Deploy failed — running previous version${version != null ? ` ${version}` : ''}`;
		return 'Deploy failed — scheduled runs wait until a deploy works';
	}
	return healthLabel[location.health];
}

export const eventKindLabel: Record<PlatformEventKind, string> = {
	tick_overdue: 'Schedule overdue',
	scheduler_no_leader: 'Scheduler stopped',
	location_broken: 'Deploy failed',
	launch_failed: 'Couldn’t start runs',
	connection_test_failed: 'Connection test failed',
	warehouse_sync_failed: 'Warehouse sync failing',
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
	trusted_proxy: 'Company proxy sign-in',
	password: 'Password'
};

/** `SCHEMA.NAME`, or `DB.SCHEMA.NAME` when `withDatabase`. */
export function formatRelation(relation: RelationName, withDatabase = false): string {
	const parts = withDatabase && relation.database ? [relation.database, relation.schema, relation.name] : [relation.schema, relation.name];
	return parts.join('.');
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

/** A location's one target in words, e.g. `prod · Snowflake`. */
export function targetWords(target: Pick<LocationTarget, 'name' | 'adapter'>): string {
	return `${target.name} · ${adapterLabel(target.adapter)}`;
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
