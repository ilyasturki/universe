import { readFileSync } from 'node:fs';
import starlight from '@astrojs/starlight';
import type { AstroIntegration } from 'astro';
import { defineConfig } from 'astro/config';
import starlightLinksValidator from 'starlight-links-validator';
import { checkBuiltLinks, writeCname } from './scripts/built';
import { generate, generatedFrom } from './scripts/generate';
import { settingsSidebar } from './scripts/settings';

const site = 'https://universelauncher.com';
const repo = 'https://github.com/ilyasturki/universe';
const cargo = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8');
const version = /^\[workspace\.package\][^[]*?^version = "([^"]+)"/ms.exec(cargo)?.[1] ?? 'unknown';
const umami = process.env.UMAMI_WEBSITE_ID;

const universe: AstroIntegration = {
    name: 'universe',
    hooks: {
        'astro:config:setup': ({ command, addWatchFile, logger }) => {
            if (command === 'preview') return;
            generate(logger);
            for (const file of generatedFrom) addWatchFile(file);
        },
        'astro:build:done': ({ dir }) => {
            checkBuiltLinks(dir);
            writeCname(dir, site);
        },
    },
};

export default defineConfig({
    site,
    trailingSlash: 'always',
    vite: {
        define: { __UNIVERSE_VERSION__: JSON.stringify(version), __UNIVERSE_REPO__: JSON.stringify(repo), __UMAMI_WEBSITE_ID__: JSON.stringify(umami ?? '') },
        server: { fs: { allow: ['..'] } },
    },
    integrations: [
        universe,
        starlight({
            title: 'Universe',
            description: 'A game launcher for Linux you drive with a controller, from the couch or on a Steam Deck.',
            logo: { light: '../brand/universe-lockup.svg', dark: '../brand/universe-lockup-dark.svg', replacesTitle: true },
            favicon: '/favicon.svg',
            social: [{ icon: 'github', label: 'GitHub', href: repo }],
            editLink: { baseUrl: `${repo}/edit/main/site/` },
            lastUpdated: true,
            customCss: ['@fontsource-variable/archivo', './src/styles/docs.css'],
            components: { SiteTitle: './src/components/SiteTitle.astro' },
            head: umami
                ? [
                      {
                          tag: 'script',
                          attrs: {
                              src: 'https://analytics.ilyasturki.com/script.js',
                              'data-website-id': umami,
                              'data-domains': new URL(site).hostname,
                              defer: true,
                          },
                      },
                  ]
                : [],
            plugins: [starlightLinksValidator()],
            sidebar: [
                { label: 'Overview', slug: 'docs' },
                { label: 'Getting started', items: [{ autogenerate: { directory: 'docs/getting-started' } }] },
                { label: 'Using Universe', items: [{ autogenerate: { directory: 'docs/using' } }] },
                { label: 'Your library', items: [{ autogenerate: { directory: 'docs/library' } }] },
                { label: 'Running games', items: [{ autogenerate: { directory: 'docs/running' } }] },
                { label: 'Controllers', items: [{ autogenerate: { directory: 'docs/controllers' } }] },
                { label: 'Settings', collapsed: true, items: settingsSidebar() },
                { label: 'Add-ons & modules', items: [{ autogenerate: { directory: 'docs/add-ons' } }] },
                {
                    label: 'Reference',
                    collapsed: true,
                    items: [
                        { label: 'CLI', collapsed: true, items: [{ autogenerate: { directory: 'docs/reference/cli' } }] },
                        'docs/reference/launch-keys',
                        'docs/reference/runners',
                        'docs/reference/config',
                        'docs/reference/nix',
                    ],
                },
                { label: 'Developers', collapsed: true, items: [{ autogenerate: { directory: 'docs/developers' } }] },
                { label: 'Changelog', slug: 'docs/changelog' },
            ],
        }),
    ],
});
