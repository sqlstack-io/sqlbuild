<script lang="ts">
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
			<div class="seg-group">
				{#each [['raw', 'Raw'], ['compiled', 'Compiled']] as [m, label] (m)}
					<button class:on={mode === m} onclick={() => (mode = m as 'raw' | 'compiled')}>{label}</button>
				{/each}
			</div>
		{/if}
		{#if onOpen}
			<button class="icon" class:ml-auto={language !== 'sql'} onclick={onOpen} title="Open full view" aria-label="Open full view"><Maximize2Icon size={13} /></button>
		{/if}
	</div>
	<div class="body">
		{#each lines.slice(0, maxLines) as line, i (i)}
			<div class="line"><span class="ln">{i + 1}</span><code>{@html line || ' '}</code></div>
		{/each}
	</div>
	{#if lines.length > maxLines && onOpen}
		<button class="more" onclick={onOpen}>Show all {lines.length} lines</button>
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
	.seg-group {
		display: flex;
		gap: 2px;
		margin-left: auto;
		padding: 2px;
		border-radius: 2px;
		background: var(--sb-hover);
	}
	.seg-group button {
		padding: 2px 8px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.seg-group button.on {
		background: var(--card);
		color: var(--foreground);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
	}
	.icon {
		display: grid;
		width: 26px;
		height: 26px;
		place-items: center;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
	}
	.icon:hover {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	.ml-auto {
		margin-left: auto;
	}
	.body {
		padding: 8px 0;
		overflow-x: auto;
		color: var(--sb-mono);
		font: 12px/1.7 var(--font-mono);
	}
	.line {
		display: flex;
		min-width: max-content;
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
		font: inherit;
		white-space: pre;
	}
	.more {
		display: block;
		width: 100%;
		height: 32px;
		border: 0;
		border-top: 1px solid var(--border);
		background: var(--card);
		color: var(--primary);
		font-size: 12px;
	}
	.more:hover {
		background: var(--sb-hover);
	}
</style>
