// The Couldn't run blocks from the server's own wire shapes, through the HTTP client: a mock client once hid that they never showed.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { fixText, runDetailCantRun } from '$lib/run/cant-run';
import { createHttpClient } from './http-client';

type Wire = components['schemas'];

const LOCATION = '0191f0aa-orders';
const SCOPE = { locationId: `${LOCATION}:production` };
const RUN = '0191f0aa-0000-7000-8000-000000000001';
const AT = '2026-11-22T06:00:00Z';

/** A run as the server lists it, every field present. */
function runRecord(reason: string | null, change: Partial<Wire['RunRecord']> = {}): Wire['RunRecord'] {
	return {
		run_id: RUN,
		location_id: LOCATION,
		target: 'production',
		snapshot_id: '0191f0aa-0000-7000-8000-0000000000aa',
		snapshot_generation: 4,
		git_revision: null,
		git_dirty: false,
		sqb_version: '0.259.0',
		image_id: null,
		job_key: 'nightly_job',
		kind: 'job',
		origin: 'schedule',
		parent_run_id: null,
		root_run_id: RUN,
		trigger_id: null,
		trigger_name: null,
		tick_id: null,
		rerun_of_run_id: null,
		requested_by: 'system:scheduler',
		requester_name: null,
		priority: 0,
		rewrite_history: false,
		status: 'failed',
		status_reason: reason,
		queued_at: AT,
		launched_at: AT,
		started_at: AT,
		ended_at: AT,
		cancel_requested_at: null,
		selection_json: [],
		node_counts_json: { failed: 1 },
		ignored_audit_failures: 0,
		last_event_storage_id: 9,
		narrowing: null,
		...change
	};
}

/** A node as the server lists it, every field present. */
function nodeRecord(key: string, change: Partial<Wire['RunNodeRecord']>): Wire['RunNodeRecord'] {
	return {
		node_key: key,
		status: 'failed',
		started_at: null,
		ended_at: AT,
		rows_affected: null,
		table_rows: null,
		rows_so_far: null,
		no_change_reason: null,
		materialization: null,
		error_code: null,
		missing_secret: null,
		skipped_because_of: null,
		...change
	};
}

const failed = (key: string, message: string | null) => nodeRecord(key, { error_code: 'D012', missing_secret: message });
const skipped = (key: string, cause: string) => nodeRecord(key, { status: 'skipped', skipped_because_of: cause });
const notRun = (key: string) => nodeRecord(key, { status: 'not_run' });

/** The run page's blocks for a server with these runs and nodes, and every path the client asked for. */
async function blocks(runs: Wire['RunRecord'][], nodes: Record<string, Wire['RunNodeRecord'][]>) {
	const paths: string[] = [];
	vi.stubGlobal('document', { cookie: '' });
	vi.stubGlobal(
		'fetch',
		vi.fn(async (request: Request) => {
			const path = decodeURIComponent(new URL(request.url).pathname);
			paths.push(path);
			if (path.endsWith('/runs')) return Response.json(runs);
			const run = /\/runs\/([^/]+)\/nodes$/.exec(path)?.[1];
			return run ? Response.json(nodes[run] ?? []) : Response.json({ error: 'unexpected read' }, { status: 500 });
		})
	);
	const detail = await createHttpClient({ baseUrl: 'http://ui.test', onUnauthorized: () => {} }).getRunDetail(SCOPE, RUN);
	if (!detail) throw new Error('the run was not found');
	return { entries: runDetailCantRun(detail), paths };
}

afterEach(() => vi.unstubAllGlobals());

