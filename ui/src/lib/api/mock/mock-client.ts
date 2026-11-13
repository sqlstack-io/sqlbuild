import type { ApiClient } from '../client';
import { ApiError } from '../client';
import { LocalChangeBus, type ConnectionState } from '../signals';
import type {
	AdhocRunCheck,
	PlanResult,
	AdhocRunRequest,
	Bootstrap,
	BuiltInRole,
	Capability,
	CurrentUser,
	Job,
	JobDetail,
	JobLaunchOptions,
	NavCounts,
	PlatformEvent,
	QueueEntry,
	RerunMode,
	Run,
	RunDetail,
	RunEventPage,
	RunListQuery,
	RunNodeError,
	RunOutputLine,
	RunStatement,
	Scope,
	Snapshot,
	SnapshotNode,
	Timeline,
	Trigger,
	TriggerDetail,
	TriggerState,
	WarehouseSync
} from '../types';
import { createFixtures, ROLE_CAPABILITIES, type Fixtures } from './fixtures';
import { DEMO_USER } from './principals';
import { MockOperate } from './operate-store';
import type { PlatformApi } from '../platform-client';
import { MockPlatformApi } from './platform';
import { MockBuild } from './build';

export type MockOptions = {
	role?: BuiltInRole;
	/** `default` shows three warnings; `none` is all clear; `all` adds critical events. */
	events?: 'default' | 'none' | 'all';
	/** Simulated request latency in milliseconds. */
	latencyMs?: number;
	/** Simulated state of the live-signal connection. */
	connection?: ConnectionState;
};

/** In-memory `ApiClient` with neutral example data. Mutations persist per page load. */
export class MockApiClient implements ApiClient {
	readonly changes = new LocalChangeBus();

	supports(): boolean {
		return true;
	}
	readonly #options: Required<MockOptions>;
	readonly #fixtures: Fixtures;
	readonly #createdAt = Date.now();
	readonly #operate: MockOperate;
	readonly #syncs = new Map<string, WarehouseSync>();
	readonly #platform: MockPlatformApi;
	readonly #build: MockBuild;

