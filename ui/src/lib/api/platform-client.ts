import type {
	AccessDecision,
	AccessExplanation,
	AccessOverview,
	AccessQuery,
	Account,
	ApiToken,
	ApiTokenRequest,
	AuditEntry,
	AuditQuery,
	ConnectionTest,
	IssuedToken,
	LocationDetail,
	LocationSummary,
	LocationToken,
	MaintenanceHistory,
	PlatformEventQuery,
	PlatformEventRecord,
	PlatformHealth,
	ProviderConnection,
	RoleApproval,
	RoleAssignment,
	RoleAssignmentRequest,
	RoleDefinition,
	RoleDetail,
	EffectiveAccess,
	Scope,
	ServerSettings,
	ServiceAccountRequest,
	SsoMapping,
	WarehouseConnection
} from './types';

/**
 * Platform (server status, locations, connections, providers, maintenance)
 * and admin (users, service accounts, roles, audit, settings) endpoints.
 * Part of `ApiClient`.
 *
 * Mutations are audited server-side; the capability noted on each is the
 * server's check, mirrored in the UI only to hide or disable controls.
 */
export interface PlatformApi {
	// ---- server status and problems (shown on Locations) ----
	/** Scheduler, server replicas, heartbeat and event forwarding. */
	platformHealth(): Promise<PlatformHealth>;
	/** Problems recorded as platform events, newest first; by location, or server-wide with `locationId: null`. */
	listPlatformEventHistory(query?: PlatformEventQuery): Promise<PlatformEventRecord[]>;

	// ---- locations ----
	listLocations(): Promise<LocationSummary[]>;
	getLocation(locationId: string): Promise<LocationDetail | null>;
	/** Runs `sqb debug`'s connection test in the location. `connection.test` */
	testConnection(scope: Scope, connection: string): Promise<ConnectionTest>;
	/** `location.manage` */
	listLocationTokens(locationId: string): Promise<LocationToken[]>;
	/** `location.manage` */
	createLocationToken(locationId: string): Promise<IssuedToken<LocationToken>>;
	/** Issues a new token and revokes the old one. `location.manage` */
	rotateLocationToken(tokenId: string): Promise<IssuedToken<LocationToken>>;
	/** `location.manage` */
	revokeLocationToken(tokenId: string): Promise<LocationToken>;

	// ---- warehouse connections and providers ----
	/** Warehouse connections of the selected target. */
	listWarehouseConnections(scope: Scope): Promise<WarehouseConnection[]>;
	/** Providers declared in the project, with where each one is used. Read-only. */
	listProviders(scope: Scope): Promise<ProviderConnection[]>;

	// ---- maintenance ----
	maintenanceHistory(scope: Scope): Promise<MaintenanceHistory>;

	// ---- people, service accounts and their tokens ----
	/** `user.manage` */
	listAccounts(): Promise<Account[]>;
	/** Blocks sign-in and revokes the account's sessions. You can't disable yourself. `user.manage` */
	disableAccount(name: string): Promise<Account>;
	/** Lets a disabled account sign in again. Sessions revoked on disable stay revoked; its unrevoked API tokens work again. `user.manage` */
	enableAccount(name: string): Promise<Account>;
	/** A non-person account for scripts and CI; it signs in with API tokens only. `user.manage` */
	createServiceAccount(request: ServiceAccountRequest): Promise<Account>;
	/** Tokens of every service account; people have none. `token.manage` */
	listServiceAccountTokens(): Promise<ApiToken[]>;
	/** Issues a token for a service account; the secret is returned once. `token.manage` */
	createServiceAccountToken(account: string, request: ApiTokenRequest): Promise<IssuedToken<ApiToken>>;
	/** Issues a new token with the same label and revokes the old one. `token.manage` */
	rotateServiceAccountToken(tokenId: string): Promise<IssuedToken<ApiToken>>;
	/** `token.manage` */
	revokeServiceAccountToken(tokenId: string): Promise<ApiToken>;

	// ---- roles and approvals ----
	/** Roles available in a location: built-in, server config and the location's project roles. `role.assign` */
	listRoles(locationId: string): Promise<RoleDefinition[]>;
	/** A role with what its selectors match today and who holds it. `role.assign` */
	getRole(locationId: string, name: string): Promise<RoleDetail | null>;
	/** `role.assign` */
	listRoleAssignments(): Promise<RoleAssignment[]>;
	/** You can only give a role you hold in that scope. `role.assign` */
	createRoleAssignment(request: RoleAssignmentRequest): Promise<RoleAssignment>;
	/** Manual assignments only; SSO, policy and built-in ones follow their source. `role.assign` */
	deleteRoleAssignment(assignmentId: string): Promise<void>;
	/** SSO group mappings from the server config. Read-only. `role.assign` */
	listSsoMappings(): Promise<SsoMapping[]>;
	/** `role.assign` */
	listRoleApprovals(): Promise<RoleApproval[]>;
	/** A comment is required. You can't approve a change you committed. `role.assign` */
	decideRoleApproval(approvalId: string, decision: 'approve' | 'reject', note: string): Promise<RoleApproval>;

	// ---- audit and access ----
	/** `audit.view` */
	listAuditEntries(query?: AuditQuery): Promise<AuditEntry[]>;
	/** Explaining another account requires `role.assign` or `audit.view`. */
	explainAccess(query: AccessQuery): Promise<AccessExplanation>;
	/** Looks up a denial by the decision ID in its error message. Your own, or anyone's with `audit.view`. */
	lookupDecision(decisionId: string): Promise<AccessDecision>;
	/** Every action × location for one account. Another account requires `role.assign` or `audit.view`. */
	effectiveAccess(principal: string): Promise<EffectiveAccess>;
	/** Risky actions per account, for the People and Service accounts tables. `user.manage` or `role.assign` */
	listAccessOverview(): Promise<AccessOverview[]>;

	// ---- settings ----
	serverSettings(): Promise<ServerSettings>;
}
