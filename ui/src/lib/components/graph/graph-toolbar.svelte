<script lang="ts">
	import type { Snippet } from 'svelte';
	import ChevronsDownUpIcon from '@lucide/svelte/icons/chevrons-down-up';
	import ChevronsUpDownIcon from '@lucide/svelte/icons/chevrons-up-down';
	import FocusIcon from '@lucide/svelte/icons/focus';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import PanelRightIcon from '@lucide/svelte/icons/panel-right';
	import RotateCcwIcon from '@lucide/svelte/icons/rotate-ccw';
	import Button from '$lib/components/ui/button/button.svelte';

	// The toolbar over every graph, as on Lineage: page controls first (`start`: search, filters), then
	// Folders, Expand all and Collapse all, page chips (`middle`), Focus and Reset, a summary at the right
	// (`end`) and the details toggle. A control shows when the page gives it; Reset always does.
	let {
		start,
		middle,
		end,
		folders,
		expand,
		focus,
		onreset,
		details
	}: {
		start?: Snippet;
		middle?: Snippet;
		end?: Snippet;
		/** One card per folder, or every node. */
		folders?: { on: boolean; toggle: () => void };
		/** Open or close everything that opens in place; `note` says where opening stopped. */
		expand?: { all: () => void; collapse: () => void; canCollapse: boolean; allTitle?: string; collapseTitle?: string; note?: string };
		/** Show only the selection and what it connects to. */
		focus?: { on: boolean; can: boolean; toggle: () => void; title?: string };
		onreset: () => void;
		details?: { open: boolean; toggle: () => void };
	} = $props();
</script>

<div class="graph-toolbar">
	{#if start}
		{@render start()}
		<span class="toolbar-sep"></span>
	{/if}
	{#if folders}
		<Button variant="ghost" aria-pressed={folders.on} onclick={folders.toggle} title={folders.on ? 'Draw every node' : 'One card per folder'}><FolderIcon />Folders</Button>
	{/if}
	{#if expand}
		<Button variant="ghost" onclick={expand.all} title={expand.allTitle ?? 'Open everything in view'}><ChevronsUpDownIcon />Expand all</Button>
		<Button variant="ghost" onclick={expand.collapse} disabled={!expand.canCollapse} title={expand.collapseTitle ?? 'Close everything in view'}><ChevronsDownUpIcon />Collapse all</Button>
		{#if expand.note}<span class="expand-note" role="status">{expand.note}</span>{/if}
	{/if}
	{@render middle?.()}
	{#if focus}
		<Button variant="ghost" aria-pressed={focus.on} onclick={focus.toggle} disabled={!focus.on && !focus.can} title={!focus.on && !focus.can ? 'Select something to focus on it' : (focus.title ?? 'Show only the selection and what it connects to')}>
			<FocusIcon />{focus.on ? 'Show all' : 'Focus'}
		</Button>
	{/if}
	<Button variant="ghost" onclick={onreset} title="Reset layout"><RotateCcwIcon />Reset</Button>
	<div class="end">{@render end?.()}</div>
	{#if details}
		<Button variant="ghost" size="icon-bar" aria-pressed={details.open} onclick={details.toggle} title={details.open ? 'Hide details' : 'Show details'} aria-label="Toggle details"><PanelRightIcon /></Button>
	{/if}
</div>

<style>
	.graph-toolbar {
		display: flex;
		height: 46px;
		flex: 0 0 46px;
		align-items: center;
		gap: 4px;
		padding: 0 10px;
		border-bottom: 1px solid var(--border);
		background: var(--card);
	}
	.toolbar-sep {
		width: 1px;
		height: 18px;
		margin: 0 6px;
		background: var(--border);
	}
	.expand-note {
		color: var(--muted-foreground);
		font-size: 12px;
		white-space: nowrap;
	}
	.end {
		display: flex;
		align-items: center;
		margin-left: auto;
	}
</style>
