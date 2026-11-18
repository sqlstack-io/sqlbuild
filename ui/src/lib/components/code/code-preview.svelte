<script lang="ts">
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Maximize2Icon from '@lucide/svelte/icons/maximize-2';
	import { compileRefs, highlightLines } from '$lib/sql/outline';

	let {
		file,
		code,
		language = 'sql',
		maxLines = 12,
		compiled,
		mode = $bindable<'raw' | 'compiled'>('raw'),
		onOpen
	}: {
		file: string;
		code: string;
		language?: 'sql' | 'text';
		maxLines?: number;
		/** Compiled SQL from the server; defaults to substituting refs in `code`. */
		compiled?: string;
		mode?: 'raw' | 'compiled';
		onOpen?: () => void;
	} = $props();

	const shown = $derived(language === 'sql' && mode === 'compiled' ? (compiled ?? compileRefs(code)) : code);
	const lines = $derived(
		language === 'sql'
			? highlightLines(shown, false)
			: shown.split('\n').map((l) => l.replace(/&/g, '&amp;').replace(/</g, '&lt;'))
	);
</script>

<div class="code-preview">
	<div class="head">
		<span class="file">{file}</span>
		{#if language === 'sql'}
			<div class="ml-auto">
				<SegmentedFilter label="SQL" bind:value={mode} options={[{ value: 'raw', label: 'Raw' }, { value: 'compiled', label: 'Compiled' }]} />
			</div>
		{/if}
		{#if onOpen}
			<Button variant="ghost" size="icon-sm" class={language !== 'sql' ? 'ml-auto' : ''} onclick={onOpen} title="Open full view" aria-label="Open full view"><Maximize2Icon /></Button>
		{/if}
	</div>
	<div class="body">
		{#each lines.slice(0, maxLines) as line, i (i)}
			<div class="line"><span class="ln">{i + 1}</span><code>{@html line || ' '}</code></div>
		{/each}
	</div>
	{#if lines.length > maxLines && onOpen}
		<div class="more"><Button variant="link" size="inline" class="text-[12px]" onclick={onOpen}>Show all {lines.length} lines</Button></div>
	{/if}
</div>

<style>
	.code-preview {
		overflow: hidden;
		border: 1px solid var(--panel-border);
		box-shadow: var(--panel-shadow);
		border-radius: 2px;
		background: var(--sb-inset);
	}
	.head {
		display: flex;
		height: 36px;
		align-items: center;
		gap: 8px;
		padding: 0 6px 0 12px;
		border-bottom: 1px solid var(--border);
		background: var(--card);
	}
	.file {
		overflow: hidden;
		font: 500 12px var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	/* A preview is read at a glance, so long lines wrap; the full viewer scrolls. */
	.body {
		padding: 8px 0;
		color: var(--sb-mono);
		font: 12px/1.7 var(--font-mono);
	}
	.line {
		display: flex;
		padding-right: 12px;
	}
	.ln {
		width: 38px;
		flex: 0 0 38px;
		padding-right: 12px;
		color: var(--sb-text-faint);
		text-align: right;
		user-select: none;
	}
	.line code {
		min-width: 0;
		font: inherit;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.more {
		display: flex;
		height: 32px;
		align-items: center;
		justify-content: center;
		border-top: 1px solid var(--border);
		background: var(--card);
	}
</style>
