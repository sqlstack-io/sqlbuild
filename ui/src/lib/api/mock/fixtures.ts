// Neutral example data for the mock client: a fictional waffle shop's orders,
// customers and payments. Times are relative to `now` so ages stay realistic.

import type { BuiltInRole, Capability, Location, PlatformEvent, Snapshot } from '../types';
import { CAPABILITIES } from '../types';

const MIN = 60_000;

const OPERATE: Capability[] = ['view', 'view.sql', 'job.run', 'run.cancel', 'run.rerun', 'trigger.toggle', 'sensor.reset_cursor'];
const DEVELOP: Capability[] = [...OPERATE, 'view.evidence', 'run.rewrite_history', 'run.adhoc', 'connection.test', 'plan.preview'];

/** Mock-only approximation of the built-in roles; the server owns the real grants. */
export const ROLE_CAPABILITIES: Record<BuiltInRole, Capability[]> = {
	viewer: ['view'],
	operator: OPERATE,
	developer: DEVELOP,
	admin: [...CAPABILITIES]
};

export const LOCATION_SHOP = '0d9c6a52-3f1e-4b7a-9c43-5b8e2f61a7d4';
export const LOCATION_SHOP_STAGING = '5e8b1f37-a04c-4c2e-b6d9-2f7a91c3e508';
export const LOCATION_SHOP_DEV = 'c2f47d90-1b6e-4a85-93fd-6e0a5b28c171';
export const LOCATION_MARKETING = '7a41e0c8-62b5-4d19-8f3a-c1e95d0b2e66';
/** The waffle-shop project's locations, one per target; they share the project's roles and providers. */
export const SHOP_LOCATIONS: readonly string[] = [LOCATION_SHOP, LOCATION_SHOP_STAGING, LOCATION_SHOP_DEV];

export function createFixtures(now: number) {
	const ago = (minutes: number) => new Date(now - minutes * MIN).toISOString();

	const snapshot = (generation: number, revision: string, minutesAgo: number, author: string, dirty = false): Snapshot => ({
		id: `snap-${revision}`,
		generation,
		gitRevision: revision,
		gitDirty: dirty,
		gitAuthor: author,
		publishedAt: ago(minutesAgo),
		sqbVersion: '0.185.0'
	});

	const idleSync = (minutesAgo: number) => ({
		lastSyncedAt: ago(minutesAgo),
		lastError: null
	});

	const locations: Location[] = [
		{
			id: LOCATION_SHOP,
			name: 'waffle-shop',
			health: 'healthy',
			lastSeenAt: ago(0.2),
			target: { name: 'prod', adapter: 'snowflake', connectionSummary: 'WAFFLE_SHOP on example-account' },
			snapshot: snapshot(42, '7d2e41a', 190, 'Alex Rivera'),
			warehouseSync: idleSync(4)
		},
		{
			id: LOCATION_SHOP_STAGING,
			name: 'waffle-shop-staging',
			health: 'degraded',
			lastSeenAt: ago(0.1),
			target: { name: 'staging', adapter: 'snowflake', connectionSummary: 'WAFFLE_SHOP_STAGING on example-account' },
			snapshot: snapshot(57, 'b81c09e', 35, 'Sam Okafor'),
			warehouseSync: idleSync(11)
		},
		{
			id: LOCATION_SHOP_DEV,
			name: 'waffle-shop-dev',
			health: 'healthy',
			lastSeenAt: ago(0.3),
			target: { name: 'dev', adapter: 'duckdb', connectionSummary: 'warehouse.duckdb' },
			snapshot: snapshot(311, 'b81c09e', 2, 'Demo User', true),
			warehouseSync: idleSync(1)
		},
		{
			id: LOCATION_MARKETING,
			name: 'marketing-analytics',
			health: 'falling_back',
			lastSeenAt: ago(0.5),
			target: { name: 'prod', adapter: 'postgres', connectionSummary: 'analytics on db.example.com' },
			snapshot: snapshot(18, '3e5f7aa', 1440, 'Jordan Lee'),
			warehouseSync: idleSync(26)
		}
	];

	const platformEvents: PlatformEvent[] = [
		{ id: 'pe-1', kind: 'location_broken', severity: 'warning', occurredAt: ago(48), active: true, locationId: LOCATION_MARKETING, subject: 'marketing-analytics', message: 'marketing-analytics stopped checking in, so its scheduled runs aren’t starting.' },
		{ id: 'pe-2', kind: 'tick_overdue', severity: 'warning', occurredAt: ago(12), active: true, locationId: LOCATION_SHOP, subject: 'segments_hourly', message: 'The segments_hourly schedule should have started a run 12 minutes ago.' },
		{ id: 'pe-6', kind: 'warehouse_sync_failed', severity: 'warning', occurredAt: ago(95), active: true, locationId: LOCATION_SHOP_STAGING, subject: '_sqlbuild_janitor_events', message: 'Permission denied on _sqlbuild_janitor_events: the catalog connection’s role can’t use schema ARCHIVE, so cleanup history isn’t up to date.' },
		{ id: 'pe-3', kind: 'heartbeat', severity: 'info', occurredAt: ago(1), active: false, locationId: null, subject: null, message: 'Server heartbeat.' }
	];
	const extraPlatformEvents: PlatformEvent[] = [
		{ id: 'pe-4', kind: 'scheduler_no_leader', severity: 'critical', occurredAt: ago(2), active: true, locationId: null, subject: null, message: 'No server is running the scheduler, so schedules and sensors aren’t starting runs.' },
		{ id: 'pe-5', kind: 'launch_failed', severity: 'critical', occurredAt: ago(6), active: true, locationId: LOCATION_SHOP, subject: 'revenue_refresh', message: 'The last 3 runs couldn’t start: the image couldn’t be downloaded.' }
	];

	return { locations, platformEvents, extraPlatformEvents };
}

export type Fixtures = ReturnType<typeof createFixtures>;
