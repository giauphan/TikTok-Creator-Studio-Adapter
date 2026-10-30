"""Self-contained HTML UI owned inside the TikTok Creator Studio module."""


def render_module_ui_html(namespace: str = "/custom/tiktok") -> str:
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>TikTok Creator Studio Config &amp; HAR Import</title>
<style>
  body {{ font-family: system-ui, sans-serif; background: #0d0f14; color: #e2e8f0; padding: 32px 20px; }}
  .container {{ max-width: 760px; margin: 0 auto; }}
  .card {{ background: #151922; border: 1px solid #232a3b; border-radius: 12px; padding: 24px; margin-bottom: 20px; }}
  input, select {{ width: 100%; padding: 10px; border-radius: 8px; background: #0d1017; border: 1px solid #2a3246; color: #f8fafc; margin-bottom: 12px; }}
  .btn {{ padding: 10px 18px; border-radius: 8px; border: none; background: #fe2c55; color: #fff; font-weight: 600; cursor: pointer; }}
</style>
</head>
<body>
<div class="container">
  <h1>TikTok Creator Studio Config &amp; HAR Import</h1>
  <p>Self-contained module UI routed under <code>{namespace}</code> · HAR endpoint: <code>{namespace}/v1/config/har</code></p>
  <div class="card">
    <h2>Import HAR Capture (download-tiktok.har)</h2>
    <input type="file" id="har-file" accept=".har,.json">
    <div id="har-status"></div>
  </div>
</div>
<script>
document.getElementById("har-file").addEventListener("change", function(e) {{
  const f = e.target.files[0];
  if (!f) return;
  f.text().then(function(txt) {{
    return fetch("{namespace}/v1/config/har", {{ method: "POST", headers: {{ "Content-Type": "application/json" }}, body: txt }});
  }}).then(function(r) {{ return r.json(); }}).then(function(d) {{
    document.getElementById("har-status").textContent = d.message || JSON.stringify(d);
  }});
}});
</script>
</body>
</html>"""
