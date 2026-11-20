<?php
/**
 * GET /penumbra/api/v1/manifest.php?item=<id>&version=<v>  [Authorization: Bearer <token>]
 * Returns the zstd-compressed manifest; its ed25519 signature is in the X-Penumbra-Sig header.
 * Gated manifests live in data/content/ (never served statically); public ones in content/ (the launcher may
 * also fetch those directly, which Cloudflare caches).
 */
declare(strict_types=1);

require __DIR__ . '/_lib.php';

pl_method('GET', 'HEAD');

$item = (string)($_GET['item'] ?? '');
$version = (string)($_GET['version'] ?? '');
if (!preg_match('/^[a-z0-9-]{1,32}$/', $item) || !preg_match('/^[A-Za-z0-9._+-]{1,64}$/', $version) || $version[0] === '.') {
    pl_error(400, 'bad_request', 'invalid item or version');
}

$items = pl_catalog_items();
if (!isset($items[$item])) {
    pl_error(404, 'unknown_item');
}
$data = $items[$item]['data'];
$user = pl_user();

// Item rule, plus the rule of the channel that publishes this version (if it has one).
$rules = [(string)($data['access'] ?? 'public')];
foreach (($data['channels'] ?? []) as $ch) {
    if (is_array($ch) && ($ch['version'] ?? null) === $version && isset($ch['access'])) {
        $rules[] = (string)$ch['access'];
    }
}
foreach ($rules as $rule) {
    if (!pl_can_download($rule, $user)) {
        pl_error($user === null ? 401 : 403, $user === null ? 'login_required' : 'forbidden');
    }
}

foreach ([PL_ROOT . '/data/content', PL_ROOT . '/content'] as $base) {
    $file = "$base/$item/$version.json.zst";
    if (is_file($file)) {
        $sig = trim((string)@file_get_contents("$base/$item/$version.json.sig"));
        if ($sig === '') {
            pl_error(500, 'unsigned_manifest');
        }
        header('Content-Type: application/zstd');
        header('X-Penumbra-Sig: ' . $sig);
        header('Cache-Control: private, max-age=300');
        header('Content-Length: ' . filesize($file));
        readfile($file);
        exit;
    }
}
pl_error(404, 'unknown_version');
