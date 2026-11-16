<script lang="ts">
	import CircleHelpIcon from '@lucide/svelte/icons/circle-question-mark';
	import type { Snippet } from 'svelte';
	import * as Tooltip from '$lib/components/ui/tooltip';

	// Help that used to be a paragraph on the page: a "?" that shows it on hover or focus,
	// and an optional link to the docs.
	let { text, children, docs, label = 'More about this' }: { text?: string; children?: Snippet; docs?: string; label?: string } = $props();
</script>

<span class="help-tip">
	{#if text || children}
		<Tooltip.Provider delayDuration={150}>
			<Tooltip.Root>
				<Tooltip.Trigger class="help-trigger" aria-label={label}><CircleHelpIcon size={13} /></Tooltip.Trigger>
				<Tooltip.Content side="top" sideOffset={4} class="help-content">
					{#if children}{@render children()}{:else}{text}{/if}
				</Tooltip.Content>
			</Tooltip.Root>
		</Tooltip.Provider>
	{/if}
	{#if docs}<a class="docs" href={docs} target="_blank" rel="noreferrer">Docs ↗</a>{/if}
</span>

<style>
	.help-tip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		vertical-align: middle;
	}
	.help-tip :global(.help-trigger) {
		display: inline-grid;
		place-items: center;
		color: var(--sb-text-faint);
		cursor: help;
	}
	.help-tip :global(.help-trigger:hover),
	.help-tip :global(.help-trigger:focus-visible) {
		color: var(--foreground);
	}
	:global(.help-content) {
		display: block !important;
		max-width: 320px;
		font-size: 12px;
		line-height: 1.45;
		white-space: normal;
	}
	:global(.help-content code) {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.docs {
		color: var(--primary);
		font-size: 12px;
		font-weight: 500;
		white-space: nowrap;
	}
	.docs:hover {
		text-decoration: underline;
	}
</style>
