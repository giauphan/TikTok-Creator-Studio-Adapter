# Publishing Videos to TikTok via Creator Studio Adapter

This guide walks through the end-to-end process of publishing videos to TikTok using the **TikTok Creator Studio Adapter** (`tiktok-studio`) connected through **Toolcase Gateway**.

---

## 1. Overview & Architecture

The adapter automates TikTok Creator Studio's web publishing pipeline without requiring a formal enterprise API approval. It authenticates through your Creator session cookies and talks directly to the official TikTok Creator Center endpoints:

```
[ Your App / Client ]
         │
         ▼
[ Toolcase Gateway ] (Routes: /tiktok/v1/* or /custom/tiktok/v1/*)
         │
         ▼
[ TikTok-Creator-Studio-Adapter ]
         │ (Authenticated with Session Cookie & CSRF Token)
         ▼
[ TikTok Creator Center API ] (aid=1988, app_name=tiktok_creator_center)
```

---

## 2. Prerequisites

Before publishing, ensure the adapter is configured and authenticated.

1. **Session Cookie**: Valid TikTok session containing `sessionid` (or full browser cookie header).
2. **CSRF Token**: Active `tt-csrf-token` / `X-Secsdk-Csrf-Token`.
3. **Module Enabled**: Marked as `enabled: true` in Gateway Dashboard.

### Verifying Account Readiness

Run a quick status check:

```bash
curl -s http://127.0.0.1:28130/tiktok/v1/status
```

Expected response:
```json
{
  "module": "tiktok-studio",
  "status": "enabled",
  "configured": true,
  "has_cookie": true,
  "has_csrf_token": true
}
```

Check your authenticated user profile:

```bash
curl -s http://127.0.0.1:28130/tiktok/v1/user
```

Expected response:
```json
{
  "user_id": "7672793713294509077",
  "nickname": "Creator",
  "status_code": 0
}
```

---

## 3. The 3-Step Publishing Lifecycle

Publishing a video requires three steps:
1. **Get Upload Authorization**: Request upload credentials and target storage region.
2. **Upload Binary**: Upload the video file using the returned session authorization tokens.
3. **Publish Post**: Call the publish endpoint with the `video_id`, caption, and hashtags.

---

### Step 1: Request Upload Authorization

Request upload authorization tokens from TikTok Creator Studio:

```bash
curl -s http://127.0.0.1:28130/tiktok/v1/upload/auth
```

#### Response
```json
{
  "video_token_v5": "v1c025g50000dbi47gfog65vcfe2cs80",
  "audio_token_v5": "a1c025g50000dbi47gfog65vcfe2cs80",
  "store_region": "alisg",
  "status_code": 0
}
```

- `video_token_v5`: Unique identifier for the pending video asset.
- `store_region`: Target storage cluster (e.g. `alisg` for Asia, `us-east` for North America).

---

### Step 2: Upload Video Binary

Upload your `.mp4` video binary directly to TikTok's media ingestion cluster using the `video_token_v5` and target region:

```bash
# Example uploading to TikTok media store
curl -X POST "https://tos-${store_region}.tiktok.com/upload/video" \
  -H "Authorization: Bearer ${video_token_v5}" \
  -H "Content-Type: video/mp4" \
  --data-binary "@my_video.mp4"
```

Once the upload completes, the `video_token_v5` (or returned upload `video_id`) is ready to be published.

---

### Step 3: Publish Video Post with Visibility / Status

Submit the publish request with your video identifier, caption text, hashtags, and visibility setting (`only-me`, `public`, or `friends`):

```bash
curl -X POST http://127.0.0.1:28130/tiktok/v1/publish \
  -H "Content-Type: application/json" \
  -d '{
    "video_id": "v1c025g50000dbi47gfog65vcfe2cs80",
    "text": "Exciting AI video generated with Toolcase Gateway! #ai #tech #creator",
    "visibility": "only-me"
  }'
```

