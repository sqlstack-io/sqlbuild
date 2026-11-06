import createFetch from 'openapi-fetch';
import type { components, paths } from '$control/schema.d.ts';
import { ApiError, type ApiClient } from './client';
import type { ChangeSubscription, ChangedChannel, ChangedSignal, ConnectionState } from './signals';
import type { AuthMode, Bootstrap, Capability, Location, Scope } from './types';

type Wire = components['schemas'];

/** The client methods the server has endpoints for; the shell hides pages that need others. */
const SUPPORTED: ReadonlySet<string> = new Set<keyof ApiClient>(['supports', 'bootstrap', 'changes']);

/** A UI location is one server location and target; its id carries both. */
export function locationKey(locationId: string, target: string): string {
	return `${locationId}:${target}`;
}

export function parseLocationKey(key: string): { locationId: string; target: string } {
	const split = key.indexOf(':');
	return { locationId: key.slice(0, split), target: key.slice(split + 1) };
}

export function toBootstrap(wire: Wire['ControlBootstrap']): Bootstrap {
	const authMode: AuthMode = wire.login_mode;
	return {
		authMode,
		user: { kind: 'user', name: wire.access.name, displayName: wire.access.name },
		capabilities: wire.access.actions as Capability[],
		locations: wire.access.locations.map(
			(location): Location => ({
				id: locationKey(location.location_id, location.target),
				name: location.name,
				health: null,
				lastSeenAt: null,
				target: { name: location.target, adapter: null, connectionSummary: null },
				warehouseSync: { lastSyncedAt: null, lastError: null },
				capabilities: location.actions as Capability[]
			})
		)
	};
}

/** The change feed of the followed location: one `EventSource`, recreated when the scope changes. */
export class HttpChangeFeed implements ChangeSubscription {
	#handlers = new Set<(signal: ChangedSignal) => void>();
	#stateHandlers = new Set<(state: ConnectionState) => void>();
	#state: ConnectionState = 'connecting';
	#source: EventSource | null = null;
	#retry: ReturnType<typeof setTimeout> | null = null;
	#failures = 0;

	/** `stillListed` re-reads the bootstrap before a retry, so a lost session or location stops retrying. */
	constructor(
		private readonly baseUrl: string,
		private readonly stillListed: (scope: Scope) => Promise<boolean>
	) {}

	subscribe(handler: (signal: ChangedSignal) => void): () => void {
		this.#handlers.add(handler);
		return () => this.#handlers.delete(handler);
	}

