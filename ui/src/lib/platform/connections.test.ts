import { describe, expect, it } from 'vitest';
import type { ConnectionDetails, ConnectionTest, WarehouseConnection, WarehouseTableSync } from '$lib/api';
import { detailRows, formatCount, groupConnections, lastCheckLine, lastFullSync, syncSummary } from './connections';

const MINUTE = 60_000;
const now = Date.parse('2026-11-16T12:00:00Z');
const ago = (minutes: number) => new Date(now - minutes * MINUTE).toISOString();

const table = (name: string, minutesAgo: number | null, mirroredTotal: number | null = null): WarehouseTableSync => ({
	table: name,
	at: ago(minutesAgo ?? 1),
	recordsSent: 5,
	elapsedMs: 200,
	failure: null,
	waiting: false,
	lastSyncedAt: minutesAgo === null ? null : ago(minutesAgo),
	mirroredTotal
});

const details: ConnectionDetails = { adapter: 'snowflake', account: 'example-account', host: null, user: 'SQB_BUILDER', role: 'TRANSFORMER', warehouse: 'ORDERS_WH', database: 'ORDERS', authMethod: 'key_pair' };

const connection = (id: string, d: ConnectionDetails | null): WarehouseConnection => ({ id, location: 'Orders', target: id, details: d, stateLocation: null, lastTest: null, nextSyncAt: null, tables: [] });

describe('connection details', () => {
	it('keeps the same rows before details are published', () => {
		expect(detailRows(null).map((r) => [r.label, r.value])).toEqual([
			['Adapter', null],
			['Account', null],
			['User', null],
			['Role', null],
			['Warehouse', null],
			['Default database', null],
			['Authentication', null]
		]);
	});

	it('labels the sign-in method by name and shows a host when there is no account', () => {
		expect(detailRows(details).at(-1)).toEqual({ label: 'Authentication', value: 'Key pair' });
		expect(detailRows({ ...details, account: null, host: 'db.example.com' })[1]).toEqual({ label: 'Host', value: 'db.example.com' });
	});

	it('groups targets that publish the same connection, and keeps unpublished ones apart', () => {
		const groups = groupConnections([connection('prod', details), connection('preview', details), connection('dev', null), connection('test', null)]);
		expect(groups.map((g) => g.users.map((u) => u.id))).toEqual([['prod', 'preview'], ['dev'], ['test']]);
	});
});

describe('last check', () => {
	it('reads like "passed at 01:22 (startup) · 410 ms" in local time', () => {
		const at = new Date(2026, 10, 16, 1, 22).toISOString();
		const test: ConnectionTest = { outcome: 'ok', at, durationMs: 410, trigger: 'startup', by: null, message: '' };
		expect(lastCheckLine(test)).toBe('passed at 01:22 (startup) · 410 ms');
		expect(lastCheckLine({ ...test, outcome: 'failed', trigger: 'publish', durationMs: 0 })).toBe('failed at 01:22 (after deploy)');
	});
});

describe('state history sync summary', () => {
	it('counts tables and dates the last full sync by the table synced longest ago', () => {
		const tables = [table('orders', 13), table('customers', 4)];
		expect(lastFullSync(tables)).toBe(ago(13));
		expect(syncSummary({ tables, nextSyncAt: null }, now)).toBe('2 tables · last full sync 13m ago');
		expect(lastFullSync([...tables, table('inventory', null)])).toBeNull();
	});

	it('adds the mirrored total and the next sync once reported', () => {
		const tables = [table('orders', 13, 120_000), table('customers', 4, 10_400)];
		expect(syncSummary({ tables, nextSyncAt: new Date(now + 17 * MINUTE).toISOString() }, now)).toBe('2 tables · last full sync 13m ago · 130k records mirrored · next in 17m');
	});

	it('shortens large counts', () => {
		expect([formatCount(950), formatCount(1_200), formatCount(130_400), formatCount(2_340_000)]).toEqual(['950', '1.2k', '130k', '2.3M']);
	});
});
