<script lang="ts">
	import type { Capability } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { actionTone, type ActionWords } from '$lib/platform/labels';
	import GatedButton from './gated-button.svelte';
	import Button from '$lib/components/ui/button/button.svelte';

	/**
	 * One status that changes on its own and the action beside it, e.g. "Warehouse sync · synced 2m ago  Sync now".
	 * Calm by default: a healthy status has a neutral dot and the action is a quiet button, always shown,
	 * disabled while it runs with since when in its tooltip. A standing failure links to where it is
	 * explained in full (`href`, the Problems list); a click that failed offers its error behind "View error".
	 * The parts sit in the parent's flex row, which shows an opened error under every status.
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
	type Props = { lead?: string; words?: ActionWords | null; action?: Action; cause?: string | null; href?: string | null; testid?: string; size?: 'bar' | 'sm' };
	let { lead, words = null, action, cause = null, href = null, testid, size = 'sm' }: Props = $props();
	/** Only what needs a look takes a colour; a healthy status reads in neutral text. */
	let showCause = $state(false);
	const tone = $derived(words ? (words.tone === 'ok' ? 'var(--sb-text-faint)' : actionTone[words.tone]) : '');
</script>

<div class="action-status" data-testid={testid}>
	<span class="group">
		{#if lead}<span class="lead">{lead}</span>{/if}
		{#if words}
			{#if href}
				<a class="status" {href} title={words.title} data-tone={words.tone}><StatusLabel {tone} label={words.label} /></a>
			{:else}
				<span class="status" title={words.title} data-tone={words.tone}><StatusLabel {tone} label={words.label} muted={words.tone === 'never'} /></span>
			{/if}
		{/if}
		{#if cause}<Button variant="link" size="inline" class="text-[12px]" aria-expanded={showCause} onclick={() => (showCause = !showCause)}>{showCause ? 'Hide error' : 'View error'}</Button>{/if}
		{#if action}
			<span class="action" title={action.busy ? action.busyTitle : undefined}>
				<GatedButton requires={action.requires} busy={action.busy} onclick={action.onclick} title={action.title} variant="ghost" {size}>{action.label}</GatedButton>
			</span>
		{/if}
	</span>
	{#if cause && showCause}<pre class="cause">{lead ? `${lead}: ` : ''}{cause}</pre>{/if}
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
		gap: 6px;
		margin-left: 16px;
	}
	.lead {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.status {
		min-width: 0;
	}
	a.status:hover :global(.status-label) {
		text-decoration: underline;
	}
	.action {
		display: inline-flex;
	}
	/* An opened error, after every status in the strip, on a line of its own. */
	.cause {
		order: 10;
		flex-basis: 100%;
		max-height: 240px;
		margin: 4px 0 0 16px;
		overflow: auto;
		padding: 6px 8px;
		border-radius: 2px;
		background: var(--sb-inset);
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
