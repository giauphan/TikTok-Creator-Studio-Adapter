"""Configuration persistence and upstream HTTP client for TikTok Creator Studio."""

import base64
import hashlib
import hmac
import json
import os
import random
import string
import urllib.parse
import urllib.request
import uuid
import zlib
from datetime import datetime
from pathlib import Path
from .domain import (
    mask_secret,
    parse_item_list_response,
    validate_count,
    validate_item_id,
)
from .signer import TikTokSigner
from .upload import create_project, upload_video_binary


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
    env_json = os.environ.get("MODULE_CONFIG_JSON", "").strip()
    if env_json:
        try:
            parsed_env = json.loads(env_json)
            if isinstance(parsed_env, dict):
                default_cfg.update(parsed_env)
        except Exception:
            pass

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
        or req_headers.get("x-tiktok-csrf-token")
        or cfg.get("csrf_token")
        or ""
    )
    base_url = (cfg.get("base_url") or "https://www.tiktok.com").rstrip("/")
    headers = {
        "User-Agent": (
            req_headers.get("user-agent")
            or "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
            "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
        ),
        "Accept": "application/json, text/plain, */*",
        "Referer": f"{base_url}/tiktokstudio/content",
        "Origin": base_url,
    }
    if cookie:
        headers["Cookie"] = cookie
    if csrf:
        headers["x-tt-csrf-token"] = csrf
        headers["tt-csrf-token"] = csrf
    return headers


def fetch_upstream_json(
    url: str, method: str = "GET", headers: dict = None, body: bytes = None
) -> dict:
    req = urllib.request.Request(url, data=body, headers=headers or {}, method=method)
    with urllib.request.urlopen(req, timeout=20) as resp:
        raw = resp.read().decode("utf-8", errors="replace")
        return json.loads(raw)


def _require_cookie(cfg: dict, req_headers: dict) -> tuple:
    if not cfg.get("enabled", True):
        raise PermissionError("TikTok Creator Studio module is currently disabled.")
    base_url = (cfg.get("base_url") or "https://www.tiktok.com").rstrip("/")
    headers = build_upstream_headers(cfg, req_headers)
    if not headers.get("Cookie"):
        raise PermissionError(
            "Missing TikTok session cookie. Configure credentials or import HAR first."
        )
    return base_url, headers


def fetch_user_profile(cfg: dict, req_headers: dict) -> dict:
    base_url, headers = _require_cookie(cfg, req_headers)
    studio_url = f"{base_url}/tiktokstudio/api/web/user"
    try:
        raw = fetch_upstream_json(studio_url, method="GET", headers=headers)
        if isinstance(raw, dict) and raw.get("userId"):
            base_info = raw.get("userBaseInfo") or {}
            return {
                "user_id": str(raw.get("userId") or ""),
                "nickname": str(base_info.get("nickname") or "Creator"),
                "avatar_thumb": base_info.get("avatarUrl"),
                "status_code": int(raw.get("statusCode") or 0),
            }
    except Exception:
        pass

    fallback_url = f"{base_url}/api/v1/user/profile/upload/?locale=en&aid=1988&priority_region=US"
    raw = fetch_upstream_json(fallback_url, method="GET", headers=headers)
    user = (raw or {}).get("user") or {}
    avatar = ((user.get("avatar_thumb") or {}).get("url_list") or [None])[0]
    return {
        "user_id": str(user.get("uid") or ""),
        "nickname": str(user.get("nickname") or ""),
        "sec_uid": user.get("sec_uid"),
        "avatar_thumb": avatar,
        "status_code": int((raw or {}).get("status_code") or 0),
    }


def fetch_items(
    cfg: dict, req_headers: dict, cursor: str = "0", count: int = 10
) -> dict:
    validate_count(count)
    base_url, headers = _require_cookie(cfg, req_headers)
    try:
        cursor_int = int(cursor or 0)
    except Exception:
        cursor_int = 0
    url = (
        f"{base_url}/tiktok/creator/manage/item_list/v1/"
        "?locale=en&aid=1988&priority_region=US&app_name=tiktok_creator_center"
    )
    payload = json.dumps(
        {
            "cursor": cursor_int,
            "size": count,
            "query": {
                "conditions": [],
                "sort_orders": [{"field_name": "post_time", "order": 2}],
                "is_recent_posts": False,
            },
        }
    ).encode("utf-8")
    post_headers = {**headers, "Content-Type": "application/json"}
    raw = fetch_upstream_json(url, method="POST", headers=post_headers, body=payload)
    return parse_item_list_response(raw)


def resolve_download(cfg: dict, req_headers: dict, item_id: str) -> dict:
    clean_id = validate_item_id(item_id)
    listing = fetch_items(cfg, req_headers, cursor="0", count=50)
    for item in listing.get("items", []):
        if item.get("id") == clean_id and item.get("download_url"):
            return {
                "item_id": clean_id,
                "download_url": item["download_url"],
                "direct_download_url": item["download_url"],
                "play_addrs": item.get("play_urls", []),
                "cover_url": item.get("cover_url", ""),
                "duration_ms": item.get("duration_ms", 0),
            }
    raise LookupError(f"Item '{clean_id}' not found or has no downloadable stream")


