import type { ApiClient } from './client';
import { createHttpClient } from './http-client';
import { MockApiClient, type MockOptions } from './mock/mock-client';
import { replayFetch, type Recording } from './replay';
import { LocalChangeBus } from './signals';
import { to } from '$lib/paths';
import type { BuiltInRole } from './types';

export type { ApiClient } from './client';
export { ApiError, isRefusal } from './client';
export * from './types';
export * from './signals';
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
 * adjust it for demos.
 */
export function createClient(search: URLSearchParams, onUnauthorized: () => void = () => {}): ApiClient {
	if (DEMO) return createHttpClient({ onUnauthorized, fetch: replayFetch(loadRecording), changes: new LocalChangeBus() });
	if (!search.has('mock') && import.meta.env.VITE_SQB_MOCK !== '1') return createHttpClient({ onUnauthorized });
	const role = search.get('as');
	const events = search.get('events');
	const live = search.get('live');
	return new MockApiClient({
		role: ROLES.find((r) => r === role),
		events: EVENTS.find((e) => e === events),
		connection: LIVE.find((l) => l === live)
	});
}
