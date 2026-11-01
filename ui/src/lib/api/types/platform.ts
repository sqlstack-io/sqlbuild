// Wire types for the Platform (Locations with server status, Connections,
// Providers, Maintenance) and Admin (users, service accounts, roles, audit,
// settings) areas.

import type {
	AuthMode,
	Capability,
	Iso,
	LocationHealth,
	LocationTarget,
	PlatformEvent,
	Principal,
	Snapshot
} from '../types';

// ---- health: system status and platform events ----

export type SchedulerInstance = {
	/** Server replica ID, e.g. the container host name. */
	instanceId: string;
	version: string;
	/** False for replicas started with `--no-scheduler`. */
	schedulerEnabled: boolean;
	startedAt: Iso;
	lastSeenAt: Iso;
};

export type SchedulerLease = {
	holder: string;
	acquiredAt: Iso;
	renewedAt: Iso;
	expiresAt: Iso;
};

export type SchedulerStatus = {
	/** Null when no replica holds the leader lease. */
	lease: SchedulerLease | null;
	instances: SchedulerInstance[];
};

export type HeartbeatStatus = {
	intervalSeconds: number;
	lastEmittedAt: Iso | null;
};

export type SinkKind = 'webhook' | 'kafka';
/** The server-side platform-event sink. Server config; read-only in the UI. */
export type SinkStatus = {
	kind: SinkKind;
	/** Where the destination comes from, never the value, e.g. `env: SQB_EVENT_SINK_URL`. */
	destinationSource: string;
	/** Platform event kinds forwarded; empty means all. */
	eventKinds: PlatformEvent['kind'][];
	lastDeliveredAt: Iso | null;
	lastAttemptAt: Iso | null;
	/** Events recorded but not yet delivered (the sink's cursor lag). */
	pendingEvents: number;
	consecutiveFailures: number;
	lastError: string | null;
};

export type PlatformHealth = {
	scheduler: SchedulerStatus;
	heartbeat: HeartbeatStatus;
	/** Null when no server-side sink is configured. */
	sink: SinkStatus | null;
};

export type PlatformEventLink =
	| { kind: 'trigger'; id: string; name: string; locationId: string }
	| { kind: 'job'; key: string; locationId: string }
	| { kind: 'run'; id: string; locationId: string }
	| { kind: 'location'; id: string };

/**
 * How a sensor that consumes platform events handled one event: `consumed`
 * once an evaluation advanced its cursor past it, `failed` while the latest
 * evaluation covering it failed, `pending` until an evaluation reaches it.
 */
export type PlatformEventReactionOutcome = 'consumed' | 'failed' | 'pending';

export type PlatformEventReaction = {
	triggerId: string;
	triggerName: string;
	locationId: string;
	outcome: PlatformEventReactionOutcome;
	/** The evaluation that consumed the event, or the latest one that tried. */
	evaluationId: string | null;
	/** Evaluations that covered the event, including failed attempts. */
	attempts: number;
	at: Iso | null;
};

export type PlatformEventRecord = PlatformEvent & {
	resolvedAt: Iso | null;
	links: PlatformEventLink[];
	/** Sensors whose `platform_event` condition matches this event's kind. */
	reactions: PlatformEventReaction[];
	/** Typed payload fields of the event kind, rendered as key/value pairs. */
	payload: Record<string, string>;
};

export type PlatformEventQuery = {
	kinds?: PlatformEvent['kind'][];
	activeOnly?: boolean;
	/** Events of one location; `null` for server-wide events only (no location). */
	locationId?: string | null;
	/** Heartbeats are left out unless asked for. */
	includeHeartbeats?: boolean;
	limit?: number;
};

// ---- locations ----

export type TestOutcome = 'ok' | 'failed';

export type BrokenDeployPolicy = 'fallback' | 'queue';

export type FailedDeploy = {
	gitRevision: string;
	imageId: string;
	attemptedAt: Iso;
	/** The last load error reported by the location while compiling or publishing. */
	error: string;
};

export type ConnectionTest = {
	outcome: TestOutcome;
	at: Iso;
	durationMs: number;
	/** `publish` when the location ran it after publishing a snapshot. */
	trigger: 'publish' | 'manual';
	by: Principal | null;
	message: string;
};

export type LocationInstance = {
	id: string;
	imageId: string;
	gitRevision: string;
	startedAt: Iso;
	lastSeenAt: Iso;
	/** `stale` once heartbeats stop; `gone` after deregistering. */
	state: 'live' | 'stale' | 'gone';
};

