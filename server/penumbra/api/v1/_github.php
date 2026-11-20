<?php
/**
 * Resolves GitHub-sourced catalog channels to concrete downloads (docs/11-sources-github.md).
 * Results go into the catalog's unsigned `resolved` map; the launcher only accepts URLs inside the repository
 * pinned by the signed item. GitHub responses are cached for 10 minutes (conditional requests after that), so all
 * launchers together cost a handful of API calls per hour. Optional token: data/github-token (read-only scope).
 */
declare(strict_types=1);

const PL_GH_TTL = 600;

/** GET https://api.github.com/<path> with a file cache. Returns decoded JSON or null. */
function pl_gh_get(string $path): mixed
{
    $dir = PL_ROOT . '/data/github-cache';
    if (!is_dir($dir)) {
        @mkdir($dir, 0755, true);
    }
    $file = $dir . '/' . md5($path) . '.json';
    $cached = is_file($file) ? json_decode((string)file_get_contents($file), true) : null;
    if (is_array($cached) && time() - (int)$cached['fetched'] < PL_GH_TTL) {
        return $cached['body'];
    }
    $headers = ['Accept: application/vnd.github+json', 'User-Agent: PenumbraLauncher-Server', 'X-GitHub-Api-Version: 2022-11-28'];
    $token = trim((string)@file_get_contents(PL_ROOT . '/data/github-token'));
    if ($token !== '') {
        $headers[] = 'Authorization: Bearer ' . $token;
    }
    if (is_array($cached) && !empty($cached['etag'])) {
        $headers[] = 'If-None-Match: ' . $cached['etag'];
    }
    $etag = null;
    $ch = curl_init('https://api.github.com/' . $path);
    curl_setopt_array($ch, [
        CURLOPT_RETURNTRANSFER => true,
        CURLOPT_HTTPHEADER => $headers,
        CURLOPT_TIMEOUT => 6,
        CURLOPT_CONNECTTIMEOUT => 3,
        CURLOPT_HEADERFUNCTION => function ($c, string $h) use (&$etag) {
            if (stripos($h, 'etag:') === 0) {
                $etag = trim(substr($h, 5));
            }
            return strlen($h);
        },
    ]);
    $body = curl_exec($ch);
    $status = (int)curl_getinfo($ch, CURLINFO_RESPONSE_CODE);
    curl_close($ch);

    if ($status === 304 && is_array($cached)) {
        $cached['fetched'] = time();
        @file_put_contents($file, json_encode($cached), LOCK_EX);
        return $cached['body'];
    }
    if ($status === 200 && is_string($body)) {
        $decoded = json_decode($body, true);
        @file_put_contents($file, json_encode(['fetched' => time(), 'etag' => $etag, 'body' => $decoded]), LOCK_EX);
        return $decoded;
    }
    if ($status === 404) {
        @file_put_contents($file, json_encode(['fetched' => time(), 'etag' => null, 'body' => null]), LOCK_EX);
        return null;
    }
    error_log("penumbra api: GitHub $path -> HTTP $status");
    return is_array($cached) ? $cached['body'] : null; // stale beats nothing
}

/** @return array<string, array> channel name => resolved download */
function pl_github_resolve(array $item): array
{
    $repo = (string)($item['source']['repo'] ?? '');
    if (!preg_match('#^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$#', $repo)) {
        return [];
    }
    $out = [];
    foreach (($item['channels'] ?? []) as $name => $ch) {
        if (!is_array($ch)) {
            continue;
        }
        $r = match ($ch['mode'] ?? 'release') {
            'release' => pl_gh_release($repo, $ch),
            'branch' => pl_gh_branch($repo, $ch),
            'tag' => pl_gh_tag($repo, $ch),
            default => null,
        };
        if ($r !== null) {
            $out[$name] = $r;
        }
    }
    return $out;
}

function pl_gh_release(string $repo, array $ch): ?array
{
    $releases = pl_gh_get("repos/$repo/releases?per_page=20");
    if (!is_array($releases)) {
        return null;
    }
    foreach ($releases as $rel) {
        if (!empty($rel['draft']) || (!empty($rel['prerelease']) && empty($ch['prerelease']))) {
            continue;
        }
        $tag = (string)$rel['tag_name'];
        $res = ['version' => $tag, 'date' => $rel['published_at'] ?? null];
        $pattern = $ch['asset'] ?? null;
        foreach (($rel['assets'] ?? []) as $a) {
            if ($pattern !== null && fnmatch($pattern, (string)$a['name'], FNM_CASEFOLD)) {
                $digest = (string)($a['digest'] ?? '');
                return $res + [
                    'url' => $a['browser_download_url'],
                    'download' => (int)$a['size'],
                    'sha256' => str_starts_with($digest, 'sha256:') ? substr($digest, 7) : null,
                ];
            }
        }
        return $res + ['url' => "https://codeload.github.com/$repo/zip/refs/tags/" . rawurlencode($tag)];
    }
    return null;
}

function pl_gh_branch(string $repo, array $ch): ?array
{
    $branch = $ch['branch'] ?? null;
    if ($branch === null) {
        $info = pl_gh_get("repos/$repo");
        $branch = is_array($info) ? ($info['default_branch'] ?? null) : null;
    }
    if (!is_string($branch) || $branch === '') {
        return null;
    }
    $c = pl_gh_get("repos/$repo/commits/" . rawurlencode($branch));
    if (!is_array($c) || empty($c['sha'])) {
        return null;
    }
    return [
        'version' => $branch . '@' . substr((string)$c['sha'], 0, 7),
        'date' => $c['commit']['committer']['date'] ?? null,
        'url' => "https://codeload.github.com/$repo/zip/" . $c['sha'],
    ];
}

function pl_gh_tag(string $repo, array $ch): ?array
{
    $tags = pl_gh_get("repos/$repo/tags?per_page=50");
    if (!is_array($tags)) {
        return null;
    }
    $pattern = $ch['tagPattern'] ?? '*';
    foreach ($tags as $t) {
        if (fnmatch($pattern, (string)$t['name'], FNM_CASEFOLD)) {
            return [
                'version' => (string)$t['name'],
                'url' => "https://codeload.github.com/$repo/zip/refs/tags/" . rawurlencode((string)$t['name']),
            ];
        }
    }
    return null;
}
