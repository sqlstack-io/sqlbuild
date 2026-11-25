<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import Button from '$lib/components/ui/button/button.svelte';
	import { plural } from '$lib/format/plural';

	// The line a filter shows while it is on: what it shows, how many of how many, and ✕ to clear it.
	// `typed`: `what` is filter text as typed, shown as code.
	let { what, shown, total, noun, onclear, testid, typed = false }: { what: string; shown: number; total: number; noun: string; onclear: () => void; testid?: string; typed?: boolean } = $props();
</script>

<div class="active-filter" role="status" data-testid={testid}>
	<span>Showing {#if typed}<code>{what}</code>{:else}{what}{/if} · {shown.toLocaleString('en-GB')} of {plural(total, noun)}</span>
	<Button variant="ghost" size="icon-xs" onclick={onclear} aria-label="Clear the filter" title="Show everything"><XIcon /></Button>
</div>

<style>
	.active-filter {
		display: flex;
		flex: none;
		align-items: center;
		gap: 6px;
		padding: 3px 24px;
		border-block: 1px solid color-mix(in srgb, var(--primary) 25%, var(--border-subtle));
		background: color-mix(in srgb, var(--primary) 6%, var(--background));
		color: var(--muted-foreground);
		font-size: 12px;
	}
	code {
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
</style>
