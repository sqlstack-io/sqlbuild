import { describe, expect, it } from 'vitest';
import { declarationFolder, memberFolderTree, membersIn, memberStandIn } from './member-folders';

const files: Record<string, string> = {
	load_orders: 'jobs/load/orders.py:3',
	load_customers: 'jobs/load/customers.py:3',
	build_orders: 'jobs/build/orders/daily.py:1',
	build_returns: 'jobs/build/orders/returns.py:1',
	build_regions: 'jobs/build/regions.py:1',
	export_inventory: 'jobs/exports/inventory.py:9',
	cleanup: 'jobs/cleanup.py:1'
};
const members = Object.keys(files);
const tree = memberFolderTree(members, (m) => declarationFolder(files[m]));

describe('member folders', () => {
	it('takes the folder from a declaration', () => {
		expect(declarationFolder('jobs/orders/daily.py:12')).toBe('jobs/orders');
		expect(declarationFolder('daily.py:12')).toBe('');
		expect(declarationFolder(null)).toBe('');
	});

	it('opens the folders every member shares, and keeps the rest as Lineage does', () => {
		expect(tree.roots).toEqual(['jobs/load', 'jobs/build']);
		expect(tree.folders.get('jobs/load')!.label).toBe('load');
		expect(tree.folders.get('jobs/build')!.folders).toEqual(['jobs/build/orders']);
		expect(tree.folders.get('jobs/build/orders')!.label).toBe('orders');
		expect(membersIn(tree, 'jobs/build')).toEqual(['build_regions', 'build_orders', 'build_returns']);
	});

	it('draws a folder holding one member as that member’s card', () => {
		expect(tree.folders.has('jobs/exports')).toBe(false);
		expect(tree.loose).toEqual(['export_inventory', 'cleanup']);
		expect(tree.folders.get('jobs/build')!.members).toEqual(['build_regions']);
	});

	it('stands a member in for its outermost closed folder', () => {
		const standIn = memberStandIn(tree, new Set(['jobs/build']));
		expect(standIn('build_orders')).toBe('jobs/build/orders');
		expect(standIn('build_regions')).toBe(null);
		expect(standIn('load_orders')).toBe('jobs/load');
		expect(standIn('cleanup')).toBe(null);
	});
});