	onConnectionState(handler: (state: ConnectionState) => void): () => void {
		this.#stateHandlers.add(handler);
		handler(this.#state);
		return () => this.#stateHandlers.delete(handler);
	}

	follow(scope: Scope | null): void {
		this.#failures = 0;
		this.#connect(scope);
	}

	#connect(scope: Scope | null) {
		this.#source?.close();
		this.#source = null;
		if (this.#retry) clearTimeout(this.#retry);
		this.#retry = null;
		if (!scope) return this.#setState('offline');
		const { locationId, target } = parseLocationKey(scope.locationId);
		const source = new EventSource(
			`${this.baseUrl}/api/control/locations/${encodeURIComponent(locationId)}/targets/${encodeURIComponent(target)}/changes`
		);
		this.#source = source;
		this.#setState('connecting');
		source.onopen = () => {
			this.#failures = 0;
			this.#setState('live');
		};
		// The browser retries a dropped stream by itself, but gives up on any failed response.
		source.onerror = () => {
			if (source.readyState !== EventSource.CLOSED) return this.#setState('reconnecting');
			this.#failures += 1;
			this.#setState(this.#failures > 3 ? 'offline' : 'reconnecting');
			const delay = Math.min(30_000, 1000 * 2 ** (this.#failures - 1));
			this.#retry = setTimeout(() => {
				void this.stillListed(scope)
					.catch(() => true)
					.then((listed) => (listed && this.#source === source ? this.#connect(scope) : this.#setState('offline')));
			}, delay);
		};
		source.addEventListener('changed', (event) => {
			const notice = JSON.parse((event as MessageEvent<string>).data) as Wire['ChangeNotice'];
			this.#emit(channelOf(notice.topic, scope));
		});
	}

	#emit(channel: ChangedChannel) {
		const signal = { channel, at: new Date().toISOString() };
		for (const handler of this.#handlers) handler(signal);
	}

	#setState(state: ConnectionState) {
		this.#state = state;
		for (const handler of this.#stateHandlers) handler(state);
	}
}

/** Server topics name the server location; the UI's channels name the UI location. */
export function channelOf(topic: string, scope: Scope): ChangedChannel {
	if (topic.startsWith('snapshot:')) return `snapshot:${scope.locationId}`;
	if (topic.startsWith('node_status:')) return `node_status:${scope.locationId}`;
	return topic === 'runs' || topic === 'ticks' ? topic : 'all';
}

export type HttpClientOptions = {
	baseUrl?: string;
	/** Called when the session is missing or rejected, before the request fails. */
	onUnauthorized: () => void;
};

/**
 * The server-backed client. Methods without an endpoint throw, and `supports`
 * reports them so the shell never shows a page it cannot load.
 */
export function createHttpClient({ baseUrl = '', onUnauthorized }: HttpClientOptions): ApiClient {
	const http = createFetch<paths>({ baseUrl, credentials: 'same-origin' });
	const implemented = {
		supports: (method: keyof ApiClient) => SUPPORTED.has(method),
		changes: new HttpChangeFeed(baseUrl, async (scope) => {
			const { data, response } = await http.GET('/api/control/bootstrap');
			if (response.status === 401) onUnauthorized();
			if (!data) return response.status >= 500;
			return toBootstrap(data).locations.some((location) => location.id === scope.locationId);
		}),
		async bootstrap(): Promise<Bootstrap> {
			const { data, error, response } = await http.GET('/api/control/bootstrap');
			if (response.status === 401) onUnauthorized();
			if (!data) throw new ApiError(errorText(error, response), response.status);
			return toBootstrap(data);
		}
	};
	return new Proxy(implemented, {
		get(target, property, receiver) {
			if (property in target) return Reflect.get(target, property, receiver);
			return () => Promise.reject(new ApiError(`${String(property)} is not available on this server yet`, 501));
		}
	}) as unknown as ApiClient;
}

function errorText(error: unknown, response: Response): string {
	const message = (error as { message?: unknown; error?: unknown } | undefined)?.message ?? (error as { error?: unknown } | undefined)?.error;
	return typeof message === 'string' ? message : `${response.status} ${response.statusText}`;
}

/** The CSRF token the server sets in a readable cookie at sign-in; writes send it back. */
export function csrfToken(cookies: string): string {
	for (const cookie of cookies.split(';')) {
		const [name, ...value] = cookie.trim().split('=');
		if (name === '__Host-sqb-csrf') return value.join('=');
	}
	return '';
}

/** Signs in with a password; resolves to whether the server accepted it. */
export async function loginWithPassword(username: string, password: string, baseUrl = ''): Promise<'ok' | 'rejected' | 'throttled' | 'unavailable'> {
	const http = createFetch<paths>({ baseUrl, credentials: 'same-origin' });
	const { response } = await http.POST('/api/auth/password/login', { body: { username, password } });
	if (response.ok) return 'ok';
	if (response.status === 429) return 'throttled';
	return response.status === 401 ? 'rejected' : 'unavailable';
}

export async function logout(baseUrl = ''): Promise<void> {
	const http = createFetch<paths>({ baseUrl, credentials: 'same-origin' });
	await http.POST('/api/auth/logout', { params: { header: { 'x-sqb-csrf': csrfToken(document.cookie) } } });
}
