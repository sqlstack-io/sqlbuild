import { describe, expect, it } from 'vitest';
import type { ApiClient, Capability } from '$lib/api';
import { pageAvailable, pagesBeyondTarget, visibleNav } from './nav';

const serving = (...methods: (keyof ApiClient)[]) => ({ supports: (method: keyof ApiClient) => methods.includes(method) });

const labels = (client: ReturnType<typeof serving>, capabilities: Capability[]) =>
	visibleNav(client, capabilities).map((group) => [group.section, group.items.map((item) => item.label)]);

describe('visibleNav', () => {
	it('shows only pages the server can load and drops empty groups', () => {
		expect(labels(serving('overview', 'listRunRows', 'listChecks'), ['view'])).toEqual([
			[null, ['Overview']],
			['Build', ['Checks']],
			['Operate', ['Runs']]
		]);
	});

	it('shows lineage with the catalog list, since the graph needs no node detail', () => {
		expect(labels(serving('listCatalogNodes'), ['view'])).toEqual([['Build', ['Lineage', 'Catalog']]]);
		expect(labels(serving('getCatalogNode'), ['view'])).toEqual([]);
	});

	it('hides admin pages from people without an admin capability, even when the server serves them', () => {
		expect(labels(serving('listAccounts', 'listRoles', 'listAuditEntries'), ['view', 'job.run'])).toEqual([]);
	});

	it('shows each admin page only for its own capabilities', () => {
		const client = serving('listAccounts', 'listRoles', 'listAuditEntries');
		expect(labels(client, ['audit.view'])).toEqual([['Admin', ['Audit log']]]);
		expect(labels(client, ['user.manage'])).toEqual([['Admin', ['People']]]);
		expect(labels(client, ['role.assign'])).toEqual([['Admin', ['People', 'Roles']]]);
	});
});

describe('pagesBeyondTarget', () => {
	it('offers a partial viewer only the pages that do not list the whole target', () => {
		const client = serving('overview', 'listCatalogNodes', 'listRunRows', 'listJobs', 'maintenanceHistory', 'listLocations', 'listAccounts', 'listAuditEntries');
		expect(pagesBeyondTarget(client, ['view', 'audit.view']).map((i) => i.label)).toEqual(['Locations', 'Audit log']);
		expect(pagesBeyondTarget(client, ['view']).map((i) => i.label)).toEqual(['Locations']);
	});
});

describe('pageAvailable', () => {
	const client = serving('listRunRows');

	it.each([
		['/runs', true],
		['/runs/0191f0aa-run', true],
		['/jobsheet', true],
		['/jobs', false],
		['/', false],
		['/catalog/model:orders', false],
		['/settings', true]
	])('%s → %s', (path, expected) => {
		expect(pageAvailable(client, path)).toBe(expected);
	});
});
