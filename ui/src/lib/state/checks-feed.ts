import type { ApiClient, ChangedSignal, Check, Scope } from '$lib/api';
import { isRefusal } from '$lib/api';

/** How long a burst of signals may last before one read starts. */
const SIGNAL_PAUSE_MS = 300;

/** `scopeKey` is the location key the view holds checks for, so a reader can tell them from another target's. */
export type ChecksView = { checks: Check[] | undefined; error: string | null; loading: boolean; scopeKey?: string | null };

const message = (reason: unknown) => (reason instanceof Error ? reason.message : String(reason));

/**
 * The selected location's checks with their newest results, shared by every page that lists them: the
 * Checks page, the plan page and the node view read them here instead of each reading the list. The
 * list is read once, then again after a signal that may come with new results (a run, a check, a node
 * state change, a new snapshot or a reconnect), signals of a burst coalesced into one read. The client
 * sends the held ETag, so a read after a burst that changed no result is a 304 and returns the held list.
 */
export class ChecksFeed {
	#view: ChecksView = { checks: undefined, error: null, loading: false };
	#scope: Scope | null = null;
	/** Bumped by a scope change, so reads for the old scope are dropped when they answer. */
	#epoch = 0;
	#read = 0;
	#inFlight = false;
	/** A signal arrived while a read was in flight, so one more read starts when it answers. */
	#again = false;
	#pause: ReturnType<typeof setTimeout> | null = null;

	constructor(
		private readonly client: () => ApiClient,
		private readonly publish: (view: ChecksView) => void,
		private readonly pauseMs = SIGNAL_PAUSE_MS
	) {}

	get view(): ChecksView {
		return this.#view;
	}

	/** Starts reading the scope's checks unless they are held or already being read. */
	ensure(scope: Scope): void {
		if (this.#scope?.locationId === scope.locationId && (this.#view.checks || this.#view.loading)) return;
		if (this.#scope?.locationId !== scope.locationId) this.#reset(scope);
		this.#load();
	}

	/** Reads the list again, as a page's refresh button asks. */
	refresh(): void {
		if (this.#scope) this.#load();
	}

	handle(signal: ChangedSignal): void {
		const scope = this.#scope;
		if (!scope || (!this.#view.checks && !this.#view.loading)) return;
		const location = scope.locationId;
		const { channel } = signal;
		if (channel === 'all' || channel === 'runs' || channel === 'checks' || channel === `node_status:${location}` || channel === `snapshot:${location}`) this.#schedule();
	}

	#reset(scope: Scope) {
		this.#epoch += 1;
		this.#scope = scope;
		this.#inFlight = false;
		this.#again = false;
		if (this.#pause) clearTimeout(this.#pause);
		this.#pause = null;
		this.#set({ checks: undefined, error: null, loading: false });
	}

	#set(view: Partial<ChecksView>) {
		this.#view = { ...this.#view, ...view, scopeKey: this.#scope?.locationId ?? null };
		this.publish(this.#view);
	}

	#schedule() {
		if (this.#pause) return;
		this.#pause = setTimeout(() => {
			this.#pause = null;
			if (this.#inFlight) this.#again = true;
			else this.#load();
		}, this.pauseMs);
	}

	#load() {
		const scope = this.#scope;
		if (!scope) return;
		const epoch = this.#epoch;
		const read = ++this.#read;
		this.#inFlight = true;
		this.#set({ loading: true });
		const settle = () => {
			this.#inFlight = false;
			if (!this.#again) return;
			this.#again = false;
			this.#load();
		};
		this.client()
			.listChecks(scope)
			.then(
				(checks) => {
					if (epoch !== this.#epoch || read !== this.#read) return;
					// A 304 returns the held list itself, so pages keyed on it do not redraw.
					if (checks === this.#view.checks) this.#set({ error: null, loading: false });
					else this.#set({ checks, error: null, loading: false });
					settle();
				},
				(reason: unknown) => {
					if (epoch !== this.#epoch || read !== this.#read) return;
					// A refusal drops the held checks, so a person whose access went sees the refusal, not old data.
					if (isRefusal(reason)) this.#set({ checks: undefined });
					this.#set({ error: message(reason), loading: false });
					settle();
				}
			);
	}
}
