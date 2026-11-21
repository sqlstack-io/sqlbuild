/** The browser tab's title: the page's subject, or its section when it has none, then `SQLBuild`. */
export function pageTitle(section: string, subject?: string | null): string {
	return [subject?.trim() || section, 'SQLBuild'].join(' · ');
}
