import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HttpChangeFeed } from './http-client';
import type { ChangedSignal, ConnectionState } from './signals';

/** A stand-in `EventSource` the test opens, fails and feeds by hand. */
class FakeSource {
	static readonly CLOSED = 2;
	static opened: FakeSource[] = [];
	readyState = 0;
	closed = false;
	onopen: (() => void) | null = null;
	onerror: (() => void) | null = null;
	listeners = new Map<string, (event: MessageEvent<string>) => void>();

	constructor(readonly url: string) {
		FakeSource.opened.push(this);
	}

	addEventListener(name: string, listener: (event: MessageEvent<string>) => void) {
		this.listeners.set(name, listener);
	}

	close() {
		this.closed = true;
	}

	open() {
		this.readyState = 1;
		this.onopen?.();
	}

	/** The server refused or the connection failed for good, so the browser stopped retrying. */
	fail() {
		this.readyState = FakeSource.CLOSED;
		this.onerror?.();
	}
}

const ORDERS = { locationId: '0191f0aa-orders:development' };
const INVENTORY = { locationId: '0191f0bb-inventory:development' };

function feed(stillListed: (scope: { locationId: string }) => Promise<boolean>) {
	const changes = new HttpChangeFeed('', stillListed);
	const states: ConnectionState[] = [];
	changes.onConnectionState((state) => states.push(state));
	return { changes, states };
}

const latest = () => FakeSource.opened.at(-1)!;

beforeEach(() => {
	FakeSource.opened = [];
	vi.useFakeTimers();
	vi.stubGlobal('EventSource', FakeSource);
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('HttpChangeFeed', () => {
	it('opens the location feed and is live once the stream opens', () => {
		const { changes, states } = feed(async () => true);
		changes.follow(ORDERS);
		expect(latest().url).toBe('/api/control/locations/0191f0aa-orders/targets/development/changes');
		latest().open();
		expect(states).toEqual(['connecting', 'connecting', 'live']);
	});

	it('leaves transient drops to the browser without its own retry', () => {
		const stillListed = vi.fn(async () => true);
		const { changes, states } = feed(stillListed);
		changes.follow(ORDERS);
		latest().open();
		latest().onerror?.();
		vi.advanceTimersByTime(60_000);
		expect(states.at(-1)).toBe('reconnecting');
		expect(stillListed).not.toHaveBeenCalled();
		expect(FakeSource.opened).toHaveLength(1);
	});

	it('re-reads the bootstrap before each retry and backs off 1, 2, 4 then 8 seconds', async () => {
		const stillListed = vi.fn(async () => true);
		const { changes, states } = feed(stillListed);
		changes.follow(ORDERS);
		for (const delay of [1000, 2000, 4000, 8000]) {
			const failed = latest();
			failed.fail();
			await vi.advanceTimersByTimeAsync(delay - 1);
			expect(stillListed).toHaveBeenCalledTimes(FakeSource.opened.length - 1);
			expect(latest()).toBe(failed);
			await vi.advanceTimersByTimeAsync(1);
			expect(stillListed).toHaveBeenLastCalledWith(ORDERS);
			expect(latest()).not.toBe(failed);
			expect(failed.closed).toBe(true);
		}
		expect(states.filter((state) => state !== 'connecting')).toEqual([
			'reconnecting',
			'reconnecting',
			'reconnecting',
			'offline'
		]);
	});

	it('caps the backoff at 30 seconds', async () => {
		const stillListed = vi.fn(async () => true);
		const { changes } = feed(stillListed);
		changes.follow(ORDERS);
		for (let attempt = 0; attempt < 6; attempt += 1) {
			latest().fail();
			await vi.advanceTimersByTimeAsync(30_000);
		}
		const calls = stillListed.mock.calls.length;
		latest().fail();
		await vi.advanceTimersByTimeAsync(29_999);
		expect(stillListed).toHaveBeenCalledTimes(calls);
		await vi.advanceTimersByTimeAsync(1);
		expect(stillListed).toHaveBeenCalledTimes(calls + 1);
	});

	it('resets the backoff once a retry opens', async () => {
		const stillListed = vi.fn(async () => true);
		const { changes } = feed(stillListed);
		changes.follow(ORDERS);
		latest().fail();
		await vi.advanceTimersByTimeAsync(1000);
		latest().fail();
		await vi.advanceTimersByTimeAsync(2000);
		latest().open();
		const reopened = latest();
		reopened.fail();
		await vi.advanceTimersByTimeAsync(1000);
		expect(latest()).not.toBe(reopened);
	});

	it('goes offline and stops when the bootstrap no longer lists the location', async () => {
		const { changes, states } = feed(async () => false);
		changes.follow(ORDERS);
		latest().fail();
		await vi.advanceTimersByTimeAsync(60_000);
		expect(FakeSource.opened).toHaveLength(1);
		expect(states.at(-1)).toBe('offline');
	});

	it('keeps retrying when the bootstrap read itself fails', async () => {
		const { changes } = feed(() => Promise.reject(new Error('network down')));
		changes.follow(ORDERS);
		latest().fail();
		await vi.advanceTimersByTimeAsync(1000);
		expect(FakeSource.opened).toHaveLength(2);
	});

	it('drops a pending retry when the person follows another location', async () => {
		const stillListed = vi.fn(async () => true);
		const { changes } = feed(stillListed);
		changes.follow(ORDERS);
		const orders = latest();
		orders.fail();
		changes.follow(INVENTORY);
		await vi.advanceTimersByTimeAsync(60_000);
		expect(stillListed).not.toHaveBeenCalled();
		expect(FakeSource.opened.map((source) => source.url)).toEqual([
			'/api/control/locations/0191f0aa-orders/targets/development/changes',
			'/api/control/locations/0191f0bb-inventory/targets/development/changes'
		]);
	});

	it('closes the stream and is offline when following nothing', () => {
		const { changes, states } = feed(async () => true);
		changes.follow(ORDERS);
		const orders = latest();
		changes.follow(null);
		expect(orders.closed).toBe(true);
		expect(states.at(-1)).toBe('offline');
	});

	it('maps server topics onto the followed location', () => {
		const { changes } = feed(async () => true);
		const signals: ChangedSignal['channel'][] = [];
		changes.subscribe((signal) => signals.push(signal.channel));
		changes.follow(ORDERS);
		for (const topic of ['snapshot:0191f0aa-orders', 'node_status:0191f0aa-orders', 'runs', 'audit']) {
			latest().listeners.get('changed')!(new MessageEvent('changed', { data: JSON.stringify({ topic }) }));
		}
		expect(signals).toEqual([
			`snapshot:${ORDERS.locationId}`,
			`node_status:${ORDERS.locationId}`,
			'runs',
			'all'
		]);
	});
});
