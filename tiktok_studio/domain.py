"""Domain logic, boundary validation, HAR credential extraction, and CDN URL selection."""

import json
from urllib.parse import urlparse


def mask_secret(secret: str) -> str:
    trimmed = (secret or "").strip()
    if not trimmed:
        return ""
    if len(trimmed) <= 8:
        return "****"
    return f"{trimmed[:4]}...{trimmed[-4:]}"


def validate_item_id(item_id: str) -> str:
    clean = (item_id or "").strip()
    if not clean or len(clean) > 128:
        raise ValueError("item_id must be 1..128 characters")
    if not all(c.isalnum() or c in ("-", "_") for c in clean):
        raise ValueError("item_id contains invalid characters")
    return clean


def validate_count(count: int) -> int:
    if count < 1 or count > 50:
        raise ValueError("count must be between 1 and 50")
    return count


def extract_best_download_url(url_list: list) -> str:
    if not url_list:
        return ""
    valid = [
        u.strip()
        for u in url_list
        if isinstance(u, str)
        and (u.strip().startswith("https://") or u.strip().startswith("http://"))
    ]
    if not valid:
        return ""
    for candidate in valid:
        if "tiktokcdn.com" in candidate and "/aweme/v1/play/" not in candidate:
            return candidate
    return valid[0]


def parse_item_list_response(raw_json: dict) -> dict:
    item_list = raw_json.get("item_list") or raw_json.get("itemList") or []
    items = []
    for entry in item_list:
        if not isinstance(entry, dict):
            continue
        item_id = str(entry.get("id") or entry.get("item_id") or "")
        desc = str(entry.get("desc") or entry.get("title") or "")
        create_time = int(entry.get("createTime") or entry.get("create_time") or 0)
        video = entry.get("video") or {}
        duration = int(video.get("duration") or 0)
        play_addr = video.get("playAddr") or video.get("play_addr") or {}
        if isinstance(play_addr, dict):
            url_list = play_addr.get("urlList") or play_addr.get("url_list") or []
        elif isinstance(play_addr, list):
            url_list = play_addr
        else:
            url_list = []
        cover_obj = video.get("cover") or {}
        if isinstance(cover_obj, dict):
            cover_urls = cover_obj.get("urlList") or cover_obj.get("url_list") or []
            cover_url = cover_urls[0] if cover_urls else ""
        else:
            cover_url = str(cover_obj or "")
        stats = entry.get("stats") or entry.get("statistics") or {}
        items.append(
            {
                "id": item_id,
                "desc": desc,
                "create_time": create_time,
                "duration_ms": duration,
                "cover_url": cover_url,
                "download_url": extract_best_download_url(url_list),
                "play_urls": url_list,
                "stats": {
                    "play_count": int(stats.get("playCount") or stats.get("play_count") or 0),
                    "like_count": int(stats.get("diggCount") or stats.get("like_count") or 0),
                    "comment_count": int(stats.get("commentCount") or stats.get("comment_count") or 0),
                    "share_count": int(stats.get("shareCount") or stats.get("share_count") or 0),
                },
            }
        )
    return {
        "status_code": int(raw_json.get("status_code") or 0),
        "has_more": bool(raw_json.get("has_more") or raw_json.get("hasMore") or False),
        "cursor": str(raw_json.get("cursor") or raw_json.get("max_cursor") or "0"),
        "total": int(raw_json.get("total") or len(items)),
        "items": items,
    }


def extract_tiktok_config_from_har(raw_har: str) -> dict:
    try:
        root = json.loads(raw_har)
    except Exception as exc:
        raise ValueError(f"HAR is not valid JSON: {exc}") from exc

    entries = (root.get("log") or {}).get("entries")
    if not isinstance(entries, list):
        raise ValueError("HAR missing log.entries array")

    best_cookie = ""
    best_csrf = ""
    detected_base_url = ""

    for entry in entries:
        req = (entry or {}).get("request") or {}
        url = str(req.get("url") or "")
        if "tiktok.com" not in url and "tiktokstudio" not in url:
            continue

        is_studio_api = any(
            p in url
            for p in (
                "/api/v1/item/delivery/list",
                "/tiktok/v1/upload/auth",
                "/api/v1/web/project/",
                "/passport/web/account/info",
                "/tiktokstudio",
            )
        )

        if not detected_base_url and (is_studio_api or "www.tiktok.com" in url):
            parsed = urlparse(url)
            if parsed.scheme and parsed.netloc and "tiktok.com" in parsed.netloc:
                detected_base_url = f"{parsed.scheme}://{parsed.netloc}"

        headers = req.get("headers") or []
        for hdr in headers:
            name = str(hdr.get("name") or "").lower()
            val = str(hdr.get("value") or "").strip()
            if not val:
                continue
            if name == "cookie":
                has_session = any(
                    k in val for k in ("sessionid", "sid_tt", "uid_tt", "ttwid")
                )
                if (has_session and len(val) >= len(best_cookie)) or not best_cookie:
                    best_cookie = val
            elif name in ("x-tt-csrf-token", "tt-csrf-token", "x-secsdk-csrf-token"):
                if not best_csrf:
                    best_csrf = val

        cookies = req.get("cookies") or []
        if cookies:
            pairs = []
            for c in cookies:
                c_name = str(c.get("name") or "").strip()
                c_val = str(c.get("value") or "").strip()
                if not c_name or not c_val:
                    continue
                if c_name in ("tt_csrf_token", "csrf_session_id", "tt-csrf-token") and not best_csrf:
                    best_csrf = c_val
                pairs.append(f"{c_name}={c_val}")
            joined = "; ".join(pairs)
            if len(joined) > len(best_cookie):
                best_cookie = joined

    if not best_cookie and not best_csrf:
        raise ValueError("No TikTok Creator Studio cookies or CSRF tokens found in HAR capture")

    return {
        "base_url": detected_base_url or "https://www.tiktok.com",
        "cookie": best_cookie,
        "csrf_token": best_csrf,
        "enabled": True,
    }
