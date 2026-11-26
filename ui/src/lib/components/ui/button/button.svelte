<script lang="ts" module>
	import { cn, type WithElementRef } from "$lib/utils.js";
	import type { HTMLAnchorAttributes, HTMLButtonAttributes } from "svelte/elements";
	import { type VariantProps, tv } from "tailwind-variants";

	// The UI's one button. A small fixed set of types and sizes, so pages never style buttons by hand
	// (ui/src/lib/components/ui/button/no-adhoc-buttons.test.ts fails on a raw <button> elsewhere):
	//   default        solid blue primary; one per bar
	//   outline        quiet secondary action, transparent with a border
	//   accent         tinted blue, built like `stale`: the useful next action beside a status (Rerun on a run that didn't succeed)
	//   stale          tinted purple: "needs building" (Build N changed, Build what changed)
	//   stale-outline  outlined lighter purple: nodes only behind an upstream rebuild (Rebuild N)
	//   destructive    tinted red, built like `stale`: remove, revoke, disable
	//   ghost          no border or fill until hovered: icon-only and low-key actions
	//   link           a text action in running text or a list (Show all, Retry, reset); no box
	// Sizes: `bar` (30 px, 13 px medium text, 14 px icons) in top bars, toolbars and card headers;
	// `sm` (26 px) inside rows and tables; `icon-bar`/`icon-sm` for icon-only buttons, `icon-xs`
	// inside a chip or a field; `inline` for links.
	// Corners are the UI's square --radius (2 px); icons are Lucide, never typed symbols.
	export const buttonVariants = tv({
		base: "focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:aria-invalid:border-destructive/50 rounded-(--radius) border border-transparent bg-clip-padding font-medium focus-visible:ring-3 aria-invalid:ring-3 [&_svg:not([class*='size-'])]:size-3.5 group/button inline-flex shrink-0 items-center justify-center whitespace-nowrap transition-colors outline-none select-none disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0",
		variants: {
			variant: {
				default: "bg-primary text-primary-foreground hover:brightness-108",
				outline: "border-input bg-transparent text-foreground hover:bg-(--sb-hover) aria-expanded:bg-(--sb-hover) aria-pressed:bg-(--sb-hover)",
				accent: "border-primary/45 bg-primary/14 text-primary hover:bg-primary/22 aria-expanded:bg-primary/22",
				// Purple means "needs building": tinted for building changed nodes, outlined for nodes only behind.
				// Execute's solid blue stays the bar's one solid button.
				stale: "border-(--sb-stale)/45 bg-(--sb-stale)/14 text-(--sb-stale) hover:bg-(--sb-stale)/22",
				"stale-outline": "border-(--sb-stale) bg-transparent text-(--sb-stale) hover:bg-(--sb-stale)/10",
				destructive: "border-(--sb-error)/45 bg-(--sb-error)/10 text-(--sb-error) hover:bg-(--sb-error)/18",
				ghost: "bg-transparent text-muted-foreground hover:bg-(--sb-hover) hover:text-foreground aria-expanded:text-foreground aria-pressed:bg-(--sb-hover) aria-pressed:text-foreground",
				link: "border-0 bg-transparent text-primary hover:underline underline-offset-2",
			},
			size: {
				bar: "h-[30px] gap-1.5 px-[11px] text-[13px] tracking-[.005em]",
				sm: "h-[26px] gap-1.5 px-2 text-[12.5px]",
				"icon-bar": "size-[30px]",
				"icon-sm": "size-[26px]",
				// Inside a chip or a search field: removing a filter, clearing the text.
				"icon-xs": "size-5 [&_svg:not([class*='size-'])]:size-3",
				// A link sized by the text around it, never larger than the UI's 13 px text: placed beside
				// rows in a container without a text size of its own, it would otherwise take the
				// browser's 14 px and stand out from the 12.5 px rows it sits with.
				inline: "h-auto gap-1 p-0 text-[length:min(1em,13px)] font-normal",
			},
		},
		defaultVariants: {
			variant: "outline",
			size: "bar",
		},
	});

	export type ButtonVariant = VariantProps<typeof buttonVariants>["variant"];
	export type ButtonSize = VariantProps<typeof buttonVariants>["size"];

	export type ButtonProps = WithElementRef<HTMLButtonAttributes> &
		WithElementRef<HTMLAnchorAttributes> & {
			variant?: ButtonVariant;
			size?: ButtonSize;
		};
</script>

<script lang="ts">
	let {
		class: className,
		variant = "outline",
		size = "bar",
		ref = $bindable(null),
		href = undefined,
		type = "button",
		disabled,
		children,
		...restProps
	}: ButtonProps = $props();
</script>

{#if href}
	<a
		bind:this={ref}
		data-slot="button"
		class={cn(buttonVariants({ variant, size }), className)}
		href={disabled ? undefined : href}
		aria-disabled={disabled}
		role={disabled ? "link" : undefined}
		tabindex={disabled ? -1 : undefined}
		{...restProps}
	>
		{@render children?.()}
	</a>
{:else}
	<button
		bind:this={ref}
		data-slot="button"
		class={cn(buttonVariants({ variant, size }), className)}
		{type}
		{disabled}
		{...restProps}
	>
		{@render children?.()}
	</button>
{/if}
