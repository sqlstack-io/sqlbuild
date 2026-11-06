<script lang="ts">
	import CheckIcon from '@lucide/svelte/icons/check';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import { highlightSqlbuild } from '$lib/sql/highlight';

	// Highlighted, copyable SQL with a caption, e.g. "statement 3 · fact_orders".
	let { sql, caption, maxHeight = '340px' }: { sql: string; caption?: string; maxHeight?: string } = $props();

	const html = $derived(highlightSqlbuild(sql));
	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function copy() {
		await navigator.clipboard?.writeText(sql).catch(() => undefined);
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 1200);
	}
</script>

<div class="sql-block">
	<div class="bar">
		{#if caption}<span class="caption">{caption}</span>{/if}
		<button onclick={copy} aria-label="Copy SQL">
			{#if copied}<CheckIcon size={11} /> copied{:else}<CopyIcon size={11} /> copy{/if}
		</button>
	</div>
	<pre style:max-height={maxHeight}><code>{@html html}</code></pre>
</div>

<style>
	.sql-block {
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 4px;
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 4px 8px;
		border-bottom: 1px solid var(--border);
		background: var(--sb-surface-low);
	}
	.caption {
		overflow: hidden;
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 10.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	button {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		margin-left: auto;
		padding: 2px 6px;
		border-radius: 3px;
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 10.5px;
	}
	button:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	pre {
		margin: 0;
		overflow: auto;
		padding: 10px 12px;
		background: var(--sb-inset);
		font-family: var(--font-mono);
		font-size: 11.5px;
		line-height: 1.65;
	}
</style>
