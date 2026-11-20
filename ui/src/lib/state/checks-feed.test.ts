import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError, type ApiClient, type ChangedChannel, type Check } from '$lib/api';
import { ChecksFeed, type ChecksView } from './checks-feed';

const SCOPE = { locationId: 'loc-orders:development' };
const OTHER = { locationId: 'loc-inventory:development' };

const check = (id: string) => ({ id, name: id }) as unknown as Check;

/** A server stand-in that counts reads; an unchanged list comes back as the same array, as a 304 does. */
function server() {
	let reads = 0;
	let held: Check[] = [check('orders_not_null')];
	let gate: Promise<void> | null = null;
	let failWith: unknown = null;
	const client = {
		listChecks: async () => {
			reads += 1;
			if (gate) await gate;
			if (failWith) throw failWith;
			return held;
		}
	} as unknown as ApiClient;
	return {
		client,
		get reads() {
			return reads;
		},
		/** A new result arrives: the next read returns a new list. */
		change() {
			held = [check('orders_not_null'), check('orders_unique')];
		},
		fail(reason: unknown) {
			failWith = reason;
		},
		/** Holds every read open until the returned function is called. */
		hold() {
			let open!: () => void;
			gate = new Promise<void>((resolve) => (open = resolve));
			return () => {
				gate = null;
				open();
			};
		}
	};
}

function feedOver(client: ApiClient) {
	const views: ChecksView[] = [];
	const feed = new ChecksFeed(() => client, (view) => views.push(view), 300);
	return { feed, views };
}

const signal = (channel: ChangedChannel) => ({ channel, at: '2026-01-02T00:00:00.000Z' });
const settle = async () => {
	for (let i = 0; i < 5; i++) await Promise.resolve();
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('ChecksFeed', () => {
	it('reads once for every page that asks, and again only when asked to refresh', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		feed.ensure(SCOPE);
		await settle();
		feed.ensure(SCOPE);
		expect(s.reads).toBe(1);
		expect(feed.view.checks?.map((c) => c.id)).toEqual(['orders_not_null']);
		feed.refresh();
		await settle();
		expect(s.reads).toBe(2);
	});

	it('reads again once for a burst of runs, checks and node status signals', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		for (const channel of ['runs', 'checks', `node_status:${SCOPE.locationId}`, 'runs'] as ChangedChannel[]) feed.handle(signal(channel));
		await vi.advanceTimersByTimeAsync(1000);
		expect(s.reads).toBe(2);
	});

	it('reads again for a new snapshot or a reconnect, and ignores other locations and topics', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		for (const channel of [`node_status:${OTHER.locationId}`, `snapshot:${OTHER.locationId}`, 'triggers', 'ticks', 'jobs'] as ChangedChannel[]) feed.handle(signal(channel));
		await vi.advanceTimersByTimeAsync(1000);
		expect(s.reads).toBe(1);
		feed.handle(signal(`snapshot:${SCOPE.locationId}`));
		await vi.advanceTimersByTimeAsync(1000);
		feed.handle(signal('all'));
		await vi.advanceTimersByTimeAsync(1000);
		expect(s.reads).toBe(3);
	});

	it('keeps the held list when a reread returns it unchanged, and publishes a changed one', async () => {
		const s = server();
		const { feed, views } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		const first = feed.view.checks;
		feed.handle(signal('runs'));
		await vi.advanceTimersByTimeAsync(1000);
		expect(feed.view.checks).toBe(first);
		expect(new Set(views.map((v) => v.checks).filter(Boolean)).size).toBe(1);
		s.change();
		feed.handle(signal('runs'));
		await vi.advanceTimersByTimeAsync(1000);
		expect(feed.view.checks?.map((c) => c.id)).toEqual(['orders_not_null', 'orders_unique']);
	});

	it('starts one more read for signals that arrive while a read is in flight', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		const open = s.hold();
		feed.handle(signal('runs'));
		await vi.advanceTimersByTimeAsync(400);
		feed.handle(signal('runs'));
		await vi.advanceTimersByTimeAsync(400);
		feed.handle(signal('checks'));
		await vi.advanceTimersByTimeAsync(400);
		expect(s.reads).toBe(2);
		open();
		await settle();
		expect(s.reads).toBe(3);
	});

	it('drops answers for a scope it has left', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		const open = s.hold();
		feed.ensure(SCOPE);
		feed.ensure(OTHER);
		open();
		await settle();
		expect(feed.view.scopeKey).toBe(OTHER.locationId);
		expect(s.reads).toBe(2);
	});

	it('drops the held list when access is refused, and keeps it on another error', async () => {
		const s = server();
		const { feed } = feedOver(s.client);
		feed.ensure(SCOPE);
		await settle();
		s.fail(new Error('network down'));
		feed.refresh();
		await settle();
		expect(feed.view.checks?.length).toBe(1);
		expect(feed.view.error).toBe('network down');
		s.fail(new ApiError('Missing permission: view', 403));
		feed.refresh();
		await settle();
		expect(feed.view.checks).toBeUndefined();
	});
});