describe("a run's Couldn't run blocks over HTTP", () => {
	it('names the provider, variables, configured env file, users and readers', async () => {
		const places = "the location's environment and from config/production.env";
		const { entries } = await blocks([runRecord(`Couldn't run: provider region needs REGION_CODE, REGION_TOKEN (missing from ${places}); nothing ran`)], {
			[RUN]: [failed('source:raw_regions', `provider region: REGION_CODE, REGION_TOKEN not set (missing from ${places})`), skipped('model:orders', 'source:raw_regions'), notRun('model:unrelated')]
		});
		expect(entries).toEqual([
			{ reason: 'missing_secret', provider: 'region', path: '', variables: ['REGION_CODE', 'REGION_TOKEN'], nodes: ['source:raw_regions'], readers: ['model:orders'], sinks: [], env_file: 'config/production.env' }
		]);
		expect(fixText(entries[0], 'the next run works')).toBe(
			'set REGION_CODE and REGION_TOKEN in config/production.env, then restart the location (in sqb dev, press r), or in the environment the location (or sqb dev) was started with, then restart it; after that, the next run works'
		);
	});

	it('names only the environment when the location loads no env file', async () => {
		const places = "the location's environment";
		const { entries } = await blocks([runRecord(`Couldn't run: provider region needs REGION_CODE (missing from ${places}); nothing ran`)], {
			[RUN]: [failed('source:raw_regions', `provider region: REGION_CODE not set (missing from ${places})`)]
		});
		expect(entries[0].env_file).toBeNull();
		expect(fixText(entries[0], 'the next run works')).toBe('set REGION_CODE in the environment the location (or sqb dev) was started with, then restart it; after that, the next run works');
	});

	it('gives each node its own provider, with the variables redaction took from its message', async () => {
		const places = "the location's environment and from .env";
		const { entries } = await blocks([runRecord(`Couldn't run: provider orders_api_key needs ORDERS_API_KEY, ORDERS_REGION; provider region needs REGION_CODE (missing from ${places}); nothing ran`)], {
			[RUN]: [failed('source:raw_orders', `provider orders_api_key: [REDACTED], ORDERS_REGION not set (missing from ${places})`), failed('source:raw_regions', `provider region: REGION_CODE not set (missing from ${places})`)]
		});
		expect(entries.map((e) => [e.provider, e.variables, e.nodes])).toEqual([
			['orders_api_key', ['ORDERS_API_KEY', 'ORDERS_REGION'], ['source:raw_orders']],
			['region', ['REGION_CODE'], ['source:raw_regions']]
		]);
	});

	it('reads a thousand nodes failed on one secret with the one node listing', async () => {
		const places = "the location's environment and from .env";
		const keys = Array.from({ length: 1000 }, (_, i) => `loader:orders_${i}`);
		const { entries, paths } = await blocks([runRecord(`Couldn't run: provider region needs REGION_CODE (missing from ${places}); nothing ran`)], {
			[RUN]: keys.map((key) => failed(key, `provider region: REGION_CODE not set (missing from ${places})`))
		});
		expect(entries.map((e) => e.nodes)).toEqual([keys]);
		expect(paths.map((path) => path.split('/targets/production')[1])).toEqual(['/runs', `/runs/${RUN}/nodes`]);
	});

	it('shows a sink, which fails no node, alone or beside its provider', async () => {
		const places = "the location's environment and from .env";
		const alone = await blocks([runRecord(`Couldn't run: sink run_log needs LOG_TOKEN; every run uses sinks (missing from ${places}); nothing ran`)], { [RUN]: [notRun('model:orders')] });
		expect(alone.entries).toMatchObject([{ provider: '', variables: ['LOG_TOKEN'], nodes: [], sinks: ['run_log'], env_file: '.env' }]);
		vi.unstubAllGlobals();
		const beside = await blocks([runRecord(`Couldn't run: provider chat needs CHAT_URL; sink run_log needs CHAT_URL; every run uses sinks (missing from ${places}); nothing ran`)], {
			[RUN]: [failed('asset:notify', `provider chat: CHAT_URL not set (missing from ${places})`)]
		});
		expect(beside.entries).toMatchObject([{ provider: 'chat', variables: ['CHAT_URL'], nodes: ['asset:notify'], sinks: ['run_log'] }]);
	});

	it("reads a group's blocks from its job runs", async () => {
		const child = '0191f0aa-0000-7000-8000-000000000002';
		const places = "the location's environment and from .env";
		const group = runRecord('a job failed', { kind: 'group', job_key: 'nightly_job_group' });
		const member = runRecord(`Couldn't run: provider region needs REGION_CODE (missing from ${places}); nothing ran`, { run_id: child, parent_run_id: RUN });
		const { entries } = await blocks([group, member], {
			[RUN]: [failed('source:raw_regions', null)],
			[child]: [failed('source:raw_regions', `provider region: REGION_CODE not set (missing from ${places})`)]
		});
		expect(entries).toMatchObject([{ provider: 'region', nodes: ['source:raw_regions'] }]);
	});

	it('still names the provider once history is pruned and after a note on the reason', async () => {
		const reason = "Couldn't run: provider region needs REGION_CODE (missing from the location's environment); nothing ran; not retried: this run's snapshot can't be read";
		const { entries } = await blocks([runRecord(reason)], { [RUN]: [failed('source:raw_regions', null)] });
		expect(entries).toMatchObject([{ provider: 'region', variables: ['REGION_CODE'], nodes: ['source:raw_regions'], env_file: null }]);
	});

	it('shows nothing for other failures', async () => {
		const { entries } = await blocks([runRecord('worker executor completed and closed its connections')], {
			[RUN]: [nodeRecord('asset:notify', { error_code: 'D012', started_at: AT })]
		});
		expect(entries).toEqual([]);
	});
});
