<?php

namespace App\Http\Controllers;

use Illuminate\Support\Str;
use League\CommonMark\Extension\HeadingPermalink\HeadingPermalinkExtension;

/**
 * The components' docs, at /docs: the Markdown that ships in unscripted/ui-server's docs/
 * folder, so they're always the installed version's. Its README is the front page, and its
 * lists of links are the sidebar.
 */
class DocsController
{
    public function __invoke(string $page = 'README')
    {
        $dir = base_path('vendor/unscripted/ui-server/docs');
        $file = "$dir/$page.md";
        abort_unless(preg_match('/^[a-z0-9-]+$|^README$/', $page) && is_file($file), 404);

        $nav = $this->nav(file_get_contents("$dir/README.md"));
        $html = $this->render(file_get_contents($file));

        // The page's title is its first heading, which the layout shows on its own.
        preg_match('/<h1[^>]*>(.*?)<\/h1>/s', $html, $h1);
        $title = trim(strip_tags($h1[1] ?? 'Docs'));
        $html = preg_replace('/<h1[^>]*>.*?<\/h1>/s', '', $html, 1);

        // "On this page": the second-level headings.
        preg_match_all('/<h2[^>]*id="([^"]+)"[^>]*>(.*?)<\/h2>/s', $html, $h2, PREG_SET_ORDER);
        $toc = array_map(fn($m) => ['id' => $m[1], 'title' => trim(strip_tags(preg_replace('/<a[^>]*heading-permalink.*?<\/a>/s', '', $m[2])))], $h2);

        // Previous and next, in the sidebar's order (each page once).
        $order = collect($nav)->flatMap(fn($section) => $section['links'])
            ->map(fn($link) => $link + ['page' => strtok(substr($link['href'], 6), '#') ?: 'README'])
            ->unique('page')->values()->prepend(['title' => 'Introduction', 'href' => '/docs', 'page' => 'README']);
        $at = $order->search(fn($link) => $link['page'] === $page);

        return view('docs', [
            'title' => $title,
            'html' => $html,
            'nav' => $nav,
            'toc' => $toc,
            'page' => $page,
            'previous' => $at > 0 ? $order[$at - 1] : null,
            'next' => $order[$at + 1] ?? null,
        ]);
    }

    /** Markdown to HTML: headings get ids (for links into them); links to other pages stay here. */
    private function render(string $markdown): string
    {
        $html = (string) Str::markdown($markdown, [
            'html_input' => 'escape',
            'heading_permalink' => ['apply_id_to_heading' => true, 'symbol' => '#', 'id_prefix' => '', 'fragment_prefix' => '', 'insert' => 'after', 'min_heading_level' => 2, 'max_heading_level' => 3],
        ], [new HeadingPermalinkExtension()]);

        return preg_replace_callback('/href="([a-z0-9-]+|README)\.md(#[^"]*)?"/i', fn($m) => 'href="'.($m[1] === 'README' ? '/docs' : "/docs/{$m[1]}").($m[2] ?? '').'"', $html);
    }

    /** The README's "## Section" headings, each with its list of "- [Title](page.md)" links. */
    private function nav(string $readme): array
    {
        $nav = [];
        foreach (preg_split('/^## /m', $readme) as $i => $chunk) {
            if ($i === 0) {
                continue; // the introduction
            }
            preg_match_all('/^- \[([^\]]+)\]\(([a-z0-9-]+)\.md(#[^)]*)?\)/m', $chunk, $links, PREG_SET_ORDER);
            $nav[] = [
                'title' => trim(strtok($chunk, "\n")),
                'links' => array_map(fn($l) => ['title' => trim(strip_tags(Str::markdown($l[1]))), 'href' => "/docs/{$l[2]}".($l[3] ?? '')], $links),
            ];
        }

        return $nav;
    }
}
