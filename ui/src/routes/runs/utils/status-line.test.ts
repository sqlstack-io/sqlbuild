import { describe, expect, it } from 'vitest';
import type { RunRow } from '$lib/api';
import { runStatusLabel } from '$lib/status';
import { statusLine } from './status-line';

const row = (status: string, statusReason: string | null) =>
	({ run: { status, statusReason, nodeCounts: {} }, progress: { done: 0, total: 1 }, waits: [], cancelRequestedAt: null }) as unknown as RunRow;

describe('the status line of a group member its group skipped', () => {
	it('says why it was skipped, without reading as a failure', () => {
		expect(statusLine(row('skipped', 'skipped because orders_job did not succeed'), 0)).toMatchObject({
			text: 'Skipped because orders_job did not succeed',
			tone: 'muted'
		});
		expect(statusLine(row('skipped', null), 0).text).toBe('Skipped');
		expect(runStatusLabel.skipped).toBe('Skipped');
	});
});
