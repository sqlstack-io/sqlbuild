import { describe, expect, it } from 'vitest';
import type { RunNode } from '$lib/api';
import { errorHeadline, errorTextOf, tracebackOf } from './failures';

describe('a failed node’s error', () => {
	it('finds a Python traceback in its text, from its first line, and none in a warehouse error', () => {
		const printed = 'scoring batch 3\nTraceback (most recent call last):\n  File "models/ml/score.py", line 9, in score\nKeyError: \'segment\'';
		expect([tracebackOf(printed), tracebackOf('Division by zero'), tracebackOf(null)]).toEqual([
			'Traceback (most recent call last):\n  File "models/ml/score.py", line 9, in score\nKeyError: \'segment\'',
			null,
			null
		]);
	});

	it('reads its text from the node, else from the error read for it', () => {
		const node = (extra: Partial<RunNode>) => ({ errorDetail: null, errorMessage: null, ...extra }) as RunNode;
		const loaded = { nodeKey: 'model:orders', errorCode: null, message: 'Division by zero', file: null };
		expect([errorTextOf(node({ errorDetail: 'full text' }), loaded), errorTextOf(node({}), loaded), errorTextOf(node({ errorMessage: 'first line' }), null), errorTextOf(node({}), null)]).toEqual([
			'full text',
			'Division by zero',
			'first line',
			null
		]);
	});

	it('reads a traceback by its exception and any other error by its first line', () => {
		const traceback = 'Traceback (most recent call last):\n  ... 12 earlier lines\n  File "models/ml/score.py", line 9, in score\n    row["segment"]\nKeyError: \'segment\'\n';
		expect([errorHeadline(traceback), errorHeadline('Division by zero\nin margin_pct'), errorHeadline(null)]).toEqual(["KeyError: 'segment'", 'Division by zero', null]);
	});
});
