// A control server faked inside a browser test, so the real page and its server client run against
// state the test changes as it goes. Lives only in the tests: the app has no hook for it, it answers
// the page's requests through Playwright's routing. Two targets, `shop` and `returns`.

import type { Page, Route } from '@playwright/test';

export const LOCATIONS = { shop: '0190d1a0-0000-7000-8000-000000000001', returns: '0190d1a0-0000-7000-8000-000000000002' } as const;
export type TargetName = keyof typeof LOCATIONS;
const SNAPSHOT = '0190d1a0-0000-7000-8000-0000000000aa';
const ACTIONS = ['view', 'view.sql', 'view.evidence', 'job.run', 'run.cancel', 'run.rerun', 'run.adhoc', 'audit.view'];

type Ack = { event_id: string; key: string; shape: unknown; acknowledged_at: string };
/** One request the page wrote: an acknowledgement, a withdrawal or a run's acknowledgement. */
export type Write = { target: TargetName; path: string; body: Record<string, unknown>; status: number };

/** What one target holds; tests change it between reads. */
export type TargetState = {
	/** Models whose definition changed since their last build: "N nodes out of date". */
	changed: string[];
	/** The warehouse sync: done, still going, or failed. */
	sync: 'settled' | 'running' | 'failed';
	/** Whether the scheduler has a leader; without one, every viewer sees it stopped. */
	schedulerLeader: boolean;
	acks: Ack[];
	/** A failed run on the attention list, to acknowledge. */
	failedRun: boolean;
	/** Delays the answers to this target's Overview reads. */
	readDelayMs: number;
	/** Refuses this target's Overview reads with a server error. */
	failReads: boolean;
	/** Refuses withdrawals of condition acknowledgements with a server error. */
	failWithdrawals: boolean;
	/** Delays the sync status, so it arrives after the page's first read has begun. */
	healthDelayMs: number;
};

const MODELS = ['orders', 'order_items', 'customers', 'payments', 'refunds'];

