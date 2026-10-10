import { existsSync, readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

type Section = { id: string; name: string; group: string };

const sections = new URL('../../ui/universe_ui/qml/ui/Sections.js', import.meta.url);
const pages = new URL('../src/content/docs/docs/settings/', import.meta.url);

export const settingsSlug = (name: string) =>
    name
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-|-$/g, '');

export function settingsSidebar() {
    const list = runInNewContext(`${readFileSync(sections, 'utf8').replace('.pragma library', '')}\nlist`) as Section[];
    const problems: string[] = [];
    const items = list.map((s) => {
        const slug = settingsSlug(s.name);
        const file = ['mdx', 'md'].map((ext) => new URL(`${slug}.${ext}`, pages)).find((f) => existsSync(f));
        if (!file) problems.push(`Settings › ${s.name} (${s.id}) has no page: write src/content/docs/docs/settings/${slug}.mdx`);
        else {
            const title = /^title:\s*["']?(.+?)["']?\s*$/m.exec(readFileSync(file, 'utf8'))?.[1];
            if (title !== s.name) problems.push(`docs/settings/${slug}: title "${title}" is not the app's "${s.name}"`);
        }
        return { label: s.name, slug: `docs/settings/${slug}` };
    });
    if (problems.length) throw new Error(`The Settings pages and ui/universe_ui/qml/ui/Sections.js disagree:\n${problems.join('\n')}`);
    return [{ label: 'Overview', slug: 'docs/settings' }, ...items];
}