#### Request Parameters
| Field | Type | Required | Description |
|---|---|---|---|
| `video_id` | `string` | **Yes** | Valid video token or ID returned from upload authorization |
| `text` | `string` | **Yes** | Post description, caption, hashtags (`#tag`), and mentions (`@username`) |
| `visibility` / `status` | `string` | No | Video visibility: `"only-me"` (private, default), `"public"` (everyone), or `"friends"` |
| `title` | `string` | No | Optional video title |
| `schedule_time` | `integer` | No | Unix timestamp for scheduled publishing (between 15 min and 10 days in future) |
| `allow_comment` | `boolean` | No | Whether to allow comments (default: `true`) |
| `allow_duet` | `boolean` | No | Whether to allow Duets |
| `allow_stitch` | `boolean` | No | Whether to allow Stitches |

#### Response
```json
{
  "project_id": "7702355712046828820",
  "project_status": 1,
  "visibility_type": 1,
  "visibility": "only-me",
  "status_code": 0
}
```

- `visibility_type`: `1` represents private ("Only me" / `visibility_only_you`), `0` represents public, `2` represents friends only.
- `project_id`: Creator Center project ID for the published post.
- `project_status`: Status indicator (`1` = submitted/processing).
- `status_code`: `0` indicates success.

---

## 4. Verifying Published Post

After publishing, check your Creator Studio library to verify the post is live and view analytics:

```bash
curl -s "http://127.0.0.1:28130/tiktok/v1/items?count=5"
```

Response snippet:
```json
{
  "status_code": 0,
  "has_more": false,
  "total": 1,
  "items": [
    {
      "id": "7702355712046828820",
      "item_id": "7702355712046828820",
      "desc": "Exciting AI video generated with Toolcase Gateway! #ai #tech #creator",
      "create_time": 1793344451,
      "duration_ms": 8010,
      "download_url": "https://webapp-sg.tiktok.com/...",
      "stats": {
        "play_count": 0,
        "like_count": 0,
        "comment_count": 0,
        "share_count": 0
      }
    }
  ]
}
```

---

## 5. Complete Python Publishing Example

The following script automates the complete upload and publish workflow:

```python
import json
import urllib.request
import urllib.parse

GATEWAY_BASE = "http://127.0.0.1:28130/tiktok"

def request_json(path, method="GET", body=None):
    url = f"{GATEWAY_BASE}{path}"
    headers = {"Content-Type": "application/json"}
    data = json.dumps(body).encode("utf-8") if body else None
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read().decode("utf-8"))

def publish_video(video_file_path: str, caption: str):
    # 1. Check account readiness
    user = request_json("/v1/user")
    print(f"Logged in as: {user.get('nickname')} (ID: {user.get('user_id')})")

    # 2. Get upload authorization
    auth = request_json("/v1/upload/auth")
    video_token = auth.get("video_token_v5")
    region = auth.get("store_region", "alisg")
    print(f"Obtained upload token: {video_token[:12]}... (region: {region})")

    # 3. Publish post to TikTok Creator Center
    print(f"Publishing post: '{caption}'...")
    publish_payload = {
        "video_id": video_token,
        "text": caption
    }
    result = request_json("/v1/publish", method="POST", body=publish_payload)
    project_id = result.get("project_id")
    print(f"Video published successfully! Project ID: {project_id}")
    return result

if __name__ == "__main__":
    publish_video("render.mp4", "Created with Toolcase Gateway & MuseAI! #animation #vfx")
```

---

## 6. Error Handling & Troubleshooting

| HTTP Status | Error Reason | Resolution |
|---|---|---|
| `401 Unauthorized` | Missing or expired TikTok `sessionid` cookie | Re-import your browser `.har` capture or update the session cookie in Dashboard. |
| `401 Unauthorized` | Missing or mismatched CSRF token | Ensure `tt-csrf-token` / `X-Secsdk-Csrf-Token` is set in module configuration. |
| `400 Bad Request` | Invalid `video_id` format | Ensure `video_id` is a valid identifier from upload authorization. |
| `502 Bad Gateway` | TikTok upstream service rate limit or timeout | Implement exponential backoff retry. Ensure `base_url` points to `https://www.tiktok.com`. |
| `503 Service Unavailable` | Module is disabled | Enable the module in Gateway Dashboard or send `POST /api/v1/dashboard/modules/config` with `{"id": "tiktok-studio", "config": {"enabled": true}}`. |
