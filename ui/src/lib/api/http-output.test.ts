import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { outputLive, pageQuery, toNodeOutputIndex, toNodeOutputPage } from './http-output';

type W = components['schemas'];

const line = (number: number, text: string, extra: Partial<W['OutputLine']> = {}): W['OutputLine'] => ({ number, node_key: 'model:orders', stream: 'stdout', text, level: null, ...extra });

describe('node output mapping', () => {
	it('reads forwards from a line and backwards from just before one, leaving out the whole-output stream', () => {
		expect(pageQuery('model:orders', { from: 40, rows: 200, direction: 'forward', collapse: true, stream: 'all' })).toEqual({ node: 'model:orders', from: 40, limit: 200, collapse: true });
		expect(pageQuery('model:orders', { from: 40, rows: 200, direction: 'backward', collapse: false, stream: 'stderr', around: 38 })).toEqual({
			node: 'model:orders',
			stream: 'stderr',
			before: 40,
			limit: 200,
			collapse: false,
			around: 38
		});
	});

	it('keeps line numbers and folded runs, and reads warning as warn', () => {
		const page = toNodeOutputPage({
			rows: [
				{ kind: 'similar', from: 1, count: 40, sample: line(1, 'batch 1 rows=3') },
				{ kind: 'line', line: line(41, 'WARNING: slow batch', { stream: 'stderr', level: 'warning' }) },
				{ kind: 'line', line: line(42, 'Traceback (most recent call last):', { stream: 'stderr', level: 'error' }) }
			],
			line_count: 90,
			from: 1,
			to: 43,
			next: 43,
			previous: null
		});
		expect(page).toEqual({
			total: 90,
			from: 1,
			to: 43,
			rows: [
				{ kind: 'similar', from: 1, count: 40, sample: { n: 1, stream: 'stdout', text: 'batch 1 rows=3', level: 'info' } },
				{ kind: 'line', line: { n: 41, stream: 'stderr', text: 'WARNING: slow batch', level: 'warn' } },
				{ kind: 'line', line: { n: 42, stream: 'stderr', text: 'Traceback (most recent call last):', level: 'error' } }
			]
		});
	});

	it("takes the node's counts and marks from the index, and none for a node that printed nothing", () => {
		const index: W['RunOutputIndex'] = {
			nodes: [{ node_key: 'model:orders', line_count: 12, stdout_lines: 10, stderr_lines: 2, text_bytes: 480 }],
			line_count: 13,
			omitted_bytes: 0,
			omitted_lines: 0,
			dropped_bytes: 0,
			logs: [],
			marks: [{ line: 11, stream: 'stderr', level: 'warning', text: 'WARNING: slow batch' }]
		};
		expect(toNodeOutputIndex(index, 'model:orders', true)).toEqual({ total: 12, stdout: 10, stderr: 2, bytes: 480, marks: [{ line: 11, level: 'warn', text: 'WARNING: slow batch' }], live: true });
		expect(toNodeOutputIndex(index, 'model:customers', false)).toBeNull();
	});

	it('counts output as live while its node runs, and lines outside any node while any node does', () => {
		const nodes = [
			{ nodeKey: 'model:orders', status: 'running' as const },
			{ nodeKey: 'model:customers', status: 'success' as const }
		];
		expect([outputLive(nodes, 'model:orders'), outputLive(nodes, 'model:customers'), outputLive(nodes, 'run'), outputLive(nodes.slice(1), 'run')]).toEqual([true, false, true, false]);
	});
});
