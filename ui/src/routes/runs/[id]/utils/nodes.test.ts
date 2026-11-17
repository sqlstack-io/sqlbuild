import { describe, expect, it } from 'vitest';
import type { RunNode } from '$lib/api';
import { groupNodes, notRunReason } from './nodes';

const node = (nodeKey: string, status: RunNode['status']) => ({ nodeKey, status, runId: 'r' }) as RunNode;
const nodes = [node('seed:a', 'running'), node('seed:b', 'success'), node('seed:c', 'pending'), node('seed:d', 'pending'), node('seed:e', 'not_run')];

describe('a run’s nodes by outcome', () => {
	it('reads nodes that haven’t started as waiting while the run goes on', () => {
		expect(groupNodes(nodes, true).map((g) => [g.label, g.nodes.length])).toEqual([
			['Running', 1],
			['Waiting', 2],
			['Succeeded', 1],
			['Not run', 1]
		]);
		expect(notRunReason(nodes[2], true)).toBe('Waiting');
	});

	it('reads them as not run once the run has finished', () => {
		expect(groupNodes(nodes, false).map((g) => [g.label, g.nodes.length])).toEqual([
			['Running', 1],
			['Succeeded', 1],
			['Not run', 3]
		]);
		expect(notRunReason(nodes[2], false)).toBe('Not run');
	});
});
