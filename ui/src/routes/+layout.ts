import type { ApiClient, Scope } from '$lib/api';
import { app } from '$lib/state/app.svelte';
import { prefetch } from '$lib/state/prefetch';
import { readParams, toQuery } from './runs/utils';
import type { LayoutLoad } from './$types';

// SPA mode: no SSR, no prerender. All data is fetched client-side, so the build
// is a static bundle served by the SQLBuild server.
export const ssr = false;
export const prerender = false;
export const trailingSlash = 'never';

type Read = (client: ApiClient, scope: Scope, match: RegExpMatchArray, search: URLSearchParams) => Promise<unknown>[];

const part = (match: RegExpMatchArray) => decodeURIComponent(match[1]);

/** Each page's first reads, with the same arguments the page passes, so the page takes them over. */
const PAGE_READS: [RegExp, Read][] = [
	[/^\/$/, (c, s) => [c.overview(s)]],
	[
		/^\/runs$/,
		(c, s, _, search) => {
			const params = readParams(search);
			return [c.listRunRows(s, { ...toQuery(params, Date.now()), cursor: null, limit: 50 }), c.runTabCounts(s, toQuery(params, Date.now(), 'all')), c.listQueue(s)];
		}
	],
	[/^\/runs\/([^/]+)$/, (c, s, m) => [c.getRunDetail(s, part(m))]],
	[/^\/jobs$/, (c, s) => [c.listJobs(s), c.listTriggers(s)]],
	[/^\/jobs\/([^/]+)$/, (c, s, m) => [c.getJob(s, part(m)), c.listRuns(s, { jobKey: part(m), limit: 50 }), c.listTriggers(s), c.listCatalogNodes(s)]],
	[/^\/triggers$/, (c, s) => [c.listTriggers(s)]],
	[/^\/triggers\/([^/]+)$/, (c, s, m) => [c.getTrigger(s, part(m))]],
	[/^\/catalog$/, (c, s) => [c.listCatalogNodes(s)]],
	[/^\/catalog\/([^/]+)$/, (c, s, m) => [c.getCatalogNode(s, part(m)), c.listCatalogNodes(s), c.getColumnLineage(s, part(m))]],
	[/^\/checks$/, (c, s) => [c.listChecks(s)]],
	[/^\/checks\/([^/]+)$/, (c, s, m) => [c.getCheck(s, part(m))]],
	[/^\/changes$/, (c, s) => [c.listSnapshotChanges(s)]],
	[/^\/lineage$/, (c, s) => [c.listCatalogNodes(s)]],
	[/^\/maintenance$/, (c, s) => [c.maintenanceHistory(s)]],
	[/^\/locations$/, (c) => [c.listLocations(), c.platformHealth()]],
	[/^\/locations\/([^/]+)$/, (c, _, m) => [c.getLocation(part(m))]],
	[/^\/connections$/, (c) => [c.listWarehouseConnections()]],
	[/^\/providers(\/[^/]+)?$/, (c, s) => [c.listProviders(s)]]
];

// Reads only the path, so filter changes within a page don't wait on this.
export const load: LayoutLoad = async ({ url, untrack }) => {
	const search = untrack(() => url.searchParams);
	await prefetch(app.scope, url.pathname, search, PAGE_READS);
};
