// Mock data and behaviour for the Platform and Admin areas. Neutral example
// data only: a fictional waffle shop and its analytics team.

import { ApiError } from '../client';
import type { PlatformApi } from '../platform-client';
import type { LocalChangeBus } from '../signals';
import type {
	AccessExplanation,
	AccessGrantPath,
	AccessQuery,
	Account,
	ApiToken,
	ApiTokenRequest,
	AuditEntry,
	AuditQuery,
	BuiltInRole,
	Capability,
	ConnectionTest,
	IssuedToken,
	JanitorEvent,
	Location,
	LocationDetail,
	LocationSummary,
	LocationToken,
	MaintenanceHistory,
	MigrationEvent,
	OldNameView,
	PlatformEvent,
	PlatformEventLink,
	PlatformEventReaction,
	PlatformEventQuery,
	PlatformEventRecord,
	PlatformHealth,
	Principal,
	ProviderConnection,
	RelationName,
	RoleApproval,
	RoleAssignment,
	RoleAssignmentRequest,
	RoleDefinition,
	RoleGrant,
	Scope,
	ServerSettings,
	ServiceAccountRequest,
	SettingField,
	WarehouseConnection
} from '../types';
import { LOCATION_MARKETING, LOCATION_SHOP, LOCATION_SHOP_DEV, LOCATION_SHOP_STAGING, ROLE_CAPABILITIES, SHOP_LOCATIONS } from './fixtures';
import { triggerId } from './operate';
import { ALEX, CI_ACCOUNT, DEMO_USER, JORDAN, OPS_USER, REPORTING, RILEY, SAM, SYSTEM } from './principals';

const MIN = 60_000;
const DAY = 24 * 60 * MIN;

const ADMIN_ONLY: readonly Capability[] = ['location.manage', 'user.manage', 'role.assign', 'token.manage', 'audit.view'];

/** Mock-only selector membership for node-scoped grants. */
const SELECTOR_NODES: Record<string, string[]> = {
	'group:finance': ['daily_revenue', 'fact_orders', 'revenue_refresh', 'stg_payments'],
	'tag:pii': ['dim_customers', 'stg_customers', 'fact_orders'],
	'group:support': ['support_tickets', 'ticket_backlog']
};

export type MockPlatformContext = {
	now: number;
	role: BuiltInRole;
	events: 'default' | 'none' | 'all';
	serverVersion: string;
	user: Principal;
	locations: Location[];
	changes: LocalChangeBus;
	platformEvents: () => PlatformEvent[];
	reply: <T>(value: T) => Promise<T>;
};

function relation(schema: string, name: string): RelationName {
	return { database: 'WAFFLE_SHOP', schema, name };
}

