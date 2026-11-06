<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import { app } from '$lib/state/app.svelte';
	import type { RunsState } from './state.svelte';
	import type { BulkAction } from './types';
	import { bulkCapability, bulkLabel } from './utils';

	let { list, matchingActive }: { list: RunsState; matchingActive: number | null } = $props();

	const ACTIONS: BulkAction[] = ['cancel', 'rerun', 'from_failure'];
	const total = $derived(list.selected.size);
	const scopedTab = $derived(list.params.tab === 'active' || list.params.tab === 'queued');

	function title(action: BulkAction, count: number): string {
		if (!app.can(bulkCapability[action])) return `You don't have permission (${bulkCapability[action]})`;
		if (!count) {
			if (action === 'cancel') return 'None of the selected runs is still active';
			if (action === 'rerun') return 'Only finished runs can be rerun';
			return 'None of the selected runs has failed nodes to rerun';
		}
		if (count < total) return `${total - count} of the selected runs don't apply or need more permissions`;
		return '';
	}

	function label(action: BulkAction, count: number): string {
		if (list.armed === action) return `Confirm: ${bulkLabel[action].toLowerCase()} ${count} run${count === 1 ? '' : 's'}`;
		return `${bulkLabel[action]} ${count} of ${total}`;
	}
</script>

{#if total || (scopedTab && matchingActive)}
	<div class="bulk" class:has-selection={total > 0}>
		{#if total}
			<span class="count">{total} selected</span>
			{#each ACTIONS as action (action)}
				{@const count = list.eligibleFor[action].length}
				<span title={title(action, count)}>
					<Button
						size="sm"
						variant={list.armed === action ? (action === 'cancel' ? 'destructive' : 'default') : 'outline'}
						disabled={!count || list.busy !== null}
						aria-busy={list.busy === action}
						onclick={() => list.bulk(action)}>{label(action, count)}</Button
					>
				</span>
			{/each}
			<button class="clear" onclick={() => list.clearSelection()}>Clear selection</button>
		{/if}
		{#if scopedTab && matchingActive}
			<span class="all" title={app.can('run.cancel') ? 'Cancels every active run matching the current tab and filters, not only the loaded ones' : "You don't have permission (run.cancel)"}>
				<Button
					size="sm"
					variant={list.armed === 'cancel_all' ? 'destructive' : 'outline'}
					disabled={!app.can('run.cancel') || list.busy !== null}
					aria-busy={list.busy === 'cancel_all'}
					onclick={() => list.cancelAll()}
				>
					{list.armed === 'cancel_all' ? `Confirm: cancel all ${matchingActive}` : `Cancel all ${matchingActive} matching`}
				</Button>
			</span>
		{/if}
		{#if list.armed}<span class="hint">Click again to confirm.</span>{/if}
	</div>
{/if}
{#if list.result}
	<div class="result" class:failed={list.result.failed} role="status">
		{list.result.text}
		<button onclick={() => list.dismissResult()}>Dismiss</button>
	</div>
{/if}

<style>
	.bulk {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 8px;
		padding: 7px 20px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.bulk.has-selection {
		background: color-mix(in srgb, var(--primary) 6%, var(--background));
	}
	.count {
		margin-right: 4px;
		font-weight: 600;
	}
	.all {
		margin-left: auto;
	}
	.clear,
	.result button {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.clear:hover,
	.result button:hover {
		color: var(--foreground);
	}
	.hint {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.result {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 10px;
		padding: 7px 20px;
		border-bottom: 1px solid var(--border);
		background: color-mix(in srgb, var(--sb-success) 7%, var(--background));
		font-size: 12.5px;
	}
	.result.failed {
		background: color-mix(in srgb, var(--sb-error) 8%, var(--background));
		color: var(--sb-error);
	}
</style>
