// PROTOTYPE (wip/proto-lineage-hover): a project shaped like a real one for trying Lineage's hover
// behaviours. Ten top-level folders, one big one (`orders`, about 300 nodes in nested subfolders),
// and about 2,000 links, many across folders. Turned on with `?mock&proto`. Deterministic.

import type { ApiClient } from '../client';
import type { CatalogNode, StaleReason } from '../types';

/** Folders in dependency order: each reads mostly from itself and the folders before it. */
const FOLDERS: [folder: string, count: number][] = [
	['sources', 40],
	['products', 110],
	['customers', 120],
	['orders/partners/staging', 60],
	['orders/partners', 40],
	['orders/web', 70],
	['orders/retail', 60],
	['orders', 72],
	['inventory', 100],
	['payments', 80],
	['shipping', 90],
	['finance', 80],
	['marketing', 90],
	['support', 60]
];

/** Folders open when the page first loads, so open boxes and closed folder cards both show. */
export const HOVER_PROJECT_OPEN = ['orders', 'orders/partners', 'orders/partners/staging', 'orders/web', 'orders/retail', 'payments'];

const LAYERS = 4;
const NOUNS = ['events', 'daily_totals', 'snapshot', 'latest', 'history', 'summary', 'mapped', 'enriched', 'keys', 'lookup', 'rollup', 'changes', 'facts', 'curated'];

function rng(seed: number) {
	let s = seed >>> 0;
	return () => {
		s = (s + 0x6d2b79f5) >>> 0;
		let t = s;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

const since = '2026-11-01T00:00:00Z';
const behind: StaleReason = { reason: 'behind', upstream: [], detail: 'Upstream data is newer than its last build', since };

/** The prototype project's nodes, copies of `template`. */
export function hoverProjectNodes(template: CatalogNode): CatalogNode[] {
	const rand = rng(42);
	const nodes: CatalogNode[] = [];
	const earlier: string[] = [];
	FOLDERS.forEach(([folder, count]) => {
		const own: string[] = [];
		const slug = folder.replaceAll('/', '__');
		for (let i = 0; i < count; i++) {
			const key = `${slug}__${NOUNS[i % NOUNS.length]}_${i}`;
			const upstream = new Set<string>();
			if (folder !== 'sources') {
				const parents = 1 + Math.floor(rand() * 3);
				for (let p = 0; p < parents; p++) {
					// About a third of links cross folders, so a big box has hundreds of them.
					// Within a folder, nodes sit in about four layers and read only from the layers before theirs.
					const layer = Math.floor((i * LAYERS) / count);
					const below = own.slice(0, Math.floor((layer * count) / LAYERS));
					const cross = !below.length || rand() < 0.38;
					const pool = cross ? earlier : below;
					if (pool.length) upstream.add(pool[Math.floor(rand() * pool.length)]);
				}
			}
			nodes.push({
				...template,
				key,
				kind: folder === 'sources' ? 'source' : 'table',
				group: folder === 'sources' ? 'sources' : null,
				path: folder === 'sources' ? `models/sources/${key}.yml` : `models/${folder}/${key}.sql`,
				upstream: [...upstream],
				description: `${folder.replaceAll('/', ' ')} ${NOUNS[i % NOUNS.length]}`,
				stale: rand() < 0.06 ? [behind] : [],
				lastOutcome: rand() < 0.02 ? 'failed' : 'success'
			});
			own.push(key);
		}
		earlier.push(...own);
	});
	return nodes;
}

/** The client with its catalog replaced by the prototype project. */
export function hoverProject(client: ApiClient): ApiClient {
	return new Proxy(client, {
		get(target, prop, receiver) {
			const value = Reflect.get(target, prop, receiver);
			if (prop !== 'listCatalogNodes') return typeof value === 'function' ? value.bind(target) : value;
			return async (...args: Parameters<ApiClient['listCatalogNodes']>) => {
				const real = await target.listCatalogNodes(...args);
				const template = real.find((n) => n.kind === 'table' && n.lastBuild) ?? real[0];
				return template ? hoverProjectNodes(template) : real;
			};
		}
	});
}
