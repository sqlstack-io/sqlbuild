// The static demo's network: answers the server client's requests from a recording of a real
// server, moved forward in time so the newest recorded moment reads as now. Nothing is sent.

/** One answered request, as `ui/scripts/record-demo.mjs` writes it. */
export type RecordedResponse = {
	method: string;
	/** Path and query as requested, e.g. `/api/control/audit?limit=100`. */
	url: string;
	/** The request body, for the few reads sent as POST. */
	body?: unknown;
	status: number;
	contentType: string;
	text: string;
};

export type Recording = { recordedAt: string; responses: RecordedResponse[] };

/** POSTs that only read, such as a plan preview; every other write is refused. */
const READ_POSTS = [/\/plans$/, /\/runs\/adhoc\/check$/];

export const READ_ONLY_MESSAGE = 'This demo is read-only. Run sqb dev on your own project to start runs and change settings.';

const ISO = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z/g;
const isTime = (value: string) => /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value);

const shiftTimes = (text: string, by: number) => text.replace(ISO, (iso) => new Date(Date.parse(iso) + by).toISOString());

/** A body without its caller-chosen ids, so a replayed POST matches the recorded one. */
function withoutIds(body: unknown): string {
	if (!body || typeof body !== 'object') return JSON.stringify(body ?? null);
	return JSON.stringify(Object.fromEntries(Object.entries(body).filter(([key]) => !key.endsWith('_id')).sort(([a], [b]) => a.localeCompare(b))));
}

const sortedQuery = (url: URL) => [...url.searchParams].sort(([a, b], [c, d]) => a.localeCompare(c) || b.localeCompare(d));

const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

/**
 * A `fetch` for the server client that replays `load()`'s recording. A GET matches its path and
 * query; failing that, the same non-time query values; failing that, the same values apart from
 * page size. Anything else, such as a node the recording doesn't hold, has no recording.
 * A read sent as POST matches its path and body apart from ids, and later reads of the ids it
 * chose are answered for the recorded ones.
 */
export function replayFetch(load: () => Promise<Recording>, now: () => number = Date.now): (request: Request) => Promise<Response> {
	let recording: Promise<{ shift: number; responses: (RecordedResponse & { parsed: URL })[] }> | null = null;
	const ids = new Map<string, string>();
	return async (request) => {
		recording ??= load().then((r) => ({
			shift: now() - Date.parse(r.recordedAt),
			responses: r.responses.map((response) => ({ ...response, parsed: new URL(response.url, 'http://demo') }))
		}));
		const { shift, responses } = await recording;
		const url = new URL(request.url, 'http://demo');
		let path = url.pathname;
		for (const [asked, recorded] of ids) path = path.replace(asked, recorded);
		const answer = (found: RecordedResponse) =>
			new Response(found.status === 204 ? null : shiftTimes(found.text, shift), { status: found.status, headers: { 'content-type': found.contentType } });

		if (request.method !== 'GET') {
			if (!READ_POSTS.some((pattern) => pattern.test(path))) return json(403, { message: READ_ONLY_MESSAGE });
			const body: unknown = await request.json().catch(() => null);
			const found = responses.find((r) => r.method === request.method && r.parsed.pathname === path && withoutIds(r.body) === withoutIds(body));
			if (!found) return json(404, { message: 'This demo has no recording of that request; try one of the suggested commands.' });
			for (const [key, value] of Object.entries(body ?? {})) {
				const recorded = (found.body as Record<string, unknown> | undefined)?.[key];
				if (key.endsWith('_id') && typeof value === 'string' && typeof recorded === 'string') ids.set(value, recorded);
			}
			return answer(found);
		}

		// Query times were moved forward with the data, so move them back before comparing.
		const query = sortedQuery(url).map(([key, value]): [string, string] => [key, isTime(value) ? shiftTimes(value, -shift) : value]);
		// Page sizes may differ from the recording's, but every other value names what is asked for:
		// a different node_key is a different item and must not borrow another item's answer.
		const timeless = (q: [string, string][]) => JSON.stringify(q.filter(([, value]) => !isTime(value)));
		const sizeless = (q: [string, string][]) => JSON.stringify(q.filter(([key, value]) => key !== 'limit' && !isTime(value)));
		const gets = responses.filter((r) => r.method === 'GET' && r.parsed.pathname === path);
		const found =
			gets.find((r) => JSON.stringify(sortedQuery(r.parsed)) === JSON.stringify(query)) ??
			gets.find((r) => timeless(sortedQuery(r.parsed)) === timeless(query)) ??
			gets.find((r) => sizeless(sortedQuery(r.parsed)) === sizeless(query));
		return found ? answer(found) : json(404, { message: 'This demo has no recording of this page.' });
	};
}