function randomHex(length: number): string {
	const bytes = new Uint8Array(Math.ceil(length / 2));
	crypto.getRandomValues(bytes);
	return [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('').slice(0, length);
}

function matchesSelector(selector: string, node: string): boolean {
	if (selector === '*') return true;
	return selector.split(',').every((part) => (SELECTOR_NODES[part] ?? [part]).includes(node));
}

function createPlatformFixtures(now: number) {
	const ago = (minutes: number) => new Date(now - minutes * MIN).toISOString();
	const ahead = (minutes: number) => new Date(now + minutes * MIN).toISOString();
	const days = (n: number) => n * 24 * 60;

	// ---- accounts ----
	const accounts: Account[] = [
		{ ...DEMO_USER, kind: 'user', email: 'demo@example.com', source: 'oidc', ssoGroups: ['data-platform'], createdAt: ago(days(210)), lastSeenAt: ago(0.1), disabled: false, activeSessions: 2, owner: null, description: null },
		{ ...ALEX, kind: 'user', email: 'alex@example.com', source: 'oidc', ssoGroups: ['analytics-eng'], createdAt: ago(days(180)), lastSeenAt: ago(25), disabled: false, activeSessions: 1, owner: null, description: null },
		{ ...SAM, kind: 'user', email: 'sam@example.com', source: 'oidc', ssoGroups: ['analytics-eng', 'finance-analysts'], createdAt: ago(days(95)), lastSeenAt: ago(190), disabled: false, activeSessions: 1, owner: null, description: null },
		{ ...JORDAN, kind: 'user', email: 'jordan@example.com', source: 'password', ssoGroups: [], createdAt: ago(days(40)), lastSeenAt: ago(days(3)), disabled: false, activeSessions: 0, owner: null, description: null },
		{ ...OPS_USER, kind: 'user', email: 'ops-oncall@example.com', source: 'oidc', ssoGroups: ['data-platform'], createdAt: ago(days(150)), lastSeenAt: ago(1.5), disabled: false, activeSessions: 3, owner: null, description: null },
		{ ...RILEY, kind: 'user', email: 'riley@example.com', source: 'oidc', ssoGroups: ['support-ops'], createdAt: ago(days(300)), lastSeenAt: ago(days(61)), disabled: true, activeSessions: 0, owner: null, description: null },
		{ ...CI_ACCOUNT, kind: 'service_account', email: null, source: 'service_account', ssoGroups: [], createdAt: ago(days(200)), lastSeenAt: ago(42), disabled: false, activeSessions: 0, owner: 'data-platform', description: 'Deploys snapshots from CI' },
		{ ...REPORTING, kind: 'service_account', email: null, source: 'service_account', ssoGroups: [], createdAt: ago(days(120)), lastSeenAt: ago(300), disabled: false, activeSessions: 0, owner: 'finance-analysts', description: 'Refreshes the reporting dashboards' }
	];

	// ---- tokens ----
	const apiTokens: ApiToken[] = [
		{ id: 'tok-1', label: 'CI deploys', owner: CI_ACCOUNT, prefix: 'sqbt_4c1e', createdAt: ago(days(90)), createdBy: DEMO_USER, expiresAt: ahead(days(275)), lastUsedAt: ago(42), revokedAt: null, state: 'active' },
		{ id: 'tok-2', label: 'Dashboard refresh', owner: REPORTING, prefix: 'sqbt_9a07', createdAt: ago(300), createdBy: DEMO_USER, expiresAt: ahead(days(30)), lastUsedAt: ago(120), revokedAt: null, state: 'active' },
		{ id: 'tok-4', label: 'Old CI runner', owner: CI_ACCOUNT, prefix: 'sqbt_51d2', createdAt: ago(days(160)), createdBy: DEMO_USER, expiresAt: null, lastUsedAt: ago(days(70)), revokedAt: ago(days(65)), state: 'revoked' },
		{ id: 'tok-5', label: 'Load test', owner: CI_ACCOUNT, prefix: 'sqbt_0f8c', createdAt: ago(days(100)), createdBy: DEMO_USER, expiresAt: ago(days(70)), lastUsedAt: ago(days(71)), revokedAt: null, state: 'expired' }
	];
	const locationTokens: LocationToken[] = [
		{ id: 'ltok-1', locationId: LOCATION_SHOP, prefix: 'sqbl_3f9a', createdAt: ago(days(30)), createdBy: DEMO_USER, lastUsedAt: ago(0.2), revokedAt: null, state: 'active' },
		{ id: 'ltok-0', locationId: LOCATION_SHOP, prefix: 'sqbl_a210', createdAt: ago(days(210)), createdBy: DEMO_USER, lastUsedAt: ago(days(30)), revokedAt: ago(days(30)), state: 'revoked' },
		{ id: 'ltok-2', locationId: LOCATION_MARKETING, prefix: 'sqbl_7be4', createdAt: ago(days(120)), createdBy: DEMO_USER, lastUsedAt: ago(0.5), revokedAt: null, state: 'active' }
	];

	// ---- roles ----
	const builtIn = (name: BuiltInRole, description: string): RoleDefinition => ({
		name,
		source: 'built_in',
		sourceFile: null,
		description,
		grants: [{ nodes: ['*'], permissions: ROLE_CAPABILITIES[name] }],
		pendingApprovalId: null
	});
	const roles: RoleDefinition[] = [
		builtIn('viewer', 'See everything except SQL and evidence.'),
		builtIn('operator', 'Run, cancel and rerun jobs; pause triggers; refresh warehouse sync.'),
		builtIn('developer', 'Operator, plus ad-hoc runs, plan previews, connection tests and evidence.'),
		builtIn('admin', 'Everything, including users, roles, tokens, locations and the audit log.'),
		{
			name: 'support_viewer',
			source: 'server_config',
			sourceFile: 'sqb-server.toml',
			description: 'Support team: view support models and their SQL.',
			grants: [{ nodes: ['group:support'], permissions: ['view', 'view.sql'] }],
			pendingApprovalId: null
		},
		{
			name: 'finance_operator',
			source: 'project',
			sourceFile: 'roles.toml',
			description: 'Finance analysts operate finance jobs.',
			grants: [
				{ nodes: ['group:finance'], permissions: ['job.run', 'run.adhoc', 'run.cancel', 'view.sql'] },
				{ nodes: ['group:finance,tag:pii'], permissions: ['view.evidence'] }
			],
			pendingApprovalId: 'appr-3'
		}
	];

	const assignments: RoleAssignment[] = [
		{ id: 'asg-1', principal: DEMO_USER, role: 'admin', scope: { locationId: null }, source: 'manual', ssoGroup: null, assignedBy: SYSTEM, assignedAt: ago(days(210)), stale: false },
		{ id: 'asg-2', principal: ALEX, role: 'developer', scope: { locationId: LOCATION_SHOP_STAGING }, source: 'sso', ssoGroup: 'analytics-eng', assignedBy: null, assignedAt: ago(days(180)), stale: false },
		{ id: 'asg-3', principal: ALEX, role: 'operator', scope: { locationId: LOCATION_SHOP }, source: 'manual', ssoGroup: null, assignedBy: DEMO_USER, assignedAt: ago(days(170)), stale: false },
		{ id: 'asg-4', principal: SAM, role: 'developer', scope: { locationId: LOCATION_SHOP_STAGING }, source: 'sso', ssoGroup: 'analytics-eng', assignedBy: null, assignedAt: ago(days(95)), stale: false },
		{ id: 'asg-5', principal: SAM, role: 'finance_operator', scope: { locationId: LOCATION_SHOP }, source: 'sso', ssoGroup: 'finance-analysts', assignedBy: null, assignedAt: ago(days(95)), stale: false },
		{ id: 'asg-6', principal: JORDAN, role: 'viewer', scope: { locationId: null }, source: 'manual', ssoGroup: null, assignedBy: DEMO_USER, assignedAt: ago(days(40)), stale: false },
		{ id: 'asg-7', principal: JORDAN, role: 'growth_analyst', scope: { locationId: LOCATION_SHOP }, source: 'manual', ssoGroup: null, assignedBy: DEMO_USER, assignedAt: ago(days(38)), stale: true },
		{ id: 'asg-8', principal: CI_ACCOUNT, role: 'operator', scope: { locationId: LOCATION_SHOP }, source: 'manual', ssoGroup: null, assignedBy: DEMO_USER, assignedAt: ago(days(200)), stale: false },
		{ id: 'asg-9', principal: REPORTING, role: 'viewer', scope: { locationId: LOCATION_MARKETING }, source: 'manual', ssoGroup: null, assignedBy: DEMO_USER, assignedAt: ago(days(120)), stale: false },
		{ id: 'asg-11', principal: OPS_USER, role: 'operator', scope: { locationId: null }, source: 'manual', ssoGroup: null, assignedBy: DEMO_USER, assignedAt: ago(days(150)), stale: false },
		{ id: 'asg-10', principal: RILEY, role: 'support_viewer', scope: { locationId: null }, source: 'sso', ssoGroup: 'support-ops', assignedBy: null, assignedAt: ago(days(300)), stale: false }
	];

	const approvals: RoleApproval[] = [
		{
			id: 'appr-3',
			role: 'finance_operator',
			locationId: LOCATION_SHOP,
			snapshotGeneration: 42,
			gitRevision: '7d2e41a',
			gitAuthor: 'Alex Rivera',
			proposedAt: ago(190),
			added: [
				{ nodes: ['group:finance'], permissions: ['run.rewrite_history'] },
				{ nodes: ['tag:pii'], permissions: ['view.evidence'] }
			],
			removed: [{ nodes: ['group:finance,tag:pii'], permissions: ['view.evidence'] }],
			state: 'pending',
			decidedBy: null,
			decidedAt: null,
			note: null
		},
		{
			id: 'appr-2',
			role: 'support_triage',
			locationId: LOCATION_MARKETING,
			snapshotGeneration: 16,
			gitRevision: '2a9e0c4',
			gitAuthor: 'Sam Okafor',
			proposedAt: ago(days(9)),
			added: [{ nodes: ['*'], permissions: ['job.run'] }],
			removed: [],
			state: 'rejected',
			decidedBy: DEMO_USER,
			decidedAt: ago(days(8)),
			note: 'Too broad; scope it to group:support.'
		},
		{
			id: 'appr-1',
			role: 'finance_operator',
			locationId: LOCATION_SHOP,
			snapshotGeneration: 35,
			gitRevision: 'c03d1f7',
			gitAuthor: 'Sam Okafor',
			proposedAt: ago(days(21)),
			added: [{ nodes: ['group:finance'], permissions: ['run.cancel'] }],
			removed: [],
			state: 'approved',
			decidedBy: DEMO_USER,
			decidedAt: ago(days(20)),
			note: 'Finance on-call needs to cancel stuck runs.'
		}
	];

	// ---- audit ----
	let auditSeq = 0;
	const entry = (minutesAgo: number, e: Omit<AuditEntry, 'id' | 'at' | 'ip' | 'requestId'> & { ip?: string }): AuditEntry => {
		auditSeq += 1;
		return {
			id: `aud-${String(auditSeq).padStart(4, '0')}`,
			at: ago(minutesAgo),
			ip: e.ip ?? '10.0.4.17',
			requestId: `req-${randomHex(12)}`,
			...e
		};
	};
	const shopProd = { locationId: LOCATION_SHOP };
	const auditEntries: AuditEntry[] = [
		entry(2, { actor: DEMO_USER, action: 'warehouse.sync', outcome: 'allowed', scope: { ...shopProd, subject: null }, grantingRole: 'admin', evidenceRead: false, summary: 'Requested a warehouse sync (started)', before: null, after: null }),
		entry(18, { actor: SAM, action: 'view.evidence', outcome: 'allowed', scope: { ...shopProd, subject: 'fact_orders' }, grantingRole: 'finance_operator', evidenceRead: true, summary: 'Read sample rows of fact_orders', before: null, after: null, ip: '10.0.7.33' }),
		entry(26, { actor: JORDAN, action: 'run.cancel', outcome: 'denied', scope: { ...shopProd, subject: 'run-9d21' }, grantingRole: null, evidenceRead: false, summary: 'Denied: no role grants run.cancel on prod (viewer grants view only)', before: null, after: null, ip: '10.0.9.4' }),
		entry(42, { actor: CI_ACCOUNT, action: 'job.run', outcome: 'allowed', scope: { ...shopProd, subject: 'checks_only' }, grantingRole: 'operator', evidenceRead: false, summary: 'Launched checks_only', before: null, after: 'run-c40e queued', ip: '10.0.2.8' }),
		entry(55, { actor: ALEX, action: 'trigger.toggle', outcome: 'allowed', scope: { ...shopProd, subject: 'revenue_refresh' }, grantingRole: 'operator', evidenceRead: false, summary: 'Paused revenue_refresh', before: 'running', after: 'paused', ip: '10.0.7.21' }),
		entry(70, { actor: ALEX, action: 'run.adhoc', outcome: 'denied', scope: { ...shopProd, subject: 'daily_revenue+' }, grantingRole: null, evidenceRead: false, summary: 'Denied: run.adhoc missing on 4 nodes (daily_revenue, revenue_dashboard, …)', before: null, after: null, ip: '10.0.7.21' }),
		entry(190, { actor: SYSTEM, action: 'role.assign', outcome: 'allowed', scope: { locationId: LOCATION_SHOP, subject: 'finance_operator' }, grantingRole: null, evidenceRead: false, summary: 'Snapshot 42 proposed a widening of finance_operator; held for approval', before: null, after: 'pending appr-3', ip: '—' }),
		entry(300, { actor: DEMO_USER, action: 'token.manage', outcome: 'allowed', scope: { locationId: null, subject: 'tok-2' }, grantingRole: 'admin', evidenceRead: false, summary: 'Rotated token "Dashboard refresh"', before: 'sqbt_77d0', after: 'sqbt_9a07' }),
		entry(720, { actor: JORDAN, action: 'connection.test', outcome: 'denied', scope: { ...shopProd, subject: 'prod' }, grantingRole: null, evidenceRead: false, summary: 'Denied: connection.test requires developer or above', before: null, after: null, ip: '10.0.9.4' }),
		entry(days(1) + 60, { actor: SAM, action: 'view.evidence', outcome: 'denied', scope: { ...shopProd, subject: 'dim_customers' }, grantingRole: null, evidenceRead: false, summary: 'Denied: finance_operator grants view.evidence only on group:finance,tag:pii', before: null, after: null, ip: '10.0.7.33' }),
		entry(days(2), { actor: DEMO_USER, action: 'location.manage', outcome: 'allowed', scope: { locationId: LOCATION_SHOP, subject: 'ltok-1' }, grantingRole: 'admin', evidenceRead: false, summary: 'Rotated the location token', before: 'sqbl_a210', after: 'sqbl_3f9a' }),
		entry(days(8), { actor: DEMO_USER, action: 'role.reject', outcome: 'allowed', scope: { locationId: LOCATION_MARKETING, subject: 'support_triage' }, grantingRole: 'admin', evidenceRead: false, summary: 'Rejected widening of support_triage', before: 'pending', after: 'rejected' }),
		entry(days(20), { actor: DEMO_USER, action: 'role.approve', outcome: 'allowed', scope: { locationId: LOCATION_SHOP, subject: 'finance_operator' }, grantingRole: 'admin', evidenceRead: false, summary: 'Approved widening of finance_operator (+run.cancel on group:finance)', before: 'pending', after: 'approved' })
	];

	// ---- locations ----
	const publishTest = (minutesAgo: number, durationMs: number, message: string): ConnectionTest => ({
		outcome: 'ok',
		at: ago(minutesAgo),
		durationMs,
		trigger: 'publish',
		by: null,
		message
	});
	const loadError =
		"ImportError: cannot import name 'campaign_touches' from 'models.attribution' (orchestration/jobs.py, line 12)";
	type LocationExtras = Pick<LocationDetail, 'createdAt' | 'brokenDeployPolicy' | 'failedDeploy' | 'lastLoadError' | 'instances' | 'imageId' | 'warehouseConnection' | 'connectionTest'>;
	const locationExtras: Record<string, LocationExtras> = {
		[LOCATION_SHOP]: {
			createdAt: ago(days(210)),
			brokenDeployPolicy: 'fallback',
			failedDeploy: null,
			lastLoadError: null,
			imageId: 'sha256:4be1c07d9a52',
			warehouseConnection: 'prod',
			connectionTest: publishTest(190, 820, 'Connected as TRANSFORMER to WAFFLE_SHOP; warehouse TRANSFORMING_WH available'),
			instances: [
				{ id: 'location-waffle-shop-7c2d', imageId: 'sha256:4be1c07d9a52', gitRevision: '7d2e41a', startedAt: ago(192), lastSeenAt: ago(0.2), state: 'live' },
				{ id: 'location-waffle-shop-1b90', imageId: 'sha256:ad37e9015b64', gitRevision: '5c1a9e2', startedAt: ago(days(3)), lastSeenAt: ago(193), state: 'gone' }
			]
		},
		// A deploy in progress: the new process is loading while the old one keeps running.
		[LOCATION_SHOP_STAGING]: {
			createdAt: ago(days(180)),
			brokenDeployPolicy: 'fallback',
			failedDeploy: null,
			lastLoadError: null,
			imageId: 'sha256:9f02b6e4c1a8',
			warehouseConnection: 'staging',
			connectionTest: publishTest(35, 760, 'Connected as TRANSFORMER to WAFFLE_SHOP_STAGING'),
			instances: [
				{ id: 'location-waffle-shop-staging-c7e3', imageId: 'sha256:61d0a4f7b2e9', gitRevision: 'e09a6c3', startedAt: ago(1), lastSeenAt: ago(0.1), state: 'live' },
				{ id: 'location-waffle-shop-staging-5a1f', imageId: 'sha256:9f02b6e4c1a8', gitRevision: 'b81c09e', startedAt: ago(36), lastSeenAt: ago(0.1), state: 'live' }
			]
		},
		[LOCATION_SHOP_DEV]: {
			createdAt: ago(days(14)),
			brokenDeployPolicy: 'queue',
			failedDeploy: null,
			lastLoadError: null,
			imageId: null,
			warehouseConnection: 'dev',
			connectionTest: publishTest(2, 12, 'Opened warehouse.duckdb'),
			instances: [{ id: 'location-waffle-shop-dev-local', imageId: 'local', gitRevision: 'b81c09e', startedAt: ago(30), lastSeenAt: ago(0.3), state: 'live' }]
		},
		[LOCATION_MARKETING]: {
			createdAt: ago(days(120)),
			brokenDeployPolicy: 'fallback',
			failedDeploy: { gitRevision: '9c4b2d1', imageId: 'sha256:9c4b2d18e07f', attemptedAt: ago(48), error: loadError },
			lastLoadError: loadError,
			imageId: 'sha256:3e5f7aa20c19',
			warehouseConnection: 'prod',
			connectionTest: publishTest(1440, 140, 'Connected to analytics as sqb_builder'),
			instances: [
				{ id: 'location-marketing-analytics-03f6', imageId: 'sha256:3e5f7aa20c19', gitRevision: '3e5f7aa', startedAt: ago(days(1) + 30), lastSeenAt: ago(0.5), state: 'live' },
				{ id: 'location-marketing-analytics-e41a', imageId: 'sha256:9c4b2d18e07f', gitRevision: '9c4b2d1', startedAt: ago(48), lastSeenAt: ago(47), state: 'gone' }
			]
		}
	};

	// ---- connections ----
	const snowflakeSettings = (database: string): { settings: SettingField[]; values: Record<string, string | number | boolean> } => ({
		settings: [
			{ key: 'account', type: 'string', required: true, description: 'Account identifier' },
			{ key: 'database', type: 'string', required: true, description: 'Database the target builds into' },
			{ key: 'schema', type: 'string', default: 'ANALYTICS', description: 'Default schema' },
			{ key: 'warehouse', type: 'string', required: true, description: 'Compute warehouse' },
			{ key: 'role', type: 'string', default: 'TRANSFORMER', description: 'Session role' },
			{ key: 'threads', type: 'int', default: 8, min: 1, max: 64, description: 'Concurrent statements' },
			{ key: 'authenticator', type: 'enum', enum: ['snowflake', 'snowflake_jwt', 'externalbrowser'], default: 'snowflake_jwt', description: 'Authentication method' },
			{ key: 'private_key', type: 'secret', required: true, secretSource: 'env: SNOWFLAKE_PRIVATE_KEY', description: 'Key-pair private key' }
		],
		values: { account: 'example-account', database, warehouse: 'TRANSFORMING_WH' }
	});
	const warehouseByLocation: Record<string, WarehouseConnection[]> = {
		[LOCATION_SHOP]: [
			{ name: 'prod', target: 'prod', adapter: 'snowflake', purpose: 'target', sourceFile: 'sqlbuild.toml', ...snowflakeSettings('WAFFLE_SHOP'), lastTest: locationExtras[LOCATION_SHOP].connectionTest },
			{
				name: 'prod_catalog',
				target: 'prod',
				adapter: 'snowflake',
				purpose: 'catalog',
				sourceFile: 'sqlbuild.toml',
				settings: snowflakeSettings('WAFFLE_SHOP').settings.map((f) => (f.key === 'role' ? { ...f, default: 'CATALOG_READER' } : f)),
				values: { account: 'example-account', database: 'WAFFLE_SHOP', warehouse: 'CATALOG_WH' },
				lastTest: null
			}
		],
		[LOCATION_SHOP_STAGING]: [
			{ name: 'staging', target: 'staging', adapter: 'snowflake', purpose: 'target', sourceFile: 'sqlbuild.toml', ...snowflakeSettings('WAFFLE_SHOP_STAGING'), lastTest: locationExtras[LOCATION_SHOP_STAGING].connectionTest }
		],
		[LOCATION_SHOP_DEV]: [
			{
				name: 'dev',
				target: 'dev',
				adapter: 'duckdb',
				purpose: 'target',
				sourceFile: 'sqlbuild.toml',
				settings: [
					{ key: 'path', type: 'string', required: true, description: 'Database file' },
					{ key: 'threads', type: 'int', default: 4, min: 1, max: 32, description: 'Concurrent statements' }
				],
				values: { path: 'warehouse.duckdb' },
				lastTest: locationExtras[LOCATION_SHOP_DEV].connectionTest
			}
		],
		[LOCATION_MARKETING]: [
			{
				name: 'prod',
				target: 'prod',
				adapter: 'postgres',
				purpose: 'target',
				sourceFile: 'sqlbuild.toml',
				settings: [
					{ key: 'host', type: 'string', required: true, description: 'Server host' },
					{ key: 'port', type: 'int', default: 5432, min: 1, max: 65535, description: 'Server port' },
					{ key: 'dbname', type: 'string', required: true, description: 'Database' },
					{ key: 'user', type: 'string', required: true, description: 'Login role' },
					{ key: 'password', type: 'secret', required: true, secretSource: 'env: PGPASSWORD', description: 'Login password' }
				],
				values: { host: 'db.example.com', dbname: 'analytics', user: 'sqb_builder' },
				lastTest: locationExtras[LOCATION_MARKETING].connectionTest
			}
		]
	};
	const providers: ProviderConnection[] = [
		{
			name: 'orders_api',
			type: 'rest',
			description: 'Partner orders API used by the raw orders loader.',
			sourceFile: 'providers/orders_api.py',
			settings: [
				{ key: 'base_url', type: 'string', required: true, description: 'API base URL' },
				{ key: 'page_size', type: 'int', default: 500, min: 1, max: 5000, description: 'Records per request' },
				{ key: 'timeout_seconds', type: 'int', default: 30, min: 1, max: 300, description: 'Request timeout' },
				{ key: 'api_key', type: 'secret', required: true, secretSource: 'env: ORDERS_API_KEY', description: 'API key' }
			],
			values: { base_url: 'https://orders.example.com/v2' },
			usedBy: { nodes: ['raw__orders'], jobs: ['load_sources', 'nightly_all'], triggers: [] }
		},
		{
			name: 'export_bucket',
			type: 's3',
			description: 'Object storage for CSV exports of reporting models.',
			sourceFile: 'providers/storage.py',
			settings: [
				{ key: 'bucket', type: 'string', required: true, description: 'Bucket name' },
				{ key: 'prefix', type: 'string', default: 'exports/', description: 'Key prefix' },
				{ key: 'region', type: 'string', required: true, description: 'Region' },
				{ key: 'compression', type: 'enum', enum: ['none', 'gzip', 'zstd'], default: 'gzip', description: 'File compression' },
				{ key: 'secret_access_key', type: 'secret', required: true, secretSource: 'secret store: exports/bucket', description: 'Access key secret' }
			],
			values: { bucket: 'example-exports', region: 'eu-west-1' },
			usedBy: { nodes: ['export_revenue_csv'], jobs: ['revenue_refresh', 'reporting_exports'], triggers: [] }
		},
		{
			name: 'support_desk',
			type: 'rest',
			description: 'Support ticket system read by the tickets loader.',
			sourceFile: 'providers/support_desk.py',
			settings: [
				{ key: 'base_url', type: 'string', required: true, description: 'API base URL' },
				{ key: 'include_closed', type: 'bool', default: false, description: 'Also load closed tickets' },
				{ key: 'token', type: 'secret', required: true, secretSource: 'env: SUPPORT_DESK_TOKEN', description: 'Access token' }
			],
			values: { base_url: 'https://support.example.com/api', include_closed: true },
			usedBy: { nodes: ['raw__support_tickets'], jobs: ['load_sources'], triggers: [] }
		},
		{
			name: 'chat_webhook',
			type: 'webhook',
			description: 'Incoming webhook of the team chat room; sensors post messages to it.',
			sourceFile: 'providers/chat.py',
			settings: [
				{ key: 'url', type: 'secret', required: true, secretSource: 'env: CHAT_WEBHOOK_URL', description: 'Webhook URL' },
				{ key: 'timeout_seconds', type: 'int', default: 10, min: 1, max: 60, description: 'Request timeout' }
			],
			values: {},
			usedBy: { nodes: [], jobs: [], triggers: [{ id: triggerId('platform_health_webhook'), name: 'platform_health_webhook' }] }
		}
	];

	// ---- maintenance ----
	const migrations: MigrationEvent[] = [
		{ eventId: 'mig-5', targetName: 'prod', originModel: 'customer_status_v2', origin: relation('MARTS', 'CUSTOMER_STATUS_V2'), destinationModel: 'customer_status', destination: relation('MARTS', 'CUSTOMER_STATUS'), discovery: 'manual', decision: 'superseded_replace', runId: 'run-77b3', createdAt: ago(days(1) + 340) },
		{ eventId: 'mig-4', targetName: 'prod', originModel: 'activity_hourly', origin: relation('ANALYTICS', 'ACTIVITY_HOURLY'), destinationModel: 'hourly_activity_ctx', destination: relation('ANALYTICS', 'HOURLY_ACTIVITY_CTX'), discovery: 'manual', decision: 'redo', runId: 'run-e3f8', createdAt: ago(days(2)) },
		{ eventId: 'mig-3', targetName: 'prod', originModel: 'customer_segments', origin: relation('ANALYTICS', 'CUSTOMER_SEGMENTS'), destinationModel: 'customer_segments', destination: relation('MARTS', 'CUSTOMER_SEGMENTS'), discovery: 'manual', decision: 'migrate', runId: 'run-2b70', createdAt: ago(days(4)) },
		{ eventId: 'mig-2', targetName: 'prod', originModel: 'revenue_by_day', origin: relation('ANALYTICS', 'REVENUE_BY_DAY'), destinationModel: 'daily_revenue', destination: relation('ANALYTICS', 'DAILY_REVENUE'), discovery: 'manual', decision: 'migrate', runId: 'run-91cd', createdAt: ago(days(9)) },
		{ eventId: 'mig-1', targetName: 'prod', originModel: 'stg_payment', origin: relation('STAGING', 'STG_PAYMENT'), destinationModel: 'stg_payments', destination: relation('STAGING', 'STG_PAYMENTS'), discovery: 'automatic', decision: 'renamed', runId: 'run-77b3', createdAt: ago(days(20)) },
		{ eventId: 'mig-0', targetName: 'prod', originModel: 'orders_v1', origin: relation('ANALYTICS', 'ORDERS_V1'), destinationModel: 'fact_orders', destination: relation('ANALYTICS', 'FACT_ORDERS'), discovery: 'manual', decision: 'migrate', runId: 'run-c40e', createdAt: ago(days(41)) }
	];
	const step = (eventType: OldNameView['steps'][number]['eventType'], minutesAgo: number, runId: string) => ({ eventType, at: ago(minutesAgo), runId });
	const oldNameViews: OldNameView[] = [
		{
			migrationEventId: 'mig-5', destinationModel: 'customer_status', old: relation('MARTS', 'CUSTOMER_STATUS_V2'), new: relation('MARTS', 'CUSTOMER_STATUS'), retention: '30d',
			archiveName: '_SQB_ARCHIVE__20261027T010000Z__CUSTOMER_STATUS_V2', columnAliases: [], grantsCopied: 2, expiresAt: null,
			dropReason: 'Removed early on request (--drop-old-name-view)',
			steps: [step('required', days(1) + 340, 'run-77b3'), step('origin_archived', days(1) + 339, 'run-77b3'), step('view_created', days(1) + 339, 'run-77b3'), step('view_dropped', 600, 'run-0c7d')]
		},
		{
			migrationEventId: 'mig-4', destinationModel: 'hourly_activity_ctx', old: relation('ANALYTICS', 'ACTIVITY_HOURLY'), new: relation('ANALYTICS', 'HOURLY_ACTIVITY_CTX'), retention: 'false',
			archiveName: '_SQB_ARCHIVE__20261027T091500Z__ACTIVITY_HOURLY', columnAliases: [], grantsCopied: null, expiresAt: null, dropReason: null,
			steps: [step('required', days(2), 'run-e3f8'), step('origin_archived', days(2), 'run-e3f8')]
		},
		{
			migrationEventId: 'mig-3', destinationModel: 'customer_segments', old: relation('ANALYTICS', 'CUSTOMER_SEGMENTS'), new: relation('MARTS', 'CUSTOMER_SEGMENTS'), retention: '7d',
			archiveName: '_SQB_ARCHIVE__20261025T021200Z__CUSTOMER_SEGMENTS', columnAliases: [], grantsCopied: 1, expiresAt: ago(days(4) - days(7)), dropReason: null,
			steps: [step('required', days(4), 'run-2b70'), step('origin_archived', days(4), 'run-2b70'), step('view_created', days(4), 'run-2b70')]
		},
		{
			migrationEventId: 'mig-2', destinationModel: 'daily_revenue', old: relation('ANALYTICS', 'REVENUE_BY_DAY'), new: relation('ANALYTICS', 'DAILY_REVENUE'), retention: '30d',
			archiveName: '_SQB_ARCHIVE__20261020T021500Z__REVENUE_BY_DAY', columnAliases: [{ old: 'amount', new: 'revenue' }, { old: 'day', new: 'revenue_date' }], grantsCopied: 3,
			expiresAt: ago(days(9) - days(30)), dropReason: null,
			steps: [step('required', days(9), 'run-91cd'), step('origin_archived', days(9), 'run-91cd'), step('view_created', days(9), 'run-91cd')]
		},
		{
			migrationEventId: 'mig-0', destinationModel: 'fact_orders', old: relation('ANALYTICS', 'ORDERS_V1'), new: relation('ANALYTICS', 'FACT_ORDERS'), retention: '30d',
			archiveName: '_SQB_ARCHIVE__20260918T020000Z__ORDERS_V1', columnAliases: [{ old: 'order_total', new: 'amount' }], grantsCopied: 4, expiresAt: ago(days(11)), dropReason: 'Removed at its end date',
			steps: [step('required', days(41), 'run-c40e'), step('origin_archived', days(41), 'run-c40e'), step('view_created', days(41), 'run-c40e'), step('view_dropped', days(10), 'run-jan-8b1')]
		},
		{
			migrationEventId: 'mig-6', destinationModel: 'weekly_orders', old: relation('MARTS', 'ORDERS_WEEKLY'), new: relation('MARTS', 'WEEKLY_ORDERS'), retention: '14d',
			archiveName: '_SQB_ARCHIVE__20261012T020000Z__ORDERS_WEEKLY', columnAliases: [], grantsCopied: 0, expiresAt: ago(days(3)), dropReason: null,
			steps: [step('required', days(17), 'run-1a05'), step('origin_archived', days(17), 'run-1a05'), step('view_created', days(17), 'run-1a05')]
		}
	];
	migrations.push({ eventId: 'mig-6', targetName: 'prod', originModel: 'orders_weekly', origin: relation('MARTS', 'ORDERS_WEEKLY'), destinationModel: 'weekly_orders', destination: relation('MARTS', 'WEEKLY_ORDERS'), discovery: 'automatic', decision: 'migrate', runId: 'run-1a05', createdAt: ago(days(17)) });
	migrations.sort((a, b) => b.createdAt.localeCompare(a.createdAt));

	const janitor = (id: string, eventType: 'archive' | 'delete', minutesAgo: number, schema: string, type: 'table' | 'view' | null, original: string | null, archive: string, archivedMinutesAgo: number, runId: string): JanitorEvent => ({
		eventId: id,
		eventType,
		occurredAt: ago(minutesAgo),
		runId,
		relationSchema: schema,
		relationType: type,
		originalQualifiedName: original,
		archiveQualifiedName: archive,
		archivedAt: ago(archivedMinutesAgo)
	});
	const janitorEvents: JanitorEvent[] = [
		janitor('jan-9', 'archive', days(1) + 120, 'ANALYTICS', 'table', 'WAFFLE_SHOP.ANALYTICS.LEGACY_ORDER_ITEMS', 'WAFFLE_SHOP.ANALYTICS._SQB_ARCHIVE__20261028T040000Z__LEGACY_ORDER_ITEMS', days(1) + 120, 'run-jan-9c3'),
		janitor('jan-8', 'archive', days(1) + 120, 'MARTS', 'view', 'WAFFLE_SHOP.MARTS.CUSTOMER_COHORTS_OLD', 'WAFFLE_SHOP.MARTS._SQB_ARCHIVE__20261028T040000Z__CUSTOMER_COHORTS_OLD', days(1) + 120, 'run-jan-9c3'),
		janitor('jan-7', 'delete', days(1) + 121, 'ANALYTICS', 'table', 'WAFFLE_SHOP.ANALYTICS.PROMO_CODES_2025', 'WAFFLE_SHOP.ANALYTICS._SQB_ARCHIVE__20261013T040000Z__PROMO_CODES_2025', days(16), 'run-jan-9c3'),
		janitor('jan-6', 'delete', days(1) + 121, 'ANALYTICS', null, null, 'WAFFLE_SHOP.ANALYTICS._SQB_ARCHIVE__20261012T091500Z__STG__A1B2C3', days(17), 'run-jan-9c3'),
		janitor('jan-5', 'delete', days(10), 'ANALYTICS', 'table', 'WAFFLE_SHOP.ANALYTICS.ORDERS_V1', 'WAFFLE_SHOP.ANALYTICS._SQB_ARCHIVE__20260918T020000Z__ORDERS_V1', days(41), 'run-jan-8b1'),
		janitor('jan-4', 'archive', days(16), 'ANALYTICS', 'table', 'WAFFLE_SHOP.ANALYTICS.PROMO_CODES_2025', 'WAFFLE_SHOP.ANALYTICS._SQB_ARCHIVE__20261013T040000Z__PROMO_CODES_2025', days(16), 'run-jan-6d0'),
		janitor('jan-3', 'archive', days(16), 'STAGING', 'view', 'WAFFLE_SHOP.STAGING.STG_COUPONS', 'WAFFLE_SHOP.STAGING._SQB_ARCHIVE__20261013T040000Z__STG_COUPONS', days(16), 'run-jan-6d0')
	];

	return { accounts, apiTokens, locationTokens, roles, assignments, approvals, auditEntries, locationExtras, warehouseByLocation, providers, migrations, oldNameViews, janitorEvents };
}

/** How the platform-event sensor (see `platform_health_webhook` in operate.ts) handled an event. */
function reaction(outcome: PlatformEventReaction['outcome'], sequence: number | null, attempts: number, at: string | null): PlatformEventReaction {
	return {
		triggerId: triggerId('platform_health_webhook'),
		triggerName: 'platform_health_webhook',
		locationId: LOCATION_SHOP,
		outcome,
		evaluationId: sequence === null ? null : `eval-platform-health-webhook-${sequence}`,
		attempts,
		at
	};
}

/** Enrichment of the shared platform event fixtures, by event ID. */
function eventDetails(now: number): Record<string, Pick<PlatformEventRecord, 'resolvedAt' | 'links' | 'reactions' | 'payload'>> {
	const ago = (minutes: number) => new Date(now - minutes * MIN).toISOString();
	const shop = { locationId: LOCATION_SHOP };
	return {
		'pe-1': { resolvedAt: null, links: [{ kind: 'location', id: LOCATION_MARKETING }], reactions: [reaction('consumed', 75, 1, ago(48))], payload: { failed_revision: '9c4b2d1', running_revision: '3e5f7aa', policy: 'fallback' } },
		'pe-2': { resolvedAt: null, links: [{ kind: 'trigger', id: triggerId('segments_hourly'), name: 'segments_hourly', ...shop }], reactions: [reaction('consumed', 77, 2, ago(11.5))], payload: { trigger: 'segments_hourly', scheduled_for: ago(12), overdue_by: '12m' } },
		'pe-3': { resolvedAt: ago(1), links: [], reactions: [], payload: { interval: '60s' } },
		'pe-4': { resolvedAt: null, links: [], reactions: [reaction('pending', null, 0, null)], payload: { last_holder: 'sqb-server-1', lease_expired_at: ago(2) } },
		'pe-5': { resolvedAt: null, links: [{ kind: 'job', key: 'revenue_refresh', ...shop }, { kind: 'run', id: 'run-a7c1', ...shop }], reactions: [reaction('pending', null, 0, null)], payload: { job: 'revenue_refresh', attempts: '3', error: 'image pull error' } }
	};
}

function historicalEvents(now: number): PlatformEventRecord[] {
	const ago = (minutes: number) => new Date(now - minutes * MIN).toISOString();
	const shop = { locationId: LOCATION_SHOP };
	return [
		{ id: 'pe-h3', kind: 'launch_failed', severity: 'critical', occurredAt: ago(1500), active: false, locationId: LOCATION_SHOP, subject: 'activity_microbatch', message: 'A run of activity_microbatch stopped reporting in and was marked failed.', resolvedAt: ago(1490), links: [{ kind: 'job', key: 'activity_microbatch', ...shop }, { kind: 'run', id: 'run-e3f8', ...shop }], reactions: [reaction('consumed', 52, 1, ago(1499))], payload: { job: 'activity_microbatch', attempts: '1', error: 'worker lost' } },
		{ id: 'pe-h2', kind: 'scheduler_no_leader', severity: 'critical', occurredAt: ago(2 * 1440 + 30), active: false, locationId: null, subject: null, message: 'No server ran the scheduler for 41s while a server restarted.', resolvedAt: ago(2 * 1440 + 29), links: [], reactions: [reaction('consumed', 41, 1, ago(2 * 1440 + 29))], payload: { last_holder: 'sqb-server-2', lease_expired_at: ago(2 * 1440 + 30) } },
		{ id: 'pe-h4', kind: 'connection_test_failed', severity: 'warning', occurredAt: ago(2 * 1440 + 400), active: false, locationId: LOCATION_MARKETING, subject: 'prod', message: 'The warehouse connection test after publishing failed: password authentication failed for user sqb_builder.', resolvedAt: ago(2 * 1440 + 380), links: [{ kind: 'location', id: LOCATION_MARKETING }], reactions: [], payload: { connection: 'prod', trigger: 'publish', error: 'password authentication failed' } },
		{ id: 'pe-h1', kind: 'tick_overdue', severity: 'warning', occurredAt: ago(3 * 1440), active: false, locationId: LOCATION_SHOP, subject: 'core_build', message: 'The core_build schedule started 4 minutes late.', resolvedAt: ago(3 * 1440 - 4), links: [{ kind: 'trigger', id: triggerId('core_build'), name: 'core_build', ...shop }], reactions: [reaction('consumed', 33, 1, ago(3 * 1440 - 1))], payload: { trigger: 'core_build', scheduled_for: ago(3 * 1440), overdue_by: '4m' } }
	];
}

/** Mock implementation of `PlatformApi`, delegated to by `MockApiClient`. */
export class MockPlatformApi implements PlatformApi {
	readonly #ctx: MockPlatformContext;
	readonly #data: ReturnType<typeof createPlatformFixtures>;
	readonly #connectionTests = new Map<string, ConnectionTest>();

	constructor(ctx: MockPlatformContext) {
		this.#ctx = ctx;
		this.#data = createPlatformFixtures(ctx.now);
	}

	/** Records an audited read of check evidence (`view.evidence`). */
	recordEvidenceRead(scope: Scope, subject: string, summary: string): void {
		this.#audit('view.evidence', { ...scope, subject }, summary);
		this.#data.auditEntries[this.#data.auditEntries.length - 1].evidenceRead = true;
	}

	pendingApprovalCount(): number {
		return this.#data.approvals.filter((a) => a.state === 'pending').length;
	}

	// ---- health ----

	async platformHealth(): Promise<PlatformHealth> {
		const { now, events } = this.#ctx;
		const ago = (minutes: number) => new Date(now - minutes * MIN).toISOString();
		const ahead = (seconds: number) => new Date(Date.now() + seconds * 1000).toISOString();
		// The heartbeat fires every minute; the mock reports the latest one.
		const beat = new Date(Date.now() - ((Date.now() - now + 37_000) % 60_000)).toISOString();
		const noLeader = events === 'all';
		const failing = events === 'all';
		return this.#ctx.reply({
			scheduler: {
				lease: noLeader ? null : { holder: 'sqb-server-1', acquiredAt: ago(2 * 1440 + 29), renewedAt: new Date(Date.now() - 4000).toISOString(), expiresAt: ahead(26) },
				instances: [
					{ instanceId: 'sqb-server-1', version: this.#ctx.serverVersion, schedulerEnabled: true, startedAt: ago(2 * 1440 + 31), lastSeenAt: noLeader ? ago(3) : ago(0.05) },
					{ instanceId: 'sqb-server-2', version: this.#ctx.serverVersion, schedulerEnabled: true, startedAt: ago(2 * 1440 + 28), lastSeenAt: ago(0.1) },
					{ instanceId: 'sqb-server-api-1', version: this.#ctx.serverVersion, schedulerEnabled: false, startedAt: ago(5 * 1440), lastSeenAt: ago(0.1) }
				]
			},
			heartbeat: { intervalSeconds: 60, lastEmittedAt: beat },
			sink: {
				kind: 'webhook',
				destinationSource: 'env: SQB_EVENT_SINK_URL',
				eventKinds: [],
				lastDeliveredAt: failing ? ago(7) : beat,
				lastAttemptAt: beat,
				pendingEvents: failing ? 6 : 0,
				consecutiveFailures: failing ? 4 : 0,
				lastError: failing ? 'HTTP 503 Service Unavailable from the sink endpoint' : null
			}
		});
	}

	async listPlatformEventHistory(query: PlatformEventQuery = {}): Promise<PlatformEventRecord[]> {
		const details = eventDetails(this.#ctx.now);
		const current: PlatformEventRecord[] = this.#ctx.platformEvents().map((event) => ({
			...event,
			...(details[event.id] ?? { resolvedAt: null, links: [], reactions: [], payload: {} })
		}));
		let events = [...current, ...historicalEvents(this.#ctx.now)];
		if (query.kinds?.length) events = events.filter((e) => query.kinds!.includes(e.kind));
		else if (!query.includeHeartbeats) events = events.filter((e) => e.kind !== 'heartbeat');
		if (query.locationId !== undefined) events = events.filter((e) => e.locationId === query.locationId);
		if (query.activeOnly) events = events.filter((e) => e.active);
		events.sort((a, b) => b.occurredAt.localeCompare(a.occurredAt));
		if (query.limit) events = events.slice(0, query.limit);
		return this.#ctx.reply(events);
	}

	// ---- locations ----

	async listLocations(): Promise<LocationSummary[]> {
		const details = this.#ctx.locations.map((l) => this.#locationDetail(l));
		return this.#ctx.reply(
			details.map((d) => ({
				id: d.id,
				name: d.name,
				health: d.health,
				brokenDeployPolicy: d.brokenDeployPolicy,
				failedDeploy: d.failedDeploy,
				activeProblems: this.#ctx.platformEvents().filter((e) => e.active && e.locationId === d.id && e.kind !== 'heartbeat').length,
				liveInstances: d.instances.filter((i) => i.state === 'live').length,
				lastSeenAt: d.instances.map((i) => i.lastSeenAt).sort().at(-1) ?? null,
				target: d.target,
				snapshot: d.snapshot,
				connectionTest: d.connectionTest
			}))
		);
	}

	async getLocation(locationId: string): Promise<LocationDetail | null> {
		const location = this.#ctx.locations.find((l) => l.id === locationId);
		return this.#ctx.reply(location ? this.#locationDetail(location) : null);
	}

	async testConnection(scope: Scope, connection: string): Promise<ConnectionTest> {
		this.#require('connection.test');
		const conn = this.#warehouse(scope).find((c) => c.name === connection);
		if (!conn) throw new ApiError(`Unknown connection ${connection}`, 404);
		const started = Date.now();
		await new Promise((resolve) => setTimeout(resolve, 900));
		const result: ConnectionTest = {
			outcome: 'ok',
			at: new Date().toISOString(),
			durationMs: Date.now() - started,
			trigger: 'manual',
			by: this.#ctx.user,
			message: conn.adapter === 'duckdb' ? `Opened ${conn.values.path}` : `Connected to ${conn.values.database ?? conn.values.dbname} (sqb debug: all checks passed)`
		};
		this.#connectionTests.set(`${scope.locationId}/${connection}`, result);
		this.#audit('connection.test', { ...scope, subject: connection }, `Tested connection ${connection} (${result.outcome})`);
		return this.#ctx.reply(result);
	}

	async listLocationTokens(locationId: string): Promise<LocationToken[]> {
		this.#require('location.manage');
		return this.#ctx.reply(structuredClone(this.#data.locationTokens.filter((t) => t.locationId === locationId)));
	}

	async createLocationToken(locationId: string): Promise<IssuedToken<LocationToken>> {
		this.#require('location.manage');
		if (!this.#ctx.locations.some((l) => l.id === locationId)) throw new ApiError('Unknown location', 404);
		const issued = this.#issueLocationToken(locationId);
		this.#audit('location.manage', { locationId, subject: issued.token.id }, 'Created a location token', null, issued.token.prefix);
		return this.#ctx.reply(issued);
	}

	async rotateLocationToken(tokenId: string): Promise<IssuedToken<LocationToken>> {
		this.#require('location.manage');
		const old = this.#data.locationTokens.find((t) => t.id === tokenId && t.state === 'active');
		if (!old) throw new ApiError('Token not found or not active', 404);
		old.state = 'revoked';
		old.revokedAt = new Date().toISOString();
		const issued = this.#issueLocationToken(old.locationId);
		this.#audit('location.manage', { locationId: old.locationId, subject: issued.token.id }, 'Rotated the location token', old.prefix, issued.token.prefix);
		return this.#ctx.reply(issued);
	}

	async revokeLocationToken(tokenId: string): Promise<LocationToken> {
		this.#require('location.manage');
		const token = this.#data.locationTokens.find((t) => t.id === tokenId && t.state === 'active');
		if (!token) throw new ApiError('Token not found or not active', 404);
		token.state = 'revoked';
		token.revokedAt = new Date().toISOString();
		this.#audit('location.manage', { locationId: token.locationId, subject: token.id }, 'Revoked a location token', token.prefix, null);
		return this.#ctx.reply(structuredClone(token));
	}

	// ---- connections ----

	async listWarehouseConnections(scope: Scope): Promise<WarehouseConnection[]> {
		const warehouse = this.#warehouse(scope).map((c) => ({
			...c,
			lastTest: this.#connectionTests.get(`${scope.locationId}/${c.name}`) ?? c.lastTest
		}));
		return this.#ctx.reply(structuredClone(warehouse));
	}

	async listProviders(scope: Scope): Promise<ProviderConnection[]> {
		return this.#ctx.reply(structuredClone(SHOP_LOCATIONS.includes(scope.locationId) ? this.#data.providers : []));
	}

	// ---- maintenance ----

	async maintenanceHistory(scope: Scope): Promise<MaintenanceHistory> {
		if (scope.locationId !== LOCATION_SHOP && scope.locationId !== LOCATION_SHOP_STAGING) {
			return this.#ctx.reply({ migrations: [], oldNameViews: [], janitorEvents: [] });
		}
		const { migrations, oldNameViews, janitorEvents } = this.#data;
		const target = this.#ctx.locations.find((l) => l.id === scope.locationId)?.target.name ?? '';
		const retarget = <T extends { targetName: string }>(rows: T[]) => rows.map((r) => ({ ...r, targetName: target }));
		return this.#ctx.reply(structuredClone({ migrations: retarget(migrations), oldNameViews, janitorEvents }));
	}

	// ---- users and tokens ----

	async listAccounts(): Promise<Account[]> {
		this.#require('user.manage');
		return this.#ctx.reply(structuredClone(this.#data.accounts));
	}

	async disableAccount(name: string): Promise<Account> {
		this.#require('user.manage');
		const account = this.#data.accounts.find((a) => a.name === name);
		if (!account) throw new ApiError(`Unknown account ${name}`, 404);
		if (account.name === this.#ctx.user.name) throw new ApiError("You can't disable your own account", 409);
		if (account.disabled) throw new ApiError(`${name} is already disabled`, 409);
		const sessions = account.activeSessions;
		account.disabled = true;
		account.activeSessions = 0;
		const revoked = sessions === 1 ? '1 session' : `${sessions} sessions`;
		this.#audit('user.manage', { locationId: null, subject: name }, `Disabled ${name} and revoked ${revoked}`, 'active', 'disabled');
		return this.#ctx.reply(structuredClone(account));
	}

	async enableAccount(name: string): Promise<Account> {
		this.#require('user.manage');
		const account = this.#data.accounts.find((a) => a.name === name);
		if (!account) throw new ApiError(`Unknown account ${name}`, 404);
		if (!account.disabled) throw new ApiError(`${name} is already active`, 409);
		account.disabled = false;
		this.#audit('user.manage', { locationId: null, subject: name }, `Enabled ${name}`, 'disabled', 'active');
		return this.#ctx.reply(structuredClone(account));
	}

	async createServiceAccount(request: ServiceAccountRequest): Promise<Account> {
		this.#require('user.manage');
		const name = request.name.trim();
		if (!/^[a-z][a-z0-9-]{2,39}$/.test(name)) throw new ApiError('Use 3 to 40 lowercase letters, digits or hyphens, starting with a letter', 422);
		if (this.#data.accounts.some((a) => a.name === name)) throw new ApiError(`An account named ${name} already exists`, 409);
		const description = request.description.trim();
		const account: Account = {
			kind: 'service_account',
			name,
			displayName: description || name,
			email: null,
			source: 'service_account',
			ssoGroups: [],
			createdAt: new Date().toISOString(),
			lastSeenAt: null,
			disabled: false,
			activeSessions: 0,
			owner: this.#ctx.user.name,
			description: description || null
		};
		this.#data.accounts.push(account);
		this.#audit('user.manage', { locationId: null, subject: name }, `Created service account ${name}`, null, name);
		return this.#ctx.reply(structuredClone(account));
	}

	async listServiceAccountTokens(): Promise<ApiToken[]> {
		this.#require('token.manage');
		return this.#ctx.reply(structuredClone(this.#data.apiTokens));
	}

	async createServiceAccountToken(account: string, request: ApiTokenRequest): Promise<IssuedToken<ApiToken>> {
		this.#require('token.manage');
		const owner = this.#data.accounts.find((a) => a.name === account);
		if (!owner) throw new ApiError(`Unknown account ${account}`, 404);
		if (owner.kind !== 'service_account') throw new ApiError('Only service accounts have API tokens; people sign in with sqb login', 422);
		if (owner.disabled) throw new ApiError(`${account} is disabled; enable it first`, 409);
		if (!request.label.trim()) throw new ApiError('A label is required', 422);
		const issued = this.#issueApiToken(request.label.trim(), owner, request.expiresInDays);
		this.#audit('token.manage', { locationId: null, subject: issued.token.id }, `Created token "${issued.token.label}" for ${owner.name}`, null, issued.token.prefix);
		return this.#ctx.reply(issued);
	}

	async rotateServiceAccountToken(tokenId: string): Promise<IssuedToken<ApiToken>> {
		this.#require('token.manage');
		const old = this.#data.apiTokens.find((t) => t.id === tokenId && t.state === 'active');
		if (!old) throw new ApiError('Token not found or not active', 404);
		old.state = 'revoked';
		old.revokedAt = new Date().toISOString();
		const remaining = old.expiresAt ? Math.max(1, Math.round((Date.parse(old.expiresAt) - Date.now()) / DAY)) : null;
		const owner = this.#data.accounts.find((a) => a.name === old.owner.name) ?? old.owner;
		const issued = this.#issueApiToken(old.label, owner, remaining);
		this.#audit('token.manage', { locationId: null, subject: issued.token.id }, `Rotated token "${old.label}"`, old.prefix, issued.token.prefix);
		return this.#ctx.reply(issued);
	}

	async revokeServiceAccountToken(tokenId: string): Promise<ApiToken> {
		this.#require('token.manage');
		const token = this.#data.apiTokens.find((t) => t.id === tokenId && t.state === 'active');
		if (!token) throw new ApiError('Token not found or not active', 404);
		token.state = 'revoked';
		token.revokedAt = new Date().toISOString();
		this.#audit('token.manage', { locationId: null, subject: token.id }, `Revoked token "${token.label}"`, token.prefix, null);
		return this.#ctx.reply(structuredClone(token));
	}

	// ---- roles and approvals ----

	async listRoles(locationId: string): Promise<RoleDefinition[]> {
		this.#require('role.assign');
		const roles = this.#data.roles.filter((r) => r.source !== 'project' || SHOP_LOCATIONS.includes(locationId));
		return this.#ctx.reply(structuredClone(roles));
	}

	async listRoleAssignments(): Promise<RoleAssignment[]> {
		this.#require('role.assign');
		return this.#ctx.reply(structuredClone(this.#data.assignments));
	}

	async createRoleAssignment(request: RoleAssignmentRequest): Promise<RoleAssignment> {
		this.#require('role.assign');
		const principal = this.#data.accounts.find((a) => a.name === request.principal);
		if (!principal) throw new ApiError(`Unknown principal ${request.principal}`, 404);
		if (!this.#data.roles.some((r) => r.name === request.role)) throw new ApiError(`Unknown role ${request.role}`, 404);
		const duplicate = this.#data.assignments.some(
			(a) => a.principal.name === principal.name && a.role === request.role && a.scope.locationId === request.scope.locationId
		);
		if (duplicate) throw new ApiError('That assignment already exists', 409);
		const assignment: RoleAssignment = {
			id: `asg-${randomHex(6)}`,
			principal: { kind: principal.kind, name: principal.name, displayName: principal.displayName },
			role: request.role,
			scope: request.scope,
			source: 'manual',
			ssoGroup: null,
			assignedBy: this.#ctx.user,
			assignedAt: new Date().toISOString(),
			stale: false
		};
		this.#data.assignments.push(assignment);
		this.#audit('role.assign', { ...request.scope, subject: principal.name }, `Assigned ${request.role} to ${principal.name}`, null, request.role);
		return this.#ctx.reply(structuredClone(assignment));
	}

	async deleteRoleAssignment(assignmentId: string): Promise<void> {
		this.#require('role.assign');
		const index = this.#data.assignments.findIndex((a) => a.id === assignmentId);
		if (index < 0) throw new ApiError('Assignment not found', 404);
		const assignment = this.#data.assignments[index];
		if (assignment.source === 'sso') throw new ApiError('SSO-derived assignments follow the group mapping in the server config', 409);
		const ownAdmin = this.#effectiveAssignments(this.#ctx.user.name).filter((a) => a.role === 'admin' && !a.stale);
		if (assignment.principal.name === this.#ctx.user.name && assignment.role === 'admin' && ownAdmin.length <= 1) {
			const scope = { ...assignment.scope, subject: assignment.principal.name };
			this.#audit('role.assign', scope, `Refused removing ${assignment.principal.name}'s last admin assignment`, 'admin', 'admin', 'denied');
			throw new ApiError("You can't remove your own last admin assignment: you would lose administration immediately. Ask another admin to remove it.", 409);
		}
		this.#data.assignments.splice(index, 1);
		this.#audit('role.assign', { ...assignment.scope, subject: assignment.principal.name }, `Removed ${assignment.role} from ${assignment.principal.name}`, assignment.role, null);
		return this.#ctx.reply(undefined);
	}

	async listRoleApprovals(): Promise<RoleApproval[]> {
		this.#require('role.assign');
		return this.#ctx.reply(structuredClone(this.#data.approvals));
	}

	async decideRoleApproval(approvalId: string, decision: 'approve' | 'reject', note: string): Promise<RoleApproval> {
		this.#require('role.assign');
		const approval = this.#data.approvals.find((a) => a.id === approvalId);
		if (!approval) throw new ApiError('Approval not found', 404);
		if (approval.state !== 'pending') throw new ApiError(`Already ${approval.state}`, 409);
		approval.state = decision === 'approve' ? 'approved' : 'rejected';
		approval.decidedBy = this.#ctx.user;
		approval.decidedAt = new Date().toISOString();
		approval.note = note.trim() || null;
		const role = this.#data.roles.find((r) => r.name === approval.role);
		if (role) {
			role.pendingApprovalId = null;
			if (decision === 'approve') role.grants = applyDiff(role.grants, approval);
		}
		this.#audit(decision === 'approve' ? 'role.approve' : 'role.reject', { locationId: approval.locationId, subject: approval.role }, `${decision === 'approve' ? 'Approved' : 'Rejected'} widening of ${approval.role}`, 'pending', approval.state);
		return this.#ctx.reply(structuredClone(approval));
	}

	// ---- audit and access ----

	async listAuditEntries(query: AuditQuery = {}): Promise<AuditEntry[]> {
		this.#require('audit.view');
		let entries = [...this.#data.auditEntries].sort((a, b) => b.at.localeCompare(a.at));
		if (query.actor) entries = entries.filter((e) => e.actor.name === query.actor);
		if (query.action) entries = entries.filter((e) => e.action === query.action);
		if (query.outcome) entries = entries.filter((e) => e.outcome === query.outcome);
		if (query.evidenceOnly) entries = entries.filter((e) => e.evidenceRead);
		if (query.limit) entries = entries.slice(0, query.limit);
		return this.#ctx.reply(structuredClone(entries));
	}

	async explainAccess(query: AccessQuery): Promise<AccessExplanation> {
		if (query.principal !== this.#ctx.user.name && !this.#can('role.assign') && !this.#can('audit.view')) {
			throw new ApiError('Explaining another principal requires role.assign or audit.view', 403);
		}
		const account = this.#data.accounts.find((a) => a.name === query.principal);
		if (!account) throw new ApiError(`Unknown principal ${query.principal}`, 404);
		const grantedBy: AccessGrantPath[] = [];
		const ignored: AccessExplanation['ignored'] = [];
		const assignments = this.#effectiveAssignments(account.name);
		for (const a of assignments) {
			if (a.scope.locationId !== null && a.scope.locationId !== query.locationId) continue;
			if (a.stale) {
				ignored.push({ assignmentId: a.id, role: a.role, reason: 'Role removed from code; the assignment grants nothing' });
				continue;
			}
			const grant = this.#matchingGrant(a.role, query);
			if (grant) {
				grantedBy.push({ role: a.role, assignmentId: a.id, assignmentSource: a.source, ssoGroup: a.ssoGroup, scope: a.scope, nodes: grant.nodes.includes('*') ? null : grant.nodes });
			}
		}
		const assigned = new Set(grantedBy.map((g) => g.role));
		const wouldBeGrantedBy = this.#data.roles
			.filter((r) => !assigned.has(r.name) && this.#matchingGrant(r.name, query) !== null)
			.map((r) => r.name);
		const allowed = grantedBy.length > 0;
		const on = query.subject ? `${query.subject} in ` : '';
		const where = `${on}${this.#locationName(query.locationId)}`;
		const summary = allowed
			? `${account.name} can ${query.action} on ${where}, granted by ${[...assigned].join(', ')}.`
			: `${account.name} can't ${query.action} on ${where}: no assigned role grants it${wouldBeGrantedBy.length ? `; ${wouldBeGrantedBy.join(', ')} would` : ''}.`;
		return this.#ctx.reply({ query, allowed, grantedBy, wouldBeGrantedBy, ignored, summary });
	}

	// ---- settings ----

	async serverSettings(): Promise<ServerSettings> {
		return this.#ctx.reply({
			serverVersion: this.#ctx.serverVersion,
			storage: 'postgresql',
			schemaVersion: 14,
			latestSchemaVersion: 14,
			schemaMigratedAt: new Date(this.#ctx.now - 5 * DAY).toISOString(),
			authMode: 'oidc',
			schedulerEnabled: true,
			retention: [
				{ name: 'Run history', value: '90 days', description: 'Runs, schedule history and their events.' },
				{ name: 'Run logs', value: '30 days', description: 'Captured run output.' },
				{ name: 'Platform events', value: '30 days', description: 'Problems such as late schedules. Where each sensor is up to is always kept.' },
				{ name: 'Security audit log', value: '400 days', description: 'Can’t be changed or deleted early. Forward events to keep them longer.' },
				{ name: 'Warehouse statistics', value: '90 days', description: 'Row counts and table sizes, sampled over time.' },
				{ name: 'Activity telemetry', value: '14 days', description: 'Page views, kept apart from the audit log.' }
			]
		});
	}

	// ---- helpers ----

	#locationDetail(location: Location): LocationDetail {
		const extras = this.#data.locationExtras[location.id];
		return structuredClone({
			id: location.id,
			name: location.name,
			health: location.health,
			...extras,
			target: location.target,
			snapshot: location.snapshot,
			connectionTest: this.#connectionTests.get(`${location.id}/${extras.warehouseConnection}`) ?? extras.connectionTest
		});
	}

	#warehouse(scope: Scope): WarehouseConnection[] {
		return this.#data.warehouseByLocation[scope.locationId] ?? [];
	}

	#locationName(locationId: string): string {
		return this.#ctx.locations.find((l) => l.id === locationId)?.name ?? locationId;
	}

	/** Manual and SSO-derived assignments of one principal. */
	#effectiveAssignments(principal: string): RoleAssignment[] {
		return this.#data.assignments.filter((a) => a.principal.name === principal);
	}

	#matchingGrant(roleName: string, query: AccessQuery): RoleGrant | null {
		const role = this.#data.roles.find((r) => r.name === roleName);
		if (!role) return null;
		if (role.source !== 'built_in' && ADMIN_ONLY.includes(query.action)) return null;
		if (role.source === 'project' && !SHOP_LOCATIONS.includes(query.locationId)) return null;
		return (
			role.grants.find(
				(g) => g.permissions.includes(query.action) && g.nodes.some((selector) => selector === '*' || (query.subject !== null && matchesSelector(selector, query.subject)))
			) ?? null
		);
	}

	#issueLocationToken(locationId: string): IssuedToken<LocationToken> {
		const secret = `sqbl_${randomHex(40)}`;
		const token: LocationToken = {
			id: `ltok-${randomHex(6)}`,
			locationId,
			prefix: secret.slice(0, 9),
			createdAt: new Date().toISOString(),
			createdBy: this.#ctx.user,
			lastUsedAt: null,
			revokedAt: null,
			state: 'active'
		};
		this.#data.locationTokens.unshift(token);
		return { token: structuredClone(token), secret };
	}

	#issueApiToken(label: string, owner: Principal, expiresInDays: number | null): IssuedToken<ApiToken> {
		const secret = `sqbt_${randomHex(40)}`;
		const token: ApiToken = {
			id: `tok-${randomHex(6)}`,
			label,
			owner: { kind: owner.kind, name: owner.name, displayName: owner.displayName },
			prefix: secret.slice(0, 9),
			createdAt: new Date().toISOString(),
			createdBy: this.#ctx.user,
			expiresAt: expiresInDays === null ? null : new Date(Date.now() + expiresInDays * DAY).toISOString(),
			lastUsedAt: null,
			revokedAt: null,
			state: 'active'
		};
		this.#data.apiTokens.unshift(token);
		return { token: structuredClone(token), secret };
	}

	#audit(
		action: AuditEntry['action'],
		scope: AuditEntry['scope'],
		summary: string,
		before: string | null = null,
		after: string | null = null,
		outcome: AuditEntry['outcome'] = 'allowed'
	) {
		this.#data.auditEntries.push({
			id: `aud-${randomHex(8)}`,
			at: new Date().toISOString(),
			actor: this.#ctx.user,
			action,
			outcome,
			scope,
			grantingRole: outcome === 'allowed' ? this.#ctx.role : null,
			evidenceRead: false,
			summary,
			before,
			after,
			ip: '127.0.0.1',
			requestId: `req-${randomHex(12)}`
		});
	}

	#can(capability: Capability): boolean {
		return ROLE_CAPABILITIES[this.#ctx.role].includes(capability);
	}

	#require(capability: Capability) {
		if (!this.#can(capability)) throw new ApiError(`Missing permission: ${capability}`, 403);
	}
}

function grantKey(grant: RoleGrant): string {
	return grant.nodes.join('|');
}

/** Applies an approved proposal: removals first, then additions merged per selector. */
function applyDiff(grants: RoleGrant[], approval: RoleApproval): RoleGrant[] {
	const next = grants.map((g) => ({ nodes: [...g.nodes], permissions: [...g.permissions] }));
	for (const removed of approval.removed) {
		const grant = next.find((g) => grantKey(g) === grantKey(removed));
		if (grant) grant.permissions = grant.permissions.filter((p) => !removed.permissions.includes(p));
	}
	for (const added of approval.added) {
		const grant = next.find((g) => grantKey(g) === grantKey(added));
		if (grant) grant.permissions = [...new Set([...grant.permissions, ...added.permissions])];
		else next.push({ nodes: [...added.nodes], permissions: [...added.permissions] });
	}
	return next.filter((g) => g.permissions.length > 0);
}
