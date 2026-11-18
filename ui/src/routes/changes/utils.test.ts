import { describe, expect, it } from 'vitest';
import { shortRevision } from './utils';

describe('shortRevision', () => {
	it('shortens a full commit hash to 7 characters', () => {
		expect(shortRevision('7d2e41a9c0b3f18e2d4a6b7c9e0f1a2b3c4d5e6f')).toBe('7d2e41a');
	});

	it('keeps a short hash, a tag or a branch name as it is', () => {
		expect(shortRevision('7d2e41a')).toBe('7d2e41a');
		expect(shortRevision('v1.4.0')).toBe('v1.4.0');
		expect(shortRevision('orders-release')).toBe('orders-release');
	});
});
