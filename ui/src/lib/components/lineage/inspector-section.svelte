<script lang="ts">
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import type { Snippet } from 'svelte';

	let {
		title,
		open = $bindable(true),
		meta,
		children
	}: { title: string; open?: boolean; meta?: Snippet; children: Snippet } = $props();
</script>

<section class="insp-sec" class:closed={!open}>
	<div class="insp-sec-head">
		<DisclosureButton tone="default" {open} onclick={() => (open = !open)} class="px-1.5 py-1 text-[12.5px] font-semibold">{title}</DisclosureButton>
		{#if meta}<div class="meta">{@render meta()}</div>{/if}
	</div>
	{#if open}<div class="insp-sec-body">{@render children()}</div>{/if}
</section>

<style>
	.insp-sec {
		border-bottom: 1px solid var(--border);
	}
	.insp-sec-head {
		display: flex;
		height: 36px;
		align-items: center;
		padding: 0 14px 0 8px;
		background: color-mix(in srgb, var(--sb-hover) 45%, var(--card));
	}
	.insp-sec:not(.closed) .insp-sec-head {
		border-bottom: 1px solid var(--border-subtle);
	}
	.meta {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-left: auto;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.insp-sec-body {
		padding: 12px 16px 14px;
	}
</style>
