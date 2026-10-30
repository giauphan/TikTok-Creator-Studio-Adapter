"""Self-contained HTML UI owned inside the TikTok Creator Studio module."""

from .client import build_masked_config_view, load_module_config


def render_module_ui_html(namespace: str = "/tiktok") -> str:
    clean_ns = (namespace or "/tiktok").rstrip("/") or "/tiktok"
    cfg = load_module_config()
    masked = build_masked_config_view(cfg)
    base_url = masked.get("base_url") or "https://www.tiktok.com"
    enabled = masked.get("enabled", True)
    configured = masked.get("configured", False)
    masked_cookie = masked.get("masked_cookie") or "not set"
    masked_csrf = masked.get("masked_csrf_token") or "not set"
    badge_text = "Configured" if configured else "Not Configured"

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>TikTok Creator Studio Config &amp; HAR Import</title>
<style>
  *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ font-family: system-ui, -apple-system, sans-serif; background: #0d0f14; color: #e2e8f0; padding: 32px 20px; }}
  .container {{ max-width: 780px; margin: 0 auto; }}
  .header {{ display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 24px; gap: 12px; flex-wrap: wrap; }}
  h1 {{ font-size: 1.5rem; color: #f8fafc; margin-bottom: 6px; }}
  .sub {{ color: #94a3b8; font-size: 0.9rem; }}
  .nav-links {{ display: flex; gap: 8px; flex-wrap: wrap; }}
  .nav-link {{ padding: 8px 14px; border-radius: 8px; background: #1e2433; border: 1px solid #2e364f; color: #cbd5e1; text-decoration: none; font-size: 0.85rem; font-weight: 600; }}
  .card {{ background: #151922; border: 1px solid #232a3b; border-radius: 12px; padding: 24px; margin-bottom: 20px; }}
  .card-header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; gap: 10px; }}
  h2 {{ font-size: 1.1rem; color: #f1f5f9; }}
  .badge {{ padding: 4px 10px; border-radius: 999px; font-size: 0.75rem; font-weight: 600; background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.3); }}
  .grid-2 {{ display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }}
  @media (max-width: 640px) {{ .grid-2 {{ grid-template-columns: 1fr; }} }}
  .field {{ margin-bottom: 14px; }}
  label {{ display: block; font-size: 0.85rem; font-weight: 600; color: #cbd5e1; margin-bottom: 6px; }}
  input, select {{ width: 100%; padding: 10px 12px; border-radius: 8px; background: #0d1017; border: 1px solid #2a3246; color: #f8fafc; font-size: 0.9rem; }}
  .hint {{ font-size: 0.78rem; color: #64748b; margin-top: 4px; }}
  .btn {{ padding: 10px 18px; border-radius: 8px; border: none; background: #fe2c55; color: #fff; font-weight: 600; cursor: pointer; font-size: 0.9rem; }}
  .msg {{ margin-top: 12px; padding: 10px 14px; border-radius: 8px; font-size: 0.86rem; display: none; }}
  .msg-ok {{ display: block; background: rgba(16, 185, 129, 0.14); border: 1px solid rgba(16, 185, 129, 0.35); color: #6ee7b7; }}
  .msg-err {{ display: block; background: rgba(239, 68, 68, 0.14); border: 1px solid rgba(239, 68, 68, 0.35); color: #fca5a5; }}
  code {{ background: #0d1017; padding: 2px 6px; border-radius: 4px; color: #38bdf8; font-size: 0.84rem; }}
</style>
</head>
<body>
<div class="container">
  <div class="header">
    <div>
      <h1>TikTok Creator Studio Config &amp; HAR Import</h1>
      <p class="sub">Self-contained module UI routed under <code>{clean_ns}</code> · HAR endpoint: <code>{clean_ns}/v1/config/har</code></p>
    </div>
    <div class="nav-links">
      <a class="nav-link" href="/dashboard#provider-settings">← Provider Settings</a>
      <a class="nav-link" href="/dashboard#modules-section">Installed Modules</a>
    </div>
  </div>

  <div class="card">
    <div class="card-header">
      <h2>Import HAR Capture (download-tiktok.har)</h2>
      <code>{clean_ns}/v1/config/har</code>
    </div>
    <div class="field">
      <label for="har-file">Select or drop HAR capture (.har or .json) to auto-extract Cookie and CSRF token</label>
      <input type="file" id="har-file" accept=".har,.json">
    </div>
    <div id="har-status" class="msg"></div>
  </div>

  <div class="card">
    <div class="card-header">
      <h2>TikTok Creator Studio Settings</h2>
      <span class="badge" id="cfg-badge">{badge_text}</span>
    </div>
    <form id="tt-cfg-form">
      <div class="grid-2">
        <div class="field">
          <label for="tt-base-url">TikTok Base URL</label>
          <input type="text" id="tt-base-url" value="{base_url}" placeholder="https://www.tiktok.com">
        </div>
        <div class="field">
          <label for="tt-enabled">Module Status</label>
          <select id="tt-enabled">
            <option value="true" {"selected" if enabled else ""}>Enabled</option>
            <option value="false" {"" if enabled else "selected"}>Disabled</option>
          </select>
        </div>
      </div>
      <div class="grid-2">
        <div class="field">
          <label for="tt-cookie">Session Cookie</label>
          <input type="password" id="tt-cookie" autocomplete="off" placeholder="Leave empty to keep existing cookie">
          <div class="hint" id="hint-cookie">Current: {masked_cookie}</div>
        </div>
        <div class="field">
          <label for="tt-csrf">CSRF Token (tt-csrf-token)</label>
          <input type="password" id="tt-csrf" autocomplete="off" placeholder="Leave empty to keep existing CSRF token">
          <div class="hint" id="hint-csrf">Current: {masked_csrf}</div>
        </div>
      </div>
      <button type="submit" class="btn">Save TikTok Configuration</button>
      <div id="cfg-status" class="msg"></div>
    </form>
  </div>
</div>
<script>
(function() {{
  const ns = "{clean_ns}";
  const harStatus = document.getElementById("har-status");
  const cfgStatus = document.getElementById("cfg-status");
  const hintCookie = document.getElementById("hint-cookie");
  const hintCsrf = document.getElementById("hint-csrf");
  const badge = document.getElementById("cfg-badge");

  function updateHints(d) {{
    if (d.masked_cookie !== undefined) hintCookie.textContent = "Current: " + (d.masked_cookie || "not set");
    if (d.masked_csrf_token !== undefined) hintCsrf.textContent = "Current: " + (d.masked_csrf_token || "not set");
    if (d.configured !== undefined) badge.textContent = d.configured ? "Configured" : "Not Configured";
  }}

  document.getElementById("har-file").addEventListener("change", function(e) {{
    const f = e.target.files[0];
    if (!f) return;
    f.text().then(function(txt) {{
      return fetch(ns + "/v1/config/har", {{ method: "POST", headers: {{ "Content-Type": "application/json" }}, body: txt }});
    }}).then(function(r) {{ return r.json().then(function(j) {{ return {{ ok: r.ok, json: j }}; }}); }}).then(function(res) {{
      harStatus.textContent = res.ok ? (res.json.message || "HAR credentials extracted and saved.") : ((res.json.error && res.json.error.message) || "HAR import failed");
      harStatus.className = "msg " + (res.ok ? "msg-ok" : "msg-err");
      if (res.ok) updateHints(res.json);
    }});
  }});

  document.getElementById("tt-cfg-form").addEventListener("submit", function(e) {{
    e.preventDefault();
    const payload = {{
      base_url: document.getElementById("tt-base-url").value.trim(),
      enabled: document.getElementById("tt-enabled").value === "true"
    }};
    const cookieVal = document.getElementById("tt-cookie").value.trim();
    const csrfVal = document.getElementById("tt-csrf").value.trim();
    if (cookieVal) payload.cookie = cookieVal;
    if (csrfVal) payload.csrf_token = csrfVal;

    fetch(ns + "/v1/config", {{
      method: "POST",
      headers: {{ "Content-Type": "application/json" }},
      body: JSON.stringify(payload)
    }}).then(function(r) {{ return r.json().then(function(j) {{ return {{ ok: r.ok, json: j }}; }}); }}).then(function(res) {{
      cfgStatus.textContent = res.ok ? (res.json.message || "Configuration saved.") : ((res.json.error && res.json.error.message) || "Failed to save");
      cfgStatus.className = "msg " + (res.ok ? "msg-ok" : "msg-err");
      if (res.ok) {{
        document.getElementById("tt-cookie").value = "";
        document.getElementById("tt-csrf").value = "";
        updateHints(res.json);
      }}
    }});
  }});
}})();
</script>
</body>
</html>"""
