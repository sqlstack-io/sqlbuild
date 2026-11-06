import { describe, expect, it } from 'vitest';
import { ApiError } from '$lib/api/client';
import { bootFailure } from './boot-failure';

describe('bootFailure', () => {
	it('explains a missing bootstrap endpoint as missing person sign-in, not an outage', () => {
		const failure = bootFailure(new ApiError('404 Not Found', 404));
		expect(failure.title).toBe('This server has no person sign-in configured');
		expect(failure.configuration).toBe(true);
	});

	it('keeps server errors as unreachable with their message', () => {
		const failure = bootFailure(new ApiError('503 Service Unavailable', 503));
		expect(failure).toEqual({ title: "Can't reach the SQLBuild server", detail: '503 Service Unavailable', configuration: false });
	});

	it('keeps network errors as unreachable', () => {
		const failure = bootFailure(new TypeError('Failed to fetch'));
		expect(failure.title).toBe("Can't reach the SQLBuild server");
		expect(failure.detail).toBe('Failed to fetch');
	});
});
