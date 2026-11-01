import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightLlmsTxt from 'starlight-llms-txt';
import { sqlbuildDark, sqlbuildLight } from './src/code-themes.mjs';
import { redirects, sidebar } from './src/navigation.mjs';

export default defineConfig({
	site: 'https://sqlbuild.com',
	redirects,
	integrations: [
		starlight({
			title: 'SQLBuild',
			description: 'Verify early, test properly, and refactor safely.',
			logo: {
				light: './src/assets/logo-light.png',
				dark: './src/assets/logo-dark.png',
				replacesTitle: true,
			},
			favicon: '/favicon.png',
			head: [
				{ tag: 'link', attrs: { rel: 'stylesheet', href: '/brand.css' } },
				{ tag: 'meta', attrs: { property: 'og:image', content: 'https://sqlbuild.com/assets/og-sqlbuild-docs.png' } },
				{ tag: 'meta', attrs: { property: 'og:image:width', content: '1200' } },
				{ tag: 'meta', attrs: { property: 'og:image:height', content: '630' } },
				{ tag: 'meta', attrs: { name: 'twitter:image', content: 'https://sqlbuild.com/assets/og-sqlbuild-docs.png' } },
			],
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/sqlstack-io/sqlbuild' },
				{ icon: 'discord', label: 'Discord', href: 'https://discord.gg/tYU4nXdsT' },
			],
			components: {
				PageFrame: './src/components/PageFrame.astro',
				TwoColumnContent: './src/components/TwoColumnContent.astro',
				Header: './src/components/Header.astro',
				PageTitle: './src/components/PageTitle.astro',
				SiteTitle: './src/components/SiteTitle.astro',
				Pagination: './src/components/Pagination.astro',
				ThemeSelect: './src/components/ThemeSelect.astro',
				Footer: './src/components/Footer.astro',
			},
			customCss: ['./src/styles/fonts.css', './src/styles/sqlbuild.css'],
			expressiveCode: {
				themes: [sqlbuildDark, sqlbuildLight],
				defaultProps: {
					// Plain boxes for shell snippets, without the fake terminal title bar.
					overridesByLang: { 'bash,sh,shell,zsh,console,powershell': { frame: 'none' } },
				},
				styleOverrides: {
					borderRadius: '2px',
					borderWidth: '1px',
					codeFontFamily: 'var(--sl-font-mono)',
					codeFontSize: '13px',
					codeLineHeight: '1.65',
					uiFontFamily: 'var(--sl-font)',
					frames: {
						shadowColor: 'transparent',
						inlineButtonBorder: 'transparent',
						inlineButtonBackgroundIdleOpacity: '0',
						inlineButtonBackgroundHoverOrFocusOpacity: '0.08',
						inlineButtonForeground: 'var(--sqb-muted)',
						frameBoxShadowCssValue: 'none',
						editorTabBarBackground: 'var(--sqb-code-bg)',
						editorActiveTabBackground: 'var(--sqb-code-bg)',
						editorActiveTabIndicatorTopColor: 'var(--sqb-primary)',
						editorTabBarBorderBottomColor: 'var(--sqb-line)',
						terminalTitlebarBackground: 'var(--sqb-code-bg)',
						terminalTitlebarBorderBottomColor: 'var(--sqb-line)',
						terminalTitlebarDotsOpacity: '0.35',
						terminalBackground: 'var(--sqb-code-bg)',
					},
					borderColor: 'var(--sqb-line)',
				},
			},
			sidebar,
			plugins: [
				starlightLlmsTxt({
					projectName: 'SQLBuild',
					description:
						'SQLBuild is a free, open-source framework for building SQL and Python data pipelines, with compile-time checks, tests, plans and diffs.',
				}),
			],
		}),
	],
});
