import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError, type ApiClient, type CatalogNode, type ChangedChannel, type NodeState, type NodeStates, type NodeStatusDelta } from '$lib/api';
import { CatalogFeed, type CatalogView } from './catalog-feed';

const SCOPE = { locationId: 'loc-orders:development' };
const OTHER = { locationId: 'loc-inventory:development' };

function node(key: string, upstream: string[] = []): CatalogNode {
	return {
		key,
		kind: 'table',
		materialization: 'table',
		group: null,
		tags: [],
		description: '',
		path: '',
		upstream,
		relation: null,
		columnCount: 0,
		lastBuild: { fingerprintRunId: 'r1', runId: null, jobKey: null, builtAt: '2026-01-01T00:00:00.000Z', fingerprint: 'f' },
		lastOutcome: 'success',
		stale: [],
		sourceFreshness: null,
		checks: null
	};
}

const failed = (runId: string): NodeState => ({
	lastBuild: { fingerprintRunId: runId, runId: null, jobKey: null, builtAt: '2026-01-02T00:00:00.000Z', fingerprint: 'g' },
	lastOutcome: 'failed',
	definitionChanged: false,
	checks: null
});

/** A server stand-in that counts reads and answers with what the test sets. */
function server({ states = true }: { states?: boolean | 'missing' } = {}) {
	const reads = { listing: 0, states: 0 };
	let snapshotId = 's1';
	let version = 1;
	let held: Record<string, NodeState> = {};
	const client = {
		supports: (method: string) => method !== 'listNodeStates' || states !== false,
		listCatalogNodes: async () => {
			reads.listing += 1;
			return [node('model:orders'), node('model:order_items', ['model:orders'])];
		},
		listNodeStates: async (): Promise<NodeStates> => {
			reads.states += 1;
			if (states === 'missing') throw new ApiError('not found', 404);
			return { snapshotId, version, states: held };
		}
	} as unknown as ApiClient;
	return {
		client,
		reads,
		set(next: { version?: number; states?: Record<string, NodeState>; snapshotId?: string }) {
			version = next.version ?? version;
			held = next.states ?? held;
			snapshotId = next.snapshotId ?? snapshotId;
		},
		/** Holds the next states read open until the returned function is called. */
		hold() {
			const original = client.listNodeStates;
			let open!: () => void;
			const gate = new Promise<void>((resolve) => (open = resolve));
			client.listNodeStates = async (scope) => {
				await gate;
				client.listNodeStates = original;
				return original(scope);
			};
			return open;
		}
	};
}

function feedOver(client: ApiClient) {
	const views: CatalogView[] = [];
	const feed = new CatalogFeed(() => client, (view) => views.push(view), 300);
	return { feed, views };
}

const signal = (channel: ChangedChannel, nodeStatus?: NodeStatusDelta) => ({ channel, at: '2026-01-02T00:00:00.000Z', ...(nodeStatus ? { nodeStatus } : {}) });
const settle = async () => {
	for (let i = 0; i < 5; i++) await Promise.resolve();
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('CatalogFeed', () => {
	it('reads the listing and states once, and again only for the same scope when asked', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		feed.ensure(SCOPE);
		await settle();
		feed.ensure(SCOPE);
		expect(s.reads).toEqual({ listing: 1, states: 1 });
		expect(feed.view.nodes?.map((n) => n.key)).toEqual(['model:orders', 'model:order_items']);
		expect(feed.version).toBe(1);
	});

	it('applies a node_status change in place, without reading anything', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 1, version: 2, states: { 'model:orders': failed('r2') } }));
		await vi.advanceTimersByTimeAsync(1000);
		expect(s.reads).toEqual({ listing: 1, states: 1 });
		expect(feed.version).toBe(2);
		expect(feed.view.nodes?.[0].lastOutcome).toBe('failed');
		// The newer upstream build puts its downstream behind.
		expect(feed.view.nodes?.[1].stale?.map((r) => r.reason)).toEqual(['behind']);
	});

	it('reads the states again after a version gap or a full change, and applies the answer', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		s.set({ version: 4, states: { 'model:orders': failed('r4') } });
		feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 3, version: 4, states: {} }));
		await settle();
		expect(s.reads).toEqual({ listing: 1, states: 2 });
		expect(feed.version).toBe(4);
		expect(feed.view.nodes?.[0].lastOutcome).toBe('failed');
		feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 4, version: 5, full: true }));
		await settle();
		expect(s.reads).toEqual({ listing: 1, states: 3 });
	});

	it('ignores runs and checks signals, and changes already covered', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		feed.handle(signal('runs'));
		feed.handle(signal('checks'));
		feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 0, version: 1, states: { 'model:orders': failed('old') } }));
		feed.handle(signal(`node_status:${OTHER.locationId}`, { previousVersion: 1, version: 2, states: { 'model:orders': failed('other') } }));
		await vi.advanceTimersByTimeAsync(1000);
		expect(s.reads).toEqual({ listing: 1, states: 1 });
		expect(feed.view.nodes?.[0].lastOutcome).toBe('success');
	});

	it('reads the listing and states again on a new snapshot or a reconnect, coalescing a burst', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		feed.handle(signal(`snapshot:${SCOPE.locationId}`));
		feed.handle(signal('all'));
		await vi.advanceTimersByTimeAsync(400);
		expect(s.reads).toEqual({ listing: 2, states: 2 });
	});

	it('holds changes that arrive while the states are read, and applies the ones after the answer', async () => {
		const s = server();
		const open = s.hold();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		s.set({ version: 3 });
		feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 2, version: 3, states: { 'model:orders': failed('covered') } }));
		feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 3, version: 4, states: { 'model:order_items': failed('r4') } }));
		open();
		await settle();
		await settle();
		expect(s.reads.states).toBe(1);
		expect(feed.version).toBe(4);
		expect(feed.view.nodes?.map((n) => n.lastOutcome)).toEqual(['success', 'failed']);
	});

	it('reads the listing again when the states belong to a newer snapshot', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		s.set({ snapshotId: 's2', version: 7 });
		feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 5, version: 6, states: {} }));
		await settle();
		await vi.advanceTimersByTimeAsync(400);
		expect(s.reads).toEqual({ listing: 2, states: 2 });
	});

	it('reads the listing on node_status when the server has no node-states read or sends no states', async () => {
		const missing = server({ states: 'missing' });
		const a = feedOver(missing.client);
		a.feed.ensure(SCOPE);
		await settle();
		a.feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 1, version: 2, states: {} }));
		await vi.advanceTimersByTimeAsync(400);
		expect(missing.reads).toEqual({ listing: 2, states: 1 });

		const older = server();
		const b = feedOver(older.client);
		b.feed.ensure(SCOPE);
		await settle();
		b.feed.handle(signal(`node_status:${SCOPE.locationId}`));
		await vi.advanceTimersByTimeAsync(400);
		expect(older.reads).toEqual({ listing: 2, states: 1 });
	});

	it('starts again for another scope and drops answers for the old one', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		feed.ensure(OTHER);
		await settle();
		expect(s.reads).toEqual({ listing: 2, states: 2 });
		expect(feed.view.nodes).toHaveLength(2);
		feed.handle(signal(`node_status:${SCOPE.locationId}`, { previousVersion: 1, version: 2, states: { 'model:orders': failed('r2') } }));
		expect(feed.view.nodes?.[0].lastOutcome).toBe('success');
	});
});
