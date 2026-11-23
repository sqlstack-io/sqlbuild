import { describe, expect, it } from 'vitest';
import type { RunDetail, RunRow } from '$lib/api';
import { eligible } from '../../utils/bulk';
import { rerunChoices } from './rerun';

const detail = (status: string, unfinished: string[]) =>
	({ run: { status, kind: 'job', rewriteHistory: false }, rerunFromFailureNodes: unfinished }) as unknown as RunDetail;
const row = (status: string, count: number) => ({ run: { status }, rerunFromFailureCount: count }) as unknown as RunRow;

describe('rerun of a cancelled run', () => {
	it.each([
		['with unfinished steps', ['asset:fulfil[pack]', 'asset:fulfil[ship]'], 'from_failure', true],
		['with every step finished', [], 'all', false]
	] as const)('%s', (_, unfinished, expectedFirstOffered, expectedEligible) => {
		const cancelled = detail('cancelled', [...unfinished]);
		const choices = rerunChoices(cancelled, [], () => true);
		expect(choices.find((c) => !c.disabledReason)?.mode).toBe(expectedFirstOffered);
		expect(choices.find((c) => c.mode === 'from_failure')?.disabledReason === null).toBe(expectedEligible);
		expect(eligible('from_failure', row('cancelled', unfinished.length))).toBe(expectedEligible);
	});
});
