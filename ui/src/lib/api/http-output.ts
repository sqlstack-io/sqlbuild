import type { components } from '$control/schema.d.ts';
import type { NodeOutputIndex, NodeOutputLevel, NodeOutputLine, NodeOutputPage, NodeOutputRow, RunEvent, RunNode, RunOutputLine } from './types/operate';

type Wire = components['schemas'];

/** Where a page of a node's output starts, which way it reads, and how. */
export type NodeOutputAt = { from: number; rows: number; direction: 'forward' | 'backward'; collapse: boolean; stream?: 'all' | 'stdout' | 'stderr'; around?: number };

/** The query of a page of a node's output: forwards from `from`, or backwards from just before it. */
export function pageQuery(nodeKey: string, at: NodeOutputAt) {
	return {
		node: nodeKey,
		...(at.stream && at.stream !== 'all' ? { stream: at.stream } : {}),
		...(at.direction === 'forward' ? { from: Math.max(1, at.from) } : { before: Math.max(1, at.from) }),
		limit: at.rows,
		collapse: at.collapse,
		...(at.around == null ? {} : { around: at.around })
	};
}

function levelOf(level: Wire['OutputLevel'] | null | undefined): NodeOutputLevel {
	return level === 'error' ? 'error' : level === 'warning' ? 'warn' : 'info';
}

export function toNodeOutputLine(wire: Wire['OutputLine']): NodeOutputLine {
	return { n: wire.number, stream: wire.stream, text: wire.text, level: levelOf(wire.level) };
}

function toRow(wire: Wire['OutputRow']): NodeOutputRow {
	return wire.kind === 'line' ? { kind: 'line', line: toNodeOutputLine(wire.line) } : { kind: 'similar', from: wire.from, count: wire.count, sample: toNodeOutputLine(wire.sample) };
}

export function toNodeOutputPage(wire: Wire['OutputPage']): NodeOutputPage {
	return { total: wire.line_count, from: wire.from, to: wire.to, rows: wire.rows.map(toRow) };
}

/** A node's index entry and marks; `live` while the node, or for lines outside any node the run, is still running. */
export function toNodeOutputIndex(wire: Wire['RunOutputIndex'], nodeKey: string, live: boolean): NodeOutputIndex | null {
	const node = wire.nodes.find((n) => n.node_key === nodeKey);
	if (!node) return null;
	return {
		total: node.line_count,
		stdout: node.stdout_lines,
		stderr: node.stderr_lines,
		bytes: node.text_bytes,
		marks: wire.marks.map((m) => ({ line: m.line, level: m.level === 'error' ? 'error' : 'warn', text: m.text })),
		live
	};
}

/** Whether a node's output may still grow: the node is running, or, for lines outside any node, any node still is. */
export function outputLive(nodes: readonly Pick<RunNode, 'nodeKey' | 'status'>[], nodeKey: string): boolean {
	const running = (n: Pick<RunNode, 'status'>) => n.status === 'running' || n.status === 'pending';
	return nodeKey === 'run' ? nodes.some(running) : nodes.some((n) => n.nodeKey === nodeKey && running(n));
}

/** The lines a run's nodes logged with `ctx.log`, as events of the run; they come with its output, which needs `view.evidence`. */
export function toNodeLogEvents(wire: Wire['RunOutputIndex'], runId: string): RunEvent[] {
	return wire.logs.map((log, i) => ({
		id: `${runId}-log-${i + 1}`,
		storageId: 0,
		runId,
		producer: 'sqb',
		type: 'node_log',
		occurredAt: log.logged_at,
		level: log.level === 'error' ? 'error' : log.level === 'warning' ? 'warn' : 'info',
		invocationId: null,
		nodeKey: log.node_key,
		statementId: null,
		attempt: null,
		message: log.message
	}));
}

/** The run's newest printed lines the log shows; earlier ones are in each node's output viewer. */
export const LIVE_OUTPUT_ROWS = 2000;

/**
 * The run's newest printed lines, each with its node (none for lines printed outside any node), and,
 * before a node's first line shown, how many of its earlier lines are only in its viewer.
 */
export function toRunOutputLines(page: Wire['OutputPage'], index: Wire['RunOutputIndex']): RunOutputLine[] {
	const lines: Wire['OutputLine'][] = page.rows.flatMap((row) => (row.kind === 'line' ? [row.line] : []));
	const shown = new Map<string, number>();
	for (const line of lines) shown.set(line.node_key, (shown.get(line.node_key) ?? 0) + 1);
	const totals = new Map(index.nodes.map((n) => [n.node_key, n.line_count]));
	const marked = new Set<string>();
	return lines.flatMap((line): RunOutputLine[] => {
		const nodeKey = line.node_key === 'run' ? null : line.node_key;
		const printed: RunOutputLine = { invocationId: '', stream: line.stream, at: line.at, text: line.text, error: false, nodeKey };
		const earlier = (totals.get(line.node_key) ?? 0) - (shown.get(line.node_key) ?? 0);
		if (!nodeKey || earlier <= 0 || marked.has(line.node_key)) return [printed];
		marked.add(line.node_key);
		return [{ ...printed, text: '', omitted: earlier }, printed];
	});
}
