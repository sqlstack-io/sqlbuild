// Mock-up: looks for the run header's actions, switchable with ?actions=1|2|3|4; 4 is the default and
// ?actions=0 is the look before the mock-up. Pure.

export type ActionLook = 'before' | '1' | '2' | '3' | '4';

export function actionLook(param: string | null): ActionLook {
	return param === '0' ? 'before' : param === '1' || param === '2' || param === '3' ? param : '4';
}

const TINTED = 'border-primary/45 bg-primary/14 text-primary hover:bg-primary/22';
const QUIET = 'border-transparent text-muted-foreground hover:text-primary focus-visible:text-primary';

/** Extra classes on the outline (or ghost) buttons, from existing tokens only. */
export const ACTION_CLASS: Record<'rerun' | 'details', Record<ActionLook, string>> = {
	rerun: {
		before: '',
		// 1: both outline, in the display font, the same chevron.
		'1': 'font-[family-name:var(--font-display)] tracking-[.01em]',
		// 2: the primary action tinted in the accent, as the `stale` variant tints in purple.
		'2': TINTED,
		// 3: quiet icon and label; colour only on hover or focus.
		'3': QUIET,
		// 4: Rerun tinted as in 2, Details quiet as in 3.
		'4': TINTED
	},
	details: {
		before: '',
		'1': 'font-[family-name:var(--font-display)] tracking-[.01em]',
		'2': '',
		'3': QUIET,
		'4': QUIET
	}
};
