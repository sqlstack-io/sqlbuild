import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HttpChangeFeed, createHttpClient, feedAllowed } from './http-client';
import type { ChangedSignal, ConnectionState } from './signals';

/** A stand-in `EventSource` the test opens, fails and feeds by hand. */
class FakeSource {
	static readonly CLOSED = 2;
	static opened: FakeSource[] = [];
	readyState = 0;
	closed = false;
	onopen: (() => void) | null = null;
	onerror: (() => void) | null = null;
	listeners = new Map<string, (event: MessageEvent<string>) => void>();

	constructor(readonly url: string) {
		FakeSource.opened.push(this);
	}

	addEventListener(name: string, listener: (event: MessageEvent<string>) => void) {
		this.listeners.set(name, listener);
	}

	close() {
		this.closed = true;
	}

	open() {
		this.readyState = 1;
		this.onopen?.();
	}

	/** The server refused or the connection failed for good, so the browser stopped retrying. */
	fail() {
		this.readyState = FakeSource.CLOSED;
		this.onerror?.();
	}
}

const ORDERS = { locationId: '0191f0aa-orders:development' };
const INVENTORY = { locationId: '0191f0bb-inventory:development' };

function feed(stillAllowed: (scope: { locationId: string }) => Promise<boolean>) {
	const changes = new HttpChangeFeed('', stillAllowed);
	const states: ConnectionState[] = [];
	changes.onConnectionState((state) => states.push(state));
	return { changes, states };
}

const latest = () => FakeSource.opened.at(-1)!;

