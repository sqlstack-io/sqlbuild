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

	it('finds an ad-hoc run it launched while an older runs listing was still in flight', async () => {
		let launched: string | null = null;
		let releaseOlderListing = () => {};
		const olderListingHeld = new Promise<void>((release) => (releaseOlderListing = release));
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				if (request.method === 'POST') {
					launched = (await request.json()).run_id;
					releaseOlderListing();
					return Response.json({ run_id: launched }, { status: 201 });
				}
				// The listing answers with the runs queued when it was asked; the first is held until the launch is queued.
				const runs = launched ? [queuedAdhocRun(launched)] : [];
				if (!launched) await olderListingHeld;
				return Response.json(new URL(request.url).pathname.endsWith('/runs') ? runs : []);
			})
		);
		const client = createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} });
		const olderListing = client.listRuns(ORDERS);
		const run = await client.launchAdhocRun(ORDERS, { command: ['build', '--select', 'orders'], priority: 0 });
		expect(run.id).toBe(launched);
		expect(await olderListing).toEqual([]);
	});
});

function queuedAdhocRun(id: string) {
	return {
		run_id: id,
		root_run_id: id,
		kind: 'adhoc',
		origin: 'manual',
		status: 'queued',
		job_key: null,
		last_event_storage_id: 1,
		location_id: '0191f0aa-orders',
		node_counts_json: {},
		priority: 0,
		queued_at: '2026-11-08T10:00:00Z',
		requested_by: 'principal:p',
		requester_name: 'Ada Admin',
		rewrite_history: false,
		selection_json: ['model.orders'],
		snapshot_id: '0191f0dd-0000-7000-8000-000000000000',
		target: 'development'
	};
}

describe('overview', () => {
	function server(nodesStatus: number) {
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				const path = new URL(request.url).pathname;
				if (path.endsWith('/nodes') && nodesStatus !== 200) return Response.json({ error: 'refused' }, { status: nodesStatus });
				if (path.endsWith('/platform/problems')) return Response.json({ scheduler_leader: 'server-1', scheduler_lease_expires_at: null, broken_locations: [], overdue_ticks: [] });
				return Response.json([]);
			})
		);
		return createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} });
	}

	it('is null for a person who may view only part of the target, with no partial counts', async () => {
		expect(await server(403).overview(ORDERS)).toBeNull();
	});

	it('still fails on any other error', async () => {
		await expect(server(500).overview(ORDERS)).rejects.toMatchObject({ status: 500 });
	});

	it('summarises the target when every listing is allowed', async () => {
		expect((await server(200).overview(ORDERS))?.metrics.runs24h.total).toBe(0);
	});
});

describe('locations', () => {
	const access = (target: string) => ({ location_id: '0191f0aa-orders', name: 'Orders', target, actions: [] });
	const report = (session: string, secondsAgo: number) => ({
		received_at: new Date(Date.now() - secondsAgo * 1000).toISOString(),
		report: { event_id: `e-${session}-${secondsAgo}`, session_id: session, target: 'development', health: { kind: 'heartbeat' } }
	});

	/** Orders development is readable and orders preview refuses its health read, as for a partial viewer. */
	function server() {
		const paths: string[] = [];
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				const path = new URL(request.url).pathname;
				paths.push(path);
				if (path.endsWith('/bootstrap')) return Response.json({ login_mode: 'password', access: { name: 'ada', principal_id: 'p', actions: [], locations: [access('development'), access('preview')] } });
				if (path.endsWith('/platform/problems'))
					return Response.json({
						scheduler_leader: true,
						scheduler_lease_expires_at: '2026-11-09T10:00:30Z',
						broken_locations: [{ location_id: '0191f0aa-orders', location_name: 'Orders', target: 'development', last_seen_at: '2026-11-09T09:00:00Z' }],
						overdue_ticks: []
					});
				if (path.includes('/targets/preview/')) return Response.json({ error: 'refused' }, { status: 403 });
				if (path.endsWith('/health')) return Response.json([report('s-2', 10), report('s-1', 400), report('s-2', 20)]);
				return Response.json([]);
			})
		);
		return { paths, client: createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} }) };
	}

	it('opens a readable location with one process per session', async () => {
		const detail = await server().client.getLocation('0191f0aa-orders:development');
		expect(detail?.instances.map((i) => [i.id, i.state])).toEqual([
			['s-2', 'live'],
			['s-1', 'stale']
		]);
	});

	it('fails the page with the refusal when the person may not read the target health', async () => {
		await expect(server().client.getLocation('0191f0aa-orders:preview')).rejects.toMatchObject({ status: 403 });
	});

	it('is null for a location the person is not given, without reading its health', async () => {
		const { paths, client } = server();
		expect(await client.getLocation('0191f0aa-orders:production')).toBeNull();
		expect(paths.some((p) => p.endsWith('/health'))).toBe(false);
	});

	it('counts open problems per location target', async () => {
		const listed = await server().client.listLocations();
		expect(listed.map((l) => [l.id, l.activeProblems, l.health])).toEqual([
			['0191f0aa-orders:development', 1, 'healthy'],
			['0191f0aa-orders:preview', 0, null]
		]);
	});
});

