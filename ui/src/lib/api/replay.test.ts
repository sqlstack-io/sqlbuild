import { describe, expect, it } from 'vitest';
import { READ_ONLY_MESSAGE, replayFetch, type Recording } from './replay';

const ok = (method: string, url: string, body: unknown, extra: object = {}) => ({ method, url, status: 200, contentType: 'application/json', text: JSON.stringify(body), ...extra });

const recording: Recording = {
	recordedAt: '2026-11-13T12:00:00.000Z',
	responses: [
		ok('GET', '/api/runs?limit=50', [{ id: 'newest', queued_at: '2026-11-13T11:00:00.000Z' }]),
		ok('GET', '/api/runs?before_queued_at=2026-11-13T11:00:00.000Z&limit=50', [{ id: 'older', queued_at: '2026-11-12T09:30:00.000Z' }]),
		ok('GET', '/api/stats?after_observed_at=2026-11-06T12:00:00.000Z&name=orders', { name: 'orders' }),
		ok('POST', '/api/l/plans', { accepted: true }, { body: { request_id: 'recorded-plan', command: 'sqb build --select orders' } }),
		ok('GET', '/api/l/plans/recorded-plan', { request_id: 'recorded-plan', models: 3 })
	]
};

// Seventeen days after the recording, so recorded times move forward by that much.
const now = Date.parse('2026-11-30T12:00:00.000Z');
const replay = () => replayFetch(async () => recording, () => now);
const get = (fetch: ReturnType<typeof replayFetch>, url: string) => fetch(new Request(`http://localhost${url}`));
const post = (fetch: ReturnType<typeof replayFetch>, url: string, body: unknown) =>
	fetch(new Request(`http://localhost${url}`, { method: 'POST', body: JSON.stringify(body), headers: { 'content-type': 'application/json' } }));

describe('demo replay', () => {
	it('answers with the recorded body, its times moved so the recording ends now', async () => {
		expect(await (await get(replay(), '/api/runs?limit=50')).json()).toEqual([{ id: 'newest', queued_at: '2026-11-30T11:00:00.000Z' }]);
	});

	it('pages with the moved times it handed out', async () => {
		const response = await get(replay(), '/api/runs?limit=50&before_queued_at=2026-11-30T11:00:00.000Z');
		expect(await response.json()).toEqual([{ id: 'older', queued_at: '2026-11-29T09:30:00.000Z' }]);
	});

	it('ignores times worked out from the clock and page sizes, but not other query values', async () => {
		const fetch = replay();
		expect((await get(fetch, '/api/stats?name=orders&after_observed_at=2026-11-23T13:05:00.000Z')).status).toBe(200);
		expect(await (await get(fetch, '/api/runs?limit=1')).json()).toEqual([{ id: 'newest', queued_at: '2026-11-30T11:00:00.000Z' }]);
		// Another node's statistics are not this node's.
		expect((await get(fetch, '/api/stats?name=customers&after_observed_at=2026-11-23T13:05:00.000Z')).status).toBe(404);
		expect(await (await get(fetch, '/api/nothing-recorded')).json()).toEqual({ message: 'This demo has no recording of this page.' });
	});

	it('refuses every write with the read-only message', async () => {
		const response = await post(replay(), '/api/control/runs', { run_id: 'x', job_key: 'nightly' });
		expect(response.status).toBe(403);
		expect(await response.json()).toEqual({ message: READ_ONLY_MESSAGE });
	});

	it('previews a recorded plan under the id the page chose', async () => {
		const fetch = replay();
		expect((await post(fetch, '/api/l/plans', { request_id: 'fresh-id', command: 'sqb build --select customers' })).status).toBe(404);
		expect((await post(fetch, '/api/l/plans', { request_id: 'fresh-id', command: 'sqb build --select orders' })).status).toBe(200);
		expect(await (await get(fetch, '/api/l/plans/fresh-id')).json()).toEqual({ request_id: 'recorded-plan', models: 3 });
	});
});
