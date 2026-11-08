// Wire types for the SQLBuild server API.
//
// These mirror the server's resource shapes so that a client generated from the
// OpenAPI schema can replace them later. Timestamps are ISO-8601 strings and IDs
// are opaque strings.

export type Iso = string;

// ---- identity, roles and capabilities ----

export type BuiltInRole = 'viewer' | 'operator' | 'developer' | 'admin';

export const CAPABILITIES = [
	'view',
	'view.sql',
	'view.evidence',
	'job.run',
	'run.cancel',
	'run.rerun',
	'run.rewrite_history',
	'run.prioritise',
	'trigger.toggle',
	'sensor.reset_cursor',
	'run.adhoc',
	'connection.test',
	'plan.preview',
	'location.manage',
	'user.manage',
	'role.assign',
	'audit.view'
] as const;
export type Capability = (typeof CAPABILITIES)[number];

export type PrincipalKind = 'user' | 'scheduler' | 'sensor' | 'system';
export type Principal = {
	kind: PrincipalKind;
	/** Stable handle, e.g. a user name or `scheduler`. */
	name: string;
	displayName: string;
};

export type CurrentUser = Principal & {
	kind: 'user';
	/** The built-in role, when one role describes the person; the server reports actions instead. */
	role?: BuiltInRole;
};

export type AuthMode = 'oidc' | 'trusted_proxy' | 'password' | 'disabled';

// ---- locations, targets and snapshots ----

export type Snapshot = {
	id: string;
	/** Increases per (location, target); an older compile never replaces a newer one. */
	generation: number;
	gitRevision: string;
	gitDirty: boolean;
	gitAuthor: string;
	publishedAt: Iso;
	sqbVersion: string;
};

/** The location reads SQLBuild's own tables back from the warehouse on its own schedule. */
export type WarehouseSync = {
	lastSyncedAt: Iso | null;
	/** Why the latest sync failed; null when it succeeded. */
	lastError: string | null;
};

/** The one target a location is deployed against, e.g. `prod` on Snowflake. */
export type LocationTarget = {
	name: string;
	/** Null until snapshots record the adapter. */
	adapter: string | null;
	/** Where it builds, e.g. `WAFFLE_SHOP on example-account`; never secrets. Null when unknown. */
	connectionSummary: string | null;
};

export type LocationHealth = 'healthy' | 'degraded' | 'falling_back' | 'broken';
/** A project deployed against exactly one target. Staging and dev are separate locations. */
export type Location = {
	/** Opaque server-generated UUID; the name is a mutable label. */
	id: string;
	name: string;
	/** Null when the server does not report health yet. */
	health: LocationHealth | null;
	lastSeenAt: Iso | null;
	target: LocationTarget;
	/** Null when nothing is deployed; absent when the server does not report it here. */
	snapshot?: Snapshot | null;
	warehouseSync: WarehouseSync;
	/** Actions allowed somewhere in this location, added to the server-wide `capabilities`. */
	capabilities?: Capability[];
};

export type Scope = { locationId: string };

// ---- bootstrap ----

export type Bootstrap = {
	serverVersion?: string;
	authMode: AuthMode;
	user: CurrentUser;
	capabilities: Capability[];
	locations: Location[];
};

// ---- platform events ----

export type PlatformEventKind =
	| 'tick_overdue'
	| 'scheduler_no_leader'
	| 'location_broken'
	| 'launch_failed'
	| 'connection_test_failed'
	| 'warehouse_sync_failed'
	| 'heartbeat';
export type PlatformEventSeverity = 'info' | 'warning' | 'critical';
export type PlatformEvent = {
	id: string;
	kind: PlatformEventKind;
	severity: PlatformEventSeverity;
	occurredAt: Iso;
	/** Still unresolved; only active events surface in the banner. */
	active: boolean;
	locationId: string | null;
	subject: string | null;
	message: string;
};

// ---- navigation ----

export type NavCounts = {
	catalogNodes: number;
	failingChecks: number;
	activeRuns: number;
	pausedTriggers: number;
	activePlatformEvents: number;
};

export * from './types/operate';
export * from './types/build';
export * from './types/platform';
