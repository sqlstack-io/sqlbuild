<script lang="ts">
	import SearchIcon from '@lucide/svelte/icons/search';
	import ClearInputButton from '$lib/components/clear-input-button.svelte';

	// The log's filter, read like the selector syntax: `node:`, `kind:`, `level:`, `job:` and
	// `stream:` narrow the log, anything else is a free search. Typing lists the options that fit the
	// word at the caret; arrows move, Enter or Tab picks, Escape closes.
	export type FilterOption = { prefix: string; hint: string; values: string[] };

	let { value = $bindable(''), options }: { value?: string; options: FilterOption[] } = $props();

	let input = $state<HTMLInputElement>();
	let open = $state(false);
	let active = $state(0);

	/** The word being typed: the last one, unless the text ends in a space. */
	const word = $derived(/\s$/.test(value) ? '' : (value.split(/\s+/).pop() ?? ''));
	type Suggestion = { text: string; hint: string; done: boolean };
	const suggestions = $derived.by((): Suggestion[] => {
		const w = word.toLowerCase();
		const colon = w.indexOf(':');
		if (colon > 0) {
			const opt = options.find((o) => o.prefix === w.slice(0, colon));
			const part = w.slice(colon + 1);
			return (opt?.values ?? []).filter((v) => v.toLowerCase().includes(part)).slice(0, 12).map((v) => ({ text: `${opt!.prefix}:${v}`, hint: '', done: true }));
		}
		const prefixes = options.filter((o) => o.values.length && o.prefix.startsWith(w)).map((o) => ({ text: `${o.prefix}:`, hint: o.hint, done: false }));
		// A bare word also offers the matching nodes, the most common thing to look for.
		const nodes = w ? (options.find((o) => o.prefix === 'node')?.values ?? []).filter((v) => v.toLowerCase().includes(w)).slice(0, 6).map((v) => ({ text: `node:${v}`, hint: '', done: true })) : [];
		return [...prefixes, ...nodes];
	});

	function choose(s: Suggestion) {
		const head = /\s$/.test(value) ? value : value.slice(0, value.length - word.length);
		value = `${head}${s.text}${s.done ? ' ' : ''}`;
		active = 0;
		open = true;
		input?.focus();
	}

	/** Empties the filter and keeps the caret in it. */
	function clear() {
		value = '';
		open = false;
		input?.focus();
	}

	function onkeydown(e: KeyboardEvent) {
		// Esc clears the filter and keeps focus; an empty filter's Esc closes its options.
		if (e.key === 'Escape') {
			if (value) clear();
			else open = false;
			e.preventDefault();
			return;
		}
		if (!open || !suggestions.length) {
			if (e.key === 'ArrowDown') open = true;
			return;
		}
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			active = (active + (e.key === 'ArrowDown' ? 1 : -1) + suggestions.length) % suggestions.length;
		} else if ((e.key === 'Enter' || e.key === 'Tab') && word) {
			e.preventDefault();
			choose(suggestions[active]);
		}
	}
</script>

<div class="log-filter">
	<label class="field">
		<SearchIcon size={13} />
		<input
			bind:this={input}
			bind:value
			placeholder="Filter…  node:  kind:  level:"
			aria-label="Filter the log"
			autocomplete="off"
			spellcheck="false"
			oninput={() => ((open = true), (active = 0))}
			onfocus={() => (open = true)}
			onblur={() => setTimeout(() => (open = false), 120)}
			{onkeydown}
		/>
		{#if value}<ClearInputButton label="Clear the log filter" onclear={clear} />{/if}
	</label>
	{#if open && suggestions.length}
		<ul class="menu" role="listbox" aria-label="Filter options">
			{#each suggestions as s, i (s.text)}
				<li role="option" aria-selected={i === active}>
					<!-- raw-button: an option of the filter's dropdown -->
					<button class:on={i === active} onmousedown={(e) => (e.preventDefault(), choose(s))}><code>{s.text}</code>{#if s.hint}<span>{s.hint}</span>{/if}</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.log-filter {
		position: relative;
	}
	/* A fixed width: the text scrolls inside, and the ✕ appearing never resizes the box. */
	.field {
		box-sizing: border-box;
		width: 300px;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 3px 8px;
		border: 1px solid var(--border-subtle);
		border-radius: 2px;
		color: var(--sb-text-faint);
	}
	input {
		flex: 1;
		min-width: 0;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.menu {
		position: absolute;
		top: calc(100% + 4px);
		left: 0;
		z-index: 50;
		min-width: 100%;
		max-height: 280px;
		overflow: auto;
		padding: 4px;
		border: 1px solid var(--border);
		border-radius: 3px;
		background: var(--popover, var(--card));
		box-shadow: 0 6px 18px rgb(0 0 0 / 0.25);
	}
	.menu button {
		display: flex;
		width: 100%;
		align-items: baseline;
		gap: 10px;
		padding: 4px 8px;
		border-radius: 2px;
		text-align: left;
	}
	.menu button.on,
	.menu button:hover {
		background: var(--sb-hover);
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.menu span {
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
</style>
