// The plain second line under a run's status: what is happening, in words.

import type { RunRow } from '$lib/api';
import { formatAgo, formatStamp } from '$lib/format/time';
import { formatSeconds, outcomeCounts, waitTechnical, waitText } from '$lib/operate/labels';
import type { StatusLine } from '../types';
import { plural } from '$lib/format/plural';


export function statusLine(row: RunRow, now: number): StatusLine {
	const { run, progress } = row;
	const nodes = progress.total ? `${progress.done}/${progress.total} nodes` : null;
	if (row.cancelRequestedAt && (run.status === 'running' || run.status === 'launched' || run.status === 'unresponsive' || run.status === 'queued')) {
		return { text: `Cancelling… requested ${formatAgo(row.cancelRequestedAt, now)}`, tone: 'warn', title: `Cancel requested ${formatStamp(row.cancelRequestedAt)}` };
	}
	switch (run.status) {
		case 'queued':
			if (!row.waits.length) return { text: 'Next pass', tone: 'muted', title: 'Nothing blocks it; starts on the next scheduler pass' };
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
		case 'failed':
			if (!run.startedAt) return { text: "Couldn't start", tone: 'error', title: run.statusReason ?? undefined };
			return { text: outcomeCounts(run.nodeCounts) || 'Failed', tone: 'error', title: run.statusReason ?? undefined };
		case 'presumed_failed':
			return {
				text: 'Lost contact',
				tone: 'error',
				title: [`Marked failed after ${formatSeconds(row.lostContactLimitSeconds).replace(/m 00s$/, ' min')} without contact`, run.statusReason].filter(Boolean).join('. ')
			};
		case 'cancelled':
			return { text: run.statusReason?.startsWith('Cancelled') ? run.statusReason : 'Cancelled', tone: 'muted' };
		case 'skipped':
			return { text: run.statusReason ? run.statusReason.charAt(0).toUpperCase() + run.statusReason.slice(1) : 'Skipped', tone: 'muted', title: 'Its group did not run it because a job it depends on failed' };
	}
}
