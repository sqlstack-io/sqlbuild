import { describe, expect, it } from 'vitest';
import { syncLine } from './sync-line';

const failing = { since: '2026-11-25T17:42:00Z', reason: '250001 Could not connect to Snowflake backend', halted: false, unreachable: false, retrySeconds: 60, retryAt: '2026-11-25T18:30:00Z' };

describe('syncLine', () => {
	it('says what stands, since when, how it retries and why', () => {
		expect([
			syncLine(failing, '17:42'),
			syncLine({ ...failing, unreachable: true }, '17:42'),
			syncLine({ ...failing, halted: true, retrySeconds: null, retryAt: null }, '17:42')
		]).toEqual([
			'Run history not syncing since 17:42 · retrying every 60 s · 250001 Could not connect to Snowflake backend',
			"Run history can't reach the warehouse since 17:42 · retrying every 60 s · 250001 Could not connect to Snowflake backend",
			'Run history sync stopped since 17:42 · 250001 Could not connect to Snowflake backend'
		]);
	});
});
