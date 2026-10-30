#!/usr/bin/env python3
"""
TikTok Creator Studio Adapter for Toolcase Gateway (stdio-stream protocol).
"""

import json
import os
import sys
import time
import urllib.parse
import uuid
from tiktok_studio.client import (
    build_masked_config_view,
    fetch_items,
    load_module_config,
    resolve_download,
    save_module_config,
)
from tiktok_studio.domain import extract_tiktok_config_from_har
from tiktok_studio.ui import render_module_ui_html


def json_resp(status: int, payload: dict) -> dict:
    return {
        "status": status,
        "headers": {"content-type": "application/json"},
        "body": json.dumps(payload),
    }


def html_resp(status: int, html: str) -> dict:
    return {
        "status": status,
        "headers": {"content-type": "text/html; charset=utf-8"},
        "body": html,
    }


def normalize_headers(raw_headers) -> dict:
    if isinstance(raw_headers, dict):
        return {str(k).lower(): str(v) for k, v in raw_headers.items()}
    if isinstance(raw_headers, list):
        out = {}
        for item in raw_headers:
            if isinstance(item, (list, tuple)) and len(item) == 2:
                out[str(item[0]).lower()] = str(item[1])
        return out
    return {}


def handle_request(req: dict) -> dict:
    method = str(req.get("method") or "GET").upper()
    raw_path = str(req.get("path") or "/")
    clean_path, _, query_str = raw_path.partition("?")
    sub = clean_path.strip("/")
    headers = normalize_headers(req.get("headers"))
    raw_body = str(req.get("body") or "")
    namespace = os.environ.get("MODULE_NAMESPACE", "/custom/tiktok")
    cfg = load_module_config()

    # 1. Self-contained UI config page
    if method == "GET" and sub in ("", "ui", "settings", "config-page", "v1/config-page"):
        return html_resp(200, render_module_ui_html(namespace))

    # 2. Status endpoint
    if method == "GET" and sub in ("status", "v1/status", "studio/status"):
        masked = build_masked_config_view(cfg)
        return json_resp(
            200,
            {
                "module": "tiktok-studio",
                "status": "enabled" if cfg.get("enabled", True) else "disabled",
                "namespace": namespace,
                **masked,
            },
        )

    # 3. Config read/write
    if sub in ("config", "v1/config", "studio/config"):
        if method == "GET":
            return json_resp(200, {"status": "ok", **build_masked_config_view(cfg)})
        if method == "POST":
            try:
                body_json = json.loads(raw_body) if raw_body else {}
                updated = save_module_config(body_json)
                return json_resp(
                    200,
                    {
                        "status": "ok",
                        "message": "TikTok Creator Studio configuration updated and saved.",
                        **build_masked_config_view(updated),
                    },
                )
            except Exception as exc:
                return json_resp(400, {"error": {"code": 400, "message": str(exc)}})

    # 4. HAR Import endpoint
    if method == "POST" and sub in ("config/har", "v1/config/har", "studio/config/har"):
        try:
            extracted = extract_tiktok_config_from_har(raw_body)
            updated = save_module_config(extracted)
            return json_resp(
                200,
                {
                    "status": "ok",
                    "message": "HAR credentials extracted and saved.",
                    **build_masked_config_view(updated),
                },
            )
        except Exception as exc:
            return json_resp(400, {"error": {"code": 400, "message": str(exc)}})

    # 5. Items & Download endpoints
    if sub in ("items", "v1/items", "items/list"):
        try:
            q = urllib.parse.parse_qs(query_str)
            cursor = q.get("cursor", ["0"])[0]
            count = int(q.get("count", ["10"])[0])
            return json_resp(200, fetch_items(cfg, headers, cursor=cursor, count=count))
        except PermissionError as exc:
            return json_resp(401, {"error": {"code": 401, "message": str(exc)}})
        except ValueError as exc:
            return json_resp(400, {"error": {"code": 400, "message": str(exc)}})
        except Exception as exc:
            return json_resp(502, {"error": {"code": 502, "message": str(exc)}})

    if sub in ("download", "v1/download", "items/download"):
        try:
            q = urllib.parse.parse_qs(query_str)
            item_id = q.get("item_id", [""])[0]
            return json_resp(200, resolve_download(cfg, headers, item_id=item_id))
        except ValueError as exc:
            return json_resp(400, {"error": {"code": 400, "message": str(exc)}})
        except PermissionError as exc:
            return json_resp(401, {"error": {"code": 401, "message": str(exc)}})
        except LookupError as exc:
            return json_resp(404, {"error": {"code": 404, "message": str(exc)}})
        except Exception as exc:
            return json_resp(502, {"error": {"code": 502, "message": str(exc)}})

    # 6. OpenAI-compatible models & chat completions for Gateway compatibility
    if sub.endswith("models"):
        return json_resp(
            200,
            {
                "object": "list",
                "data": [
                    {
                        "id": "tiktok-studio",
                        "object": "model",
                        "created": int(time.time()),
                        "owned_by": "tiktok-studio",
                    }
                ],
            },
        )

    return json_resp(
        200,
        {
            "id": f"chatcmpl-tt-{uuid.uuid4().hex[:8]}",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": "tiktok-studio",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": f"[TikTok Creator Studio Adapter] Ready at {namespace}",
                    },
                    "finish_reason": "stop",
                }
            ],
            "usage": {"prompt_tokens": 5, "completion_tokens": 8, "total_tokens": 13},
        },
    )


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
            resp = handle_request(req)
        except Exception as exc:
            resp = json_resp(500, {"error": {"code": 500, "message": str(exc)}})
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
