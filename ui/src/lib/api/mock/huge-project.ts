// A project of about 10,000 nodes, for the performance budgets of the Overview and the catalog's folder
// view: 40 top-level folders, each with five subfolders, some of those with their own, and most folders
// with something changed, behind or failed. Turned on with `?mock&huge`. Pure apart from wrapping the
// client it is given.

import type { ApiClient } from '../client';
import { groupStatusOf } from '../http-overview';
import { withConditions } from '$lib/build-area/attention-conditions';
import type { AttentionAcknowledgement, AttentionItem, CatalogNode, Overview, StaleReason } from '../types';

const FOLDERS = [
	'analytics', 'billing', 'catalog_data', 'customers', 'exports', 'finance', 'forecasting', 'fulfilment', 'inventory', 'marketing',
	'orders', 'partners', 'payments', 'pricing', 'reference', 'regions', 'returns', 'sales', 'shipping', 'support',
	'suppliers', 'subscriptions', 'reviews', 'warehouses', 'logistics', 'promotions', 'loyalty', 'accounts', 'invoices', 'refunds',
	'tax', 'audit', 'search', 'sessions', 'devices', 'campaigns', 'channels', 'carriers', 'stores', 'staff'
];
const SUBFOLDERS = ['staging', 'intermediate/cleaned', 'intermediate/joined', 'marts', 'reports'];
const NOUNS = ['events', 'daily_totals', 'snapshot', 'latest', 'history', 'summary', 'mapped', 'enriched', 'keys', 'lookup', 'rollup', 'changes'];
const PER_FOLDER = 50;

const since = '2026-11-01T00:00:00Z';
const changed: StaleReason = { reason: 'changed', change: 'definition', detail: 'Its definition changed since its last build', since };
const behind: StaleReason = { reason: 'behind', upstream: [], detail: 'Upstream data is newer than its last build', since };

/** The huge project's nodes, copies of `template`; deterministic. */
export function hugeProjectNodes(template: CatalogNode): CatalogNode[] {
	const nodes: CatalogNode[] = [];
	FOLDERS.forEach((folder, f) =>
		SUBFOLDERS.forEach((sub, s) => {
			for (let i = 0; i < PER_FOLDER; i++) {
				const key = `${folder}__${sub.replace('/', '__')}__${NOUNS[i % NOUNS.length]}_${i}`;
				const healthy = f % 3 === 2;
				const stale: StaleReason[] = [];
				if (!healthy && (i + s + f) % 9 === 0) stale.push(behind);
				if (!healthy && (i * 7 + f) % 23 === 1) stale.push(changed);
				nodes.push({
					...template,
					key,
					kind: 'table',
					group: null,
					path: `sql/models/${folder}/${sub}/${key}.sql`,
					upstream: [],
					description: `${folder} ${sub.replace('/', ' ')} ${NOUNS[i % NOUNS.length]}`,
					stale,
					lastOutcome: !healthy && i === 3 && s === f % 5 ? 'failed' : 'success'
				});
			}
		})
	);
	return nodes;
}

/** The client with its catalog, and the Overview built from it, replaced by the huge project. */
export function hugeProject(client: ApiClient): ApiClient {
	let held: Promise<CatalogNode[]> | null = null;
	const project = (scope: Parameters<ApiClient['listCatalogNodes']>[0]) =>
		(held ??= client.listCatalogNodes(scope).then((real) => hugeProjectNodes(real.find((n) => n.kind === 'table' && n.lastBuild) ?? real[0])));
	return new Proxy(client, {
		get(target, prop, receiver) {
			if (prop === 'listCatalogNodes') return project;
			if (prop === 'overview') {
				return async (...args: Parameters<ApiClient['overview']>): Promise<Overview | null> => {
					const [base, nodes] = await Promise.all([target.overview(...args), project(args[0])]);
					if (!base) return base;
					const keys = (test: (s: StaleReason) => boolean) => nodes.filter((n) => n.stale?.some(test)).map((n) => n.key);
					// The nodes out of date are the huge project's, so their condition is worked out again for them.
					const held = 'mockConditionAcknowledgements' in target ? (target as { mockConditionAcknowledgements(): AttentionAcknowledgement[] }).mockConditionAcknowledgements() : [];
					const attention = base.attention.map((item): AttentionItem =>
						item.kind === 'stale_models' ? withConditions([{ kind: 'stale_models', changed: keys((s) => s.reason !== 'behind'), behind: keys((s) => s.reason === 'behind') }], held, Date.now())[0] : item
					);
					return { ...base, attention, groups: groupStatusOf(nodes) };
				};
			}
			const value = Reflect.get(target, prop, receiver);
			return typeof value === 'function' ? value.bind(target) : value;
		}
	});
}
