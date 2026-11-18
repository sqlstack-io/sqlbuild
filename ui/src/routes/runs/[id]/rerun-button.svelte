<script lang="ts">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import Button, { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import { cn } from '$lib/utils';
	import type { RunDetailState } from './state.svelte';

	let { view }: { view: RunDetailState } = $props();

	const primary = $derived(view.primary);
	// A split button: rerunning from the failure is the bar's solid primary action, any other rerun is outlined.
	const variant = $derived(primary?.mode === 'from_failure' ? 'default' : 'outline');
</script>

{#if primary}
	<div class="split">
		<span title={primary.disabledReason ?? primary.help}>
			<Button {variant} class="rounded-r-none" disabled={!!primary.disabledReason || view.busy !== null} aria-busy={view.busy === primary.mode} onclick={() => view.rerun(primary.mode)}>
				<RotateCwIcon />{primary.label}
			</Button>
		</span>
		<DropdownMenu.Root>
			<DropdownMenu.Trigger class={cn(buttonVariants({ variant, size: 'icon-bar' }), 'rounded-l-none', variant === 'default' ? 'border-l-primary-foreground/30' : 'border-l-0')} aria-label="More rerun options" disabled={view.busy !== null}>
				<ChevronDownIcon />
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
