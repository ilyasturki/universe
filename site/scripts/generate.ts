import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, posix } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { AstroIntegrationLogger } from 'astro';

const root = fileURLToPath(new URL('../../', import.meta.url));
const docs = fileURLToPath(new URL('../src/content/docs/docs/', import.meta.url));
const repo = 'https://github.com/ilyasturki/universe';
const strict = process.env.DOCS_STRICT === '1' || process.env.CI === 'true';

const developers = [
    { file: 'api.md', slug: 'api', title: 'Core API', description: 'The core, the CLI, config, the module and source protocols.' },
    { file: 'frontends.md', slug: 'frontends', title: 'Frontends', description: 'How the UI is built, and how to write another frontend.' },
    { file: 'extensions.md', slug: 'extensions', title: 'Extensions', description: 'Modules, sources and themes that do not ship with Universe.' },
];

export const generatedFrom = [...developers.map((d) => `docs/${d.file}`), 'CHANGELOG.md', 'nix/home-manager.nix', 'nix/nixos.nix'].map((f) => join(root, f));

type Front = {
    title: string;
    description?: string;
    editUrl?: string | false;
    lastUpdated?: string | false;
    sidebar?: { label?: string; order?: number };
    tableOfContents?: { maxHeadingLevel: number };
};

function yaml(value: unknown, indent = ''): string {
    return Object.entries(value as Record<string, unknown>)
        .filter(([, v]) => v !== undefined)
        .map(([k, v]) => {
            if (v !== null && typeof v === 'object') return `${indent}${k}:\n${yaml(v, `${indent}  `)}`;
            // A bare YYYY-MM-DD is a date to the frontmatter parser, which lastUpdated needs.
            if (k === 'lastUpdated' && typeof v === 'string') return `${indent}${k}: ${v}\n`;
            return `${indent}${k}: ${typeof v === 'string' ? JSON.stringify(v) : String(v)}\n`;
        })
        .join('');
}

function page(path: string, front: Front, body: string) {
    const file = join(docs, path);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, `---\n${yaml(front)}---\n\n${body.trim()}\n`);
}

function changed(file: string): string | undefined {
    try {
        return execFileSync('git', ['log', '-1', '--format=%cs', '--', file], { cwd: root, encoding: 'utf8' }).trim() || undefined;
    } catch {
        return undefined;
    }
}

function missing(logger: AstroIntegrationLogger, what: string, how: string): string {
    const message = `${what} is generated at build time and this build could not: ${how}`;
    if (strict) throw new Error(message);
    logger.warn(message);
    return `:::caution\nNot generated in this build: ${how}\n:::`;
}

/** Outside code, `<id>` would render as an unknown HTML tag and disappear. */
function escapeTags(markdown: string): string {
    const blocks: { code: boolean; lines: string[] }[] = [];
    let fence = '';
    for (const line of markdown.split('\n')) {
        const opener = /^\s*(`{3,}|~{3,})/.exec(line)?.[1];
        const code = Boolean(fence || opener);
        if (fence && opener?.startsWith(fence)) fence = '';
        else if (!fence && opener) fence = opener;
        const last = blocks.at(-1);
        if (last && last.code === code && (code || line.trim())) last.lines.push(line);
        else blocks.push({ code, lines: [line] });
    }
    // A code span can wrap across the lines of its paragraph, so spans are matched paragraph by paragraph.
    const outsideSpans = (text: string) =>
        text
            .split(/((`+)[\s\S]*?\2)/)
            .map((part, i) => (i % 3 === 0 ? part.replace(/<(?!!--|https?:|mailto:)(?=[A-Za-z/])/g, '&lt;') : i % 3 === 1 ? part : ''))
            .join('');
    return blocks.map((b) => (b.code ? b.lines.join('\n') : outsideSpans(b.lines.join('\n')))).join('\n');
}

