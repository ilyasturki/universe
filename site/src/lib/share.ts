export const shareImage = {
    path: '/og.png',
    width: 1200,
    height: 630,
    alt: 'Universe’s home screen beside the Universe logo and the line “A game launcher for Linux you drive with a controller.”',
};

export function shareTags(site: string | URL): { property?: string; name?: string; content: string }[] {
    const image = new URL(shareImage.path, site).href;
    return [
        { property: 'og:image', content: image },
        { property: 'og:image:width', content: String(shareImage.width) },
        { property: 'og:image:height', content: String(shareImage.height) },
        { property: 'og:image:alt', content: shareImage.alt },
        { name: 'twitter:card', content: 'summary_large_image' },
        { name: 'twitter:image', content: image },
        { name: 'twitter:image:alt', content: shareImage.alt },
    ];
}
