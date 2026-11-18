import type { ApiClient, CatalogNode, ChangedBuild, ChangedSignal, NodeState, NodeStatusDelta, Scope } from '$lib/api';
import { isRefusal, isUnsupported } from '$lib/api';
import { applyNodeStates, deltaAction } from '$lib/api/node-states';

/** How long a burst of signals may last before one read starts. */
const SIGNAL_PAUSE_MS = 300;

/** `changedBuild` comes with the node states; null until they are read, or when the server has none. */
export type CatalogView = { nodes: CatalogNode[] | undefined; error: string | null; loading: boolean; changedBuild: ChangedBuild | null };

const message = (reason: unknown) => (reason instanceof Error ? reason.message : String(reason));

/**
 * The selected location's catalog nodes, shared by every page. The listing is read once per
 * snapshot; node states are read once with their version, and `node_status` changes are applied
 * to the held nodes. A change that skips a version, a `full` change, or a reconnect reads the
 * states again; a new snapshot reads the listing again, which the server answers with a 304 when
 * nothing changed. `runs` and `checks` signals read nothing: their effect on nodes arrives as a
 * `node_status` change.
 *
 * The listing carries no states, so the held states stay applied while both are read again. Against
 * a recording without the node-states read, whose listing carries the states, or signals that carry
 * no states, a `node_status` signal reads the listing again.
 */
export class CatalogFeed {
	#view: CatalogView = { nodes: undefined, error: null, loading: false, changedBuild: null };
	#scope: Scope | null = null;
	/** Bumped by a scope change, so reads for the old scope are dropped when they answer. */
	#epoch = 0;
	#listingRead = 0;
	#statesRead = 0;
	/** The listing as read, before states are applied. */
	#base: CatalogNode[] | null = null;
	/** States read or announced since the listing's states read began, applied over it. */
	#overlay: Record<string, NodeState> = {};
	#version: number | null = null;
	#snapshotId: string | null = null;
	#statesUnsupported = false;
	#statesInFlight = false;
	/** Changes that arrived while the states were being read, applied once they answer. */
	#queued: NodeStatusDelta[] = [];
	#pause: ReturnType<typeof setTimeout> | null = null;
	#want = { listing: false, states: false };

	constructor(
		private readonly client: () => ApiClient,
		private readonly publish: (view: CatalogView) => void,
		private readonly pauseMs = SIGNAL_PAUSE_MS
	) {}

	get view(): CatalogView {
		return this.#view;
	}

	/** The status version held, or null before the states are read or when the server has no such read. */
	get version(): number | null {
		return this.#version;
	}