beforeEach(() => {
	FakeSource.opened = [];
	vi.useFakeTimers();
	vi.stubGlobal('EventSource', FakeSource);
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('HttpChangeFeed', () => {
	it('opens the location feed and is live once the stream opens', () => {
		const { changes, states } = feed(async () => true);
		changes.follow(ORDERS);
		expect(latest().url).toBe('/api/control/locations/0191f0aa-orders/targets/development/changes');
		latest().open();
		expect(states).toEqual(['connecting', 'connecting', 'live']);
	});

	it('leaves transient drops to the browser without its own retry', () => {
		const stillAllowed = vi.fn(async () => true);
		const { changes, states } = feed(stillAllowed);
		changes.follow(ORDERS);
		latest().open();
		latest().onerror?.();
		vi.advanceTimersByTime(60_000);
		expect(states.at(-1)).toBe('reconnecting');
		expect(stillAllowed).not.toHaveBeenCalled();
		expect(FakeSource.opened).toHaveLength(1);
	});

	it('checks access before each retry and backs off 1, 2, 4 then 8 seconds', async () => {
		const stillAllowed = vi.fn(async () => true);
		const { changes, states } = feed(stillAllowed);
		changes.follow(ORDERS);
		for (const delay of [1000, 2000, 4000, 8000]) {
			const failed = latest();
			failed.fail();
			await vi.advanceTimersByTimeAsync(delay - 1);
			expect(stillAllowed).toHaveBeenCalledTimes(FakeSource.opened.length - 1);
			expect(latest()).toBe(failed);
			await vi.advanceTimersByTimeAsync(1);
			expect(stillAllowed).toHaveBeenLastCalledWith(ORDERS);
			expect(latest()).not.toBe(failed);
			expect(failed.closed).toBe(true);
		}
		expect(states.filter((state) => state !== 'connecting')).toEqual([
			'reconnecting',
			'reconnecting',
			'reconnecting',
			'offline'
		]);
	});

	it('caps the backoff at 30 seconds', async () => {
		const stillAllowed = vi.fn(async () => true);
		const { changes } = feed(stillAllowed);
		changes.follow(ORDERS);
		for (let attempt = 0; attempt < 6; attempt += 1) {
			latest().fail();
			await vi.advanceTimersByTimeAsync(30_000);
		}
		const calls = stillAllowed.mock.calls.length;
		latest().fail();
		await vi.advanceTimersByTimeAsync(29_999);
		expect(stillAllowed).toHaveBeenCalledTimes(calls);
		await vi.advanceTimersByTimeAsync(1);
		expect(stillAllowed).toHaveBeenCalledTimes(calls + 1);
	});

	it('resets the backoff once a retry opens', async () => {
		const stillAllowed = vi.fn(async () => true);
		const { changes } = feed(stillAllowed);
		changes.follow(ORDERS);
		latest().fail();
		await vi.advanceTimersByTimeAsync(1000);
		latest().fail();
		await vi.advanceTimersByTimeAsync(2000);
		latest().open();
		const reopened = latest();
		reopened.fail();
		await vi.advanceTimersByTimeAsync(1000);
		expect(latest()).not.toBe(reopened);
	});

	it('goes offline and stops when access is refused', async () => {
		const { changes, states } = feed(async () => false);
		changes.follow(ORDERS);
		latest().fail();
		await vi.advanceTimersByTimeAsync(60_000);
		expect(FakeSource.opened).toHaveLength(1);
		expect(states.at(-1)).toBe('offline');
	});

	it('keeps retrying when the access check itself fails', async () => {
		const { changes } = feed(() => Promise.reject(new Error('network down')));
		changes.follow(ORDERS);
		latest().fail();
		await vi.advanceTimersByTimeAsync(1000);
		expect(FakeSource.opened).toHaveLength(2);
	});

	it('drops a pending retry when the person follows another location', async () => {
		const stillAllowed = vi.fn(async () => true);
		const { changes } = feed(stillAllowed);
		changes.follow(ORDERS);
		const orders = latest();
		orders.fail();
		changes.follow(INVENTORY);
		await vi.advanceTimersByTimeAsync(60_000);
		expect(stillAllowed).not.toHaveBeenCalled();
		expect(FakeSource.opened.map((source) => source.url)).toEqual([
			'/api/control/locations/0191f0aa-orders/targets/development/changes',
			'/api/control/locations/0191f0bb-inventory/targets/development/changes'
		]);
	});

	it('closes the stream and is offline when following nothing', () => {
		const { changes, states } = feed(async () => true);
		changes.follow(ORDERS);
		const orders = latest();
		changes.follow(null);
		expect(orders.closed).toBe(true);
		expect(states.at(-1)).toBe('offline');
	});

	it('maps server topics onto the followed location', () => {
		const { changes } = feed(async () => true);
		const signals: ChangedSignal['channel'][] = [];
		changes.subscribe((signal) => signals.push(signal.channel));
		changes.follow(ORDERS);
		for (const topic of ['snapshot:0191f0aa-orders', 'node_status:0191f0aa-orders', 'runs', 'audit']) {
			latest().listeners.get('changed')!(new MessageEvent('changed', { data: JSON.stringify({ topic }) }));
		}
		expect(signals).toEqual([
			`snapshot:${ORDERS.locationId}`,
			`node_status:${ORDERS.locationId}`,
			'runs',
			'all'
		]);
	});
});

describe('feedAllowed', () => {
	const probe = (status: number) => {
		const signals: AbortSignal[] = [];
		vi.stubGlobal(
			'fetch',
			vi.fn(async (_url: string, init: RequestInit) => {
				signals.push(init.signal!);
				return new Response(null, { status });
			})
		);
		return signals;
	};

	it.each([
		[200, true, false],
		[503, true, false],
		[403, false, false],
		[401, false, true]
	])('a %s answer allows a retry: %s, and signs out: %s', async (status, allowed, signedOut) => {
		const signals = probe(status);
		const onUnauthorized = vi.fn();
		expect(await feedAllowed('', ORDERS, onUnauthorized)).toBe(allowed);
		expect(onUnauthorized).toHaveBeenCalledTimes(signedOut ? 1 : 0);
		expect(vi.mocked(fetch)).toHaveBeenCalledWith('/api/control/locations/0191f0aa-orders/targets/development/changes', expect.anything());
		expect(signals[0].aborted).toBe(true);
	});

	it('stops a partial viewer, whom the bootstrap lists but the feed refuses, after one refusal', async () => {
		probe(403);
		const changes = new HttpChangeFeed('', (scope) => feedAllowed('', scope, () => {}));
		const states: ConnectionState[] = [];
		changes.onConnectionState((state) => states.push(state));
		changes.follow(ORDERS);
		latest().fail();
		await vi.advanceTimersByTimeAsync(10 * 60_000);
		expect(FakeSource.opened).toHaveLength(1);
		expect(vi.mocked(fetch)).toHaveBeenCalledTimes(1);
		expect(states.at(-1)).toBe('offline');
	});

	it('rejects when the network fails, so the feed keeps retrying', async () => {
		vi.stubGlobal('fetch', vi.fn(() => Promise.reject(new TypeError('network down'))));
		await expect(feedAllowed('', ORDERS, () => {})).rejects.toThrow('network down');
	});
});

describe('listing requests', () => {
	const tick = { tick_id: 'k-2', trigger_id: 't-orders', scheduled_for: '2026-11-07T06:00:00Z', evaluated_at: '2026-11-07T06:00:02Z', epoch: 1, outcome: 'requested', run_id: null, covered_by_run_id: null };
	const declaration = { description: 'Daily orders', job_key: 'orders', schedule: { kind: 'cron', expression: '0 6 * * *', timezone: 'UTC' } };
	const triggers = [
		{ trigger_id: 't-orders', name: 'orders_daily', job_key: 'orders', definition_json: declaration, status: 'running', last_tick_for: null, next_due_at: null, latest_tick: tick },
		{ trigger_id: 't-stock', name: 'stock_daily', job_key: 'stock', definition_json: declaration, status: 'running', last_tick_for: null, next_due_at: null, latest_tick: null }
	];

	function server() {
		const calls: string[] = [];
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				const path = new URL(request.url).pathname.replace('/api/control/locations/0191f0aa-orders/targets/development', '');
				calls.push(`${request.method} ${path}`);
				const body = request.method === 'POST' ? { applied: true } : path === '/triggers' ? triggers : [];
				return new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } });
			})
		);
		return { calls, client: createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} }) };
	}

	it('lists triggers with their newest tick in one request, never one per trigger', async () => {
		const { calls, client } = server();
		const listed = await client.listTriggers(ORDERS);
		expect(listed.map((t) => t.lastTick?.id ?? null)).toEqual(['k-2', null]);
		expect(calls.sort()).toEqual(['GET /runs', 'GET /triggers']);
	});

	it('shares listings between pages that refresh together, and reads again after a write', async () => {
		const { calls, client } = server();
		await Promise.all([client.listJobs(ORDERS), client.listTriggers(ORDERS)]);
		expect(calls.sort()).toEqual(['GET /jobs', 'GET /runs', 'GET /triggers']);
		calls.length = 0;
		await client.setTriggerStates(ORDERS, ['t-orders'], 'paused');
		expect([calls[0], ...calls.slice(1).sort()]).toEqual(['POST /api/control/triggers', 'GET /runs', 'GET /triggers']);
	});
});

