import { recordPlanRows } from './plan-rows-log';
import type { ApiClient } from '../client';
import { ApiError } from '../client';
import { LocalChangeBus, type ConnectionState } from '../signals';
import { stateOf } from '../node-states';
import type {
	AuthMode,
	AdhocRunCheck,
	AttentionAcknowledgement,
	AutomaticWithdrawal,
	ConditionShape,
	SelectionEvaluation,
	SelectionRequest,
	SelectionVocabulary,
	PlanResult,
	PlanPage,
	PlanDiff,
	PlanPageSection,
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
	NodeOutputIndex,
	NodeOutputPage,
	NodeOutputSearch,
	CheckSample,
	RunStatement,
	RunCostRow,
	IgnoredAuditFailure,
	CostSummary,
	NodeBuildCost,
	NodeState,
	NodeStates,
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
	/** Runs never advance on their own (`?still`), for the browser suite's work budgets. */
	still?: boolean;
	/** Simulated request latency in milliseconds. */
	latencyMs?: number;
	/** Simulated state of the live-signal connection. */
	connection?: ConnectionState;
	/** How people sign in (`?signin=password`); SSO by default. */
	signIn?: AuthMode;
	/** `none` starts with no service accounts (`?serviceaccounts=none`). */
	serviceAccounts?: 'default' | 'none';
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
	/** Node states as last read or announced, by location, so a finished run announces only what changed. */
	readonly #nodeStates = new Map<string, { version: number; states: Record<string, NodeState> }>();
	#announcing: Promise<void> = Promise.resolve();

	constructor(options: MockOptions = {}) {
		this.#options = {
			role: options.role ?? 'admin',
			events: options.events ?? 'default',
			latencyMs: options.latencyMs ?? 120,
			connection: options.connection ?? 'live',
			still: options.still ?? false,
			signIn: options.signIn ?? 'oidc',
			serviceAccounts: options.serviceAccounts ?? 'default'
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
			reply: (value) => this.#reply(value),
			still: this.#options.still
		});
		this.#platform = new MockPlatformApi({
			now: this.#createdAt,
			role: this.#options.role,
			events: this.#options.events,
			serverVersion: '0.185.0',
			authMode: this.#options.signIn,
			user: DEMO_USER,
			serviceAccounts: this.#options.serviceAccounts,
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
		// As the server does: when runs or checks change, the node states that changed go out on `node_status`.
		this.changes.subscribe((signal) => {
			if (signal.channel === 'runs' || signal.channel === 'checks') this.#announcing = this.#announcing.then(() => this.#announceNodeStates());
		});
	}

	async #currentStates(scope: Scope): Promise<Record<string, NodeState>> {
		const nodes = await this.#build.listCatalogNodes(scope);
		return Object.fromEntries(nodes.map((n) => [n.key, stateOf(n)]));
	}

	async #announceNodeStates(): Promise<void> {
		for (const [locationId, held] of this.#nodeStates) {
			const current = await this.#currentStates({ locationId });
			const changed = Object.fromEntries(Object.entries(current).filter(([key, state]) => JSON.stringify(held.states[key]) !== JSON.stringify(state)));
			if (!Object.keys(changed).length) continue;
			const previousVersion = held.version;
			this.#nodeStates.set(locationId, { version: previousVersion + 1, states: current });
			this.changes.emit(`node_status:${locationId}`, { version: previousVersion + 1, previousVersion, states: changed });
		}
	}

	async bootstrap(): Promise<Bootstrap> {
		const user: CurrentUser = { ...DEMO_USER, kind: 'user', role: this.#options.role };
		return this.#reply({
			serverVersion: '0.185.0',
			authMode: this.#options.signIn,
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
		return this.#build.overview(args[0]);
	}
	listCatalogNodes(...args: Parameters<ApiClient['listCatalogNodes']>) {
		this.#location(args[0]);
		return this.#build.listCatalogNodes(...args);
	}
	/** The first read starts the location's versions; later runs announce changes against it. */
	async listNodeStates(scope: Scope): Promise<NodeStates> {
		const location = this.#location(scope);
		await this.#announcing;
		const states = await this.#currentStates(scope);
		const held = this.#nodeStates.get(scope.locationId);
		const version = held && JSON.stringify(held.states) === JSON.stringify(states) ? held.version : (held?.version ?? 0) + 1;
		this.#nodeStates.set(scope.locationId, { version, states });
		// The demo's location plans what changed: each changed node, and every node reading one of them.
		const nodes = await this.#build.listCatalogNodes(scope);
		const changed = nodes.filter((n) => states[n.key]?.definitionChanged).map((n) => n.key);
		const downstream = nodes.filter((n) => !changed.includes(n.key) && n.upstream.some((u) => changed.includes(u))).map((n) => n.key);
		return { snapshotId: location.snapshot?.id ?? 'unpublished', version, states, changedBuild: { generation: 1, changed, downstream, failing: null } };
	}
	getCatalogNode(...args: Parameters<ApiClient['getCatalogNode']>) {
		this.#location(args[0]);
		return this.#build.getCatalogNode(...args);
	}
	getColumnLineage(...args: Parameters<ApiClient['getColumnLineage']>) {
		this.#location(args[0]);
		return this.#build.getColumnLineage(args[0], args[1]);
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

	async listRunIgnoredAudits(scope: Scope, runId: string): Promise<IgnoredAuditFailure[]> {
		this.#location(scope);
		return this.#build.listRunIgnoredAudits(scope, runId);
	}

	async listRunCost(scope: Scope, runId: string): Promise<RunCostRow[]> {
		this.#location(scope);
		return this.#operate.listRunCost(scope, runId);
	}

	async getCostSummary(scope: Scope, days: number): Promise<CostSummary> {
		this.#location(scope);
		return this.#operate.getCostSummary(scope, days);
	}

	async listNodeCosts(scope: Scope, nodeKey: string, days: number): Promise<NodeBuildCost[]> {
		this.#location(scope);
		return this.#operate.listNodeCosts(scope, nodeKey, days);
	}

	async listRunOutput(scope: Scope, runId: string): Promise<RunOutputLine[]> {
		this.#location(scope);
		return this.#operate.listRunOutput(scope, runId);
	}

	// A node's bulk output, paged as the output store would serve it (prototype, CHI-1597).
	async getNodeOutputIndex(scope: Scope, runId: string, nodeKey: string): Promise<NodeOutputIndex | null> {
		this.#location(scope);
		return this.#operate.getNodeOutputIndex(scope, runId, nodeKey);
	}
	async getNodeOutputPage(scope: Scope, runId: string, nodeKey: string, at: { from: number; rows: number; direction: 'forward' | 'backward'; collapse: boolean; stream?: 'all' | 'stdout' | 'stderr'; around?: number }): Promise<NodeOutputPage> {
		this.#location(scope);
		return this.#operate.getNodeOutputPage(scope, runId, nodeKey, at);
	}
	async searchNodeOutput(scope: Scope, runId: string, nodeKey: string, query: string, from: number, direction: 'forward' | 'backward'): Promise<NodeOutputSearch> {
		this.#location(scope);
		return this.#operate.searchNodeOutput(scope, runId, nodeKey, query, from, direction);
	}
	nodeOutputDownloadUrl(scope: Scope, runId: string, nodeKey: string): string {
		return this.#operate.nodeOutputDownloadUrl(scope, runId, nodeKey);
	}
	async getCheckSample(scope: Scope, runId: string, statementId: string): Promise<CheckSample | null> {
		this.#location(scope);
		return this.#operate.getCheckSample(scope, runId, statementId);
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

	/** The conditions' acknowledgements the mock holds, for wrappers that replace a condition, such as the huge project. */
	mockConditionAcknowledgements(): AttentionAcknowledgement[] {
		return this.#operate.conditionAcknowledgements();
	}

	async acknowledgeCondition(scope: Scope, key: string, shape: ConditionShape, acknowledged: boolean, automatic?: AutomaticWithdrawal): Promise<void> {
		this.#location(scope);
		this.#operate.acknowledgeCondition(key, shape, acknowledged, 'you', automatic?.withdraws);
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

	async evaluateSelection(scope: Scope, request: SelectionRequest): Promise<SelectionEvaluation> {
		this.#location(scope);
		return this.#operate.evaluateSelection(scope, request);
	}

	async selectionVocabulary(scope: Scope): Promise<SelectionVocabulary> {
		this.#location(scope);
		return this.#operate.selectionVocabulary(scope);
	}

	async checkAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<AdhocRunCheck> {
		this.#location(scope);
		return this.#operate.checkAdhocRun(scope, request);
	}

	async launchAdhocRun(scope: Scope, request: AdhocRunRequest): Promise<Run> {
		this.#location(scope);
		return this.#operate.launchAdhocRun(scope, request);
	}

	async planBuild(scope: Scope, command: string[], _options: { fresh?: boolean } = {}): Promise<PlanResult> {
		this.#location(scope);
		// `--changed` starts from the nodes whose own definition changed, as the catalog marks them.
		const changed = command.some((t) => t === '--changed' || t.startsWith('--changed='))
			? new Map((await this.#build.listCatalogNodes(scope)).filter((n) => n.stale?.some((r) => r.reason === 'changed')).map((n) => [n.key, 'definition changed']))
			: null;
		return this.#operate.planBuild(scope, command, changed);
	}

	/** The mock plans whole documents, so no section is paged; `?large` serves a paged plan (large-project.ts). */
	async planRows(scope: Scope, requestId: string, section: PlanPageSection, offset: number, limit?: number): Promise<PlanPage> {
		this.#location(scope);
		recordPlanRows(section, offset, limit);
		void requestId;
		return { total: 0, section_total: 0, rows: [], runs: [], warnings: [], cursor_window_starts: {} };
	}

	/** The mock's plans carry their diffs whole, so none is read apart. */
	async planDiff(scope: Scope, requestId: string, model: string): Promise<PlanDiff | null> {
		this.#location(scope);
		void requestId;
		void model;
		return null;
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

	async skipSensorEvent(scope: Scope, id: string, eventId: string, reason: string): Promise<TriggerDetail> {
		this.#location(scope);
		return this.#operate.skipSensorEvent(scope, id, eventId, reason);
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
	createPasswordAccount(...args: Parameters<PlatformApi['createPasswordAccount']>) {
		return this.#platform.createPasswordAccount(...args);
	}
	listServiceAccounts(...args: Parameters<PlatformApi['listServiceAccounts']>) {
		return this.#platform.listServiceAccounts(...args);
	}
	createServiceAccount(...args: Parameters<PlatformApi['createServiceAccount']>) {
		return this.#platform.createServiceAccount(...args);
	}
	rotateServiceAccountKey(...args: Parameters<PlatformApi['rotateServiceAccountKey']>) {
		return this.#platform.rotateServiceAccountKey(...args);
	}
	revokeServiceAccountKey(...args: Parameters<PlatformApi['revokeServiceAccountKey']>) {
		return this.#platform.revokeServiceAccountKey(...args);
	}
	setServiceAccountDisabled(...args: Parameters<PlatformApi['setServiceAccountDisabled']>) {
		return this.#platform.setServiceAccountDisabled(...args);
	}
	setServiceAccountRole(...args: Parameters<PlatformApi['setServiceAccountRole']>) {
		return this.#platform.setServiceAccountRole(...args);
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