describe('launchJob', () => {
	function server(answer: Response) {
		const posts: unknown[] = [];
		vi.stubGlobal('document', { cookie: 'sqb-csrf=token-1' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				if (request.method === 'POST') {
					posts.push(await request.json());
					return answer.clone();
				}
				return Response.json([]);
			})
		);
		return { posts, client: createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} }) };
	}

	it('sends a backfill window with the job run request', async () => {
		const { posts, client } = server(Response.json({ error: 'stop here' }, { status: 409 }));
		await expect(client.launchJob(ORDERS, 'daily', { window: { type: 'timestamp', start: '1200', end: '1500' } })).rejects.toThrow();
		expect(posts[0]).toMatchObject({ job_key: 'daily', target: 'development', window: { kind: 'integer', start: 1200, end: 1500 } });
	});

	it('leaves the window and full refresh out of an ordinary run', async () => {
		const { posts, client } = server(Response.json({ error: 'stop here' }, { status: 409 }));
		await expect(client.launchJob(ORDERS, 'daily')).rejects.toThrow();
		expect(posts[0]).not.toHaveProperty('window');
		expect(posts[0]).not.toHaveProperty('full_refresh');
	});

	it("asks for a full refresh and shows the server's refusal as it comes", async () => {
		const { posts, client } = server(Response.json({ error: 'run.rewrite_history is required; decision 7' }, { status: 403 }));
		await expect(client.launchJobFromScratch(ORDERS, 'daily')).rejects.toMatchObject({ status: 403, message: expect.stringContaining('run.rewrite_history') });
		expect(posts[0]).toMatchObject({ job_key: 'daily', full_refresh: true, reason: 'Full refresh started from the web UI' });
		expect(posts[0]).not.toHaveProperty('window');
	});

	it("shows a plain-text refusal of a body the server couldn't read", async () => {
		const text = 'Failed to deserialize the JSON body into the target type: window: input contains invalid characters';
		const { client } = server(new Response(text, { status: 422, headers: { 'content-type': 'text/plain; charset=utf-8' } }));
		await expect(client.launchJob(ORDERS, 'daily', { window: { type: 'timestamp', start: '2026-10-01', end: '2026-10-31T00:00:00Z' } })).rejects.toMatchObject({ status: 422, message: text });
	});

	it('shows the status, not an HTML error page from something in front of the server', async () => {
		const { client } = server(new Response('<html><body>Bad gateway</body></html>', { status: 502, statusText: 'Bad Gateway', headers: { 'content-type': 'text/html' } }));
		await expect(client.launchJob(ORDERS, 'daily')).rejects.toMatchObject({ status: 502, message: '502 Bad Gateway' });
	});

	it('shows only the status for a plain-text server failure, which can name internal types', async () => {
		const text = 'Missing request extension: Extension of type `example::AuditContext` was not found.';
		const { client } = server(new Response(text, { status: 500, statusText: 'Internal Server Error', headers: { 'content-type': 'text/plain; charset=utf-8' } }));
		await expect(client.launchJob(ORDERS, 'daily')).rejects.toMatchObject({ status: 500, message: '500 Internal Server Error' });
	});

	it("shows the server's refusal of a window as it comes", async () => {
		const { client } = server(Response.json({ error: 'backfill window start must not follow end' }, { status: 400 }));
		await expect(client.launchJob(ORDERS, 'daily', { window: { type: 'timestamp', start: '20', end: '10' } })).rejects.toMatchObject({
			status: 400,
			message: expect.stringContaining('backfill window start must not follow end')
		});
	});
});