def fetch_upload_auth(cfg: dict, req_headers: dict) -> dict:
    base_url, headers = _require_cookie(cfg, req_headers)
    url = f"{base_url}/api/v1/video/upload/auth/?aid=1988"
    raw = fetch_upstream_json(url, method="GET", headers=headers)
    return {
        "video_token_v5": str(raw.get("video_token_v5") or ""),
        "audio_token_v5": str(raw.get("audio_token_v5") or ""),
        "store_region": str(raw.get("store_region") or "alisg"),
        "status_code": int(raw.get("status_code") or 0),
    }


def publish_video_post(cfg: dict, req_headers: dict, body_json: dict) -> dict:
    base_url, headers = _require_cookie(cfg, req_headers)

    # Upload video binary if provided directly, otherwise validate video_id
    video_bytes = None
    if "video_path" in body_json or "file_path" in body_json:
        p = Path(str(body_json.get("video_path") or body_json.get("file_path")))
        if p.is_file():
            video_bytes = p.read_bytes()
    elif "video_base64" in body_json:
        video_bytes = base64.b64decode(str(body_json["video_base64"]))

    if video_bytes:
        video_id = upload_video_binary(cfg, req_headers, video_bytes)
    else:
        video_id = validate_item_id(str(body_json.get("video_id") or ""))

    text = str(body_json.get("text") or body_json.get("title") or "").strip()

    # Determine visibility_type:
    # 0 = Public (everyone)
    # 1 = Only me (private / ony-me)
    # 2 = Friends / Followers
    vis_raw = str(
        body_json.get("visibility")
        or body_json.get("privacy")
        or body_json.get("status")
        or "only-me"
    ).lower().strip()

    if vis_raw in ("0", "public", "everyone"):
        visibility_type = 0
    elif vis_raw in ("2", "friends", "follower", "followers"):
        visibility_type = 2
    else:
        visibility_type = 1

    allow_comment = 1 if bool(body_json.get("allow_comment", True)) else 0
    allow_duet = 1 if bool(body_json.get("allow_duet", False)) else 0
    allow_stitch = 1 if bool(body_json.get("allow_stitch", False)) else 0

    # 1. Create project
    cid = create_project(base_url, headers)

    # 2. Cryptographic signature (_signature and X-Bogus)
    ua = headers.get("User-Agent") or "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
    ms_token = ""
    cookie_str = headers.get("Cookie") or ""
    for pair in cookie_str.split(";"):
        if "=" in pair:
            k, v = pair.strip().split("=", 1)
            if k == "msToken":
                ms_token = v
                break

    base_sign = "https://www.tiktok.com/api/v1/web/project/post/"
    query_sign = (
        f"app_name=tiktok_web&channel=tiktok_web&device_platform=web"
        f"&aid=1988&msToken={ms_token}"
    )
    with TikTokSigner(ua) as signer:
        sig = signer.sign(f"{base_sign}?{query_sign}")

    # 3. Finalize and publish post
    params = {
        "app_name": "tiktok_web",
        "channel": "tiktok_web",
        "device_platform": "web",
        "aid": "1988",
        "msToken": ms_token,
        "X-Bogus": sig["x_bogus"],
        "_signature": sig["signature"],
    }
    post_url = f"{base_url}/tiktok/web/project/post/v1/?{urllib.parse.urlencode(params)}"
    post_payload = json.dumps(
        {
            "post_common_info": {
                "creation_id": cid,
                "enter_post_page_from": 1,
                "post_type": 3,
            },
            "feature_common_info_list": [
                {
                    "geofencing_regions": [],
                    "playlist_name": "",
                    "playlist_id": "",
                    "tcm_params": '{"commerce_toggle_info":{}}',
                    "sound_exemption": 0,
                    "anchors": [],
                    "vedit_common_info": {
                        "draft": "",
                        "video_id": video_id,
                    },
                    "privacy_setting_info": {
                        "visibility_type": visibility_type,
                        "allow_duet": allow_duet,
                        "allow_stitch": allow_stitch,
                        "allow_comment": allow_comment,
                    },
                }
            ],
            "single_post_req_list": [
                {
                    "batch_index": 0,
                    "video_id": video_id,
                    "is_long_video": 0,
                    "single_post_feature_info": {
                        "text": text,
                        "text_extra": [],
                        "markup_text": text,
                        "music_info": {},
                        "poster_delay": 0,
                    },
                }
            ],
        }
    ).encode("utf-8")

    post_headers = {
        **headers,
        "Content-Type": "application/json",
        "Origin": base_url,
        "Referer": f"{base_url}/",
    }
    raw = fetch_upstream_json(post_url, method="POST", headers=post_headers, body=post_payload)

    single_resp = (raw or {}).get("single_post_resp_list", [{}])[0]
    item_id = str(single_resp.get("item_id") or "")

    return {
        "project_id": str(raw.get("project_id") or ""),
        "item_id": item_id,
        "project_status": int(raw.get("project_status") or 0),
        "visibility_type": visibility_type,
        "visibility": "only-me" if visibility_type == 1 else ("public" if visibility_type == 0 else "friends"),
        "status_code": int(raw.get("status_code") or 0),
        "status": "published" if raw.get("status_code") == 0 else "error",
    }
