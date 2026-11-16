<script lang="ts">
	import type { Snippet } from 'svelte';
	import HelpTip from '$lib/components/help-tip.svelte';

	// Label and value pairs. A value is a short fact; `help` puts the explanation in a "?" after the label.
	type Row = { label: string; value?: string; title?: string; help?: string; tone?: string; mono?: boolean; content?: Snippet };
	let { rows }: { rows: Row[] } = $props();
</script>

<dl class="key-values">
	{#each rows as row (row.label)}
		<dt>{row.label}{#if row.help}<HelpTip text={row.help} label="About {row.label}" />{/if}</dt>
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
		display: inline-flex;
		align-items: center;
		gap: 4px;
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
