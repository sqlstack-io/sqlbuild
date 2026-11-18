<script lang="ts">
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import type { Snippet } from 'svelte';
	import { cn } from '$lib/utils';

	// The one way to show or hide a section: a chevron that turns when open and a label, in muted text
	// that brightens on hover. Pass layout only (width, height, padding, margin, text size) through
	// `class`; the text size otherwise follows the surrounding text.
	let {
		open,
		onclick,
		tone = 'muted',
		title,
		class: className,
		children
	}: {
		open: boolean;
		/** Optional when an enclosing row handles the click. */
		onclick?: (event: MouseEvent) => void;
		/** `default` for a disclosure that is the section's own heading line. */
		tone?: 'muted' | 'default';
		title?: string;
		class?: string;
		children: Snippet;
	} = $props();
</script>

<button type="button" class={cn('disclosure', className)} class:strong={tone === 'default'} aria-expanded={open} {title} {onclick}>
	<ChevronRightIcon size={14} class="chevron" />
	{@render children()}
</button>

<style>
	.disclosure {
		display: inline-flex;
		min-width: 0;
		align-items: center;
		gap: 6px;
		color: var(--muted-foreground);
		text-align: left;
	}
	.disclosure.strong {
		color: var(--foreground);
	}
	.disclosure:hover {
		color: var(--foreground);
	}
	.disclosure :global(.chevron) {
		flex: 0 0 auto;
		color: var(--sb-text-faint);
		transition: transform 120ms;
	}
	.disclosure[aria-expanded='true'] :global(.chevron) {
		transform: rotate(90deg);
	}
</style>