	/** Starts reading the scope's nodes unless they are held or already being read. */
	ensure(scope: Scope): void {
		if (this.#scope?.locationId === scope.locationId && (this.#view.nodes || this.#view.loading)) return;
		if (this.#scope?.locationId !== scope.locationId) this.#reset(scope);
		this.#load(true, true);
	}

	/** Reads the listing and states again, as the page's refresh button asks. */
	refresh(): void {
		if (this.#scope) this.#load(true, true);
	}

	handle(signal: ChangedSignal): void {
		const scope = this.#scope;
		if (!scope || (!this.#base && !this.#view.loading)) return;
		// Sent on connect and after a reconnect, when changes may have been missed, including a new snapshot.
		if (signal.channel === 'all' || signal.channel === `snapshot:${scope.locationId}`) return this.#schedule(true, true);
		if (signal.channel !== `node_status:${scope.locationId}`) return;
		if (!signal.nodeStatus || this.#statesUnsupported) return this.#schedule(true, false);
		this.#changed(signal.nodeStatus);
	}

	#reset(scope: Scope) {
		this.#epoch += 1;
		this.#scope = scope;
		this.#base = null;
		this.#overlay = {};
		this.#version = null;
		this.#snapshotId = null;
		this.#statesUnsupported = false;
		this.#statesInFlight = false;
		this.#queued = [];
		if (this.#pause) clearTimeout(this.#pause);
		this.#pause = null;
		this.#want = { listing: false, states: false };
		this.#set({ nodes: undefined, error: null, loading: false, changedBuild: null });
	}

	#set(view: Partial<CatalogView>) {
		this.#view = { ...this.#view, ...view };
		this.publish(this.#view);
	}

	#schedule(listing: boolean, states: boolean) {
		this.#want = { listing: this.#want.listing || listing, states: this.#want.states || states };
		if (this.#pause) return;
		this.#pause = setTimeout(() => {
			this.#pause = null;
			const { listing, states } = this.#want;
			this.#want = { listing: false, states: false };
			this.#load(listing, states);
		}, this.pauseMs);
	}

	#load(listing: boolean, states: boolean) {
		const scope = this.#scope;
		if (!scope) return;
		const epoch = this.#epoch;
		if (states && !this.#statesUnsupported && this.client().supports('listNodeStates')) {
			this.#readStates(scope, epoch);
		} else if (states) this.#statesUnsupported = true;
		if (listing) this.#readListing(scope, epoch);
	}

	#readListing(scope: Scope, epoch: number) {
		const read = ++this.#listingRead;
		this.#set({ loading: true });
		this.client()
			.listCatalogNodes(scope)
			.then(
				(nodes) => {
					if (epoch !== this.#epoch || read !== this.#listingRead) return;
					this.#base = nodes;
					this.#set({ nodes: applyNodeStates(nodes, this.#overlay), error: null, loading: false });
				},
				(reason: unknown) => {
					if (epoch !== this.#epoch || read !== this.#listingRead) return;
					// A refusal drops the held nodes, so a person whose access went sees the refusal, not old data.
					if (isRefusal(reason)) {
						this.#base = null;
						this.#set({ nodes: undefined });
					}
					this.#set({ error: message(reason), loading: false });
				}
			);
	}

	#readStates(scope: Scope, epoch: number) {
		const read = ++this.#statesRead;
		this.#statesInFlight = true;
		this.client()
			.listNodeStates(scope)
			.then(
				(answer) => {
					if (epoch !== this.#epoch || read !== this.#statesRead) return;
					this.#statesInFlight = false;
					const snapshotChanged = this.#snapshotId !== null && this.#snapshotId !== answer.snapshotId;
					this.#snapshotId = answer.snapshotId;
					this.#version = answer.version;
					this.#overlay = { ...answer.states };
					this.#set({ changedBuild: answer.changedBuild ?? null });
					if (this.#base) this.#set({ nodes: applyNodeStates(this.#base, answer.states) });
					// A publication the snapshot signal has not announced yet: the listing is out of date too.
					if (snapshotChanged && this.#listingRead > 0) this.#schedule(true, false);
					this.#drain();
				},
				(reason: unknown) => {
					if (epoch !== this.#epoch || read !== this.#statesRead) return;
					this.#statesInFlight = false;
					this.#version = null;
					this.#queued = [];
					// The listing carries the states, so from now on a change reads it again.
					if (isUnsupported(reason)) this.#statesUnsupported = true;
				}
			);
	}

	#drain() {
		const queued = this.#queued.sort((a, b) => a.version - b.version);
		this.#queued = [];
		for (const [index, delta] of queued.entries()) {
			if (!this.#changed(delta)) {
				// A gap: the states are being read again, so the rest wait for that answer.
				this.#queued.push(...queued.slice(index + 1));
				return;
			}
		}
	}

	/** Applies or ignores a change; false when it started a states read instead. */
	#changed(delta: NodeStatusDelta): boolean {
		if (this.#statesInFlight) {
			this.#queued.push(delta);
			return false;
		}
		const action = deltaAction(this.#version, delta);
		if (action === 'ignore') return true;
		const scope = this.#scope;
		if (action === 'refetch' || delta.full) {
			if (scope) this.#readStates(scope, this.#epoch);
			return false;
		}
		this.#version = delta.version;
		this.#overlay = { ...this.#overlay, ...delta.states };
		const nodes = this.#view.nodes;
		if (nodes) this.#set({ nodes: applyNodeStates(nodes, delta.states) });
		return true;
	}
}
