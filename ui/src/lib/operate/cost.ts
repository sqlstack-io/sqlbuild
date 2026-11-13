import type { CatalogNode, CostGroup, RunCostBreakdown, RunCostRow, RunCostStatus } from '$lib/api';
import { topFolder } from '$lib/lineage/catalog-graph';

/** The resource type of the row carrying a collection's totals. */
export const TOTAL = '_total';

/** Shown wherever figures read in credits, because the project set no credit price. */
export const CREDIT_PRICE_HINT = 'Set usd_per_credit under [cost] in sqlbuild_project.toml to see dollars.';

/** Node costs summed per top-level folder, most expensive first; nodes no longer deployed count as `other`. */
export function costByFolder(byNode: CostGroup[], nodes: CatalogNode[]): { key: string; credits: number; usd: number | null; nodes: number }[] {
	const folders = new Map(nodes.map((n) => [n.key, topFolder(n)]));
	const totals = new Map<string, { key: string; credits: number; usd: number | null; nodes: number }>();
	for (const g of byNode) {
		const key = (g.key && folders.get(g.key)) ?? 'other';
		const t = totals.get(key) ?? { key, credits: 0, usd: 0, nodes: 0 };
		t.credits += g.credits;
		t.usd = sumUsd(t.usd, g.usd);
		t.nodes += 1;
		totals.set(key, t);
	}
	return [...totals.values()].sort((a, b) => b.credits - a.credits);
}

/** Least complete first, so a group run shows the weakest of its job runs' collections. */
const STATUS_RANK: Record<RunCostStatus, number> = { none: 0, collection_failed: 1, unavailable: 2, pending: 3, partial: 4, complete: 5 };

const sumUsd = (a: number | null, b: number | null): number | null => (a == null || b == null ? null : a + b);

/** Totals from each collection's run row; resources and warehouses folded from the other rows. */
export function runCostOf(rows: RunCostRow[]): RunCostBreakdown {
	const runs = rows.filter((r) => r.resourceType === TOTAL);
	const parts = rows.filter((r) => r.resourceType !== TOTAL);
	const status: RunCostStatus = runs.length === 0 ? 'none' : runs.map((r) => r.status).reduce((worst, s) => (STATUS_RANK[s] < STATUS_RANK[worst] ? s : worst));
	const resources = new Map<string, RunCostBreakdown['resources'][number]>();
	const warehouses = new Map<string, RunCostBreakdown['warehouses'][number]>();
	for (const r of parts) {
		const nodeKey = r.nodeKey ?? r.resourceName;
		const resource = resources.get(nodeKey) ?? { nodeKey, credits: 0, usd: 0, seconds: 0, queryCount: 0, bytesScanned: 0, warehouses: [] };
		resource.credits += r.credits;
		resource.usd = sumUsd(resource.usd, r.usd);
		resource.seconds += r.seconds;
		resource.queryCount += r.queryCount;
		resource.bytesScanned += r.bytesScanned;
		if (r.warehouseName && !resource.warehouses.includes(r.warehouseName)) resource.warehouses.push(r.warehouseName);
		resources.set(nodeKey, resource);
		const warehouseKey = `${r.warehouseName}|${r.warehouseSize}`;
		const warehouse = warehouses.get(warehouseKey) ?? { name: r.warehouseName, size: r.warehouseSize, credits: 0, usd: 0, seconds: 0, queryCount: 0 };
		warehouse.credits += r.credits;
		warehouse.usd = sumUsd(warehouse.usd, r.usd);
		warehouse.seconds += r.seconds;
		warehouse.queryCount += r.queryCount;
		warehouses.set(warehouseKey, warehouse);
	}
	const byCost = <T extends { credits: number }>(a: T, b: T) => b.credits - a.credits;
	return {
		status,
		credits: runs.reduce((total, r) => total + r.credits, 0),
		usd: runs.length === 0 ? null : runs.map((r) => r.usd).reduce(sumUsd),
		seconds: runs.reduce((total, r) => total + r.seconds, 0),
		queryCount: runs.reduce((total, r) => total + r.queryCount, 0),
		statementsExpected: runs.reduce((total, r) => total + r.statementsExpected, 0),
		statementsObserved: runs.reduce((total, r) => total + r.statementsObserved, 0),
		resources: [...resources.values()].sort(byCost),
		warehouses: [...warehouses.values()].sort(byCost)
	};
}

/** Dollars when the project priced credits; otherwise credits, so no price is ever guessed. */
export function formatQueryCost(cost: { credits: number; usd: number | null }): string {
	if (cost.usd != null) return cost.usd < 0.01 && cost.usd > 0 ? '< $0.01' : `$${cost.usd.toFixed(2)}`;
	return `${cost.credits < 0.001 && cost.credits > 0 ? '< 0.001' : cost.credits.toFixed(3)} credits`;
}

/** What the run's cost figure says; missing data never reads as $0. */
export function costSummary(cost: RunCostBreakdown): { text: string; note: string | null } {
	switch (cost.status) {
		case 'none':
			return { text: 'No cost yet', note: null };
		case 'pending':
			return { text: 'Cost pending', note: 'The warehouse has not reported this run\'s queries yet.' };
		case 'collection_failed':
			return { text: 'Cost unavailable', note: 'Reading the warehouse query history failed; a later run retries.' };
		case 'unavailable':
			return { text: 'Cost unavailable', note: null };
		case 'partial':
			// Every statement can be seen while the warehouse still left out some of their details.
			return {
				text: formatQueryCost(cost),
				note: cost.statementsObserved < cost.statementsExpected ? `Partial: ${cost.statementsObserved} of ${cost.statementsExpected} statements` : 'Partial: the warehouse left out some query details'
			};
		case 'complete':
			return { text: formatQueryCost(cost), note: null };
	}
}