describe('getJob', () => {
	function server(job: Response) {
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => (new URL(request.url).pathname.endsWith('/jobs/nightly') ? job.clone() : Response.json([])))
		);
		return createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} });
	}

	it('treats an unknown job as not found, and keeps a refusal as an error', async () => {
		await expect(server(Response.json({ error: 'job not found' }, { status: 404 })).getJob(ORDERS, 'nightly')).resolves.toBeNull();
		await expect(server(Response.json({ error: 'view is required; decision 3' }, { status: 403 })).getJob(ORDERS, 'nightly')).rejects.toMatchObject({ status: 403 });
	});
});

describe('administration writes', () => {
	const ada = { principal_id: 'p-ada', name: 'ada', created_at: '2026-11-01T00:00:00Z', disabled_at: null, has_password: true, groups: [], active_sessions: 2, last_signed_in_at: null };

	function server(refuse = false) {
		const posts: [string, unknown][] = [];
		const state = { disabled: false, assignments: [{ assignment_id: 'a-1', principal_id: 'p-ada', name: 'ada', role_key: 'operator', location_id: 'loc-orders', target: 'development' }] };
		vi.stubGlobal('document', { cookie: 'sqb-csrf=token-1' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				const path = new URL(request.url).pathname;
				if (request.method === 'POST') {
					const body = await request.json();
					posts.push([path, body]);
					if (refuse) return Response.json({ error: 'cannot remove or disable the final installation administrator' }, { status: 403 });
					if (body === 'disable') state.disabled = true;
					if (body?.role && !body.revoke) state.assignments.push({ assignment_id: 'a-2', principal_id: body.principal_id, name: 'ada', role_key: body.role, location_id: body.location_id, target: body.target });
					return Response.json({ applied: true });
				}
				if (path === '/api/admin/people') return Response.json([{ ...ada, disabled_at: state.disabled ? '2026-11-09T00:00:00Z' : null }]);
				if (path === '/api/admin/roles') return Response.json({ assignments: state.assignments, group_roles: [] });
				return Response.json([]);
			})
		);
		return { posts, client: createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} }) };
	}

	it('gives a role in one location target by person ID and returns the new assignment', async () => {
		const { posts, client } = server();
		const given = await client.createRoleAssignment({ principal: 'ada', role: 'developer', scope: { locationId: 'loc-orders:development' } });
		expect(posts).toEqual([['/api/admin/roles', { principal_id: 'p-ada', role: 'developer', location_id: 'loc-orders', target: 'development', revoke: false }]]);
		expect([given.id, given.scope.locationId]).toEqual(['a-2', 'loc-orders:development']);
	});

	it('gives a role everywhere with no location or target', async () => {
		const { posts, client } = server();
		await client.createRoleAssignment({ principal: 'ada', role: 'viewer', scope: { locationId: null } });
		expect(posts[0][1]).toMatchObject({ location_id: null, target: null });
	});

	it('removes an assignment by revoking exactly its person, role and scope', async () => {
		const { posts, client } = server();
		await client.deleteRoleAssignment('a-1');
		expect(posts).toEqual([['/api/admin/roles', { principal_id: 'p-ada', role: 'operator', location_id: 'loc-orders', target: 'development', revoke: true }]]);
	});

	it('disables a person by ID and returns them disabled', async () => {
		const { posts, client } = server();
		const account = await client.disableAccount('ada');
		expect(posts).toEqual([['/api/admin/people/p-ada', 'disable']]);
		expect(account.disabled).toBe(true);
	});

	it('reports a refusal from the server', async () => {
		const { client } = server(true);
		await expect(client.disableAccount('ada')).rejects.toMatchObject({ status: 403 });
	});
});

