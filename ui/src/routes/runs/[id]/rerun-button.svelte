<script lang="ts">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import type { RunDetailState } from './state.svelte';

	let { view }: { view: RunDetailState } = $props();

	const primary = $derived(view.primary);
</script>

{#if primary}
	<div class="split">
		<span title={primary.disabledReason ?? primary.help}>
			<button class="main" class:accent={primary.mode === 'from_failure'} disabled={!!primary.disabledReason || view.busy !== null} aria-busy={view.busy === primary.mode} onclick={() => view.rerun(primary.mode)}>
				<RotateCwIcon size={13} />{primary.label}
			</button>
		</span>
		<DropdownMenu.Root>
			<DropdownMenu.Trigger class="caret {primary.mode === 'from_failure' ? 'accent' : ''}" aria-label="More rerun options" disabled={view.busy !== null}>
				<ChevronDownIcon size={14} />
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end" class="w-80">
				{#each view.choices as c (c.mode)}
					<DropdownMenu.Item disabled={!!c.disabledReason} onSelect={() => view.rerun(c.mode)}>
						<div class="item">
							<b>{c.label}</b>
							<span class:reason={!!c.disabledReason}>{c.disabledReason ?? c.help}</span>
						</div>
					</DropdownMenu.Item>
				{/each}
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	</div>
{/if}

<style>
	.split {
		display: inline-flex;
		align-items: stretch;
	}
	.main,
	.split :global(.caret) {
		display: inline-flex;
		height: 32px;
		align-items: center;
		gap: 6px;
		padding: 0 12px;
		border: 1px solid var(--border);
		background: var(--background);
		font-size: 13px;
		font-weight: 500;
	}
	.main {
		border-radius: 6px 0 0 6px;
	}
	.split :global(.caret) {
		padding: 0 7px;
		border-left: 0;
		border-radius: 0 6px 6px 0;
	}
	.main.accent,
	.split :global(.caret.accent) {
		border-color: var(--primary);
		background: var(--primary);
		color: var(--primary-foreground);
	}
	.split :global(.caret.accent) {
		border-left: 1px solid color-mix(in srgb, var(--primary-foreground) 30%, var(--primary));
	}
	.main:hover:not(:disabled),
	.split :global(.caret:hover) {
		filter: brightness(1.08);
	}
	.main:disabled,
	.split :global(.caret:disabled) {
		opacity: 0.5;
	}
	.item {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
	.item b {
		font-weight: 500;
	}
	.item span {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.item span.reason {
		color: var(--sb-warning);
	}
</style>
