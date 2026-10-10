import type { ImageMetadata } from 'astro';
import { parse } from 'smol-toml';
import takesToml from '../../media/takes.toml?raw';

type Take = { name: string; look: string; size: string; kind: 'still' | 'clip' | 'gamescope-clip'; alt: string };
type Recorded = { kind: string; files: Record<string, string>; w: number; h: number; alt: string; version: string };

const takes = new Map(((parse(takesToml) as { take?: Take[] }).take ?? []).map((t) => [t.name, t]));
// The media come from the site-media branch: a checkout without them builds with placeholders.
const manifests = import.meta.glob<{ takes: Record<string, Recorded> }>('../assets/media/manifest.json', { eager: true, import: 'default' });
const recorded = Object.values(manifests)[0]?.takes ?? {};
const images = import.meta.glob<ImageMetadata>('../assets/media/**/*.{avif,webp,png,jpg}', { eager: true, import: 'default' });
const strict = process.env.DOCS_STRICT === '1';

export type Media = { take: Take; aspect: string; recorded?: Recorded };

function take(name: string, kinds: Take['kind'][], component: string): Media {
    const found = takes.get(name);
    if (!found) throw new Error(`<${component} name="${name}" />: site/media/takes.toml has no take named ${name}`);
    if (!kinds.includes(found.kind)) throw new Error(`<${component} name="${name}" />: ${name} is a ${found.kind} take`);
    const entry = recorded[name];
    if (!entry && strict) throw new Error(`<${component} name="${name}" />: ${name} is not recorded yet, and DOCS_STRICT=1 refuses a placeholder`);
    const [w, h] = entry ? [entry.w, entry.h] : found.size.split('x').map(Number);
    return { take: found, aspect: `${w} / ${h}`, recorded: entry };
}

export function still(name: string): Media & { image?: ImageMetadata } {
    const media = take(name, ['still'], 'Shot');
    const file = media.recorded?.files.webp ?? media.recorded?.files.avif;
    const image = file ? images[`../assets/media/${file}`] : undefined;
    if (file && !image) throw new Error(`<Shot name="${name}" />: manifest.json names ${file}, which is not in src/assets/media`);
    return { ...media, image };
}

export function clip(name: string): Media {
    return take(name, ['clip', 'gamescope-clip'], 'Clip');
}
