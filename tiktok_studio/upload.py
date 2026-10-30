"""ByteDance TOS upload and project creation helpers for TikTok Creator Studio."""

from __future__ import annotations

import hashlib
import hmac
import json
import random
import string
import urllib.parse
import uuid
import zlib
from datetime import datetime


def sigv4_headers(
    sk: str,
    ak: str,
    st: str,
    method: str,
    host: str,
    path: str,
    query_params: dict,
    body_bytes: bytes,
    service: str = "vod",
    region: str = "ap-singapore-1",
) -> tuple[dict, str]:
    now = datetime.utcnow()
    amz_date = now.strftime("%Y%m%dT%H%M%SZ")
    date_stamp = now.strftime("%Y%m%d")

    canonical_headers = (
        f"host:{host}\nx-amz-date:{amz_date}\nx-amz-security-token:{st}\n"
    )
    signed_headers = "host;x-amz-date;x-amz-security-token"
    payload_hash = hashlib.sha256(body_bytes).hexdigest()

    canonical_querystring = "&".join(
        [
            f"{urllib.parse.quote(k, safe='')}={urllib.parse.quote(query_params[k], safe='')}"
            for k in sorted(query_params.keys())
        ]
    )
    canonical_request = (
        f"{method}\n{path}\n{canonical_querystring}\n"
        f"{canonical_headers}\n{signed_headers}\n{payload_hash}"
    )

    algorithm = "AWS4-HMAC-SHA256"
    credential_scope = f"{date_stamp}/{region}/{service}/aws4_request"
    string_to_sign = (
        f"{algorithm}\n{amz_date}\n{credential_scope}\n"
        f"{hashlib.sha256(canonical_request.encode('utf-8')).hexdigest()}"
    )

    def _sign(key: bytes, msg: str) -> bytes:
        return hmac.new(key, msg.encode("utf-8"), hashlib.sha256).digest()

    k_date = _sign(("AWS4" + sk).encode("utf-8"), date_stamp)
    k_region = _sign(k_date, region)
    k_service = _sign(k_region, service)
    signing_key = _sign(k_service, "aws4_request")
    signature = hmac.new(
        signing_key, string_to_sign.encode("utf-8"), hashlib.sha256
    ).hexdigest()

    auth_header = (
        f"{algorithm} Credential={ak}/{credential_scope}, "
        f"SignedHeaders={signed_headers}, Signature={signature}"
    )
    headers = {
        "Authorization": auth_header,
        "X-Amz-Date": amz_date,
        "X-Amz-Security-Token": st,
    }
    return headers, canonical_querystring


def create_project(base_url: str, headers: dict) -> str:
    from .client import fetch_upstream_json

    cid = "".join(random.choices(string.ascii_letters + string.digits, k=21))
    create_url = f"{base_url}/api/v1/web/project/create/?creation_id={cid}&type=1&aid=1988"
    fetch_upstream_json(create_url, method="POST", headers=headers)
    return cid


def upload_video_binary(cfg: dict, req_headers: dict, video_bytes: bytes) -> str:
    """Uploads raw video bytes to ByteDance TOS and commits the video, returning the Vid."""
    from .client import _require_cookie, fetch_upstream_json

    base_url, headers = _require_cookie(cfg, req_headers)
    raw = fetch_upstream_json(
        f"{base_url}/api/v1/video/upload/auth/?aid=1988", headers=headers
    )
    v_tok = (raw or {}).get("video_token_v5", {})
    ak = v_tok.get("access_key_id", "")
    sk = v_tok.get("secret_acess_key", "")
    st = v_tok.get("session_token", "")
    if not (ak and sk and st):
        raise PermissionError("Failed to obtain video upload token from TikTok")

    rnd_s = "".join(random.choices(string.ascii_lowercase + string.digits, k=11))
    apply_params = {
        "Action": "ApplyUploadInner",
        "Version": "2020-11-19",
        "SpaceName": "tiktok",
        "FileType": "video",
        "IsInner": "1",
        "FileSize": str(len(video_bytes)),
        "s": rnd_s,
    }
    sig_headers, apply_qs = sigv4_headers(
        sk, ak, st, "GET", "www.tiktok.com", "/top/v1", apply_params, b""
    )
    apply_url = f"{base_url}/top/v1?{apply_qs}"
    apply_data = fetch_upstream_json(
        apply_url, headers={**headers, **sig_headers}
    )
    node = (
        apply_data.get("Result", {})
        .get("InnerUploadAddress", {})
        .get("UploadNodes", [{}])[0]
    )
    vid = node.get("Vid", "")
    upload_host = node.get("UploadHost", "")
    session_key = node.get("SessionKey", "")
    store_info = node.get("StoreInfos", [{}])[0]
    store_uri = store_info.get("StoreUri", "")
    store_auth = store_info.get("Auth", "")
    if not (vid and upload_host and store_uri and store_auth):
        raise RuntimeError("ApplyUploadInner did not return upload endpoints")

    upload_id = str(uuid.uuid4())
    crc_val = format(zlib.crc32(video_bytes) & 0xFFFFFFFF, "08x")

    transfer_url = (
        f"https://{upload_host}/{store_uri}?"
        f"partNumber=1&uploadID={upload_id}&phase=transfer"
    )
    transfer_headers = {
        "Authorization": store_auth,
        "Content-Type": "application/octet-stream",
        "Content-Disposition": 'attachment; filename="undefined"',
        "Content-Crc32": crc_val,
    }
    fetch_upstream_json(
        transfer_url,
        method="POST",
        headers=transfer_headers,
        body=video_bytes,
    )

    finish_url = (
        f"https://{upload_host}/{store_uri}?"
        f"uploadID={upload_id}&phase=finish&uploadmode=part"
    )
    finish_headers = {
        "Authorization": store_auth,
        "Content-Type": "text/plain;charset=UTF-8",
    }
    fetch_upstream_json(
        finish_url,
        method="POST",
        headers=finish_headers,
        body=f"1:{crc_val}".encode("utf-8"),
    )

    commit_body = json.dumps(
        {"SessionKey": session_key, "Functions": [{"name": "GetMeta"}]}
    ).encode("utf-8")
    commit_params = {
        "Action": "CommitUploadInner",
        "Version": "2020-11-19",
        "SpaceName": "tiktok",
    }
    commit_sig_headers, commit_qs = sigv4_headers(
        sk, ak, st, "POST", "www.tiktok.com", "/top/v1", commit_params, commit_body
    )
    commit_url = f"{base_url}/top/v1?{commit_qs}"
    fetch_upstream_json(
        commit_url,
        method="POST",
        headers={
            **headers,
            **commit_sig_headers,
            "Content-Type": "application/json",
        },
        body=commit_body,
    )
    return vid