	constructor(options: MockOptions = {}) {
		this.#options = {
			role: options.role ?? 'admin',
			events: options.events ?? 'default',
			latencyMs: options.latencyMs ?? 120,
			connection: options.connection ?? 'live'
		};
		this.changes.setState(this.#options.connection);
		this.#fixtures = createFixtures(this.#createdAt);
		this.#operate = new MockOperate({
			changes: this.changes,
			role: () => this.#options.role,
			can: (capability) => ROLE_CAPABILITIES[this.#options.role].includes(capability),
			user: () => DEMO_USER,
			adapter: (scope) => this.#location(scope).target.adapter ?? 'duckdb',
			target: (scope) => this.#location(scope).target.name,
			reply: (value) => this.#reply(value)
		});
		this.#platform = new MockPlatformApi({
			now: this.#createdAt,
			role: this.#options.role,
			events: this.#options.events,
			serverVersion: '0.185.0',
			user: DEMO_USER,
			locations: this.#fixtures.locations,
			changes: this.changes,
			platformEvents: () => this.#platformEvents(),
			reply: (value) => this.#reply(value)
		});
		this.#build = new MockBuild({
			now: this.#createdAt,
			can: (capability) => ROLE_CAPABILITIES[this.#options.role].includes(capability),
			adapter: (scope) => this.#location(scope).target.adapter ?? 'duckdb',
			reply: (value) => this.#reply(value),
			operate: this.#operate,
			platform: this.#platform,
			platformEvents: () => this.#platformEvents()
		});
	}

	async bootstrap(): Promise<Bootstrap> {
		const user: CurrentUser = { ...DEMO_USER, kind: 'user', role: this.#options.role };
		return this.#reply({
			serverVersion: '0.185.0',
			authMode: 'oidc',
			user,
			capabilities: ROLE_CAPABILITIES[this.#options.role],
			locations: this.#fixtures.locations.map((location) => ({
				...location,
				warehouseSync: this.#sync({ locationId: location.id })
			}))
		});
	}

	async navCounts(scope: Scope): Promise<NavCounts> {
		this.#location(scope);
		return this.#reply({
			catalogNodes: this.#build.nodeCount(),
			failingChecks: this.#build.failingCheckCount(scope),
			activeRuns: this.#operate.activeRunCount(),
			pausedTriggers: this.#operate.pausedTriggerCount(scope),
			activePlatformEvents: this.#platformEvents().filter((e) => e.active).length
		});
	}

	// ---- BUILD: delegated to the build store (build.ts) ----

	overview(...args: Parameters<ApiClient['overview']>) {
		this.#location(args[0]);
		return this.#build.overview(...args);
	}
	listCatalogNodes(...args: Parameters<ApiClient['listCatalogNodes']>) {
		this.#location(args[0]);
		return this.#build.listCatalogNodes(...args);
	}
	getCatalogNode(...args: Parameters<ApiClient['getCatalogNode']>) {
		this.#location(args[0]);
		return this.#build.getCatalogNode(...args);
	}
	getColumnLineage(...args: Parameters<ApiClient['getColumnLineage']>) {
		this.#location(args[0]);
		return this.#build.getColumnLineage(...args);
	}
	listChecks(...args: Parameters<ApiClient['listChecks']>) {
		this.#location(args[0]);
		return this.#build.listChecks(...args);
	}
	getCheck(...args: Parameters<ApiClient['getCheck']>) {
		this.#location(args[0]);
		return this.#build.getCheck(...args);
	}
	async getCheckSql(...args: Parameters<ApiClient['getCheckSql']>) {
		this.#location(args[0]);
		return this.#build.getCheckSql(...args);
	}
	async getCheckEvidence(...args: Parameters<ApiClient['getCheckEvidence']>) {
		this.#location(args[0]);
		return this.#build.getCheckEvidence(...args);
	}
	listSnapshotChanges(...args: Parameters<ApiClient['listSnapshotChanges']>) {
		this.#location(args[0]);
		return this.#build.listSnapshotChanges(...args);
	}

	// ---- end BUILD ----

	// ---- OPERATE: delegated to the operate store (operate-store.ts) ----

	async listRuns(scope: Scope, query: RunListQuery = {}): Promise<Run[]> {
		this.#location(scope);
		return this.#operate.listRuns(scope, query);
	}

	listRunRows(...args: Parameters<ApiClient['listRunRows']>) {
		this.#location(args[0]);
		return this.#operate.listRunRows(...args);
	}

	runTabCounts(...args: Parameters<ApiClient['runTabCounts']>) {
		this.#location(args[0]);
		return this.#operate.runTabCounts(...args);
	}

	cancelMatchingRuns(...args: Parameters<ApiClient['cancelMatchingRuns']>) {
		this.#location(args[0]);
		return this.#operate.cancelMatchingRuns(...args);
	}

	async getRun(scope: Scope, runId: string): Promise<Run | null> {
		this.#location(scope);
		return this.#operate.getRun(scope, runId);
	}

	async getRunDetail(scope: Scope, runId: string): Promise<RunDetail | null> {
		this.#location(scope);
		return this.#operate.getRunDetail(scope, runId);
	}

	async listRunEvents(scope: Scope, runId: string, query?: { after?: number; limit?: number }): Promise<RunEventPage> {
		this.#location(scope);
		return this.#operate.listRunEvents(scope, runId, query);
	}

	async listRunStatements(scope: Scope, runId: string): Promise<RunStatement[]> {
		this.#location(scope);
		return this.#operate.listRunStatements(scope, runId);
	}

	async listRunOutput(scope: Scope, runId: string): Promise<RunOutputLine[]> {
		this.#location(scope);
		return this.#operate.listRunOutput(scope, runId);
	}

	async getRunNodeError(scope: Scope, runId: string, nodeKey: string): Promise<RunNodeError> {
		const node = (await this.getRunDetail(scope, runId))?.nodes.find((n) => n.runId === runId && n.nodeKey === nodeKey);
		if (!node) throw new ApiError('node not found in this run', 404);
		return { nodeKey, errorCode: node.errorCode, message: node.errorDetail ?? node.errorMessage, file: node.errorLocation?.file ?? null };
	}

	async listQueue(scope: Scope): Promise<QueueEntry[]> {
		this.#location(scope);
		return this.#operate.listQueue(scope);
	}

	async timeline(scope: Scope, window: { since: string; until: string }): Promise<Timeline> {
		this.#location(scope);
		return this.#operate.timeline(scope, window);
	}

	async cancelRun(scope: Scope, runId: string): Promise<Run> {
		this.#location(scope);
		return this.#operate.cancelRun(scope, runId);
	}

	async acknowledgeRun(scope: Scope, runId: string, acknowledged: boolean): Promise<void> {
		this.#location(scope);
		this.#operate.acknowledgeRun(runId, acknowledged, 'you');
	}

	async rerunRun(scope: Scope, runId: string, mode: Exclude<RerunMode, 'selected'>): Promise<Run> {
		this.#location(scope);
		return this.#operate.rerunRun(scope, runId, mode);
	}

	async rerunSelectedNodes(scope: Scope, runId: string, nodes: string[]): Promise<Run> {
		this.#location(scope);
		return this.#operate.rerunRun(scope, runId, 'selected', nodes);
	}

	getStatementSql(...args: Parameters<ApiClient['getStatementSql']>) {
		this.#location(args[0]);
		return this.#operate.getStatementSql(...args);
	}

	async listSnapshotNodes(scope: Scope): Promise<SnapshotNode[]> {
		this.#location(scope);
		return this.#operate.listSnapshotNodes(scope);
	}

	async checkAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<AdhocRunCheck> {
		this.#location(scope);
		return this.#operate.checkAdhocRun(scope, request);
	}

	async launchAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<Run> {
		this.#location(scope);
		return this.#operate.launchAdhocRun(scope, request);
	}

	async planBuild(scope: Scope, command: string[]): Promise<PlanResult> {
		this.#location(scope);
		return this.#operate.planBuild(scope, command);
	}

	async listJobs(scope: Scope): Promise<Job[]> {
		this.#location(scope);
		return this.#operate.listJobs(scope);
	}

	async getJob(scope: Scope, key: string): Promise<JobDetail | null> {
		this.#location(scope);
		return this.#operate.getJob(scope, key);
	}

	async launchJob(scope: Scope, key: string, options?: JobLaunchOptions): Promise<Run> {
		this.#location(scope);
		return this.#operate.launchJob(scope, key, options);
	}

	async launchJobFromScratch(scope: Scope, key: string, options?: JobLaunchOptions): Promise<Run> {
		this.#location(scope);
		return this.#operate.launchJob(scope, key, { ...options, fullRefresh: true });
	}

	async listTriggers(scope: Scope): Promise<Trigger[]> {
		this.#location(scope);
		return this.#operate.listTriggers(scope);
	}

	async getTrigger(scope: Scope, id: string): Promise<TriggerDetail | null> {
		this.#location(scope);
		return this.#operate.getTrigger(scope, id);
	}

	async setTriggerStates(scope: Scope, ids: string[], state: TriggerState, reason?: string | null): Promise<Trigger[]> {
		this.#location(scope);
		return this.#operate.setTriggerStates(scope, ids, state, reason ?? null);
	}

	async resetTriggerToDefault(scope: Scope, id: string): Promise<Trigger> {
		this.#location(scope);
		return this.#operate.resetTriggerToDefault(scope, id);
	}

	async resetSensorCursor(scope: Scope, id: string, reason?: string | null): Promise<TriggerDetail> {
		this.#location(scope);
		return this.#operate.resetSensorCursor(scope, id, reason ?? null);
	}

	// ---- end OPERATE ----

	async listPlatformEvents(query: { activeOnly: boolean }): Promise<PlatformEvent[]> {
		const events = this.#platformEvents();
		return this.#reply(query.activeOnly ? events.filter((e) => e.active) : events);
	}

	async getLatestSnapshot(scope: Scope): Promise<Snapshot | null> {
		return this.#reply(this.#location(scope).snapshot ?? null);
	}

	async getWarehouseSync(scope: Scope): Promise<WarehouseSync> {
		this.#location(scope);
		return this.#reply({ ...this.#sync(scope) });
	}

	async requestWarehouseSync(scope: Scope): Promise<void> {
		this.#require('connection.test');
		const sync = this.#sync(scope);
		setTimeout(() => {
			const now = new Date().toISOString();
			Object.assign(sync, { lastSyncedAt: now, lastError: null, lastReportedAt: now });
			this.changes.emit(`warehouse_sync:${scope.locationId}`);
		}, 1500);
		return this.#reply(undefined);
	}

	// ---- platform and admin: delegated to MockPlatformApi (mock/platform.ts) ----

	platformHealth(...args: Parameters<PlatformApi['platformHealth']>) {
		return this.#platform.platformHealth(...args);
	}
	listPlatformEventHistory(...args: Parameters<PlatformApi['listPlatformEventHistory']>) {
		return this.#platform.listPlatformEventHistory(...args);
	}
	listLocations(...args: Parameters<PlatformApi['listLocations']>) {
		return this.#platform.listLocations(...args);
	}
	getLocation(...args: Parameters<PlatformApi['getLocation']>) {
		return this.#platform.getLocation(...args);
	}
	testConnection(...args: Parameters<PlatformApi['testConnection']>) {
		return this.#platform.testConnection(...args);
	}
	listLocationTokens(...args: Parameters<PlatformApi['listLocationTokens']>) {
		return this.#platform.listLocationTokens(...args);
	}
	createLocationToken(...args: Parameters<PlatformApi['createLocationToken']>) {
		return this.#platform.createLocationToken(...args);
	}
	rotateLocationToken(...args: Parameters<PlatformApi['rotateLocationToken']>) {
		return this.#platform.rotateLocationToken(...args);
	}
	revokeLocationToken(...args: Parameters<PlatformApi['revokeLocationToken']>) {
		return this.#platform.revokeLocationToken(...args);
	}
	listWarehouseConnections() {
		return this.#platform.listWarehouseConnections();
	}
	listProviders(...args: Parameters<PlatformApi['listProviders']>) {
		return this.#platform.listProviders(...args);
	}
	maintenanceHistory(...args: Parameters<PlatformApi['maintenanceHistory']>) {
		return this.#platform.maintenanceHistory(...args);
	}
	listAccounts(...args: Parameters<PlatformApi['listAccounts']>) {
		return this.#platform.listAccounts(...args);
	}
	disableAccount(...args: Parameters<PlatformApi['disableAccount']>) {
		return this.#platform.disableAccount(...args);
	}
	enableAccount(...args: Parameters<PlatformApi['enableAccount']>) {
		return this.#platform.enableAccount(...args);
	}
	listRoles(...args: Parameters<PlatformApi['listRoles']>) {
		return this.#platform.listRoles(...args);
	}
	listRoleAssignments(...args: Parameters<PlatformApi['listRoleAssignments']>) {
		return this.#platform.listRoleAssignments(...args);
	}
	createRoleAssignment(...args: Parameters<PlatformApi['createRoleAssignment']>) {
		return this.#platform.createRoleAssignment(...args);
	}
	deleteRoleAssignment(...args: Parameters<PlatformApi['deleteRoleAssignment']>) {
		return this.#platform.deleteRoleAssignment(...args);
	}
	listAuditEntries(...args: Parameters<PlatformApi['listAuditEntries']>) {
		return this.#platform.listAuditEntries(...args);
	}
	explainAccess(...args: Parameters<PlatformApi['explainAccess']>) {
		return this.#platform.explainAccess(...args);
	}
	getRole(...args: Parameters<PlatformApi['getRole']>) {
		return this.#platform.getRole(...args);
	}
	listSsoMappings(...args: Parameters<PlatformApi['listSsoMappings']>) {
		return this.#platform.listSsoMappings(...args);
	}
	lookupDecision(...args: Parameters<PlatformApi['lookupDecision']>) {
		return this.#platform.lookupDecision(...args);
	}
	effectiveAccess(...args: Parameters<PlatformApi['effectiveAccess']>) {
		return this.#platform.effectiveAccess(...args);
	}
	listAccessOverview(...args: Parameters<PlatformApi['listAccessOverview']>) {
		return this.#platform.listAccessOverview(...args);
	}
	serverSettings(...args: Parameters<PlatformApi['serverSettings']>) {
		return this.#platform.serverSettings(...args);
	}

	// ---- end platform and admin ----

	#sync(scope: Scope): WarehouseSync {
		const key = scopeKey(scope);
		let sync = this.#syncs.get(key);
		if (!sync) {
			sync = { ...this.#location(scope).warehouseSync };
			this.#syncs.set(key, sync);
		}
		return sync;
	}

	#platformEvents(): PlatformEvent[] {
		const { platformEvents, extraPlatformEvents } = this.#fixtures;
		if (this.#options.events === 'none') return platformEvents.filter((e) => !e.active);
		if (this.#options.events === 'all') return [...extraPlatformEvents, ...platformEvents];
		return platformEvents;
	}

	#location(scope: Scope) {
		const location = this.#fixtures.locations.find((l) => l.id === scope.locationId);
		if (!location) throw new ApiError(`Unknown location: ${scope.locationId}`, 404);
		return location;
	}

	#require(capability: Capability) {
		if (!ROLE_CAPABILITIES[this.#options.role].includes(capability)) {
			throw new ApiError(`Missing permission: ${capability}`, 403);
		}
	}

	#reply<T>(value: T): Promise<T> {
		return new Promise((resolve) => setTimeout(() => resolve(value), this.#options.latencyMs));
	}
}

function scopeKey(scope: Scope): string {
	return scope.locationId;
}
