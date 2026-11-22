// ZipNest website: progressive enhancement only. The page works without JS.
(function () {
  "use strict";

  var cfg = window.ZIPNEST_CONFIG;
  if (!cfg) return;

  var REPO_RE = /^[A-Za-z0-9-]+\/[A-Za-z0-9._-]+$/;
  var repoOk = REPO_RE.test(cfg.repo) && cfg.repo.indexOf("YOUR_GITHUB_USERNAME") === -1;
  var repoUrl = "https://github.com/" + cfg.repo;
  var releaseBase = repoUrl + "/releases/latest/download/";

  function $all(sel) {
    return Array.prototype.slice.call(document.querySelectorAll(sel));
  }

  // ----- Links -----
  $all(".js-repo-link").forEach(function (a) {
    a.href = repoUrl;
  });
  $all(".js-issues-link").forEach(function (a) {
    a.href = repoUrl + "/issues";
  });
  $all(".js-notes-link").forEach(function (a) {
    a.href = repoUrl + "/releases/latest";
    a.rel = "noopener noreferrer";
  });
  $all(".js-dl").forEach(function (a) {
    var file = cfg.assets[a.getAttribute("data-asset")];
    if (!file) return;
    a.href = releaseBase + encodeURIComponent(file);
    a.setAttribute("rel", "noopener noreferrer");
    if (a.getAttribute("data-asset") !== "checksums") a.setAttribute("download", "");
  });
  $all(".js-year").forEach(function (el) {
    el.textContent = String(new Date().getFullYear());
  });
  setVersion(cfg.fallbackVersion);

  // ----- OS detection -----
  function detectOS() {
    var uaData = navigator.userAgentData;
    var p = ((uaData && uaData.platform) || navigator.platform || "").toLowerCase();
    var ua = navigator.userAgent.toLowerCase();
    if (/android|iphone|ipad|ipod/.test(ua) || (uaData && uaData.mobile)) return "mobile";
    if (p.indexOf("win") !== -1 || ua.indexOf("windows") !== -1) return "windows";
    if (p.indexOf("mac") !== -1 || ua.indexOf("mac os") !== -1) {
      // iPadOS reports "MacIntel" but has touch.
      if (navigator.maxTouchPoints > 1) return "mobile";
      return "macos";
    }
    if (p.indexOf("linux") !== -1 || ua.indexOf("linux") !== -1 || ua.indexOf("x11") !== -1) return "linux";
    return "unknown";
  }

  var os = detectOS();
  var labels = { windows: "Download for Windows", macos: "Download for macOS", linux: "Download for Linux (.deb)" };
  var assetFor = { windows: "windows", macos: "macos", linux: "deb" };
  var iconSvg = {
    windows: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="4" width="18" height="14" rx="2"/><path d="M3 8h18M8 21h8M12 18v3"/></svg>',
    macos: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="4" y="4" width="16" height="11" rx="1.5"/><path d="M2 19h20l-1.5-4h-17Z"/></svg>',
    linux: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M13 15h4"/></svg>',
  };

  if (labels[os]) {
    $all(".js-primary-download").forEach(function (a) {
      a.href = releaseBase + encodeURIComponent(cfg.assets[assetFor[os]]);
      a.setAttribute("download", "");
      a.setAttribute("rel", "noopener noreferrer");
      a.querySelector(".js-primary-label").textContent = labels[os];
      a.querySelector(".js-os-icon").innerHTML = iconSvg[os]; // static, trusted markup
    });
    var card = document.querySelector('.dl-card[data-os="' + os + '"]');
    if (card) card.classList.add("recommended");
    selectTab(os);
  } else if (os === "mobile") {
    var note = document.querySelector(".mobile-note");
    if (note) note.hidden = false;
  }

  // ----- Not configured yet: make that obvious instead of shipping broken links -----
  if (!repoOk) {
    console.warn("[ZipNest] Set `repo` in /config.js to your GitHub owner/repo.");
    $all(".js-dl, .js-primary-download").forEach(function (a) {
      a.addEventListener("click", function (e) {
        e.preventDefault();
        alert("Downloads are not configured yet. Set `repo` in config.js.");
      });
    });
  } else {
    fetchLatest();
  }

  // ----- Live version from GitHub (optional, fails silently) -----
  function fetchLatest() {
    if (!window.fetch) return;
    var ctrl = window.AbortController ? new AbortController() : null;
    var timer = setTimeout(function () {
      if (ctrl) ctrl.abort();
    }, 4000);
    fetch("https://api.github.com/repos/" + cfg.repo + "/releases/latest", {
      headers: { Accept: "application/vnd.github+json" },
      signal: ctrl ? ctrl.signal : undefined,
      credentials: "omit",
      referrerPolicy: "no-referrer",
    })
      .then(function (r) {
        return r.ok ? r.json() : null;
      })
      .then(function (rel) {
        clearTimeout(timer);
        if (!rel || typeof rel.tag_name !== "string") return;
        var v = rel.tag_name.replace(/^v/, "");
        if (/^\d+\.\d+\.\d+([-.][0-9A-Za-z.-]+)?$/.test(v)) setVersion(v);
        if (Array.isArray(rel.assets)) showSizes(rel.assets);
      })
      .catch(function () {
        /* offline or rate-limited: keep fallback */
      });
  }

  function setVersion(v) {
    $all(".js-version").forEach(function (el) {
      el.textContent = (el.closest(".cta-meta") ? "v" : "") + v;
    });
  }

  function showSizes(assets) {
    var byName = {};
    assets.forEach(function (a) {
      if (a && typeof a.name === "string" && typeof a.size === "number") byName[a.name] = a.size;
    });
    $all(".js-dl").forEach(function (a) {
      var file = cfg.assets[a.getAttribute("data-asset")];
      var size = byName[file];
      if (!size || a.getAttribute("data-asset") === "checksums") return;
      var span = document.createElement("span");
      span.className = "size";
      span.textContent = (size / 1048576).toFixed(1) + " MB";
      a.appendChild(span);
    });
  }

  // ----- Tabs -----
  function selectTab(name) {
    $all("[data-tab]").forEach(function (b) {
      var on = b.getAttribute("data-tab") === name;
      b.setAttribute("aria-selected", on ? "true" : "false");
      b.tabIndex = on ? 0 : -1;
      var panel = document.getElementById(b.getAttribute("aria-controls"));
      if (panel) panel.hidden = !on;
    });
  }
  var tabs = $all("[data-tab]");
  tabs.forEach(function (b, i) {
    b.addEventListener("click", function () {
      selectTab(b.getAttribute("data-tab"));
    });
    b.addEventListener("keydown", function (e) {
      var d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
      if (!d) return;
      var next = tabs[(i + d + tabs.length) % tabs.length];
      selectTab(next.getAttribute("data-tab"));
      next.focus();
    });
  });

  // ----- Copy buttons -----
  $all(".copy").forEach(function (btn) {
    btn.addEventListener("click", function () {
      var code = btn.parentElement.querySelector("code");
      if (!code || !navigator.clipboard) return;
      navigator.clipboard.writeText(code.textContent).then(function () {
        btn.textContent = "Copied";
        setTimeout(function () {
          btn.textContent = "Copy";
        }, 1500);
      });
    });
  });
})();