describe('testConnection', () => {
	const record = (completed: string | null, outcome: unknown = null) => ({
		generation: 1,
		requested_at: '2026-11-09T10:00:00Z',
		requested_by: 'ada',
		expires_at: '2026-11-09T10:05:00Z',
		completed_at: completed,
		request_json: { location_id: 'loc', target: 'development', request_id: 'r', reason: 'x', work: { kind: 'check_connection' } },
		result_json: outcome ? { claim_id: 'c', session_id: 's', outcome } : null
	});

	function server(answers: unknown[]) {
		const calls: string[] = [];
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				calls.push(request.method);
				if (request.method === 'POST') return Response.json({ applied: true });
				return Response.json(answers.length > 1 ? answers.shift() : answers[0]);
			})
		);
		return { calls, client: createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} }) };
	}

	it('asks the location once and waits until it answers', async () => {
		vi.setSystemTime(Date.parse('2026-11-09T10:00:00Z'));
		const { calls, client } = server([null, record(null), record('2026-11-09T10:00:03Z', { kind: 'connection', healthy: false })]);
		const result = client.testConnection(ORDERS, 'warehouse');
		await vi.advanceTimersByTimeAsync(3000);
		expect(await result).toMatchObject({ outcome: 'failed', durationMs: 3000, trigger: 'manual' });
		expect(calls).toEqual(['POST', 'GET', 'GET', 'GET']);
	});

	it('reports a test replaced by a newer request instead of a result', async () => {
		const { client } = server([record('2026-11-09T10:00:01Z', { kind: 'superseded' })]);
		const result = client.testConnection(ORDERS, 'warehouse');
		const settled = expect(result).rejects.toThrow('A newer request replaced this connection test');
		await vi.advanceTimersByTimeAsync(1000);
		await settled;
	});

	it('gives up when the location never answers', async () => {
		const { calls, client } = server([null]);
		const result = client.testConnection(ORDERS, 'warehouse');
		const settled = expect(result).rejects.toMatchObject({ status: 504 });
		await vi.advanceTimersByTimeAsync(61_000);
		await settled;
		expect(calls.length).toBe(61);
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

describe('node error text', () => {
	const RUN = '0191f0ee-0000-7000-8000-000000000001';

	/** Evidence holds on orders but not on customers, as for a person whose grant covers one node. */
	function serve(actions: string[]) {
		const paths: string[] = [];
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				const path = decodeURIComponent(new URL(request.url).pathname);
				paths.push(path);
				if (path.endsWith('/bootstrap')) {
					return Response.json({ login_mode: 'password', access: { name: 'cara', principal_id: 'p', actions: [], locations: [{ location_id: '0191f0aa-orders', name: 'Orders', target: 'development', actions }] } });
				}
				if (path.endsWith(`/runs/${RUN}/nodes/model:orders/error`)) {
					return Response.json({ node_key: 'model:orders', error_code: 'T002', message: 'Conversion Error: password=[REDACTED]\nLINE 3', path: 'models/orders.sql' });
				}
				return Response.json({ error: 'missing view.evidence permission' }, { status: 403 });
			})
		);
		return paths;
	}

	it('reads one node by its own key and maps the file', async () => {
		const paths = serve(['view', 'view.evidence']);
		const client = createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} });
		await client.bootstrap();
		expect(await client.getRunNodeError(ORDERS, RUN, 'model:orders')).toEqual({
			nodeKey: 'model:orders',
			errorCode: 'T002',
			message: 'Conversion Error: password=[REDACTED]\nLINE 3',
			file: 'models/orders.sql'
		});
		expect(paths.at(-1)).toBe(`/api/control/locations/0191f0aa-orders/targets/development/runs/${RUN}/nodes/model:orders/error`);
	});

	it('asks for a refused node once, so each audited refusal happens only once', async () => {
		const paths = serve(['view', 'view.evidence']);
		const client = createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} });
		await client.bootstrap();
		for (let i = 0; i < 2; i++) await expect(client.getRunNodeError(ORDERS, RUN, 'model:customers')).rejects.toMatchObject({ status: 403 });
		expect(paths.filter((p) => p.endsWith('/error'))).toHaveLength(1);
	});

	it('sends no request where the location grants no evidence', async () => {
		const paths = serve(['view']);
		const client = createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} });
		await client.bootstrap();
		await expect(client.getRunNodeError(ORDERS, RUN, 'model:orders')).rejects.toMatchObject({ status: 403 });
		expect(paths.filter((p) => p.endsWith('/error'))).toHaveLength(0);
	});
});

