<script lang="ts">
	import type { Snippet } from 'svelte';
	import LockIcon from '@lucide/svelte/icons/lock';
	import HelpTip from '$lib/components/help-tip.svelte';

	/**
	 * `note` is a short fact shown beside the title; `help` is an explanation, shown on hover of a "?";
	 * `audited` marks a panel whose reads are recorded in the audit log with a lock icon.
	 */
	type Props = { title: string; note?: string; help?: string; docs?: string; audited?: boolean; actions?: Snippet; children: Snippet };
	let { title, note, help, docs, audited = false, actions, children }: Props = $props();
</script>

<section class="panel">
	<header>
		<h2>{title}</h2>
		{#if help || docs}<HelpTip text={help} {docs} label="About {title}" />{/if}
		{#if note}<span class="note">{note}</span>{/if}
		{#if audited}<span class="audited" title="Viewing is audited" aria-label="Viewing is audited"><LockIcon size={12} /></span>{/if}
		{#if actions}<div class="actions">{@render actions()}</div>{/if}
	</header>
	{@render children()}
</section>

<style>
	.panel {
		margin-bottom: 28px;
	}
	header {
		display: flex;
		min-height: 30px;
		align-items: center;
		gap: 10px;
		margin-bottom: 8px;
	}
	h2 {
		font-size: 13px;
		font-weight: 600;
	}
	.note {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.audited {
		display: inline-flex;
		color: var(--sb-text-faint);
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-left: auto;
	}
</style>
