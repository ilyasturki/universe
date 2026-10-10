import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The docs' link validator never reads the landing page, nor any link outside a page's Markdown. */
export function checkBuiltLinks(dir: URL) {
    const out = fileURLToPath(dir);
    const broken: string[] = [];
    const pages = readdirSync(out, { recursive: true, encoding: 'utf8' }).filter((f) => f.endsWith('.html'));
    for (const file of pages) {
        for (const [, url] of readFileSync(join(out, file), 'utf8').matchAll(/\s(?:href|src)="(\/[^/"][^"]*|\/)"/g)) {
            const path = decodeURI(url.replace(/[?#].*$/, ''));
            if (path.startsWith('/pagefind/') || path === '/sitemap-index.xml') continue;
            const candidates = path.endsWith('/') ? [join(out, path, 'index.html')] : [join(out, path), join(out, path, 'index.html')];
            if (!candidates.some((c) => existsSync(c))) broken.push(`${file}: ${url}`);
        }
    }
    if (broken.length) throw new Error(`Links to nothing in the built site:\n${[...new Set(broken)].join('\n')}`);
}

/** The custom domain, from `site`: site.yml's deploy checks it against the one Pages serves. */
export function writeCname(dir: URL, site: string) {
    writeFileSync(new URL('CNAME', dir), `${new URL(site).hostname}\n`);
}