const json = (route: Route, body: unknown, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });
const later = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export class FakeControl {
	readonly targets: Record<TargetName, TargetState>;
	readonly writes: Write[] = [];
	#notices: string[] = [];
	#lease = new Date(Date.now() - 60_000).toISOString();

	constructor(overrides: Partial<Record<TargetName, Partial<TargetState>>> = {}) {
		const base = (): TargetState => ({ changed: [], sync: 'settled', schedulerLeader: true, acks: [], failedRun: false, readDelayMs: 0, failReads: false, failWithdrawals: false, healthDelayMs: 0 });
		this.targets = { shop: { ...base(), ...overrides.shop }, returns: { ...base(), ...overrides.returns } };
	}

	/** Withdrawals of condition acknowledgements the page sent, in order. */
	withdrawals(target?: TargetName): Write[] {
		return this.writes.filter((w) => w.path.endsWith('/attention-acknowledgements') && w.body.acknowledged === false && (!target || w.target === target));
	}

	/** Tells the page something changed, as the server's change feed does, so it reads again. */
	notify(topic: 'runs' | 'all' = 'runs') {
		this.#notices.push(topic);
	}

	/** A new scheduler outage: the lease that ran out is a new one. */
	stopScheduler() {
		this.#lease = new Date().toISOString();
		for (const t of Object.values(this.targets)) t.schedulerLeader = false;
	}

	async install(page: Page) {
		// The first location is the one the page opens on.
		await page.addInitScript(() => localStorage.clear());
		await page.route('**/api/**', (route) => this.#answer(route));
	}

	async #answer(route: Route) {
		const request = route.request();
		const url = new URL(request.url());
		const path = url.pathname;
		if (path === '/api/control/bootstrap') {
			return json(route, {
				login_mode: 'disabled',
				access: {
					principal_id: '0190d1a0-0000-7000-8000-0000000000bb',
					name: 'tester',
					actions: ACTIONS,
					locations: (Object.keys(LOCATIONS) as TargetName[]).map((name) => ({ location_id: LOCATIONS[name], name, target: 'dev', actions: ACTIONS }))
				}
			});
		}
		if (path === '/api/control/platform/problems') {
			const leader = Object.values(this.targets).every((t) => t.schedulerLeader);
			const delay = Math.max(...Object.values(this.targets).map((t) => t.readDelayMs));
			if (delay) await later(delay);
			return json(route, { scheduler_leader: leader, scheduler_lease_expires_at: this.#lease, broken_locations: [], overdue_ticks: [], low_calendars: [] });
		}
		const scoped = /^\/api\/control\/locations\/([^/]+)\/targets\/dev\/(.+)$/.exec(path);
		const name = scoped && (Object.keys(LOCATIONS) as TargetName[]).find((n) => LOCATIONS[n] === scoped[1]);
		if (!scoped || !name) return json(route, { error: 'not found' }, 404);
		const target = this.targets[name];
		const rest = scoped[2];
		if (rest === 'changes') {
			const events = this.#notices.splice(0).map((topic) => `event: changed\ndata: ${JSON.stringify({ topic })}\n\n`);
			return route.fulfill({ status: 200, contentType: 'text/event-stream', body: `retry: 200\n\n${events.join('')}` });
		}
		if (request.method() === 'POST') return this.#write(route, name, rest, request.postDataJSON() as Record<string, unknown>);
		if (rest === 'health') {
			if (target.healthDelayMs) await later(target.healthDelayMs);
			return json(route, this.#health(target));
		}
		if (rest === 'nodes') return json(route, MODELS.map((m) => ({ ...this.#node(target, m), node_key: `model:${m}`, kind: 'model', name: m, path: `models/shop/${m}.sql`, materialization: 'table', description: null, tags: [], dependencies: [], relation: { database: null, schema: 'dev', name: m }, column_count: 3, snapshot_id: SNAPSHOT })));
		if (rest === 'node-states') return json(route, { snapshot_id: SNAPSHOT, version: 1, states: Object.fromEntries(MODELS.map((m) => [`model:${m}`, this.#node(target, m)])) });
		if (rest.startsWith('snapshots')) return json(route, [{ snapshot_id: SNAPSHOT, generation: 1, git_revision: null, git_dirty: false, git_author: null, environment_hash: 'e'.repeat(64), sqb_version: '0.0.0', published_at: '2026-11-01T00:00:00Z', changes: [], connection: null }]);
		if (rest === 'jobs' || rest === 'triggers' || rest === 'checks' || rest === 'acknowledgements') return json(route, []);
		if (rest.startsWith('cost')) return json(route, { since: '2026-11-01T00:00:00Z', credits: 0, usd: null, runs: 0, incomplete_runs: 0, by_job: [], by_warehouse_size: [], by_node: [] });
		// The Overview's own reads: slowed or refused as the test asks.
		if (target.readDelayMs) await later(target.readDelayMs);
		if (target.failReads) return json(route, { error: 'the control store is unavailable' }, 500);
		if (rest.startsWith('runs')) return json(route, target.failedRun ? [this.#failedRun(name)] : []);
		if (rest === 'attention-acknowledgements') {
			return json(route, { snooze_seconds: 86_400, acknowledgements: target.acks.map((a) => ({ ...a, acknowledged_by: 'tester', until: new Date(Date.parse(a.acknowledged_at) + 86_400_000).toISOString() })) });
		}
		return json(route, { error: 'not found' }, 404);
	}

	#write(route: Route, name: TargetName, rest: string, body: Record<string, unknown>) {
		const target = this.targets[name];
		const record = (status: number) => this.writes.push({ target: name, path: rest.startsWith('/') ? rest : `/${rest}`, body, status });
		if (rest !== 'attention-acknowledgements') {
			record(200);
			return json(route, { applied: true });
		}
		if (!body.acknowledged && target.failWithdrawals) {
			record(500);
			return json(route, { error: 'the control store is unavailable' }, 500);
		}
		record(200);
		const latest = target.acks.find((a) => a.key === body.key);
		if (body.acknowledged) {
			target.acks = [...target.acks.filter((a) => a.key !== body.key), { event_id: String(body.event_id), key: String(body.key), shape: body.shape, acknowledged_at: new Date().toISOString() }];
			return json(route, { applied: true });
		}
		// As the server: a withdrawal naming an acknowledgement applies only while that one is the latest.
		const applies = !!latest && (body.withdraws === undefined || body.withdraws === latest.event_id);
		if (applies) target.acks = target.acks.filter((a) => a.key !== body.key);
		return json(route, { applied: applies });
	}

	#node(target: TargetState, model: string) {
		const builtAt = '2026-11-01T00:00:00Z';
		return {
			last_build: { observed_at: builtAt, run_id: 'build-1', version_hash: 'f'.repeat(64) },
			last_outcome: { status: 'success', at: builtAt, run_id: '0190d1a0-0000-7000-8000-0000000000cc' },
			definition_changed: target.changed.includes(model),
			checks: { total: 0, passing: 0, warning: 0, failing: 0 }
		};
	}

	#health(target: TargetState) {
		const at = new Date().toISOString();
		const health = {
			kind: 'warehouse_sync',
			generation: 1,
			location: { database: null, schema: 'dev' },
			table: 'catalog',
			failure: target.sync === 'failed' ? 'read' : null,
			records_sent: 5,
			elapsed_millis: 10,
			progress: { position: target.sync === 'running' ? 1 : 2, kinds: 2, rows: 5, elapsed_millis: 20, completed: target.sync !== 'running' }
		};
		return [{ received_at: at, report: { event_id: '0190d1a0-0000-7000-8000-0000000000dd', session_id: '0190d1a0-0000-7000-8000-0000000000ee', target: 'dev', health } }];
	}

	#failedRun(name: TargetName) {
		const at = new Date(Date.now() - 600_000).toISOString();
		const id = '0190d1a0-0000-7000-8000-0000000000f1';
		return {
			run_id: id, location_id: LOCATIONS[name], target: 'dev', snapshot_id: SNAPSHOT, snapshot_generation: 1, git_revision: null, git_dirty: false, sqb_version: '0.0.0', image_id: null,
			job_key: null, kind: 'adhoc', origin: 'manual', parent_run_id: null, root_run_id: id, trigger_id: null, trigger_name: null, tick_id: null, rerun_of_run_id: null,
			requested_by: 'principal:tester', requester_name: 'tester', priority: 0, rewrite_history: false, status: 'failed', status_reason: 'a node failed',
			queued_at: at, launched_at: at, started_at: at, ended_at: at, cancel_requested_at: null, selection_json: ['model:orders'], node_counts_json: { failed: 1 },
			ignored_audit_failures: 0, last_event_storage_id: 1, narrowing: null
		};
	}
}