export type LocationDetail = {
	id: string;
	name: string;
	health: LocationHealth;
	createdAt: Iso;
	brokenDeployPolicy: BrokenDeployPolicy;
	/** The latest deploy that never published; null when the latest deploy is current. */
	failedDeploy: FailedDeploy | null;
	lastLoadError: string | null;
	instances: LocationInstance[];
	target: LocationTarget;
	snapshot: Snapshot | null;
	/** Image the current snapshot was published from; runs start from it. Null for local processes. */
	imageId: string | null;
	warehouseConnection: string;
	connectionTest: ConnectionTest | null;
};

export type LocationSummary = Pick<LocationDetail, 'id' | 'name' | 'health' | 'brokenDeployPolicy' | 'failedDeploy' | 'target' | 'snapshot' | 'connectionTest'> & {
	/** Unresolved platform events of this location. */
	activeProblems: number;
	liveInstances: number;
	lastSeenAt: Iso | null;
};

// ---- tokens ----

export type TokenState = 'active' | 'revoked' | 'expired';

export type LocationToken = {
	id: string;
	locationId: string;
	/** Non-secret prefix shown for identification. */
	prefix: string;
	createdAt: Iso;
	createdBy: Principal;
	lastUsedAt: Iso | null;
	revokedAt: Iso | null;
	state: TokenState;
};

/** A long-lived token of a service account. People don't have API tokens: they use `sqb login`. */
export type ApiToken = {
	id: string;
	label: string;
	/** The service account the token signs in as. */
	owner: Principal;
	prefix: string;
	createdAt: Iso;
	createdBy: Principal;
	expiresAt: Iso | null;
	lastUsedAt: Iso | null;
	revokedAt: Iso | null;
	state: TokenState;
};

/** Returned once on create or rotate; the server stores only a hash. */
export type IssuedToken<T> = { token: T; secret: string };

export type ApiTokenRequest = { label: string; expiresInDays: number | null };

// ---- connections ----

export type SettingField = {
	key: string;
	type: 'string' | 'int' | 'bool' | 'enum' | 'secret';
	required?: boolean;
	default?: string | number | boolean;
	enum?: string[];
	min?: number;
	max?: number;
	description?: string;
	/** For secrets: where the value comes from, e.g. `env: WAREHOUSE_PASSWORD`. */
	secretSource?: string;
};

export type WarehouseConnection = {
	name: string;
	target: string;
	adapter: string;
	/** `catalog_connection` override used by the warehouse sync, if any. */
	purpose: 'target' | 'catalog';
	sourceFile: string;
	settings: SettingField[];
	/** Configured values; secrets are never returned. */
	values: Record<string, string | number | boolean>;
	lastTest: ConnectionTest | null;
};

/** A connection to something other than the warehouse (an API, object storage, a chat webhook), declared in project code. */
export type ProviderConnection = {
	name: string;
	type: string;
	description: string;
	sourceFile: string;
	settings: SettingField[];
	values: Record<string, string | number | boolean>;
	/** `triggers` are sensors whose code uses the provider, by trigger ID and name. */
	usedBy: { nodes: string[]; jobs: string[]; triggers: { id: string; name: string }[] };
};

// ---- maintenance (synced warehouse state tables) ----

export type RelationName = { database: string | null; schema: string; name: string };

/** Recorded decisions in `_sqlbuild_migrations`. */
export type MigrationDecision = 'migrate' | 'redo' | 'superseded_replace' | 'forced_replace' | 'renamed';

/** One row of `_sqlbuild_migrations`. */
export type MigrationEvent = {
	eventId: string;
	targetName: string;
	originModel: string;
	origin: RelationName;
	destinationModel: string;
	destination: RelationName;
	discovery: 'manual' | 'automatic';
	decision: MigrationDecision;
	runId: string;
	createdAt: Iso;
};

export type OldNameEventType = 'required' | 'origin_archived' | 'view_created' | 'view_dropped';
export type OldNameStep = { eventType: OldNameEventType; at: Iso; runId: string };

/** The projection of one move's `_sqlbuild_old_name_views` rows. */
export type OldNameView = {
	migrationEventId: string;
	destinationModel: string;
	old: RelationName;
	new: RelationName;
	/** Retention fixed when the move was recorded, e.g. `30d`, or `false` when no view is kept. */
	retention: string;
	archiveName: string | null;
	columnAliases: { old: string; new: string }[];
	grantsCopied: number | null;
	expiresAt: Iso | null;
	dropReason: string | null;
	steps: OldNameStep[];
};

/** One row of `_sqlbuild_janitor_events`. */
export type JanitorEvent = {
	eventId: string;
	eventType: 'archive' | 'delete';
	occurredAt: Iso;
	runId: string;
	relationSchema: string;
	relationType: 'table' | 'view' | null;
	originalQualifiedName: string | null;
	archiveQualifiedName: string;
	archivedAt: Iso;
};

