<script lang="ts">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
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
		<button class="toggle" onclick={() => (open = !open)} aria-expanded={open}>
			<ChevronDownIcon size={14} />
			<span>{title}</span>
		</button>
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
	.toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 6px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--foreground);
		font-size: 12.5px;
		font-weight: 600;
	}
	.toggle :global(svg) {
		color: var(--muted-foreground);
		transition: transform 0.15s;
	}
	.closed .toggle :global(svg) {
		transform: rotate(-90deg);
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
