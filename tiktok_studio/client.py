"""Configuration persistence and upstream HTTP client for TikTok Creator Studio."""

import json
import os
import urllib.parse
import urllib.request
from pathlib import Path
from .domain import mask_secret, parse_item_list_response, validate_count, validate_item_id


def get_config_file_path() -> Path:
    module_dir = Path(__file__).resolve().parent.parent
    return module_dir / "config.json"


def load_module_config() -> dict:
    default_cfg = {
        "base_url": "https://www.tiktok.com",
        "cookie": "",
        "csrf_token": "",
        "enabled": True,
    }
    cfg_path = get_config_file_path()
    if cfg_path.is_file():
        try:
            stored = json.loads(cfg_path.read_text(encoding="utf-8"))
            if isinstance(stored, dict):
                default_cfg.update(stored)
        except Exception:
            pass
    return default_cfg


def save_module_config(updates: dict) -> dict:
    current = load_module_config()
    if isinstance(updates.get("base_url"), str) and updates["base_url"].strip():
        base = updates["base_url"].strip().rstrip("/")
        if not (base.startswith("http://") or base.startswith("https://")):
            raise ValueError("base_url must start with http:// or https://")
        current["base_url"] = base
    if isinstance(updates.get("cookie"), str) and updates["cookie"].strip():
        current["cookie"] = updates["cookie"].strip()
    if isinstance(updates.get("csrf_token"), str) and updates["csrf_token"].strip():
        current["csrf_token"] = updates["csrf_token"].strip()
    if isinstance(updates.get("enabled"), bool):
        current["enabled"] = updates["enabled"]

    cfg_path = get_config_file_path()
    cfg_path.write_text(json.dumps(current, indent=2), encoding="utf-8")
    try:
        os.chmod(cfg_path, 0o600)
    except Exception:
        pass
    return current


def build_masked_config_view(cfg: dict) -> dict:
    cookie = cfg.get("cookie") or ""
    csrf = cfg.get("csrf_token") or ""
    return {
        "base_url": cfg.get("base_url") or "https://www.tiktok.com",
        "enabled": bool(cfg.get("enabled", True)),
        "configured": bool(cookie.strip() or csrf.strip()),
        "has_cookie": bool(cookie.strip()),
        "has_csrf_token": bool(csrf.strip()),
        "masked_cookie": mask_secret(cookie),
        "masked_csrf_token": mask_secret(csrf),
    }


def build_upstream_headers(cfg: dict, req_headers: dict) -> dict:
    cookie = (
        req_headers.get("x-tiktok-cookie")
        or req_headers.get("cookie")
        or cfg.get("cookie")
        or ""
    )
    csrf = (
        req_headers.get("x-tt-csrf-token")
        or req_headers.get("tt-csrf-token")
        or cfg.get("csrf_token")
        or ""
    )
    headers = {
        "User-Agent": (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
            "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
        ),
        "Accept": "application/json, text/plain, */*",
        "Referer": "https://www.tiktok.com/tiktokstudio/content",
    }
    if cookie:
        headers["Cookie"] = cookie
    if csrf:
        headers["x-tt-csrf-token"] = csrf
    return headers


def fetch_upstream_json(url: str, method: str = "GET", headers: dict = None, body: bytes = None) -> dict:
    req = urllib.request.Request(url, data=body, headers=headers or {}, method=method)
    with urllib.request.urlopen(req, timeout=20) as resp:
        raw = resp.read().decode("utf-8", errors="replace")
        return json.loads(raw)


def fetch_items(cfg: dict, req_headers: dict, cursor: str = "0", count: int = 10) -> dict:
    validate_count(count)
    base_url = (cfg.get("base_url") or "https://www.tiktok.com").rstrip("/")
    headers = build_upstream_headers(cfg, req_headers)
    if not headers.get("Cookie"):
        raise PermissionError("Missing TikTok session cookie. Configure credentials or import HAR first.")
    params = urllib.parse.urlencode({"cursor": cursor, "count": count, "aid": "1988"})
    url = f"{base_url}/api/v1/item/delivery/list?{params}"
    raw = fetch_upstream_json(url, method="GET", headers=headers)
    return parse_item_list_response(raw)


def resolve_download(cfg: dict, req_headers: dict, item_id: str) -> dict:
    clean_id = validate_item_id(item_id)
    listing = fetch_items(cfg, req_headers, cursor="0", count=20)
    for item in listing.get("items", []):
        if item.get("id") == clean_id and item.get("download_url"):
            return {
                "item_id": clean_id,
                "download_url": item["download_url"],
                "cover_url": item.get("cover_url", ""),
                "duration_ms": item.get("duration_ms", 0),
            }
    raise LookupError(f"Item '{clean_id}' not found or has no downloadable stream")
