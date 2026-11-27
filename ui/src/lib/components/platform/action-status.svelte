<script lang="ts">
	import type { Capability } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { actionTone, type ActionWords } from '$lib/platform/labels';
	import GatedButton from './gated-button.svelte';

	/**
	 * One status that changes on its own and the action beside it, e.g. "Warehouse sync ● synced 2m ago [Sync now]".
	 * The action is always shown: disabled while it runs, with since when in its tooltip, and clickable
	 * when the thing is broken. A failure's cause is a line of its own under the strip, so it is read in full.
	 * The parts sit directly in the parent's flex row, which wraps the causes under every status.
	 */
	type Action = {
		label: string;
		requires: Capability;
		onclick: () => void;
		/** Running: the button is disabled and `busyTitle` says since when. */
		busy?: boolean;
		busyTitle?: string;
		title?: string;
	};
	type Props = { lead?: string; words?: ActionWords | null; action?: Action; cause?: string | null; testid?: string; size?: 'bar' | 'sm' };
	let { lead, words = null, action, cause = null, testid, size = 'bar' }: Props = $props();
</script>

<div class="action-status" data-testid={testid}>
	<span class="group">
		{#if lead}<span class="lead">{lead}</span>{/if}
		{#if words}<span class="status" title={words.title} data-tone={words.tone}><StatusLabel tone={actionTone[words.tone]} label={words.label} muted={words.tone === 'never'} /></span>{/if}
		{#if action}
			<span class="action" title={action.busy ? action.busyTitle : undefined}>
				<GatedButton requires={action.requires} busy={action.busy} onclick={action.onclick} title={action.title} {size}>{action.label}</GatedButton>
			</span>
		{/if}
	</span>
	{#if cause}<span class="cause" title={cause}>{lead ? `${lead}: ` : ''}{cause}</span>{/if}
</div>

<style>
	.action-status {
		display: contents;
	}
	/* Lead, status and action wrap as one. */
	.group {
		display: inline-flex;
		min-width: 0;
		align-items: center;
		gap: 8px;
		margin-left: 14px;
	}
	.lead {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.status {
		min-width: 0;
	}
	.action {
		display: inline-flex;
	}
	/* After every status in the strip, on a line of its own. */
	.cause {
		order: 10;
		flex-basis: 100%;
		overflow: hidden;
		color: var(--sb-error);
		margin-left: 14px;
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
