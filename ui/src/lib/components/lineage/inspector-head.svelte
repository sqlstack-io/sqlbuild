<script lang="ts">
	import type { Snippet } from 'svelte';

	// The top of a Lineage-style sidebar: what is shown, a link to its own page, and a line of facts under it.
	let {
		title,
		hint,
		href,
		linkLabel,
		linkTitle,
		meta
	}: { title: string; hint?: string; href?: string; linkLabel?: string; linkTitle?: string; meta?: Snippet } = $props();
</script>

<div class="inspector-head">
	<div class="flex items-center gap-2">
		<span class="truncate text-[15px] font-semibold" title={hint ?? title}>{title}</span>
		{#if href && linkLabel}<a {href} class="head-link ml-auto" title={linkTitle}>{linkLabel}</a>{/if}
	</div>
	{#if meta}<div class="inspector-meta">{@render meta()}</div>{/if}
</div>

<style>
	.inspector-head {
		padding: 14px 16px 12px;
		border-bottom: 1px solid var(--border);
	}
	.inspector-meta :global(span + span::before) {
		content: '·';
		margin-right: 12px;
		color: var(--sb-text-faint);
	}
	.inspector-meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px 12px;
		margin-top: 6px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.head-link {
		flex: 0 0 auto;
		padding: 3px 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.head-link:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
</style>
