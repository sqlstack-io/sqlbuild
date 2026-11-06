<script lang="ts">
	import type { Snippet } from 'svelte';

	type Row = { label: string; value?: string; title?: string; tone?: string; mono?: boolean; content?: Snippet };
	let { rows }: { rows: Row[] } = $props();
</script>

<dl class="key-values">
	{#each rows as row (row.label)}
		<dt>{row.label}</dt>
		<dd class:mono={row.mono} style:color={row.tone} title={row.title ?? row.value}>
			{#if row.content}{@render row.content()}{:else}{row.value ?? '—'}{/if}
		</dd>
	{/each}
</dl>

<style>
	.key-values {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr);
		gap: 7px 16px;
		margin: 0;
		font-size: 12.5px;
	}
	dt {
		color: var(--muted-foreground);
	}
	dd {
		min-width: 0;
		margin: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	dd.mono {
		font-family: var(--font-mono);
		font-size: 12px;
	}
</style>
