<script lang="ts">
	import logoForDark from '$website/src/assets/logo-dark.png';
	import logoForLight from '$website/src/assets/logo-light.png';
	import { page } from '$app/state';
	import { loginWithPassword } from '$lib/api/http-client';
	import { safeNext } from '$lib/next';

	let username = $state('');
	let password = $state('');
	let busy = $state(false);
	let problem = $state<string | null>(null);

	const problems = {
		rejected: 'That name and password did not match.',
		throttled: 'Too many attempts. Wait a minute, then try again.',
		unavailable: "Couldn't reach the SQLBuild server. Try again."
	} as const;

	/** Only same-site paths, so a crafted link cannot send people elsewhere after sign-in. */
	function destination(): string {
		return safeNext(page.url.searchParams.get('next'));
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		problem = null;
		const outcome = await loginWithPassword(username, password).catch(() => 'unavailable' as const);
		busy = false;
		if (outcome === 'ok') {
			window.location.assign(destination());
			return;
		}
		password = '';
		problem = problems[outcome];
	}
</script>

<svelte:head><title>Sign in · SQLBuild</title></svelte:head>

<div class="grid h-screen place-items-center p-8">
	<form class="flex w-full max-w-xs flex-col gap-3" onsubmit={submit}>
		<img src={logoForLight} alt="SQLBuild" class="mb-2 h-7 w-auto self-start dark:hidden" />
		<img src={logoForDark} alt="SQLBuild" class="mb-2 hidden h-7 w-auto self-start dark:block" />
		<h1 class="text-[15px] font-semibold">Sign in</h1>
		<label class="flex flex-col gap-1 text-[12.5px]">
			Name
			<input class="rounded-md border border-border bg-transparent px-2.5 py-1.5 text-[13px]" name="username" autocomplete="username" required bind:value={username} />
		</label>
		<label class="flex flex-col gap-1 text-[12.5px]">
			Password
			<input class="rounded-md border border-border bg-transparent px-2.5 py-1.5 text-[13px]" name="password" type="password" autocomplete="current-password" required bind:value={password} />
		</label>
		{#if problem}<p class="text-[12.5px]" style:color="var(--sb-error)" role="alert">{problem}</p>{/if}
		<button class="bg-primary rounded-md px-3 py-1.5 text-[13px] font-medium text-white disabled:opacity-60" disabled={busy}>{busy ? 'Signing in…' : 'Sign in'}</button>
	</form>
</div>
