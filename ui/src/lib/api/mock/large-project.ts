// A project the size of a real one, for looking at Lineage's overview and for the layout test: 13
// top-level folders seven layers deep, too wide to show whole at a readable zoom, each with staging and marts
// subfolders, about 400 nodes, more than Lineage draws one by one, so it opens by folder. Turned on
// with `?mock&large`. Pure apart from wrapping the client it is given.

import type { ApiClient } from '../client';
import type { PlanDocument, PlanPage, PlanResult } from '../types';
import { largePlanDocument, planModelDiff, planPage, planSummary } from './large-plan';
import { recordPlanRows } from './plan-rows-log';
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
			// Finance also reconciles against the orders marts directly, a read across five layers.
			if (folder === 'finance') nodes.find((n) => n.key === `${folder}_stg_1`)!.upstream.push(marts.get('orders')![0]);
		});
	});
	return nodes;
}

/** The large plans planned so far, by request, for their pages. */
const plans = new Map<string, PlanDocument>();

/** The client with its catalog replaced by the large project, and a build planned as the large plan. */
export function largeProject(client: ApiClient): ApiClient {
	return new Proxy(client, {
		get(target, prop, receiver) {
			const value = Reflect.get(target, prop, receiver);
			if (prop === 'planBuild') {
				return async (...args: Parameters<ApiClient['planBuild']>): Promise<PlanResult> => {
					const result = await target.planBuild(...args);
					if (!result.plan) return result;
					const doc = largePlanDocument(args[1].some((t) => t === '--changed' || t.startsWith('--changed=')));
					const requestId = crypto.randomUUID();
					plans.set(requestId, doc);
					return { plan: planSummary(doc), error: null, requestId };
				};
			}
			if (prop === 'planRows') {
				return async (...[scope, requestId, section, offset, limit, name]: Parameters<ApiClient['planRows']>): Promise<PlanPage> => {
					const doc = plans.get(requestId);
					if (!doc) return target.planRows(scope, requestId, section, offset, limit, name);
					recordPlanRows(section, offset, limit);
					await new Promise((resolve) => setTimeout(resolve, 120));
					return planPage(doc, section, offset, limit, name);
				};
			}
			if (prop === 'planDiff') {
				return async (...[scope, requestId, model]: Parameters<ApiClient['planDiff']>) => {
					const doc = plans.get(requestId);
					return doc ? planModelDiff(doc, model) : target.planDiff(scope, requestId, model);
				};
			}
			if (prop !== 'listCatalogNodes') return typeof value === 'function' ? value.bind(target) : value;
			return async (...args: Parameters<ApiClient['listCatalogNodes']>) => {
				const real = await target.listCatalogNodes(...args);
				const template = real.find((n) => n.kind === 'table') ?? real[0];
				return template ? largeProjectNodes(template) : real;
			};
		}
	});
}
