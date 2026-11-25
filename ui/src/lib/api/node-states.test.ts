import { describe, expect, it } from 'vitest';
import { applyNodeStates, deltaAction, stateOf, withBehind } from './node-states';
import type { CatalogNode, NodeState } from './types/build';

function node(key: string, upstream: string[] = [], builtAt: string | null = '2026-01-01T00:00:00.000Z', kind: CatalogNode['kind'] = 'table'): CatalogNode {
	return {
		key,
		kind,
		materialization: kind,
		group: null,
		tags: [],
		description: '',
		path: '',
		upstream,
		relation: null,
		columnCount: 0,
		lastBuild: builtAt ? { fingerprintRunId: `r-${key}`, runId: null, jobKey: null, builtAt, fingerprint: 'f' } : null,
		lastOutcome: builtAt ? 'success' : null,
		stale: [],
		sourceFreshness: null,
		checks: { total: 0, passing: 0, warning: 0, failing: 0 },
		status: builtAt ? 'current' : 'not_built',
		statusAt: builtAt
	};
}

const built = (key: string, builtAt: string, extra: Partial<NodeState> = {}): NodeState => ({
	lastBuild: { fingerprintRunId: `r2-${key}`, runId: null, jobKey: null, builtAt, fingerprint: 'g' },
	lastOutcome: 'success',
	definitionChanged: false,
	checks: { total: 1, passing: 1, warning: 0, failing: 0 },
	status: 'current',
	statusAt: builtAt,
	...extra
});

describe('applyNodeStates', () => {
	const listing = withBehind([node('model:orders'), node('model:order_items', ['model:orders']), node('model:customers')]);

	it('replaces the state of named nodes and leaves the rest as they were', () => {
		const applied = applyNodeStates(listing, { 'model:customers': built('model:customers', '2026-01-02T00:00:00.000Z') });
		expect(applied[2].lastBuild?.fingerprintRunId).toBe('r2-model:customers');
		expect(applied[2].checks).toEqual({ total: 1, passing: 1, warning: 0, failing: 0 });
		expect(applied[0]).toBe(listing[0]);
		expect(applied[1]).toBe(listing[1]);
	});

	it('marks downstream nodes behind when an upstream build is newer', () => {
		const applied = applyNodeStates(listing, { 'model:orders': built('model:orders', '2026-01-02T00:00:00.000Z') });
		expect(applied[1].stale).toEqual([expect.objectContaining({ reason: 'behind', upstream: ['model:orders'] })]);
		expect(applied[2]).toBe(listing[2]);
	});

	it('adds and clears the changed reason with the definition', () => {
		const changed = applyNodeStates(listing, { 'model:customers': built('model:customers', '2026-01-01T00:00:00.000Z', { definitionChanged: true }) });
		expect(changed[2].stale?.map((r) => r.reason)).toEqual(['changed']);
		expect(stateOf(changed[2]).definitionChanged).toBe(true);
		const cleared = applyNodeStates(changed, { 'model:customers': built('model:customers', '2026-01-03T00:00:00.000Z') });
		expect(cleared[2].stale).toEqual([]);
	});

	it('ignores states of nodes the listing does not hold', () => {
		expect(applyNodeStates(listing, { 'model:unknown': built('model:unknown', '2026-01-02T00:00:00.000Z') })).toBe(listing);
	});

	it('keeps a node state round-trip unchanged', () => {
		const states = Object.fromEntries(listing.map((n) => [n.key, stateOf(n)]));
		expect(applyNodeStates(listing, states)).toEqual(listing);
	});
});

describe('deltaAction', () => {
	const delta = (previousVersion: number, version: number) => ({ previousVersion, version, states: {} });

	it('applies the next version', () => expect(deltaAction(4, delta(4, 5))).toBe('apply'));
	it('reads again after a gap', () => expect(deltaAction(4, delta(5, 6))).toBe('refetch'));
	it('ignores a version already held', () => expect(deltaAction(6, delta(4, 5))).toBe('ignore'));
	it('reads again before any version is held', () => expect(deltaAction(null, delta(4, 5))).toBe('refetch'));
	it('reads again on a full change', () => expect(deltaAction(4, { previousVersion: 4, version: 5, full: true })).toBe('refetch'));
});
