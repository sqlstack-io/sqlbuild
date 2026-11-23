import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { followedOutput, outputLive, pageQuery, toNodeLogEvents, toNodeOutputIndex, toNodeOutputPage, toRunOutputLines } from './http-output';

type W = components['schemas'];

const line = (number: number, text: string, extra: Partial<W['OutputLine']> = {}): W['OutputLine'] => ({ number, node_key: 'model:orders', stream: 'stdout', text, level: null, at: '2026-11-21T09:00:00Z', ...extra });

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

	it('turns the lines nodes logged with ctx.log into the run’s events, at their level', () => {
		const index: W['RunOutputIndex'] = {
			nodes: [],
			line_count: 0,
			omitted_bytes: 0,
			omitted_lines: 0,
			dropped_bytes: 0,
			marks: [],
			logs: [
				{ node_key: 'asset:score_customers', level: 'info', message: 'scored 120 customers', logged_at: '2026-11-21T09:00:00Z' },
				{ node_key: 'asset:score_customers', level: 'warning', message: '3 customers had no orders', logged_at: '2026-11-21T09:00:01Z' }
			]
		};
		expect(toNodeLogEvents(index, 'run-1').map((e) => [e.type, e.nodeKey, e.level, e.message, e.occurredAt])).toEqual([
			['node_log', 'asset:score_customers', 'info', 'scored 120 customers', '2026-11-21T09:00:00Z'],
			['node_log', 'asset:score_customers', 'warn', '3 customers had no orders', '2026-11-21T09:00:01Z']
		]);
	});

	it('tags the run’s newest lines with their node, and counts a node’s earlier lines before its first one shown', () => {
		const page: W['OutputPage'] = {
			rows: [
				{ kind: 'line', line: line(41, 'Run inputs verified.', { node_key: 'run', stream: 'stderr' }) },
				{ kind: 'line', line: line(42, 'batch 40 done') },
				{ kind: 'line', line: line(43, 'loaded customers', { node_key: 'model:customers' }) }
			],
			line_count: 43,
			from: 41,
			to: 44,
			next: null,
			previous: 41
		};
		const index: W['RunOutputIndex'] = {
			nodes: [
				{ node_key: 'model:customers', line_count: 1, stdout_lines: 1, stderr_lines: 0, text_bytes: 17 },
				{ node_key: 'model:orders', line_count: 40, stdout_lines: 40, stderr_lines: 0, text_bytes: 600 },
				{ node_key: 'run', line_count: 2, stdout_lines: 1, stderr_lines: 1, text_bytes: 40 }
			],
			line_count: 43,
			omitted_bytes: 0,
			omitted_lines: 0,
			dropped_bytes: 0,
			marks: [],
			logs: []
		};
		expect(toRunOutputLines(page, index).map((l) => [l.nodeKey, l.stream, l.text, l.omitted ?? 0])).toEqual([
			[null, 'stderr', 'Run inputs verified.', 0],
			['model:orders', 'stdout', '', 39],
			['model:orders', 'stdout', 'batch 40 done', 0],
			['model:customers', 'stdout', 'loaded customers', 0]
		]);
	});

	it('reads a follow as the newest lines with markers, or as new lines alone, and carries on from the last line', () => {
		const index: W['RunOutputIndex'] = {
			nodes: [{ node_key: 'model:orders', line_count: 50, stdout_lines: 50, stderr_lines: 0, text_bytes: 600 }],
			line_count: 50,
			omitted_bytes: 0,
			omitted_lines: 0,
			dropped_bytes: 0,
			marks: [],
			logs: [{ node_key: 'model:orders', level: 'info', message: 'halfway', logged_at: '2026-11-21T09:00:00Z' }]
		};
		const page: W['OutputPage'] = { rows: [{ kind: 'line', line: line(50, 'done') }], line_count: 50, from: 50, to: 51, next: null, previous: 50 };
		const newest = followedOutput(page, index, 'r1', true);
		const since = followedOutput(page, index, 'r1', false);
		expect([newest.lines.map((l) => l.omitted ?? 0), since.lines.map((l) => l.omitted ?? 0), newest.last, newest.logs.length, since.replaced]).toEqual([[49, 0], [0], 50, 1, false]);
	});
});
