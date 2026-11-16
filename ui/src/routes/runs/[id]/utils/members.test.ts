import { describe, expect, it } from 'vitest';
import type { Run } from '$lib/api';
import { attemptLabel, retriedMembers } from './members';

const member = (id: string, jobKey: string, memberAttempt: number | null, rerunOfRunId: string | null = null) =>
	({ id, jobKey, memberAttempt, rerunOfRunId, parentRunId: 'group' }) as unknown as Run;

describe('member attempts of a group run', () => {
	const first = member('first', 'partner_export_job', null);
	const second = member('second', 'partner_export_job', 2, 'first');
	const orders = member('orders', 'orders_job', null);

	it('counts each retried member job once', () => {
		expect(retriedMembers([first, second, orders])).toBe(1);
		expect(retriedMembers([first, orders])).toBe(0);
	});

	it('labels the attempts of a retried member and leaves other members unlabelled', () => {
		const siblings = [first, second, orders];
		expect(attemptLabel(first, siblings)).toBe('attempt 1 · retried');
		expect(attemptLabel(second, siblings)).toBe('attempt 2');
		expect(attemptLabel(orders, siblings)).toBeNull();
	});
});
