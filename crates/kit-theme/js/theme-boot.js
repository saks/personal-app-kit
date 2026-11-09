// kit-theme boot script. Runs in <head> before first paint, so a saved theme
// never flashes Default. Load it as
//   <script src="public/kit/kit-theme/theme-boot.js" data-storage-prefix="my-app"></script>
// It reads only the localStorage mirror that kit-theme's ThemeProvider writes
// (`<prefix>.theme`, `<prefix>.mode`); the app's ThemeApi is the source of
// truth and the provider reconciles after startup. Same mapping as
// ThemePrefs::data_theme. Missing or unknown values mean Default / System,
// which leaves data-theme off.
// An external file because the CSP (script-src 'self') blocks inline scripts.
(function () {
  try {
    var script = document.currentScript;
    var prefix = script && script.dataset.storagePrefix;
    if (!prefix) return;
    var prisma = localStorage.getItem(prefix + ".theme") === "prisma";
    var mode = localStorage.getItem(prefix + ".mode");
    if (mode !== "light" && mode !== "dark") mode = "system";
    if (!prisma && mode === "system") return;
    var dark =
      mode === "dark" ||
      (mode === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    var names = prisma ? ["prismaLight", "prismaDark"] : ["appLight", "appDark"];
    document.documentElement.setAttribute("data-theme", names[dark ? 1 : 0]);
  } catch (e) {
    // No storage (or no matchMedia): Default / System, as on a fresh install.
  }
})();
