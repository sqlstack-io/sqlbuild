import type { ApiClient, CostGroup, Scope } from '$lib/api';
import { MockApiClient } from '$lib/api/mock/mock-client';
import { mockCostPage } from '$lib/api/mock/cost-page';
import type { CostPage, CostRow, Period, PriceSource } from '../types';

const row = (g: CostGroup, key: string): CostRow => ({ key, credits: g.credits, previous: 0, seconds: g.seconds, runs: g.runs, nodes: 0 });

/**
 * The Cost page's read. The mock answers the full page; the server's summary today fills only the
 * totals, jobs, warehouse sizes and models, so the rest stays empty (see the design notes).
 */
export async function loadCost(client: ApiClient, scope: Scope, days: Period, source: PriceSource): Promise<CostPage> {
	if (client instanceof MockApiClient) return mockCostPage(days, source);
	const s = await client.getCostSummary(scope, days);
	const usdPerCredit = s.usd !== null && s.credits > 0 ? s.usd / s.credits : 0;
	return {
		days,
		since: s.since,
		price: { usdPerCredit, source: 'config', detail: '' },
		credits: s.credits,
		previous: 0,
		seconds: s.byJob.reduce((t, g) => t + g.seconds, 0),
		runCount: s.runs,
		incompleteRuns: s.incompleteRuns,
		jobs: s.byJob.map((g) => row(g, g.key ?? 'Manual or CLI')),
		folders: [],
		sizes: s.byWarehouseSize.map((g) => row(g, g.key ?? 'Unknown size')),
		models: s.byNode.map((g) => ({ ...row(g, g.key ?? ''), folder: '', job: '', queries: 0, bytesScanned: 0 })),
		runs: [],
		daily: []
	};
}
