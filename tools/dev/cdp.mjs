// Drives the running launcher's WebView2 over the Chrome DevTools Protocol (dev/testing only).
// Start the app with:  $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
// Usage:
//   node tools/dev/cdp.mjs eval "<js expression, may await>"
//   node tools/dev/cdp.mjs invoke <command> '<json args>'
//   node tools/dev/cdp.mjs shot <file.png>
const [, , mode, a, b] = process.argv;
const port = process.env.CDP_PORT ?? 9222;
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((t) => t.type === "page");
if (!page) throw new Error("no page target");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => ((ws.onopen = r), (ws.onerror = j)));
let id = 0;
const pending = new Map();
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) pending.get(msg.id)(msg);
};
const send = (method, params = {}) =>
  new Promise((r) => {
    pending.set(++id, r);
    ws.send(JSON.stringify({ id, method, params }));
  });

async function evaluate(expr) {
  const r = await send("Runtime.evaluate", {
    expression: `(async () => { return (${expr}); })()`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (r.result?.exceptionDetails) {
    const ex = r.result.exceptionDetails;
    throw new Error(ex.exception?.description ?? ex.exception?.value ?? JSON.stringify(ex.exception) ?? ex.text);
  }
  return r.result?.result?.value;
}

try {
  if (mode === "eval") console.log(JSON.stringify(await evaluate(a), null, 1));
  else if (mode === "invoke")
    console.log(JSON.stringify(await evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(a)}, ${b ?? "{}"})`), null, 1));
  else if (mode === "call") {
    // invoke and print the result or the error object (doesn't throw)
    const r = await evaluate(
      `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(a)}, ${b ?? "{}"}).then(v => ({ ok: v ?? null }), e => ({ err: e }))`,
    );
    console.log(JSON.stringify(r));
    if (r.err) process.exitCode = 2;
  } else if (mode === "wait-job") {
    // waits until the newest job finishes; prints progress every 2 s; timeout in seconds (default 300)
    const deadline = Date.now() + Number(a ?? 300) * 1000;
    for (;;) {
      const b2 = await evaluate(`window.__TAURI_INTERNALS__.invoke("get_bootstrap")`);
      const j = b2.jobs.at(-1);
      if (!j) throw new Error("no jobs");
      const mb = (n) => (n / 1e6).toFixed(1);
      console.log(`${j.kind} ${j.item} ${j.version}: ${j.state} ${mb(j.downloaded)}/${mb(j.total)} MB ${mb(j.speed)} MB/s ${j.note ?? ""}${j.error ?? ""}`);
      if (["done", "error", "cancelled"].includes(j.state)) {
        if (j.state !== "done") process.exitCode = 3;
        break;
      }
      if (Date.now() > deadline) throw new Error("timeout");
      await new Promise((r) => setTimeout(r, 2000));
    }
  } else if (mode === "shot") {
    const r = await send("Page.captureScreenshot", { format: "png" });
    (await import("node:fs")).writeFileSync(a, Buffer.from(r.result.data, "base64"));
    console.log("saved", a);
  } else throw new Error("mode: eval | invoke | shot");
} catch (e) {
  console.error("ERROR:", e.message);
  process.exitCode = 1;
} finally {
  ws.close();
}
