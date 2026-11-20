import "@fontsource-variable/inter";
import "./styles/global.css";
import { mount } from "svelte";
import App from "./App.svelte";
import { api } from "./lib/api";
import { app } from "./lib/store.svelte";

// Startup path (docs/05-launcher-app.md#performance): one IPC call for all data, mount, then tell Rust
// to show the window after the first frame has actually been painted.
async function start() {
  const boot = await api.bootstrap();
  app.init(boot);
  mount(App, { target: document.getElementById("app")! });
  requestAnimationFrame(() =>
    requestAnimationFrame(async () => {
      app.startupMs = (await api.appReady(performance.now())) ?? 0;
    }),
  );
  void app.listen();
}

// Disable the default WebView context menu and browser shortcuts that don't belong in an app.
window.addEventListener("contextmenu", (e) => e.preventDefault());
if (import.meta.env.PROD) {
  window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey && ["r", "p", "f", "g", "u"].includes(e.key.toLowerCase())) || e.key === "F5")
      e.preventDefault();
  });
}

start();
