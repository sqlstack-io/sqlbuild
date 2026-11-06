// Plain banners above the run: lost contact, waiting, and cancel progress.

import type { CancelProgress, RunDetail } from '$lib/api';
import { formatAgo, formatStamp } from '$lib/format/time';
import { describeWait, formatSeconds, waitTechnical } from '$lib/operate/labels';
import type { Banner } from '../types';

const short = (seconds: number) => formatSeconds(seconds).replace(/^(\d+)m 00s$/, '$1m');

/** Cancel progress in words, e.g. "Stopping: queries cancelled 12s ago; force stop in 48s if still running". */
export function cancelWords(cancel: CancelProgress, now: number): string {
	const by = cancel.requestedBy.displayName;
	if (cancel.terminatedAt) {
		if (cancel.sigkillAt) return `Cancelled by ${by}. It didn't stop within ${cancel.graceSeconds}s, so it was force-stopped.`;
		if (!cancel.sigtermAt) return `Cancelled by ${by} before it started.`;
		return `Cancelled by ${by} ${formatAgo(cancel.terminatedAt, now)}.`;
	}
	if (!cancel.sigtermAt) return `Cancel requested by ${by} ${formatAgo(cancel.requestedAt, now)}. Asking the run to stop…`;
	const killAt = Date.parse(cancel.sigtermAt) + cancel.graceSeconds * 1000;
	const left = Math.max(0, (killAt - now) / 1000);
	return `Stopping: running queries were cancelled ${formatAgo(cancel.sigtermAt, now)}. ${left > 0 ? `Force stop in ${short(left)} if it's still running.` : 'Force-stopping now.'}`;
}

function cancelTechnical(cancel: CancelProgress): string {
	return [
		`cancel_requested ${formatStamp(cancel.requestedAt)} by ${cancel.requestedBy.name}`,
		`SIGTERM ${cancel.sigtermAt ? formatStamp(cancel.sigtermAt) : '—'} (grace ${cancel.graceSeconds}s)`,
		`SIGKILL ${cancel.sigkillAt ? formatStamp(cancel.sigkillAt) : '—'}`,
		`exit code ${cancel.exitCode ?? '—'}`
	].join('\n');
}

export function bannersFor(detail: RunDetail, now: number): Banner[] {
	const out: Banner[] = [];
	const { run, liveness: live, cancel } = detail;
	if (live?.state === 'unresponsive' && live.lastHeartbeatAt) {
		const silent = (now - Date.parse(live.lastHeartbeatAt)) / 1000;
		const left = Math.max(0, live.presumedFailedAfterSeconds - silent);
		out.push({
			key: 'lost',
			tone: 'warn',
			title: `No signal for ${short(silent)}.`,
			body: [{ text: `Rerunning is safe once it's marked failed${left > 0 ? `, in about ${short(left)} if the silence continues` : ''}. Nothing to do if it comes back.` }],
			detail: `Last heartbeat ${formatStamp(live.lastHeartbeatAt)} · unresponsive after ${live.unresponsiveAfterSeconds}s, presumed failed after ${live.presumedFailedAfterSeconds}s · ${live.workerInstance ?? ''} ${live.containerId ?? ''}`
		});
	}
	if (run.status === 'presumed_failed') {
		out.push({
			key: 'lost-final',
			tone: 'error',
			title: `Lost contact — marked failed after ${short(live?.presumedFailedAfterSeconds ?? 600)} of silence.`,
			body: [{ text: 'Rerunning is safe: the run has been stopped. Use Rerun from failure to finish the nodes it didn’t complete.' }],
			detail: run.statusReason ?? undefined
		});
	}
	if (live?.state === 'never_started') {
		out.push({
			key: 'never',
			tone: 'error',
			title: "Couldn't start.",
			body: [{ text: `The run never reported in within ${short(live.startDeadlineSeconds)} of launch, so nothing ran. Check the location's latest deploy, then rerun.` }],
			detail: run.statusReason ?? undefined
		});
	}
	if (run.status === 'queued' && detail.waits.length) {
		out.push({
			key: 'waiting',
			tone: 'info',
			title: 'Waiting in the queue.',
			body: detail.waits.flatMap((w, i) => [...(i ? [{ text: ' · ' }] : []), ...describeWait(w)]),
			detail: detail.waits.map(waitTechnical).join('\n')
		});
	}
	if (cancel && !cancel.terminatedAt) {
		const stuck = live?.state === 'unresponsive';
		out.push({
			key: 'cancel',
			tone: stuck ? 'error' : 'warn',
			title: stuck ? "Cancel can't reach the run." : 'Cancelling.',
			body: [{ text: stuck ? 'There is no signal from the run, so it can’t be told to stop. It will be marked failed if the silence continues.' : cancelWords(cancel, now) }],
			detail: cancelTechnical(cancel)
		});
	}
	return out;
}
