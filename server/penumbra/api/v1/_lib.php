<?php
/**
 * Shared helpers for the Penumbra Launcher API (docs/04-server.md).
 * Signed in = a Penumbra Account bearer token issued to the launcher app (000007).
 */
declare(strict_types=1);

const PL_APP_ID = '000007';
define('PL_ROOT', dirname(__DIR__, 2));

function pl_json(array $data, int $status = 200): never
{
    http_response_code($status);
    header('Content-Type: application/json; charset=utf-8');
    header('Cache-Control: no-store');
    echo json_encode($data, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    exit;
}

function pl_error(int $status, string $code, string $message = ''): never
{
    pl_json(['error' => $code, 'message' => $message ?: $code], $status);
}

function pl_method(string ...$allowed): void
{
    $m = $_SERVER['REQUEST_METHOD'] ?? 'GET';
    if (!in_array($m, $allowed, true)) {
        header('Allow: ' . implode(', ', $allowed));
        pl_error(405, 'method_not_allowed');
    }
}

/**
 * The signed-in user (['id','username','role']) or null. No token = anonymous, no database access.
 * An invalid/expired token is also anonymous; the response says so in X-Penumbra-Auth so the launcher can
 * refresh its token.
 */
function pl_user(): ?array
{
    static $done = false, $user = null;
    if ($done) {
        return $user;
    }
    $done = true;
    // mod_php doesn't always put Authorization into $_SERVER; getallheaders() always has it.
    $hasAuth = !empty($_SERVER['HTTP_AUTHORIZATION']) || !empty($_SERVER['HTTP_X_PENUMBRA_TOKEN'])
        || !empty($_SERVER['REDIRECT_HTTP_AUTHORIZATION']);
    if (!$hasAuth && function_exists('getallheaders')) {
        foreach (array_keys(getallheaders()) as $k) {
            if (strcasecmp((string)$k, 'Authorization') === 0) {
                $hasAuth = true;
            }
        }
    }
    if (!$hasAuth) {
        return null;
    }
    try {
        require_once dirname(PL_ROOT) . '/userbase/lib/bootstrap.php';
        $ctx = \Userbase\token_lookup(\Userbase\bearer_token());
        if ($ctx === null || ($ctx['app']['id'] ?? '') !== PL_APP_ID) {
            header('X-Penumbra-Auth: invalid');
            return null;
        }
        $user = \Userbase\public_user($ctx['user']);
        header('X-Penumbra-Auth: ok');
    } catch (\Throwable $e) {
        error_log('penumbra api: auth lookup failed: ' . $e->getMessage());
        header('X-Penumbra-Auth: error');
    }
    return $user;
}

/**
 * Access rules: "public" | "account" | "role:<role>" | "users:<id>,<id>".
 * public/account items are always listed (account items show as locked when signed out).
 * Admins see everything.
 */
function pl_can_list(string $rule, ?array $user): bool
{
    return $rule === 'public' || $rule === 'account' || pl_can_download($rule, $user);
}

function pl_can_download(string $rule, ?array $user): bool
{
    if ($rule === 'public') {
        return true;
    }
    if ($user === null) {
        return false;
    }
    if ($rule === 'account' || $user['role'] === 'admin') {
        return true;
    }
    if (str_starts_with($rule, 'role:')) {
        return $user['role'] === substr($rule, 5);
    }
    if (str_starts_with($rule, 'users:')) {
        $ids = array_map('intval', explode(',', substr($rule, 6)));
        return in_array((int)$user['id'], $ids, true);
    }
    return false;
}

/** Raw catalog item lines (each one signed JSON object) keyed by item id. */
function pl_catalog_items(): array
{
    $lines = @file(PL_ROOT . '/catalog/items.jsonl', FILE_IGNORE_NEW_LINES | FILE_SKIP_EMPTY_LINES);
    if ($lines === false) {
        pl_error(503, 'catalog_unavailable', 'The catalog has not been published yet.');
    }
    $items = [];
    foreach ($lines as $line) {
        $it = json_decode($line, true);
        if (is_array($it) && isset($it['id'])) {
            $items[$it['id']] = ['raw' => $line, 'data' => $it];
        }
    }
    return $items;
}

/** Sends $body with an ETag; answers 304 when the client already has it. */
function pl_send_etagged(string $body, string $contentType): never
{
    $etag = '"' . md5($body) . '"';
    header('ETag: ' . $etag);
    header('Cache-Control: private, no-cache');
    header('Vary: Authorization, X-Penumbra-Token');
    if (trim($_SERVER['HTTP_IF_NONE_MATCH'] ?? '') === $etag) {
        http_response_code(304);
        exit;
    }
    header('Content-Type: ' . $contentType);
    echo $body;
    exit;
}
