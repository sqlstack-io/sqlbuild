import { describe, expect, it } from 'vitest';
import { buttonVariants } from './button.svelte';

describe('link buttons', () => {
	it('take the size of the text around them, never more than the UI’s 13 px text', () => {
		const classes = buttonVariants({ variant: 'link', size: 'inline' });
		expect(classes).toContain('text-[length:min(1em,13px)]');
		expect(classes).not.toMatch(/text-(sm|base|\[1[3-9]px\])/);
	});
});
