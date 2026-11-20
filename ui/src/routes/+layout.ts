import type { ApiClient, Scope } from '$lib/api';
import { app } from '$lib/state/app.svelte';
import { prefetch } from '$lib/state/prefetch';
import { catalogNodes } from '$lib/state/catalog.svelte';
import { sharedChecks } from '$lib/state/checks.svelte';
import { appPath } from '$lib/paths';
import { readParams, toQuery } from './runs/utils';
import type { LayoutLoad } from './$types';

// SPA mode: no SSR, no prerender. All data is fetched client-side, so the build
// is a static bundle served by the SQLBuild server.
export const ssr = false;
export const prerender = false;
export const trailingSlash = 'never';

type Read = (client: ApiClient, scope: Scope, match: RegExpMatchArray, search: URLSearchParams) => Promise<unknown>[];

const part = (match: RegExpMatchArray) => decodeURIComponent(match[1]);

/** The shared catalog store's listing, which these pages read; the store holds it, so pages never read it themselves. */
const CATALOG_PAGES = /^\/(?:$|lineage$|catalog(?:\/[^/]+)?$|jobs\/[^/]+$|plan(?:\/rerun)?$)/;
// Pages that read the shared checks.
const CHECK_PAGES = /^\/(?:checks|plan|catalog\/[^/]+)$/;

/** Each page's first reads, with the same arguments the page passes, so the page takes them over. */
const PAGE_READS: [RegExp, Read][] = [
	[/^\/$/, () => []],
	[
		/^\/runs$/,
		(c, s, _, search) => {
			const params = readParams(search);
			return [c.listRunRows(s, { ...toQuery(params, Date.now()), cursor: null, limit: 50 }), c.runTabCounts(s, toQuery(params, Date.now(), 'all')), c.listQueue(s)];
		}
	],
	[/^\/runs\/([^/]+)$/, (c, s, m) => [c.getRunDetail(s, part(m))]],
	[/^\/jobs$/, (c, s) => [c.listJobs(s), c.listTriggers(s)]],
	[/^\/jobs\/([^/]+)$/, (c, s, m) => [c.getJob(s, part(m)), c.listRuns(s, { jobKey: part(m), limit: 50 }), c.listTriggers(s)]],
	[/^\/triggers$/, (c, s) => [c.listTriggers(s)]],
	[/^\/triggers\/([^/]+)$/, (c, s, m) => [c.getTrigger(s, part(m))]],
	[/^\/catalog$/, () => []],
	[/^\/catalog\/([^/]+)$/, (c, s, m) => [c.getCatalogNode(s, part(m), catalogNodes.view.nodes, sharedChecks.view.checks), c.getColumnLineage(s, part(m), catalogNodes.view.nodes)]],
	[/^\/checks$/, () => []],
	[/^\/checks\/([^/]+)$/, (c, s, m) => [c.getCheck(s, part(m))]],
	[/^\/changes$/, (c, s) => [c.listSnapshotChanges(s)]],
	[/^\/lineage$/, () => []],
	[/^\/maintenance$/, (c, s) => [c.maintenanceHistory(s)]],
	[/^\/locations$/, (c) => [c.listLocations(), c.platformHealth()]],
	[/^\/locations\/([^/]+)$/, (c, _, m) => [c.getLocation(part(m))]],
	[/^\/connections$/, (c) => [c.listWarehouseConnections()]],
	[/^\/providers(\/[^/]+)?$/, (c, s) => [c.listProviders(s)]]
];

// The first paint waits for the web fonts, up to a limit, so text doesn't reflow when they arrive.
// The build imports this module outside a browser to read its options.
const FONTS = ['13px "Inter Variable"', '600 15px "Space Grotesk Variable"', '12px "IBM Plex Mono"', '500 12px "IBM Plex Mono"'];
const fontsReady =
	typeof document === 'undefined'
		? Promise.resolve()
		: Promise.race([Promise.all(FONTS.map((font) => document.fonts.load(font))).catch(() => undefined), new Promise((resolve) => setTimeout(resolve, 1500))]);

// Reads only the path, so filter changes within a page don't wait on this.
export const load: LayoutLoad = async ({ url, untrack }) => {
	const search = untrack(() => url.searchParams);
	const path = appPath(url.pathname);
	if (app.scope && CATALOG_PAGES.test(path) && app.client.supports('listCatalogNodes')) catalogNodes.ensure(app.scope);
	if (app.scope && CHECK_PAGES.test(path)) sharedChecks.ensure(app.scope);
	prefetch(app.scope, path, search, PAGE_READS);
	await fontsReady;
};
