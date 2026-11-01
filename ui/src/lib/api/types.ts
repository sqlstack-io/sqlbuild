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
	'trigger.toggle',
	'sensor.reset_cursor',
	'run.adhoc',
	'connection.test',
	'warehouse.sync',
	'plan.preview',
	'location.manage',
	'user.manage',
	'role.assign',
	'token.manage',
	'audit.view'
] as const;
export type Capability = (typeof CAPABILITIES)[number];

export type PrincipalKind = 'user' | 'service_account' | 'scheduler' | 'sensor' | 'system';
export type Principal = {
	kind: PrincipalKind;
	/** Stable handle, e.g. a user name, service account or `scheduler`. */
	name: string;
	displayName: string;
};

export type CurrentUser = Principal & {
	kind: 'user' | 'service_account';
	role: BuiltInRole;
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

export type WarehouseSyncState = 'idle' | 'running' | 'failed';
export type WarehouseSync = {
	state: WarehouseSyncState;
	lastSyncedAt: Iso | null;
	/** Start of the sync in progress, when `state` is `running`. */
	runningSince: Iso | null;
	/** A follow-up sync is queued behind the running one (at most one). */
	followUpQueued: boolean;
	lastError: string | null;
};

export type WarehouseSyncRequestOutcome =
	/** No sync was running; a new one started. */
	| 'started'
	/** A sync was running; one follow-up sync was queued behind it. */
	| 'follow_up_queued'
	/** A sync was running and a follow-up was already queued; the request merged into it. */
	| 'merged';

export type WarehouseSyncRequestResult = {
	outcome: WarehouseSyncRequestOutcome;
	sync: WarehouseSync;
};

/** The one target a location is deployed against, e.g. `prod` on Snowflake. */
export type LocationTarget = {
	name: string;
	adapter: string;
	/** Where it builds, e.g. `WAFFLE_SHOP on example-account`; never secrets. */
	connectionSummary: string;
};

export type LocationHealth = 'healthy' | 'degraded' | 'falling_back' | 'broken';
/** A project deployed against exactly one target. Staging and dev are separate locations. */
export type Location = {
	/** Opaque server-generated UUID; the name is a mutable label. */
	id: string;
	name: string;
	health: LocationHealth;
	lastSeenAt: Iso | null;
	target: LocationTarget;
	snapshot: Snapshot | null;
	warehouseSync: WarehouseSync;
};

export type Scope = { locationId: string };

// ---- bootstrap ----

export type Bootstrap = {
	serverVersion: string;
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
	pendingApprovals: number;
};

export * from './types/operate';
export * from './types/build';
export * from './types/platform';
