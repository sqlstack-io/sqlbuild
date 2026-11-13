import { describe, expect, it } from 'vitest';
import type { RunDetail } from '$lib/api';
import { bannersFor } from './banners';

const failed = (statusReason: string | null, failedNodes = 0) =>
	({ run: { status: 'failed', statusReason, nodeCounts: { failed: failedNodes } }, liveness: null, cancel: null, waits: [] }) as unknown as RunDetail;
const text = (detail: RunDetail, cause: string | null) => bannersFor(detail, 0, cause).find((b) => b.key === 'failed')?.body.map((p) => p.text).join('');

describe('the banner of a run that failed as a whole', () => {
	it('leads with the first recorded error and keeps how the run ended beside it', () => {
		expect(text(failed('Timed out after 1h'), 'Connection reset; retrying')).toBe('Connection reset; retrying · Ended: Timed out after 1h');
	});

	it('falls back to the status reason, and leaves a failed node to explain itself', () => {
		expect(text(failed('Timed out after 1h'), null)).toBe('Timed out after 1h');
		expect(text(failed('Worker exited with code 1', 1), 'Binder Error')).toBeUndefined();
	});
});
