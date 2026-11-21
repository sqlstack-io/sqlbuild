// How a failed node's error reads: a check that stopped it, or a Python exception's traceback. Pure.

import type { RunNode, RunNodeError } from '$lib/api';

/** The error code sqb gives a node that failed on an error-severity check. */
export const CHECK_FAILED_CODE = 'R006';

const TRACEBACK = 'Traceback (most recent call last):';

/** The Python traceback in an error's text, from its first line, or null when it holds none. */
export function tracebackOf(text: string | null | undefined): string | null {
	const at = text?.indexOf(TRACEBACK) ?? -1;
	return text && at >= 0 ? text.slice(at) : null;
}

/** A failed node's error text: what the node carries, or what was read for it on request. */
export function errorTextOf(n: RunNode, loaded: RunNodeError | null): string | null {
	return n.errorDetail ?? loaded?.message ?? n.errorMessage ?? null;
}
