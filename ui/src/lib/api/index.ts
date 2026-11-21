import type { ApiClient } from './client';
import { createHttpClient } from './http-client';
import { MockApiClient, type MockOptions } from './mock/mock-client';
import { largeProject } from './mock/large-project';
import { hugeProject } from './mock/huge-project';
import { awkward } from './mock/awkward';
import { calm } from './mock/calm';
import { replayFetch, type Recording } from './replay';
import { LocalChangeBus } from './signals';
import { to } from '$lib/paths';
import type { BuiltInRole } from './types';

export type { ApiClient } from './client';
export { ApiError, isRefusal, isUnsupported } from './client';
export * from './types';
export * from './signals';
export { mergePlanPage } from './http-runs';
export { hasCapability, hasAnyCapability, ADMIN_CAPABILITIES } from './capabilities';

const ROLES: readonly BuiltInRole[] = ['viewer', 'operator', 'developer', 'admin'];
const EVENTS: readonly NonNullable<MockOptions['events']>[] = ['default', 'none', 'all'];
const LIVE: readonly NonNullable<MockOptions['connection']>[] = ['live', 'reconnecting', 'offline'];

/** The static demo (`VITE_SQB_DEMO=1`): the server client replaying a recording served beside the app. */
export const DEMO = import.meta.env.VITE_SQB_DEMO === '1';

async function loadRecording(): Promise<Recording> {
	const response = await fetch(to('/demo-recording.json.gz'));
	if (!response.ok) throw new Error(`The demo recording is missing (${response.status})`);
	const bytes = new Uint8Array(await response.arrayBuffer());
	// A host that serves the file with Content-Encoding has already unpacked it.
	const packed = bytes[0] === 0x1f && bytes[1] === 0x8b;
	const body = packed ? new Response(new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'))) : new Response(bytes);
	return body.json();
}

/**
 * The client the app uses: the server, or the mock with `?mock` or `VITE_SQB_MOCK=1`.
 * With the mock, `?as=<role>`, `?events=none|all` and `?live=reconnecting|offline`
 * adjust it for demos, `?awkward` stretches every list to thousands of rows with very long
 * names, for the layout test, `?calm` reports nothing needing attention, `?large` swaps in a catalog the size of a real project, `?huge` one
 * of about 10,000 nodes in nested folders, for the Overview's and the folder view's performance budgets,
 * and `?still` keeps runs from advancing on their own.
 */
export function createClient(search: URLSearchParams, onUnauthorized: () => void = () => {}): ApiClient {
	if (DEMO) return createHttpClient({ onUnauthorized, fetch: replayFetch(loadRecording), changes: new LocalChangeBus() });
	if (!search.has('mock') && import.meta.env.VITE_SQB_MOCK !== '1') return createHttpClient({ onUnauthorized });
	const role = search.get('as');
	const events = search.get('events');
	const live = search.get('live');
	const mock = new MockApiClient({
		role: ROLES.find((r) => r === role),
		events: EVENTS.find((e) => e === events),
		connection: LIVE.find((l) => l === live),
		still: search.has('still')
	});
	// `?summary`: the large project with a plan shaped like a large real one's summary (CHI-1563).
	const summary = search.has('summary');
	const sized = search.has('huge') ? hugeProject(mock) : search.has('large') || summary ? largeProject(mock, { summary }) : mock;
	const stretched = search.has('awkward') ? awkward(sized) : sized;
	return countedReads(search.has('calm') ? calm(stretched) : stretched);
}

/** Calls to each mock client method so far, by name, for the browser suite's work budgets. */
const mockReads: Record<string, number> = {};

/** The mock client, counting each method call in `mockReads` (also `window.__sqbMockReads`). */
function countedReads(client: ApiClient): ApiClient {
	(globalThis as { __sqbMockReads?: Record<string, number> }).__sqbMockReads = mockReads;
	return new Proxy(client, {
		get(target, name, receiver) {
			const value = Reflect.get(target, name, receiver);
			if (typeof value !== 'function' || typeof name !== 'string') return value;
			return (...args: unknown[]) => {
				mockReads[name] = (mockReads[name] ?? 0) + 1;
				return value.apply(target, args);
			};
		}
	});
}
