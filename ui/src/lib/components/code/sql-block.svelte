<script lang="ts">
	import CopyButton from '$lib/components/copy-button.svelte';
	import { highlightSqlbuild } from '$lib/sql/highlight';

	// Highlighted, copyable SQL with a caption, e.g. "statement 3 · fact_orders".
	let { sql, caption, maxHeight = '340px' }: { sql: string; caption?: string; maxHeight?: string } = $props();

	const html = $derived(highlightSqlbuild(sql));
</script>

<div class="sql-block">
	<div class="bar">
		{#if caption}<span class="caption">{caption}</span>{/if}
		<span class="copy"><CopyButton value={sql} label="Copy SQL" text="copy" showValue={false} /></span>
	</div>
	<pre style:max-height={maxHeight}><code>{@html html}</code></pre>
</div>

<style>
	.sql-block {
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.bar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 4px 8px;
		border-bottom: 1px solid var(--border);
		background: var(--sb-surface-low);
	}
	.copy {
		margin-left: auto;
	}
	.caption {
		overflow: hidden;
		color: var(--sb-text-faint);
		font-family: var(--font-mono);
		font-size: 10.5px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	pre {
		margin: 0;
		overflow: auto;
		padding: 10px 12px;
		background: var(--sb-inset);
		font-family: var(--font-mono);
		font-size: 11.5px;
		line-height: 1.65;
		/* Wrap long lines: inside table cells a scrolling block widens its column instead. */
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