function rewriteLinks(markdown: string, from: string): string {
    return markdown.replace(/\]\(([^)\s]+)\)/g, (link, target: string) => {
        if (/^([a-z]+:|#|\/)/.test(target)) return link;
        const [path, hash] = target.split('#');
        const resolved = posix.normalize(posix.join(posix.dirname(from), path));
        const doc = developers.find((d) => `docs/${d.file}` === resolved);
        const url = doc ? `/docs/developers/${doc.slug}/` : `${repo}/blob/main/${resolved}`;
        return `](${url}${hash ? `#${hash}` : ''})`;
    });
}

function withoutTitle(markdown: string): string {
    return markdown.replace(/^# .*\n+/, '');
}

function developerPages() {
    developers.forEach((d, i) => {
        const source = `docs/${d.file}`;
        const body = rewriteLinks(escapeTags(withoutTitle(readFileSync(join(root, source), 'utf8'))), source);
        page(
            `developers/${d.slug}.md`,
            { title: d.title, description: d.description, editUrl: `${repo}/edit/main/${source}`, lastUpdated: changed(source), sidebar: { order: i } },
            body,
        );
    });
}

function changelogPage() {
    const body = withoutTitle(readFileSync(join(root, 'CHANGELOG.md'), 'utf8'));
    page(
        'changelog.md',
        { title: 'Changelog', description: 'What each Universe release changed.', editUrl: false, lastUpdated: changed('CHANGELOG.md') },
        `What each release changed. The app shows the same list under Settings › About.\n\n${body}`,
    );
}

function sealed(): { env: NodeJS.ProcessEnv; done: () => void } {
    const home = mkdtempSync(join(tmpdir(), 'universe-site-'));
    const env: NodeJS.ProcessEnv = Object.fromEntries(Object.entries(process.env).filter(([k]) => !k.startsWith('UNIVERSE_')));
    for (const [variable, dir] of [
        ['UNIVERSE_DATA_HOME', 'data'],
        ['UNIVERSE_CONFIG_HOME', 'config'],
        ['UNIVERSE_STATE_HOME', 'state'],
        ['UNIVERSE_CACHE_HOME', 'cache'],
    ]) {
        mkdirSync(join(home, dir));
        env[variable] = join(home, dir);
    }
    env.UNIVERSE_MODULES_PATH = join(root, 'modules');
    env.UNIVERSE_SOURCES_PATH = join(root, 'sources');
    env.HOME = home;
    return { env, done: () => rmSync(home, { recursive: true, force: true }) };
}

type LaunchKey = {
    key: string;
    type: string;
    default: unknown;
    choices: string[];
    label: string;
    section: string;
    scope: 'game' | 'global' | 'both';
    runners: string[];
    description: string;
    advanced: boolean;
};

type Runner = {
    id: string;
    name: string;
    kind: string;
    aliases: string[];
    binaries: string[];
    extensions: string[];
    platforms: string[];
    options: { key: string; label: string; type: string; default: unknown; choices: string[] }[];
};

const code = (value: unknown) => `\`${typeof value === 'string' ? value || '""' : JSON.stringify(value)}\``;

function cliPages(bin: string, env: NodeJS.ProcessEnv) {
    const out = mkdtempSync(join(tmpdir(), 'universe-cli-'));
    try {
        execFileSync(bin, ['__generate', '--markdown', out], { env });
        const commands: string[] = [];
        const top = withoutTitle(readFileSync(join(out, 'universe.md'), 'utf8')).replace(/^- `([a-z][a-z0-9-]*)`:/gm, (_, name: string) => {
            commands.push(name);
            return `- [\`universe ${name}\`](/docs/reference/cli/${name}/):`;
        });
        page(
            'reference/cli/index.md',
            { title: 'CLI', description: 'Every universe command, as the binary describes it.', editUrl: false, sidebar: { label: 'Overview', order: 0 } },
            top,
        );
        for (const file of readdirSync(out).filter((f) => f !== 'universe.md')) {
            const name = file.replace(/\.md$/, '');
            const markdown = readFileSync(join(out, file), 'utf8');
            const about = /^# .*\n+([^\n`]+)/.exec(markdown)?.[1]?.replace(/\\(.)/g, '$1');
            const order = commands.indexOf(name) + 1;
            page(
                `reference/cli/${name}.md`,
                { title: `universe ${name}`, description: about, editUrl: false, sidebar: { label: name, order: order || commands.length + 1 } },
                withoutTitle(markdown),
            );
        }
    } finally {
        rmSync(out, { recursive: true, force: true });
    }
}

function launchKeysPage(keys: LaunchKey[], runners: Runner[]) {
    const names = new Map(runners.map((r) => [r.id, r.name]));
    const sections = [...new Set(keys.map((k) => k.section))];
    const body = sections.map((section) => {
        const entries = keys
            .filter((k) => k.section === section)
            .map((k) => {
                const kinds = k.runners.map((r) => names.get(r) ?? r);
                const home = kinds.length ? `Settings \u203a Emulators & Proton \u203a ${kinds.join(' or ')}` : 'Settings \u203a Game defaults';
                const facts = [`Key ${code(k.key)}`, `default ${code(k.default)}`];
                // An enum or a toggle takes only its choices; a size, a rate or a number takes any value, the choices being suggestions.
                if (k.choices.length && ['enum', 'toggle'].includes(k.type)) facts.push(`one of ${k.choices.map(code).join(', ')}`);
                else if (k.choices.length) facts.push(`such as ${k.choices.slice(0, 4).map(code).join(', ')}`);
                const where = {
                    game: 'Set per game, in its Game settings.',
                    global: `Set for every game, in ${home}.`,
                    both: `Set per game, over a default in ${home}.`,
                }[k.scope];
                const only = kinds.length ? ` Only ${kinds.join(' and ')} games read it.` : '';
                return `### ${k.label}\n\n${escapeTags(k.description)}\n\n${facts.join(' · ')}. ${where}${only}${k.advanced ? ' Shown with Show advanced.' : ''}`;
            });
        return `## ${section}\n\n${entries.join('\n\n')}`;
    });
    page(
        'reference/launch-keys.md',
        {
            title: 'Launch keys',
            description: 'Every setting a launch reads, with its key, default and scope.',
            editUrl: false,
            sidebar: { order: 1 },
            tableOfContents: { maxHeadingLevel: 2 },
        },
        [
            'Every setting a launch reads, as `universe launch-keys` lists them. A game\u2019s own value, set in its Game settings or with `universe set <game> key=value`, wins over the default every game shares, set in Settings or with `universe config set launch.key value`. [Launch settings](/docs/running/launch-settings/) explains how the pages show them.',
            ...body,
        ].join('\n\n'),
    );
}

function runnersPage(runners: Runner[]) {
    const kinds = [
        { kind: 'proton', title: 'Proton' },
        { kind: 'wine', title: 'Wine' },
        { kind: 'linux', title: 'Linux' },
        { kind: 'emulator', title: 'Emulators' },
    ];
    const describe = (r: Runner) => {
        const facts = [`Id ${code(r.id)}`];
        if (r.aliases.length) facts.push(`also ${r.aliases.map(code).join(', ')}`);
        if (r.platforms.length) facts.push(`runs ${r.platforms.map((p) => p.charAt(0).toUpperCase() + p.slice(1)).join(', ')} games`);
        if (r.binaries.length) facts.push(`looks for ${r.binaries.map(code).join(', ')}`);
        if (r.extensions.length) facts.push(`opens ${r.extensions.map((e) => code(`.${e}`)).join(' ')}`);
        const options = r.options.map(
            (o) => `- ${o.label}: ${code(o.key)}, default ${code(o.default)}${o.choices.length ? `, one of ${o.choices.map(code).join(', ')}` : ''}`,
        );
        return `${facts.join(' · ')}.${options.length ? `\n\nOptions (\`universe runner set ${r.id} <key>=<value>\`):\n\n${options.join('\n')}` : ''}`;
    };
    const body = kinds.flatMap(({ kind, title }) => {
        const group = runners.filter((r) => r.kind === kind);
        if (!group.length) return [];
        if (kind !== 'emulator') return [`## ${title}\n\n${group.map(describe).join('\n\n')}`];
        return [`## ${title}\n\n${group.map((r) => `### ${r.name}\n\n${describe(r)}`).join('\n\n')}`];
    });
    page(
        'reference/runners.md',
        { title: 'Runners', description: 'What starts a game: Proton, Wine, Linux and each emulator.', editUrl: false, sidebar: { order: 2 } },
        [
            'What starts a game, as `universe runner ls` lists them. A game\u2019s runner is set in its Game settings or with `universe set <game> runner=<id>`.',
            ...body,
        ].join('\n\n'),
    );
}

function configPage(logger: AstroIntegrationLogger) {
    const api = readFileSync(join(root, 'docs/api.md'), 'utf8');
    const block = /^## config\.toml\n[\s\S]*?```toml\n([\s\S]*?)```/m.exec(api)?.[1];
    if (!block) {
        page(
            'reference/config.md',
            { title: 'config.toml', editUrl: false, sidebar: { order: 3 } },
            missing(logger, 'The config.toml reference', 'docs/api.md has no ```toml block under its ## config.toml heading'),
        );
        return;
    }
    type Row = { key: string; value: string; note: string; unset: boolean };
    const sections: { name: string; note: string; rows: Row[] }[] = [{ name: '', note: '', rows: [] }];
    for (const line of block.split('\n')) {
        const [setting, note = ''] = splitComment(line.replace(/^# (?=\[|[a-z_][\w-]* =)/, (m) => m.replace('# ', '\u0000')));
        const unset = setting.startsWith('\u0000');
        const text = setting.replace('\u0000', '').trim();
        const header = /^\[{1,2}([^\]]+)\]{1,2}$/.exec(text);
        if (header) sections.push({ name: header[1], note: unset ? `${note} (none by default)`.trim() : note, rows: [] });
        else if (/^[a-z_][\w-]* =/.test(text)) {
            const [key, ...value] = text.split(' = ');
            sections[sections.length - 1].rows.push({ key, value: value.join(' = '), note, unset });
        } else if (note && !text) sections[sections.length - 1].rows.push({ key: '', value: '', note, unset: false });
    }
    const cell = (s: string) => escapeTags(s).replace(/\|/g, '\\|');
    const body = sections
        .filter((s) => s.name)
        .map((s) => {
            const rows = s.rows.filter((r) => r.key);
            const table = rows.length
                ? `| Key | Default | |\n| --- | --- | --- |\n${rows.map((r) => `| \`${r.key}\` | ${r.unset ? 'unset' : `\`${r.value.replace(/\|/g, '\\|')}\``} | ${cell(r.unset && r.value ? `${r.note} (for example \`${r.value}\`)` : r.note)} |`).join('\n')}`
                : '';
            return [`## [${s.name}]`, cell(s.note), table].filter(Boolean).join('\n\n');
        });
    page(
        'reference/config.md',
        {
            title: 'config.toml',
            description: 'Every key of ~/.config/universe/config.toml with its default.',
            editUrl: `${repo}/edit/main/docs/api.md`,
            lastUpdated: changed('docs/api.md'),
            sidebar: { order: 3 },
            tableOfContents: { maxHeadingLevel: 2 },
        },
        [
            '`~/.config/universe/config.toml` holds the settings every game shares. The settings pages and `universe config set` write it; a key left out keeps its default. A game\u2019s own settings live in its `game.toml` (see [Launch keys](/docs/reference/launch-keys/)).',
            ...body,
            `## The whole file\n\n\`\`\`toml\n${block}\`\`\``,
        ].join('\n\n'),
    );
}

function splitComment(line: string): [string, string?] {
    let quote = '';
    for (let i = 0; i < line.length; i++) {
        const c = line[i];
        if (quote) {
            if (c === quote) quote = '';
        } else if (c === '"' || c === "'") quote = c;
        else if (c === '#') return [line.slice(0, i), line.slice(i + 1).trim()];
    }
    return [line];
}

type NixOption = { name: string; description: string | null; type: string; default?: { text: string }; example?: { text: string } };

function nixPage(logger: AstroIntegrationLogger) {
    const front: Front = { title: 'Nix options', description: 'The Home Manager and NixOS modules\u2019 options.', editUrl: false, sidebar: { order: 4 } };
    let options: { 'home-manager': NixOption[]; nixos: NixOption[] };
    try {
        const expr = `import ${fileURLToPath(new URL('options.nix', import.meta.url))} { root = ${JSON.stringify(root.replace(/\/$/, ''))}; }`;
        const out = execFileSync('nix', ['eval', '--json', '--impure', '--expr', expr], {
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'pipe'],
        });
        options = JSON.parse(out);
    } catch (e) {
        const stderr = String((e as { stderr?: unknown }).stderr ?? e)
            .trim()
            .split('\n')
            .slice(-3)
            .join(' ');
        page('reference/nix.md', front, missing(logger, 'The Nix options page', `\`nix eval\` of the modules failed: ${stderr}`));
        return;
    }
    const enableFirst = (o: NixOption) => (o.name.endsWith('.enable') ? o.name.split('.').length : 99);
    const render = (list: NixOption[]) =>
        list
            .toSorted((a, b) => enableFirst(a) - enableFirst(b) || a.name.localeCompare(b.name))
            .map((o) => {
                const facts = [`Type: ${o.type}`];
                const flakePackage = /^<derivation ([^>]+)>$/.exec(o.default?.text ?? '')?.[1];
                if (flakePackage) facts.push(`default this flake’s \`${flakePackage}\``);
                else if (o.default) facts.push(`default ${code(o.default.text)}`);
                return `### ${o.name.replace(/^programs\.universe\./, '')}\n\n\`${o.name}\`\n\n${escapeTags(o.description ?? '')}\n\n${facts.join(' · ')}.${o.example ? `\n\nExample:\n\n\`\`\`nix\n${o.example.text}\n\`\`\`` : ''}`;
            })
            .join('\n\n');
    page(
        'reference/nix.md',
        front,
        [
            'The flake\u2019s modules, as `nix/home-manager.nix` and `nix/nixos.nix` declare them. [Install](/docs/getting-started/install/#nix) shows where each one goes.',
            `## Home Manager\n\n\`imports = [ inputs.universe.homeModules.default ];\`\n\n${render(options['home-manager'])}`,
            `## NixOS\n\n\`imports = [ inputs.universe.nixosModules.default ];\`\n\n${render(options.nixos)}`,
        ].join('\n\n'),
    );
}

