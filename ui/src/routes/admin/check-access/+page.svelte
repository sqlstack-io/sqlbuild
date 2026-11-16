<script lang="ts">
	import PageBody from '$lib/components/page-body.svelte';
	import { to } from '$lib/paths';
	import { page } from '$app/state';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import AccessChecker from '$lib/components/admin/access-checker.svelte';
	import { CAPABILITIES } from '$lib/api';
	import { isCapability, type CheckInput } from '$lib/platform/access';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const canCheckOthers = $derived(app.can('role.assign') || app.can('audit.view'));
	const accounts = createResource((client) => (app.can('user.manage') ? client.listAccounts() : Promise.resolve([])));

	const initial = $derived.by((): CheckInput | null => {
		const q = page.url.searchParams;
		const action = q.get('action');
		const input: CheckInput = {
			principal: q.get('principal') ?? undefined,
			action: isCapability(action, CAPABILITIES) ? action : undefined,
			locationId: q.get('location') ?? undefined,
			subject: q.get('subject') ?? undefined,
			decision: q.get('decision') ?? undefined
		};
		return Object.values(input).some(Boolean) ? input : null;
	});
</script>

<AppTopbar title="Check access" />

<PageBody padding="20px 24px 32px">
	<div class="max-w-[1100px]" title={canCheckOthers ? undefined : 'Checking others needs “Give roles” or “See the audit log”'}>
		<AccessChecker principals={canCheckOthers ? (accounts.data ?? []) : []} {initial} />
	</div>
	<p class="back"><a href={to(`/admin/people`)}>← People</a></p>
</PageBody>

<style>
	.back {
		margin-top: 24px;
		font-size: 12.5px;
	}
	a {
		color: var(--primary);
	}
</style>
