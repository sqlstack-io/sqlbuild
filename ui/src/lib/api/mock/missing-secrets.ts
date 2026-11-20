// `?secrets`: the mock location runs without the orders API's key, so plans that run the orders loader
// can't run it or what reads it.

import type { ApiClient } from '../client';
import type { PlanCantRun, PlanResult } from '../types';
import { SNAPSHOT_NODES } from './operate-graph';

/** The loader the orders API's missing key stops. */
const BLOCKED_NODE = 'raw__orders';

/** The selected keys downstream of `key`, nearest first. */
function readersOf(key: string, selected: Set<string>): string[] {
	const found: string[] = [];
	const pending = [key];
	while (pending.length) {
		const current = pending.shift()!;
		for (const node of SNAPSHOT_NODES) {
			if (node.upstream.includes(current) && selected.has(node.key) && !found.includes(node.key)) {
				found.push(node.key);
				pending.push(node.key);
			}
		}
	}
	return found;
}

/** What a plan of `selection` can't run: the orders loader, when selected, and the selected nodes that read it. */
export function ordersApiCantRun(selection: string[]): PlanCantRun[] {
	const selected = new Set(selection);
	if (!selected.has(BLOCKED_NODE)) return [];
	return [
		{
			reason: 'missing_secret',
			provider: 'orders_api',
			path: 'providers/orders_api.py',
			variables: ['ORDERS_API_KEY'],
			nodes: [BLOCKED_NODE],
			readers: readersOf(BLOCKED_NODE, selected),
			env_file: '.env'
		}
	];
}

/** The client whose build plans can't run what the orders API's missing key stops. */
export function missingSecrets(client: ApiClient): ApiClient {
	return new Proxy(client, {
		get(target, prop, receiver) {
			const value = Reflect.get(target, prop, receiver);
			if (prop !== 'planBuild') return typeof value === 'function' ? value.bind(target) : value;
			return async (...args: Parameters<ApiClient['planBuild']>): Promise<PlanResult> => {
				const result = await target.planBuild(...args);
				if (!result.plan) return result;
				const check = await target.checkAdhocRun(args[0], { command: args[1], priority: 0 });
				const cant_run = ordersApiCantRun(check.selection);
				return cant_run.length ? { ...result, plan: { ...result.plan, cant_run } } : result;
			};
		}
	});
}
