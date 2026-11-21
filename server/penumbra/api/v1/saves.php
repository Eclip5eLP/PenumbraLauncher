<?php
/**
 * Cloud saves (docs/08-accounts-and-cloud.md). One zip snapshot per upload, per user and item; the newest 5 are
 * kept. Optimistic concurrency: an upload names the revision it was based on (`base`) and is rejected with 409 when
 * another device uploaded in between, so nothing is overwritten silently.
 *
 *   GET    saves.php?item=<id>               -> {"rev","size","hash","device","updated"} | 404 no_save
 *   GET    saves.php?item=<id>&download=1    -> zip of the newest snapshot (X-Penumbra-Rev header)
 *   PUT    saves.php?item=<id>&base=<rev>&hash=<h>&device=<name>   body = zip  -> new meta | 409 conflict
 *   DELETE saves.php?item=<id>               -> removes all snapshots of that item
 *
 * Files: data/saves/<user id>/<item>/{meta.json,<rev>.zip}. Requires a Penumbra Account token (app 000007).
 */
declare(strict_types=1);

require __DIR__ . '/_lib.php';

const PL_SAVE_MAX = 50 * 1024 * 1024; // per snapshot (Cloudflare allows 100 MB per request)
const PL_SAVE_KEEP = 5;

pl_method('GET', 'PUT', 'DELETE');
$user = pl_user();
if ($user === null) {
    header('WWW-Authenticate: Bearer');
    pl_error(401, 'login_required');
}
$item = (string)($_GET['item'] ?? '');
if (!preg_match('/^[a-z0-9-]{1,32}$/', $item)) {
    pl_error(400, 'bad_request', 'invalid item');
}
$items = pl_catalog_items();
if (!isset($items[$item]) || !pl_can_download((string)($items[$item]['data']['access'] ?? 'public'), $user)) {
    pl_error(404, 'unknown_item');
}

$dir = PL_ROOT . '/data/saves/' . (int)$user['id'] . '/' . $item;
$metaFile = "$dir/meta.json";
$readMeta = fn() => is_file($metaFile) ? json_decode((string)file_get_contents($metaFile), true) : null;
$public = fn(array $m) => array_intersect_key($m, array_flip(['rev', 'size', 'hash', 'device', 'updated']));

switch ($_SERVER['REQUEST_METHOD']) {
    case 'GET':
        $meta = $readMeta();
        if (!is_array($meta)) {
            pl_error(404, 'no_save');
        }
        if (empty($_GET['download'])) {
            pl_json($public($meta));
        }
        $zip = "$dir/" . (int)$meta['rev'] . '.zip';
        if (!is_file($zip)) {
            pl_error(404, 'no_save');
        }
        header('Content-Type: application/zip');
        header('Content-Length: ' . filesize($zip));
        header('X-Penumbra-Rev: ' . (int)$meta['rev']);
        header('Cache-Control: no-store');
        readfile($zip);
        exit;

    case 'PUT':
        $base = (int)($_GET['base'] ?? 0);
        $hash = (string)($_GET['hash'] ?? '');
        $device = mb_substr(preg_replace('/[^\p{L}\p{N} ._-]/u', '', (string)($_GET['device'] ?? '')), 0, 64);
        if (!preg_match('/^[0-9a-f]{64}$/', $hash)) {
            pl_error(400, 'bad_request', 'hash must be 64 hex chars');
        }
        if (!is_dir($dir) && !@mkdir($dir, 0755, true)) {
            pl_error(500, 'storage_error');
        }
        // Body -> temp file, enforcing the size cap while reading.
        $tmp = tempnam($dir, 'up');
        $in = fopen('php://input', 'rb');
        $out = fopen($tmp, 'wb');
        $size = 0;
        while (!feof($in)) {
            $buf = fread($in, 1 << 20);
            if ($buf === false) {
                break;
            }
            $size += strlen($buf);
            if ($size > PL_SAVE_MAX) {
                fclose($out);
                @unlink($tmp);
                pl_error(413, 'too_large', 'Save data is larger than 50 MB.');
            }
            fwrite($out, $buf);
        }
        fclose($in);
        fclose($out);
        if ($size === 0 || file_get_contents($tmp, false, null, 0, 4) !== "PK\x03\x04") {
            @unlink($tmp);
            pl_error(400, 'bad_request', 'body must be a zip');
        }
        $lock = fopen("$dir/.lock", 'c');
        flock($lock, LOCK_EX);
        $meta = $readMeta() ?? ['rev' => 0, 'history' => []];
        if ((int)$meta['rev'] !== $base) {
            flock($lock, LOCK_UN);
            @unlink($tmp);
            pl_json(['error' => 'conflict', 'message' => 'Another device uploaded newer saves.'] + $public($meta), 409);
        }
        $rev = (int)$meta['rev'] + 1;
        rename($tmp, "$dir/$rev.zip");
        $meta = [
            'rev' => $rev,
            'size' => $size,
            'hash' => $hash,
            'device' => $device,
            'updated' => gmdate('Y-m-d\TH:i:s\Z'),
            'history' => array_slice(array_merge([$rev], $meta['history'] ?? []), 0, PL_SAVE_KEEP),
        ];
        file_put_contents($metaFile, json_encode($meta), LOCK_EX);
        foreach (glob("$dir/*.zip") ?: [] as $z) {
            if (!in_array((int)basename($z, '.zip'), $meta['history'], true)) {
                @unlink($z);
            }
        }
        flock($lock, LOCK_UN);
        pl_json($public($meta));

    case 'DELETE':
        foreach (glob("$dir/*") ?: [] as $f) {
            @unlink($f);
        }
        @unlink("$dir/.lock");
        @rmdir($dir);
        pl_json(['ok' => true]);
}
