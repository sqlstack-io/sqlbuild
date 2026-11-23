// Mockup switch: `?cards=new` draws graph cards with staged levels of detail and no kind word.
// Without it every graph draws as before.

import { page } from '$app/state';

export type CardsMode = 'today' | 'new';

export function cardsMode(): CardsMode {
	return page.url.searchParams.get('cards') === 'new' ? 'new' : 'today';
}
