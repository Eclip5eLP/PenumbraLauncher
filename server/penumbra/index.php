<?php
/** Landing page for https://eclip5e.com/penumbra/ — download button arrives with the first release (M6). */
declare(strict_types=1);
header('Content-Type: text/html; charset=utf-8');
$latest = json_decode((string)@file_get_contents(__DIR__ . '/launcher/latest.json'), true);
$version = is_array($latest) ? (string)($latest['version'] ?? '') : '';
?><!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Penumbra Launcher</title>
  <link rel="icon" href="/static/brand/logo-512.png">
  <style>
    :root { color-scheme: dark; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: #131313; color: #e8e8ea;
           font: 16px/1.5 "Segoe UI", system-ui, sans-serif; }
    main { text-align: center; padding: 32px 16px; max-width: 520px; }
    img { width: 128px; height: 128px; }
    h1 { margin: 16px 0 8px; font-size: 32px; }
    p { color: #9a9ca3; margin: 0 0 24px; }
    .btn { display: inline-block; padding: 14px 28px; border-radius: 12px; background: #5385c1; color: #fff;
           text-decoration: none; font-weight: 600; }
    .soon { display: inline-block; padding: 10px 18px; border-radius: 12px; border: 1px solid #2e3036; color: #9a9ca3; }
  </style>
</head>
<body>
  <main>
    <img src="/static/brand/logo-512.png" alt="">
    <h1>Penumbra Launcher</h1>
    <p>All Penumbra games and software in one place: install, update and launch.</p>
    <?php if ($version !== ''): ?>
      <a class="btn" href="launcher/<?= htmlspecialchars((string)($latest['installer'] ?? '')) ?>">Download for Windows (<?= htmlspecialchars($version) ?>)</a>
    <?php else: ?>
      <span class="soon">Coming soon</span>
    <?php endif; ?>
  </main>
</body>
</html>
