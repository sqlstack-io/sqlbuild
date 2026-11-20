import type { ApiClient, ChangedSignal, Check, Scope } from '$lib/api';
import { isRefusal } from '$lib/api';

/** How long a burst of signals may last before one read starts. */
const SIGNAL_PAUSE_MS = 300;

/**
 * `scopeKey` is the location key the view holds checks for, so a reader can tell them from another target's.
 * `revision` counts the reads that have answered or failed, a 304 included, so a page can follow each one.
 */
export type ChecksView = { checks: Check[] | undefined; error: string | null; loading: boolean; revision: number; scopeKey?: string | null };

const message = (reason: unknown) => (reason instanceof Error ? reason.message : String(reason));

/**
 * The selected location's checks with their newest results, shared by every page that lists them: the
 * Checks page, the plan page and the node view read them here instead of each reading the list. The
 * list is read once, then again after a signal that may come with new results (a run, a check, a node
 * state change, a warehouse sync, a new snapshot or a reconnect), signals of a burst coalesced into one
 * read. That read never joins one begun before the signal, which may have missed it. The client sends the
 * held ETag, so a read after a burst that changed no result is a 304 and returns the held list. While no
 * page watches, a signal only marks the list stale, and the next page to watch reads it once. A failed
 * read is retried by the next signal; after a refusal only a refresh or a new page reads again.
 */
export class ChecksFeed {
	#view: ChecksView = { checks: undefined, error: null, loading: false, revision: 0 };
	#scope: Scope | null = null;
	/** Bumped by a scope change, so reads for the old scope are dropped when they answer. */
	#epoch = 0;
	#read = 0;
	#inFlight = false;
	/** A signal arrived while a read was in flight, so one more read starts when it answers. */
	#again = false;
	#pause: ReturnType<typeof setTimeout> | null = null;
	/** Pages showing the checks; with none, signals mark the list stale instead of reading it. */
	#watchers = 0;
	#stale = false;
	/** Whether a read has started for this scope: signals before then have nothing to bring up to date. */
	#started = false;
	/** The last read was refused: signals do not read again, as each would be refused, and audited, too. */
	#refused = false;
	/** When the newest signal arrived (`performance.now()`); a read after it must begin later. */
	#signalledAt = -Infinity;

	constructor(
		private readonly client: () => ApiClient,
		private readonly publish: (view: ChecksView) => void,
		private readonly pauseMs = SIGNAL_PAUSE_MS
	) {}

	get view(): ChecksView {
		return this.#view;
	}

	/** Starts reading the scope's checks unless they are held and current, or already being read. */
	ensure(scope: Scope): void {
		const held = this.#view.checks && !this.#stale;
		if (this.#scope?.locationId === scope.locationId && (held || this.#view.loading)) return;
		if (this.#scope?.locationId !== scope.locationId) this.#reset(scope);
		this.#load();
	}

	/** A page showing the checks: they are read for `scope` and follow its signals until the returned function is called. */
	watch(scope: Scope): () => void {
		this.#watchers += 1;
		this.ensure(scope);
		let watching = true;
		return () => {
			if (watching) this.#watchers -= 1;
			watching = false;
		};
	}

	/** Reads the list again, as a page's refresh button asks. */
	refresh(): void {
		if (!this.#scope) return;
		this.#signalledAt = performance.now();
		this.#load();
	}

	handle(signal: ChangedSignal): void {
		const scope = this.#scope;
		if (!scope || !this.#started || this.#refused) return;
		const location = scope.locationId;
		const { channel } = signal;
		const relevant =
			channel === 'all' || channel === 'runs' || channel === 'checks' || channel === `node_status:${location}` || channel === `snapshot:${location}` || channel === `warehouse_sync:${location}`;
		if (!relevant) return;
		this.#signalledAt = performance.now();
		if (this.#watchers === 0) this.#stale = true;
		else this.#schedule();
	}

	#reset(scope: Scope) {
		this.#epoch += 1;
		this.#scope = scope;
		this.#inFlight = false;
		this.#again = false;
		this.#stale = false;
		this.#started = false;
		this.#refused = false;
		this.#signalledAt = -Infinity;
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
			if (this.#watchers === 0) this.#stale = true;
			else if (this.#inFlight) this.#again = true;
			else this.#load();
		}, this.pauseMs);
	}

	#load() {
		const scope = this.#scope;
		if (!scope) return;
		const epoch = this.#epoch;
		const read = ++this.#read;
		this.#inFlight = true;
		this.#stale = false;
		this.#started = true;
		this.#set({ loading: true });
		const settle = () => {
			this.#inFlight = false;
			if (!this.#again) return;
			this.#again = false;
			if (this.#watchers === 0) this.#stale = true;
			else this.#load();
		};
		this.client()
			.listChecks(scope, this.#signalledAt)
			.then(
				(checks) => {
					if (epoch !== this.#epoch || read !== this.#read) return;
					// A 304 returns the held list itself, so pages keyed on it do not redraw.
					this.#refused = false;
					const revision = this.#view.revision + 1;
					if (checks === this.#view.checks) this.#set({ error: null, loading: false, revision });
					else this.#set({ checks, error: null, loading: false, revision });
					settle();
				},
				(reason: unknown) => {
					if (epoch !== this.#epoch || read !== this.#read) return;
					// A refusal drops the held checks, so a person whose access went sees the refusal, not old data.
					this.#refused = isRefusal(reason);
					if (this.#refused) this.#set({ checks: undefined });
					this.#set({ error: message(reason), loading: false, revision: this.#view.revision + 1 });
					settle();
				}
			);
	}
}
