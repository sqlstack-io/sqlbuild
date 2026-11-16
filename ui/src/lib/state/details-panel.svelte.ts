// A graph page's Details sidebar: closed until something is selected, then the person's own choice,
// remembered in this browser.

import { browser } from '$app/environment';
import { DETAILS_KEY_PREFIX, detailsOpen, parseChoice, toggledChoice, type DetailsChoice } from '$lib/lineage/details-panel';

/** `page` names the sidebar, such as `lineage`; `hasSelection` says whether anything is selected now. */
export function createDetailsPanel(page: string, hasSelection: () => boolean) {
	const key = `${DETAILS_KEY_PREFIX}${page}`;
	let choice = $state<DetailsChoice>(browser ? parseChoice(localStorage.getItem(key)) : null);
	return {
		get open() {
			return detailsOpen(choice, hasSelection());
		},
		toggle() {
			choice = toggledChoice(choice, hasSelection());
			if (browser) localStorage.setItem(key, choice);
		}
	};
}