export type MaintenanceHistory = {
	migrations: MigrationEvent[];
	oldNameViews: OldNameView[];
	janitorEvents: JanitorEvent[];
};

// ---- users and roles ----

export type AccountSource = 'oidc' | 'trusted_proxy' | 'password' | 'service_account';

export type Account = Principal & {
	kind: 'user' | 'service_account';
	email: string | null;
	source: AccountSource;
	ssoGroups: string[];
	createdAt: Iso;
	lastSeenAt: Iso | null;
	disabled: boolean;
	/** Signed-in browser sessions; disabling the account revokes them. */
	activeSessions: number;
	/** Service accounts only: the person or team responsible. */
	owner: string | null;
	/** Service accounts only: what the account is for. */
	description: string | null;
};

export type ServiceAccountRequest = { name: string; description: string };

export type RoleGrant = { nodes: string[]; permissions: Capability[] };

export type RoleSource = 'built_in' | 'server_config' | 'project';
export type RoleDefinition = {
	name: string;
	source: RoleSource;
	/** Project roles: the policy file in the snapshot; server roles: the server config. */
	sourceFile: string | null;
	description: string;
	/** Approved grants currently in force. */
	grants: RoleGrant[];
	/** A widening change is waiting for approval. */
	pendingApprovalId: string | null;
};

/** `locationId: null` applies everywhere. */
export type AssignmentScope = { locationId: string | null };

export type RoleAssignment = {
	id: string;
	principal: Principal;
	role: string;
	scope: AssignmentScope;
	source: 'manual' | 'sso';
	/** For SSO-derived assignments: the mapped group. */
	ssoGroup: string | null;
	assignedBy: Principal | null;
	assignedAt: Iso;
	/** The role no longer exists in code; the assignment shows but grants nothing. */
	stale: boolean;
};

export type RoleAssignmentRequest = { principal: string; role: string; scope: AssignmentScope };

export type ApprovalState = 'pending' | 'approved' | 'rejected' | 'superseded';
export type RoleApproval = {
	id: string;
	role: string;
	locationId: string;
	snapshotGeneration: number;
	gitRevision: string;
	gitAuthor: string;
	proposedAt: Iso;
	/** Grants added or widened by the proposal. */
	added: RoleGrant[];
	/** Grants removed or narrowed (applied together with the widening on approval). */
	removed: RoleGrant[];
	state: ApprovalState;
	decidedBy: Principal | null;
	decidedAt: Iso | null;
	note: string | null;
};

// ---- audit ----

export type AuditOutcome = 'allowed' | 'denied';
export type AuditEntry = {
	id: string;
	at: Iso;
	actor: Principal;
	action: Capability | 'role.approve' | 'role.reject' | 'session.login';
	outcome: AuditOutcome;
	scope: AssignmentScope & { subject: string | null };
	/** Role that granted the action, or for denials null. */
	grantingRole: string | null;
	evidenceRead: boolean;
	summary: string;
	before: string | null;
	after: string | null;
	ip: string;
	requestId: string;
};

export type AuditQuery = {
	actor?: string;
	action?: string;
	outcome?: AuditOutcome;
	evidenceOnly?: boolean;
	limit?: number;
};

// ---- explain access ----

export type AccessQuery = {
	principal: string;
	action: Capability;
	locationId: string;
	/** Optional job, group or node the action applies to. */
	subject: string | null;
};

export type AccessGrantPath = {
	role: string;
	assignmentId: string;
	assignmentSource: 'manual' | 'sso';
	ssoGroup: string | null;
	scope: AssignmentScope;
	/** Node selectors of the matching grant, when the role is node-scoped. */
	nodes: string[] | null;
};

export type AccessExplanation = {
	query: AccessQuery;
	allowed: boolean;
	/** Every assignment that grants the action (grants are a union). */
	grantedBy: AccessGrantPath[];
	/** Roles that would grant it if assigned in this scope. */
	wouldBeGrantedBy: string[];
	/** Assignments that match the scope but grant nothing (stale or out of scope). */
	ignored: { assignmentId: string; role: string; reason: string }[];
	summary: string;
};

// ---- settings ----

export type RetentionSetting = { name: string; value: string; description: string };

export type ServerSettings = {
	serverVersion: string;
	storage: 'postgresql' | 'sqlite';
	schemaVersion: number;
	latestSchemaVersion: number;
	schemaMigratedAt: Iso;
	authMode: AuthMode;
	schedulerEnabled: boolean;
	retention: RetentionSetting[];
};
