// Wire types for the Platform (Locations with server status, Connections,
// Providers, Maintenance) and Admin (users, service accounts, roles, audit,
// settings) areas.

import type {
	AuthMode,
	BuiltInRole,
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
	/** Null when the move was discovered from the warehouse rather than declared. */
	originModel: string | null;
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

/** How much an action can do: look, operate runs, change warehouse data, or administer access. */
export type PermissionRisk = 'read' | 'operate' | 'changes_data' | 'administer';

/**
 * Actions on nodes. `nodes` entries are selectors (union); a comma inside an
 * entry intersects, e.g. `tag:finance,group:marts`. `*` is every node.
 */
export type RoleGrant = { nodes: string[]; permissions: Capability[] };

export type RoleSource = 'built_in' | 'server_config' | 'project';
export type RoleDefinition = {
	name: string;
	source: RoleSource;
	/** Where the role is defined, `path:line`; null for built-in roles. */
	definedIn: string | null;
	description: string;
	/** The built-in role a custom role starts from (flat: no deeper inheritance). Null for built-in roles. */
	extends: BuiltInRole | null;
	/** Built-in roles: their actions on every node. Custom roles: the additions on top of the base role. Approved grants only. */
	grants: RoleGrant[];
	/** A change giving the role more is waiting for approval. */
	pendingApprovalId: string | null;
	/** People and service accounts holding the role somewhere. */
	holders: number;
	/** Nodes the role's selectors started matching in the last 7 days (no approval needed). */
	recentlyMatched: number;
};

/** What one selector of a role matches in the current snapshot. */
export type SelectorMatch = {
	selector: string;
	nodes: string[];
	/** Nodes matched now that weren't 7 days ago. */
	newSinceLastWeek: string[];
};

export type RoleDetail = RoleDefinition & {
	/** The base role's actions on every node; empty for built-in roles. */
	baseGrants: RoleGrant[];
	selectors: SelectorMatch[];
	assignments: RoleAssignment[];
	/** The role's definition as written, read-only. */
	definition: string;
};

/** `locationId: null` applies everywhere. */
export type AssignmentScope = { locationId: string | null };

/**
 * `manual`: given in this UI. `sso`: from an SSO group mapping in the server
 * config. `policy`: declared in a policy file. `built_in`: the server's
 * default role for everyone who signs in. Only manual ones can be removed here.
 */
export type AssignmentSource = 'manual' | 'sso' | 'policy' | 'built_in';

export type RoleAssignment = {
	id: string;
	principal: Principal;
	role: string;
	scope: AssignmentScope;
	source: AssignmentSource;
	/** For SSO-derived assignments: the mapped group. */
	ssoGroup: string | null;
	/** For policy and built-in assignments: where they are set, `path:line`. */
	definedIn: string | null;
	assignedBy: Principal | null;
	assignedAt: Iso;
	/** The role no longer exists in code; the assignment shows but grants nothing. */
	stale: boolean;
};

/** You can only give roles you hold in that scope. */
export type RoleAssignmentRequest = { principal: string; role: string; scope: AssignmentScope };

/** An SSO group mapped to a role in the server config. Read-only. */
export type SsoMapping = {
	group: string;
	role: string;
	scope: AssignmentScope;
	definedIn: string;
	/** Members seen when they last signed in. Zero when nobody in the group has signed in yet. */
	membersSynced: number;
	lastSyncedAt: Iso | null;
};

/** People who gain one action on one selector if a change is approved. */
export type ApprovalImpact = {
	action: Capability;
	selector: string;
	locationId: string;
	principals: Principal[];
};

export type ApprovalState = 'pending' | 'approved' | 'rejected' | 'superseded';
export type RoleApproval = {
	id: string;
	role: string;
	locationId: string;
	snapshotGeneration: number;
	gitRevision: string;
	gitAuthor: string;
	/** The account the commit author signs in as, when known; they can't approve it. */
	gitAuthorAccount: string | null;
	proposedAt: Iso;
	/** Grants added or widened by the proposal. */
	added: RoleGrant[];
	/** Grants removed or narrowed by the same deploy; these already apply. */
	removed: RoleGrant[];
	/** The definition in force and as proposed. */
	definitionBefore: string;
	definitionAfter: string;
	/** Who gains what if approved; empty once decided. */
	impact: ApprovalImpact[];
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
	/** Denials: the decision ID returned in the permission error, for the access checker. */
	decisionId: string | null;
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

// ---- access checker ----

export type AccessQuery = {
	principal: string;
	action: Capability;
	locationId: string;
	/** Optional job, group, node or node selector the action applies to. */
	subject: string | null;
};

/** One assignment → role → grant → scope chain that allows the action. */
export type AccessGrantPath = {
	assignmentId: string;
	assignmentSource: AssignmentSource;
	ssoGroup: string | null;
	definedIn: string | null;
	scope: AssignmentScope;
	role: string;
	/** `base`: from the built-in role the role extends (or is); `addition`: one of the role's additions. */
	via: 'base' | 'addition';
	baseRole: BuiltInRole | null;
	/** The grant's selector; null when it covers every node. */
	selector: string | null;
};

export type AccessNearMissReason = 'other_nodes' | 'other_location' | 'stale' | 'disabled' | 'admin_only';
/** The closest thing they have that doesn't cover this request. */
export type AccessNearMiss = {
	reason: AccessNearMissReason;
	role: string | null;
	assignmentId: string | null;
	/** Plain sentence, e.g. "Has Run jobs on group:reporting, not on daily_revenue". */
	detail: string;
};

export type AccessExplanation = {
	query: AccessQuery;
	allowed: boolean;
	/** Every chain that allows the action (grants add up across roles). */
	grantedBy: AccessGrantPath[];
	nearMisses: AccessNearMiss[];
	/** A change waiting for approval would allow it. */
	pending: { approvalId: string; role: string } | null;
	/** Roles that would allow it if assigned here. */
	wouldBeGrantedBy: string[];
	summary: string;
};

/** A denied request, looked up by the decision ID from its error message. */
export type AccessDecision = {
	decisionId: string;
	at: Iso;
	query: AccessQuery;
	summary: string;
};

/** One action at one location in someone's effective access. */
export type EffectiveAccessCell = {
	action: Capability;
	locationId: string;
	/** `all`: every node; `some`: only the listed selectors. */
	coverage: 'all' | 'some' | 'none';
	selectors: string[];
	roles: string[];
};

export type EffectiveAccess = {
	principal: string;
	locationIds: string[];
	cells: EffectiveAccessCell[];
};

/** Per-account access facts for the People and Service accounts tables. */
export type AccessOverview = {
	principal: string;
	/** Actions that change data or administer access, anywhere. */
	riskyActions: Capability[];
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
