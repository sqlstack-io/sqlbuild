import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { stateOf } from '../node-states';
import type { ChangedSignal } from '../signals';
import { MockApiClient } from './mock-client';
import { LOCATION_SHOP } from './fixtures';

const SCOPE = { locationId: LOCATION_SHOP };

describe('mock node states', () => {
	// The mock's live runs finish nodes by the clock, the first within two seconds, so a slow test would see a state really change.
	beforeEach(() => vi.useFakeTimers({ now: Date.parse('2026-01-15T09:00:00Z'), toFake: ['Date'] }));
	afterEach(() => vi.useRealTimers());

	it('answers the node-states read with the listing states at a stable version', async () => {
		const client = new MockApiClient({ latencyMs: 0 });
		const [nodes, first] = await Promise.all([client.listCatalogNodes(SCOPE), client.listNodeStates(SCOPE)]);
		expect(first.states).toEqual(Object.fromEntries(nodes.map((n) => [n.key, stateOf(n)])));
		expect((await client.listNodeStates(SCOPE)).version).toBe(first.version);
	});

	it('announces nothing on node_status when a runs change leaves every node state as it was', async () => {
		const client = new MockApiClient({ latencyMs: 0 });
		await client.listNodeStates(SCOPE);
		const seen: ChangedSignal[] = [];
		client.changes.subscribe((signal) => seen.push(signal));
		client.changes.emit('runs');
		await client.listNodeStates(SCOPE);
		expect(seen.map((s) => s.channel)).toEqual(['runs']);
	});
});
