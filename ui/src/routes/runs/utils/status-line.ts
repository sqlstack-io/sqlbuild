// The plain second line under a run's status: what is happening, in words.

import type { RunRow } from '$lib/api';
import { formatAgo, formatStamp } from '$lib/format/time';
import { formatSeconds, waitTechnical, waitText } from '$lib/operate/labels';
import type { StatusLine } from '../types';

const plural = (n: number, noun: string) => `${n} ${noun}${n === 1 ? '' : 's'}`;

function outcomeCounts(row: RunRow): string {
	const c = row.run.nodeCounts;
	const parts = [c.failed ? `${c.failed} failed` : null, c.skipped ? `${c.skipped} skipped` : null].filter(Boolean);
	return parts.join(', ');
}

export function statusLine(row: RunRow, now: number): StatusLine {
	const { run, progress } = row;
	const nodes = progress.total ? `${progress.done}/${progress.total} nodes` : null;
	if (row.cancelRequestedAt && (run.status === 'running' || run.status === 'launched' || run.status === 'unresponsive' || run.status === 'queued')) {
		return { text: `Cancelling… requested ${formatAgo(row.cancelRequestedAt, now)}`, tone: 'warn', title: `Cancel requested ${formatStamp(row.cancelRequestedAt)}` };
	}
	switch (run.status) {
		case 'queued':
			if (!row.waits.length) return { text: 'Starting on the next scheduler pass', tone: 'muted' };
			return {
				text: waitText(row.waits[0]) + (row.waits.length > 1 ? ` (+${row.waits.length - 1} more)` : ''),
				tone: 'muted',
				title: row.waits.map(waitTechnical).join('\n')
			};
		case 'launched':
			return { text: 'Starting…', tone: 'live', title: 'Container launched; waiting for it to report in' };
		case 'running':
			return { text: nodes ?? 'Running', tone: 'live' };
		case 'unresponsive': {
			const limit = formatSeconds(row.lostContactLimitSeconds);
			return {
				text: `Lost contact ${formatAgo(row.lastContactAt, now)}`,
				tone: 'warn',
				title: `No heartbeat since ${formatStamp(row.lastContactAt)}. Marked failed after ${limit} of silence; rerunning is safe then.`
			};
		}
		case 'success':
			return { text: progress.total > 1 ? `All ${progress.total} nodes succeeded` : progress.total ? '1 node succeeded' : 'Succeeded', tone: 'muted' };
		case 'warn':
			return { text: `Succeeded with ${plural(run.nodeCounts.warn, 'warning')}`, tone: 'warn' };
		case 'partial':
			return { text: `Partial · ${outcomeCounts(row)}`, tone: 'error', title: run.statusReason ?? undefined };
		case 'failed':
			if (!run.startedAt) return { text: "Couldn't start", tone: 'error', title: run.statusReason ?? undefined };
			return { text: outcomeCounts(row) || 'Failed', tone: 'error', title: run.statusReason ?? undefined };
		case 'presumed_failed':
			return {
				text: `Lost contact — marked failed after ${formatSeconds(row.lostContactLimitSeconds).replace(/m 00s$/, ' min')}`,
				tone: 'error',
				title: run.statusReason ?? undefined
			};
		case 'cancelled':
			return { text: run.statusReason?.startsWith('Cancelled') ? run.statusReason : 'Cancelled', tone: 'muted' };
	}
}
