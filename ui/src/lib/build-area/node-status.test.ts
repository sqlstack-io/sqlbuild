import { describe, expect, it } from 'vitest';
import type { CatalogNode, NodeBuildStatus } from '$lib/api';
import { groupStatusOf } from '$lib/api/http-overview';
import { folderTree } from '$lib/build-area/folder-tree';
import { NODE_STATUS, statusLabel } from '$lib/build-area/node-status';
import { cardData } from '$lib/lineage/catalog-graph';
import { viewSummary } from '$lib/lineage/view-summary';

const KINDS: { kind: CatalogNode['kind']; path: string }[] = [
	{ kind: 'table', path: 'sql/models/marts/orders.sql' },
	{ kind: 'view', path: 'sql/models/marts/orders_view.sql' },
	{ kind: 'incremental', path: 'sql/models/marts/events.sql' },
	{ kind: 'seed', path: 'sql/seeds/regions.csv' },
	{ kind: 'udf', path: 'sql/functions/clean.sql' },
	{ kind: 'asset', path: 'python/nodes/exports/export_csv.py' },
	{ kind: 'loader', path: 'python/nodes/loaders/raw_orders.py' }
];
const BUILT_STATUSES: NodeBuildStatus[] = ['current', 'changed', 'behind', 'failed', 'not_built'];

/**
 * A node whose raw fields say the opposite of its status (no build, a successful run, no stale
 * reasons), so a page that worked the status out again from them would disagree with the server.
 */
function node(kind: CatalogNode['kind'], path: string, status: NodeBuildStatus): CatalogNode {
	return {
		key: `${kind}:n`,
		kind,
		materialization: null,
		group: null,
		tags: [],
		description: '',
		path,
		upstream: [],
		relation: null,
		columnCount: 0,
		lastBuild: null,
		lastOutcome: status === 'not_built' ? 'success' : null,
		stale: [],
		sourceFreshness: null,
		checks: null,
		status,
		statusAt: status === 'not_built' ? null : '2026-11-24T06:00:00Z'
	};
}

/** What each page shows for one node, as each page's own helper works it out. */
function pages(n: CatalogNode) {
	const [folder] = folderTree([n]);
	const [group] = groupStatusOf([n]);
	const summary = viewSummary([n]);
	const card = cardData(n);
	const counted = (counts: { current: number; changed: number; behind: number; failed: number }) => {
		const shown = (['current', 'changed', 'behind', 'failed'] as const).filter((s) => counts[s] > 0);
		return shown.length ? shown : ['not_built'];
	};
	const lineage = card.status === 'fail' ? 'failed' : card.status === 'none' ? 'not_built' : card.stale === 'behind' ? 'behind' : card.stale === 'changed' || card.stale === 'compiled' ? 'changed' : 'current';
	const needs = summary.failed ? 'failed' : summary.changed ? 'changed' : summary.behind ? 'behind' : null;
	return {
		folderTreeCounts: counted(folder),
		folderTreeRow: statusLabel(n)?.label,
		catalogList: statusLabel(n)?.label,
		nodePage: statusLabel(n)?.label,
		lineageCard: lineage,
		lineageSummary: needs ?? (lineage === 'not_built' ? 'not_built' : 'current'),
		overviewCounts: counted(group)
	};
}

describe('every page shows the server’s status', () => {
	for (const { kind, path } of KINDS) {
		it.each(BUILT_STATUSES)(`${kind} that is %s`, (status) => {
			const shown = pages(node(kind, path, status));
			expect(shown).toEqual({
				folderTreeCounts: [status],
				folderTreeRow: NODE_STATUS[status].label,
				catalogList: NODE_STATUS[status].label,
				nodePage: NODE_STATUS[status].label,
				lineageCard: status,
				lineageSummary: status,
				overviewCounts: [status]
			});
		});
	}

	it('shows a source as a source and leaves it out of the folder counts', () => {
		const source = node('source', 'sql/sources/raw.yml', 'source');
		expect([statusLabel(source)?.label, cardData(source).status, folderTree([source]), groupStatusOf([source])]).toEqual(['Source', 'src', [], []]);
	});

	it('shows nothing it would have to guess when the server has not reported a status', () => {
		const unknown = { ...node('table', 'sql/models/marts/orders.sql', 'current'), status: null };
		expect([statusLabel(unknown), cardData(unknown).status]).toEqual([null, 'none']);
	});
});
