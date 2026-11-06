import type { Scope } from './types';

// "Changed" signals: payload-free notifications that something changed on the
// server. Clients refetch through the normal endpoints when one arrives.

export type ChangedChannel =
	/** Refetch everything: sent on connect and whenever notices may have been missed. */
	| 'all'
	| 'runs'
	| `run:${string}`
	| 'ticks'
	| 'triggers'
	| 'jobs'
	| 'checks'
	| 'platform_events'
	| `snapshot:${string}`
	| `node_status:${string}`
	| `warehouse_sync:${string}`;

export type ChangedSignal = { channel: ChangedChannel; at: string };

export type ConnectionState = 'connecting' | 'live' | 'reconnecting' | 'offline';

export interface ChangeSubscription {
	/** Calls `handler` for every signal; returns an unsubscribe function. */
	subscribe(handler: (signal: ChangedSignal) => void): () => void;
	/** Calls `handler` with the current connection state and on every change. */
	onConnectionState(handler: (state: ConnectionState) => void): () => void;
	/** Follows the selected location's changes; `null` stops following. */
	follow(scope: Scope | null): void;
}

/** Whether a signal on `channel` should refresh a view listening to `topics`. */
export function channelMatches(channel: ChangedChannel, topics: readonly string[]): boolean {
	if (channel === 'all') return true;
	return topics.some((topic) => channel === topic || channel.startsWith(`${topic}:`));
}

/** In-process broadcaster used by the mock client (and by tests). */
export class LocalChangeBus implements ChangeSubscription {
	#handlers = new Set<(signal: ChangedSignal) => void>();
	#stateHandlers = new Set<(state: ConnectionState) => void>();
	#state: ConnectionState = 'live';

	subscribe(handler: (signal: ChangedSignal) => void): () => void {
		this.#handlers.add(handler);
		return () => this.#handlers.delete(handler);
	}

	onConnectionState(handler: (state: ConnectionState) => void): () => void {
		this.#stateHandlers.add(handler);
		handler(this.#state);
		return () => this.#stateHandlers.delete(handler);
	}

	follow(): void {
		// Every mock signal belongs to the one in-process store.
	}

	emit(channel: ChangedChannel): void {
		const signal = { channel, at: new Date().toISOString() };
		for (const handler of this.#handlers) handler(signal);
	}

	setState(state: ConnectionState): void {
		this.#state = state;
		for (const handler of this.#stateHandlers) handler(state);
	}
}