describe('getCatalogNode', () => {
	/** Answers the reads a node page makes; anything else, such as node status, is refused. */
	function serve(actions: string[], sqlStatus = 200) {
		const paths: string[] = [];
		const answers: [string, unknown][] = [
			['/bootstrap', { login_mode: 'password', access: { name: 'vic', principal_id: 'p', actions: [], locations: [{ location_id: '0191f0aa-orders', name: 'Orders', target: 'development', actions }] } }],
			['/nodes', [{ node_key: 'model:orders', kind: 'model', name: 'orders', materialization: 'table', tags: [], dependencies: [], snapshot_id: 's' }]],
			['/checks', []],
			['/sql', { node_key: 'model:orders', snapshot_id: 's', raw_sql: 'select 1', compiled_sql: 'select 1' }]
		];
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				const path = new URL(request.url).pathname;
				paths.push(path);
				const answer = answers.find(([end]) => path.endsWith(end));
				const refused = !answer || (answer[0] === '/sql' && sqlStatus === 403);
				return refused ? Response.json({ error: 'refused' }, { status: 403 }) : Response.json(answer[1]);
			})
		);
		return paths;
	}

	it.each([
		[['view', 'view.sql'], 'select 1'],
		[['view'], null]
	])('with %j reads SQL only when the location allows it, so a viewer causes no refused read', async (actions, raw) => {
		const paths = serve(actions);
		const client = createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} });
		await client.bootstrap();
		const detail = await client.getCatalogNode(ORDERS, 'orders');
		expect([detail?.key, detail?.sql?.raw ?? null]).toEqual(['model:orders', raw]);
		expect(paths.filter((p) => p.endsWith('/sql'))).toHaveLength(raw ? 1 : 0);
	});

	it('asks for a refused node SQL once, when view.sql holds on only some nodes', async () => {
		const paths = serve(['view', 'view.sql'], 403);
		const client = createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} });
		await client.bootstrap();
		const loads = [await client.getCatalogNode(ORDERS, 'orders'), await client.getCatalogNode(ORDERS, 'orders')];
		expect(loads.map((d) => [d?.key, d?.sql])).toEqual([['model:orders', null], ['model:orders', null]]);
		expect(paths.filter((p) => p.endsWith('/sql'))).toHaveLength(1);
	});
});