function referencePages(logger: AstroIntegrationLogger) {
    const bin = process.env.UNIVERSE_BIN;
    const env = sealed();
    try {
        if (!bin || !existsSync(bin)) {
            const stub = missing(
                logger,
                'The CLI, launch keys and runners pages',
                `UNIVERSE_BIN names no universe binary (${bin ?? 'unset'}); \`just site\` builds one`,
            );
            page('reference/cli/index.md', { title: 'CLI', editUrl: false, sidebar: { label: 'Overview', order: 0 } }, stub);
            page('reference/launch-keys.md', { title: 'Launch keys', editUrl: false, sidebar: { order: 1 } }, stub);
            page('reference/runners.md', { title: 'Runners', editUrl: false, sidebar: { order: 2 } }, stub);
        } else {
            const json = <T>(args: string[]): T => JSON.parse(execFileSync(bin, ['--json', ...args], { env: env.env, encoding: 'utf8' }));
            const runners = json<Runner[]>(['runner', 'ls']);
            cliPages(bin, env.env);
            launchKeysPage(json<LaunchKey[]>(['launch-keys']), runners);
            runnersPage(runners);
        }
    } finally {
        env.done();
    }
    configPage(logger);
    nixPage(logger);
}

export function generate(logger: AstroIntegrationLogger) {
    const favicon = fileURLToPath(new URL('../public/favicon.svg', import.meta.url));
    mkdirSync(dirname(favicon), { recursive: true });
    copyFileSync(join(root, 'ui/icons/hicolor/scalable/apps/universe-ui.svg'), favicon);
    for (const dir of ['reference', 'developers']) rmSync(join(docs, dir), { recursive: true, force: true });
    referencePages(logger);
    developerPages();
    changelogPage();
}
