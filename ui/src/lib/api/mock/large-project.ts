// A project the size of a real one, for looking at Lineage's overview and for the layout test: 13
// top-level folders seven layers deep, too wide to show whole at a readable zoom, each with staging and marts
// subfolders, about 400 nodes, more than Lineage draws one by one, so it opens by folder. Turned on
// with `?mock&large`. Pure apart from wrapping the client it is given.

import type { ApiClient } from '../client';
import type { CatalogNode } from '../types';

const LAYERS = [['sources'], ['seeds', 'orders'], ['customers', 'inventory'], ['payments', 'shipping'], ['returns', 'pricing'], ['suppliers', 'regions'], ['finance', 'marketing']];
const PER_SUBFOLDER = 15;

/** The large project's nodes, each a copy of `template` with its own key, folder and lineage. */
export function largeProjectNodes(template: CatalogNode): CatalogNode[] {
	const nodes: CatalogNode[] = [];
	const marts = new Map<string, string[]>();
	LAYERS.forEach((layer, depth) => {
		layer.forEach((folder, f) => {
			// Each folder reads the marts of two folders in the layer before it.
			const before = LAYERS[depth - 1] ?? [];
			const feeds = before.length ? [before[f % before.length], before[(f + 1) % before.length]].flatMap((b) => marts.get(b) ?? []) : [];
			const staging = Array.from({ length: PER_SUBFOLDER }, (_, i) => `${folder}_stg_${i + 1}`);
			const mart = Array.from({ length: PER_SUBFOLDER }, (_, i) => `${folder}_mart_${i + 1}`);
			staging.forEach((key, i) =>
				nodes.push({ ...template, key, group: `${folder}/staging`, path: `models/${folder}/staging/${key}.sql`, upstream: feeds.length ? [feeds[i % feeds.length]] : [], description: `Staged ${folder} data` })
			);
			mart.forEach((key, i) => nodes.push({ ...template, key, group: `${folder}/marts`, path: `models/${folder}/marts/${key}.sql`, upstream: [staging[i], staging[(i + 1) % staging.length]], description: `${folder} mart` }));
			marts.set(folder, mart);
		});
	});
	return nodes;
}

/** The client with its catalog replaced by the large project. */
export function largeProject(client: ApiClient): ApiClient {
	return new Proxy(client, {
		get(target, prop, receiver) {
			const value = Reflect.get(target, prop, receiver);
			if (prop !== 'listCatalogNodes') return typeof value === 'function' ? value.bind(target) : value;
			return async (...args: Parameters<ApiClient['listCatalogNodes']>) => {
				const real = await target.listCatalogNodes(...args);
				const template = real.find((n) => n.kind === 'table') ?? real[0];
				return template ? largeProjectNodes(template) : real;
			};
		}
	});
}
