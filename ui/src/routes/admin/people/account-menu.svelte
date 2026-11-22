<script lang="ts">
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import type { Account } from '$lib/api';
	import ConfirmDialog from '$lib/components/platform/confirm-dialog.svelte';
	import { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { app } from '$lib/state/app.svelte';
	import { setAccountEnabled } from './api';
	import { accountToggle } from './utils';

	// A person's actions behind a ⋯ menu; disabling asks first. Nobody disables themselves.
	let { account, onchange }: { account: Account; onchange: () => void } = $props();

	let asking = $state(false);
	const question = $derived(accountToggle(account));
</script>

{#if app.can('user.manage') && account.name !== app.user?.name}
	<DropdownMenu.Root>
		<DropdownMenu.Trigger class={buttonVariants({ variant: 'ghost', size: 'icon-sm' })} aria-label="More actions for {account.displayName}" data-testid="person-menu"><EllipsisIcon size={15} /></DropdownMenu.Trigger>
		<DropdownMenu.Content align="end" class="w-44">
			<DropdownMenu.Item onSelect={() => (asking = true)}>{account.disabled ? 'Enable…' : 'Disable…'}</DropdownMenu.Item>
		</DropdownMenu.Content>
	</DropdownMenu.Root>
	{#if asking}
		<ConfirmDialog open title={question.title} body={question.body} confirm={question.confirm} destructive={question.destructive} onconfirm={() => setAccountEnabled(account).then(onchange)} onclose={() => (asking = false)} />
	{/if}
{/if}
