<?php
/**
 * GET /penumbra/api/v1/catalog.php  [Authorization: Bearer <token>]
 * The catalog for this caller: restricted items only appear when the user may download them.
 * Items are passed through byte-for-byte (each carries its own signature, ADR-007).
 */
declare(strict_types=1);

require __DIR__ . '/_lib.php';

pl_method('GET', 'HEAD');

$meta = json_decode((string)@file_get_contents(PL_ROOT . '/catalog/meta.json'), true);
if (!is_array($meta)) {
    pl_error(503, 'catalog_unavailable', 'The catalog has not been published yet.');
}
$user = pl_user();

require __DIR__ . '/_github.php';

$visible = [];
$resolved = [];
foreach (pl_catalog_items() as $id => $it) {
    if (!pl_can_list((string)($it['data']['access'] ?? 'public'), $user)) {
        continue;
    }
    $visible[] = $it['raw'];
    if (($it['data']['source']['kind'] ?? '') === 'github') {
        $r = pl_github_resolve($it['data']);
        if ($r) {
            $resolved[$id] = $r;
        }
    }
}

unset($meta['items']);
$meta['resolved'] = (object)$resolved;
$head = json_encode($meta, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
$body = substr($head, 0, -1) . ',"items":[' . implode(',', $visible) . ']}';

pl_send_etagged($body, 'application/json; charset=utf-8');
