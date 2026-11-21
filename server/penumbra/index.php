<?php
/** Landing page for https://eclip5e.com/penumbra/ — download button from launcher/latest.json (tools/release.ps1). */
declare(strict_types=1);
header('Content-Type: text/html; charset=utf-8');
header('Cache-Control: no-cache');
$latest = json_decode((string)@file_get_contents(__DIR__ . '/launcher/latest.json'), true);
$version = is_array($latest) ? (string)($latest['version'] ?? '') : '';
$installer = is_array($latest) ? basename((string)($latest['installer'] ?? '')) : '';
$size = is_array($latest) ? (int)($latest['size'] ?? 0) : 0;
$date = is_array($latest) && !empty($latest['pub_date']) ? date('j M Y', strtotime((string)$latest['pub_date'])) : '';
$ready = $version !== '' && $installer !== '' && is_file(__DIR__ . '/launcher/' . $installer);
$h = fn(string $s) => htmlspecialchars($s, ENT_QUOTES);
// Public catalog items (restricted ones never appear here) for the "Open in launcher" list.
$items = [];
foreach (@file(__DIR__ . '/catalog/items.jsonl', FILE_IGNORE_NEW_LINES | FILE_SKIP_EMPTY_LINES) ?: [] as $line) {
    $it = json_decode($line, true);
    if (is_array($it) && in_array($it['access'] ?? 'public', ['public', 'account'], true)) {
        $items[] = $it;
    }
}
?><!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Penumbra Launcher</title>
  <meta name="description" content="All Penumbra games and software in one place: install, update and launch.">
  <link rel="icon" href="/static/brand/logo-512.png">
  <style>
    :root { color-scheme: dark; }
    * { box-sizing: border-box; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: radial-gradient(ellipse at 50% 0%, #2a1d4a 0%, #131313 60%); color: #e8e8ea;
           font: 16px/1.5 "Segoe UI", system-ui, sans-serif; padding: 16px; }
    main { text-align: center; padding: 32px 0; max-width: 560px; width: 100%; }
    img { width: 128px; height: 128px; filter: drop-shadow(0 8px 30px rgb(123 92 240 / .45)); }
    h1 { margin: 16px 0 8px; font-size: 34px; letter-spacing: -.01em; }
    p { color: #9a9ca3; margin: 0 0 28px; }
    .btn { display: inline-block; padding: 15px 30px; border-radius: 12px; background: linear-gradient(180deg, #6f9ad0, #5385c1); color: #fff;
           text-decoration: none; font-weight: 650; font-size: 17px; box-shadow: 0 10px 28px rgb(83 133 193 / .35); }
    .btn:hover { filter: brightness(1.08); }
    .meta { margin-top: 12px; font-size: 13px; color: #6c6e75; }
    .soon { display: inline-block; padding: 10px 18px; border-radius: 12px; border: 1px solid #2e3036; color: #9a9ca3; }
    .note { margin-top: 28px; padding: 12px 16px; border-radius: 10px; border: 1px solid #2e3036; background: #1d1e22; font-size: 13px; color: #9a9ca3; text-align: left; }
    .note b { color: #c4c5ca; }
    .items { margin-top: 36px; display: grid; gap: 10px; text-align: left; }
    .items h2 { font-size: 15px; color: #9a9ca3; font-weight: 600; margin: 0 0 4px; }
    .it { display: flex; align-items: center; gap: 14px; padding: 10px 12px; border-radius: 12px; background: #1d1e22; border: 1px solid #2e3036; }
    .it img { width: 44px; height: 44px; border-radius: 10px; object-fit: cover; filter: none; }
    .it div { flex: 1; min-width: 0; }
    .it b { display: block; }
    .it span { color: #9a9ca3; font-size: 13px; }
    .it a { flex: none; padding: 8px 14px; border-radius: 9px; background: #2d2f35; color: #e8e8ea; text-decoration: none; font-size: 14px; font-weight: 600; }
    .it a:hover { background: #3a3d44; }
  </style>
</head>
<body>
  <main>
    <img src="/static/brand/logo-512.png" alt="">
    <h1>Penumbra Launcher</h1>
    <p>All Penumbra games and software in one place: install, update and launch.</p>
    <?php if ($ready): ?>
      <a class="btn" href="launcher/<?= $h($installer) ?>">Download for Windows</a>
      <div class="meta">Version <?= $h($version) ?><?= $size ? ' · ' . number_format($size / 1048576, 1) . ' MB' : '' ?><?= $date ? ' · ' . $h($date) : '' ?> · Windows 10/11 (64-bit)</div>
      <div class="note"><b>Windows may warn about an unknown publisher.</b> The installer isn't code-signed yet. Choose
        <i>More info → Run anyway</i>. The launcher updates itself afterwards, and every update and download is
        cryptographically verified.</div>
    <?php else: ?>
      <span class="soon">Coming soon</span>
    <?php endif; ?>
    <?php if ($items): ?>
      <div class="items">
        <h2>In the launcher</h2>
        <?php foreach ($items as $it): ?>
          <div class="it">
            <?php if (!empty($it['media']['icon'])): ?><img src="<?= $h((string)$it['media']['icon']) ?>" alt=""><?php endif; ?>
            <div><b><?= $h((string)$it['name']) ?></b><span><?= $h((string)($it['tagline'] ?? '')) ?></span></div>
            <a href="penumbra://open/<?= $h((string)$it['id']) ?>">Open in launcher</a>
          </div>
        <?php endforeach; ?>
      </div>
    <?php endif; ?>
  </main>
</body>
</html>
