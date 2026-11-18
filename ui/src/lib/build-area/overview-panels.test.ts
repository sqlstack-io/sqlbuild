import { describe, expect, it } from 'vitest';
import type { GroupStatus, SnapshotChange } from '$lib/api';
import { quietDeployLine, recentDeployRows, recentDeploys, sortFolders, cappedFolders } from './overview-panels';

const HOUR = 60 * 60 * 1000;
const now = Date.parse('2026-11-16T12:00:00Z');
const at = (hoursAgo: number) => new Date(now - hoursAgo * HOUR).toISOString();

const deploy = (id: string, hoursAgo: number, changed: boolean): SnapshotChange => ({
	snapshotId: id,
	generation: 0,
	previousGeneration: null,
	gitRevision: null,
	gitDirty: false,
	gitAuthor: null,
	publishedAt: at(hoursAgo),
	publishedBy: null,
	nodes: changed ? [{ key: `model:orders_${id}`, kind: null, op: 'changed', formerKey: null, compiledOnly: false, detail: '', columns: [] }] : [],
	orchestration: [],
	roles: [],
	environment: null,
	followingRuns: [],
	costChanges: null
});

describe('recent deploys', () => {
	// Newest first: two quiet, A, B, three quiet, C, D, then older ones the panel never needs.
	const feed = [
		deploy('q1', 1, false),
		deploy('q2', 2, false),
		deploy('a', 3, true),
		deploy('b', 4, true),
		deploy('q3', 7, false),
		deploy('q4', 8, false),
		deploy('q5', 10, false),
		deploy('c', 11, true),
		deploy('d', 12, true),
		deploy('q6', 13, false),
		deploy('e', 14, true)
	];

	it('takes deploys up to the fourth that changed something', () => {
		expect(recentDeploys(feed).map((c) => c.snapshotId)).toEqual(['q1', 'q2', 'a', 'b', 'q3', 'q4', 'q5', 'c', 'd']);
	});

	it('merges each run of deploys without changes into one row', () => {
		const rows = recentDeployRows(recentDeploys(feed));
		expect(rows.map((r) => (r.kind === 'deploy' ? r.change.snapshotId : `${r.count} quiet`))).toEqual(['2 quiet', 'a', 'b', '3 quiet', 'c', 'd']);
	});

	it('words a merged row with the span of its deploys', () => {
		const [, , , quiet] = recentDeployRows(recentDeploys(feed));
		expect(quiet.kind === 'quiet' && quietDeployLine(quiet, now)).toBe('3 deploys without changes · 7h–10h ago');
		const [single] = recentDeployRows([deploy('q', 2, false)]);
		expect(single.kind === 'quiet' && quietDeployLine(single, now)).toBe('1 deploy without changes · 2h ago');
	});

	it('counts a role or environment change as a change', () => {
		const roleOnly = { ...deploy('r', 1, false), roles: [{}] } as unknown as SnapshotChange;
		expect(recentDeployRows([roleOnly]).map((r) => r.kind)).toEqual(['deploy']);
	});
});

const folder = (group: string, total: number, failed = 0, changed = 0, behind = 0): GroupStatus => ({ group, total, current: total - failed - changed, changed, behind, failed });

describe('build status by folder', () => {
	it('sorts by failed, then changed, then behind, then size, then name', () => {
		const sorted = sortFolders([folder('orders', 5), folder('customers', 9), folder('inventory', 3, 0, 0, 2), folder('billing', 4, 0, 1), folder('returns', 2, 1), folder('alpha', 9)]);
		expect(sorted.map((g) => g.group)).toEqual(['returns', 'billing', 'inventory', 'alpha', 'customers', 'orders']);
	});

	it('shows at most eight folders, most urgent first, however many need attention', () => {
		const calm = Array.from({ length: 12 }, (_, i) => folder(`calm_${String(i).padStart(2, '0')}`, 20 - i));
		const attention = Array.from({ length: 10 }, (_, i) => folder(`busy_${i}`, 1, 0, 0, 1));
		expect(cappedFolders(calm).rows.map((g) => g.group)).toEqual(calm.slice(0, 8).map((g) => g.group));
		const mixed = cappedFolders([...calm, ...attention]);
		expect(mixed.rows).toHaveLength(8);
		expect(mixed.rows.every((g) => g.behind)).toBe(true);
		expect([mixed.total, mixed.attention]).toEqual([22, 10]);
		expect(cappedFolders(calm.slice(0, 3))).toMatchObject({ total: 3, attention: 0 });
	});
});
