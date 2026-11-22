// `?secrets`: the mock location runs without the orders API's key, so its health says so, the Overview
// lists it, and plans that run the orders loader can't run it or what reads it.

import type { ApiClient } from '../client';
import type { LocationDetail, Overview, PlanCantRun, PlanResult, Scope } from '../types';
import type { MissingProviderSecrets } from '../types/platform';
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

/** `?secrets=sink`: the chat webhook's URL is missing too, and its run notifications sink runs with every command. */
const CHAT_SINK: PlanCantRun = {
	reason: 'missing_secret',
	provider: 'chat_webhook',
	path: 'providers/chat.py',
	variables: ['CHAT_WEBHOOK_URL'],
	nodes: [],
	readers: [],
	sinks: ['run_notifications'],
	env_file: '.env'
};

/** What a plan of `selection` can't run: the orders loader, when selected, and the selected nodes that read it. */
export function ordersApiCantRun(selection: string[], sink = false): PlanCantRun[] {
	const selected = new Set(selection);
	const sinks = sink ? [CHAT_SINK] : [];
	if (!selected.has(BLOCKED_NODE)) return sinks;
	return [
		...sinks,
		{
			reason: 'missing_secret',
			provider: 'orders_api',
			path: 'providers/orders_api.py',
			variables: ['ORDERS_API_KEY'],
			nodes: [BLOCKED_NODE],
			readers: readersOf(BLOCKED_NODE, selected),
			sinks: [],
			env_file: '.env'
		}
	];
}

/** The orders API as the location reports it: its key unset, and not a provider the target allows to miss it. */
const ORDERS_API: MissingProviderSecrets = {
	provider: 'orders_api',
	path: 'providers/orders_api.py',
	settings: [{ field: 'api_key', variable: 'ORDERS_API_KEY', line: 18 }],
	nodes: [BLOCKED_NODE],
	allowed: false
};

/** The location's health with the orders API's key missing. */
function withMissingKey(detail: LocationDetail | null): LocationDetail | null {
	if (!detail) return detail;
	return { ...detail, health: detail.health === 'healthy' || detail.health === 'degraded' ? 'missing_secrets' : detail.health, missingSecrets: [ORDERS_API, ...detail.missingSecrets] };
}

/** The Overview with the location's missing key among what needs attention. */
function withMissingKeyItem(overview: Overview | null, scope: Scope): Overview | null {
	if (!overview) return overview;
	const event = {
		id: `secrets:${scope.locationId}`,
		kind: 'location_missing_secrets' as const,
		severity: 'warning' as const,
		occurredAt: new Date(Date.now() - 12 * 60_000).toISOString(),
		members: ['ORDERS_API_KEY'],
		active: true,
		locationId: scope.locationId,
		subject: null,
		message: "orders_api is missing ORDERS_API_KEY in the location's environment, so runs of the node that uses it stop before the warehouse."
	};
	return { ...overview, attention: [{ kind: 'platform_event', event }, ...overview.attention] };
}

/** The client whose build plans can't run what the orders API's missing key stops. */
export function missingSecrets(client: ApiClient, { sink = false }: { sink?: boolean } = {}): ApiClient {
	return new Proxy(client, {
		get(target, prop, receiver) {
			const value = Reflect.get(target, prop, receiver);
			if (prop === 'getLocation') {
				return async (...args: Parameters<ApiClient['getLocation']>) => withMissingKey(await target.getLocation(...args));
			}
			if (prop === 'overview') {
				return async (...args: Parameters<ApiClient['overview']>) => withMissingKeyItem(await target.overview(...args), args[0]);
			}
			if (prop !== 'planBuild') return typeof value === 'function' ? value.bind(target) : value;
			return async (...args: Parameters<ApiClient['planBuild']>): Promise<PlanResult> => {
				const result = await target.planBuild(...args);
				if (!result.plan) return result;
				const check = await target.checkAdhocRun(args[0], { command: args[1], priority: 0 });
				const cant_run = ordersApiCantRun(check.selection, sink);
				if (!cant_run.length) return result;
				// A real plan loads the orders loader's source; the mock's plans list only models.
				const source_loads = check.selection.includes(BLOCKED_NODE)
					? [...result.plan.source_loads, { name: BLOCKED_NODE, kind: 'source', write_strategy: 'append', cursor_column: 'loaded_at', is_reload: false }]
					: result.plan.source_loads;
				// It also runs the Python nodes it selects, which the mock's plans leave out.
				const python_nodes = [
					...result.plan.python_nodes,
					...check.selection
						.filter((key) => SNAPSHOT_NODES.find((node) => node.key === key)?.kind === 'asset')
						.map((name) => ({ name, kind: 'asset', phase: 'after_models', identity_status: 'unchanged' }))
				];
				return { ...result, plan: { ...result.plan, source_loads, python_nodes, cant_run } };
			};
		}
	});
}