describe('listPlatformEventHistory', () => {
	const event = (storage_id: number, target: string) => ({
		storage_id,
		event_id: `e-${storage_id}`,
		occurred_at: '2026-11-09T09:00:00Z',
		location_id: '0191f0aa-orders',
		target,
		event: { kind: 'location_broken', location_id: '0191f0aa-orders', target, last_seen_at: '2026-11-09T08:00:00Z' }
	});

	/** The newest scan holds only another target's event, and the next scan holds the one asked for. */
	function server() {
		const scans: (string | null)[] = [];
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				const url = new URL(request.url);
				if (url.pathname.endsWith('/bootstrap')) return Response.json({ login_mode: 'password', access: { name: 'ada', principal_id: 'p', actions: [], locations: [] } });
				if (url.pathname.endsWith('/platform/problems')) return Response.json({ scheduler_leader: true, scheduler_lease_expires_at: null, broken_locations: [], overdue_ticks: [] });
				if (url.pathname.endsWith('/platform/events')) {
					const before = url.searchParams.get('before');
					scans.push(before);
					return Response.json(before === null ? { events: [event(9, 'preview')], before: 9 } : { events: [event(4, 'development')], before: null });
				}
				return Response.json([]);
			})
		);
		return { scans, client: createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} }) };
	}

	it("keeps scanning past a scan without the location's events until the server says none are older", async () => {
		const { scans, client } = server();
		const history = await client.listPlatformEventHistory({ locationId: '0191f0aa-orders:development' });
		expect(history.map((r) => r.id)).toEqual(['e-4']);
		expect(scans).toEqual([null, '9']);
	});
});

describe('getCatalogNode statistics', () => {
	const sample = (event_id: string, observed_at: string, row_count: number) => ({ event_id, record: { name: 'orders', state_schema: 'main', observed_at, row_count } });

	/** Two samples at first; a refresh after them finds a third. Everything else answers empty. */
	function server() {
		const reads: string[] = [];
		vi.stubGlobal('document', { cookie: '' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async (request: Request) => {
				const url = new URL(request.url);
				if (url.pathname.endsWith('/nodes'))
					return Response.json([{ node_key: 'model:orders', kind: 'model', name: 'orders', materialization: 'table', tags: [], dependencies: [], snapshot_id: 's', relation: { schema: 'main', name: 'orders' } }]);
				if (url.pathname.endsWith('/statistics')) {
					const after = url.searchParams.get('after_event_id');
					reads.push(`${url.searchParams.get('schema')}.${url.searchParams.get('name')} after ${after}`);
					return Response.json({ records: after ? [sample('e3', '2026-11-09T03:00:00Z', 30)] : [sample('e1', '2026-11-09T01:00:00Z', 10), sample('e2', '2026-11-09T02:00:00Z', 20)] });
				}
				if (url.pathname.endsWith('/node-status')) return Response.json(null);
				return Response.json([]);
			})
		);
		return { reads, client: createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} }) };
	}

	it('reads only the samples after the newest one it holds on a refresh, and keeps the earlier ones', async () => {
		const { reads, client } = server();
		const scope = { locationId: '0191f0aa-orders:development' };
		expect((await client.getCatalogNode(scope, 'orders'))?.lastRows).toBe(20);
		expect((await client.getCatalogNode(scope, 'orders'))?.lastRows).toBe(30);
		expect(reads).toEqual(['main.orders after null', 'main.orders after e2']);
	});
});
