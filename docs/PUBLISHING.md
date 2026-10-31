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

## 3. Video Publishing Workflows

You can publish videos in one of two ways:
- **Simplified 1-Step Publishing (Recommended)**: Pass `video_path` directly to `/tiktok/v1/publish`. The adapter automatically negotiates ByteDance TOS tokens, signs AWS SigV4 multipart chunks, commits the upload, and publishes or schedules the post in a single call.
- **Manual 3-Step Lifecycle**: Request upload authorization (`/v1/upload/auth`), upload chunks manually, then submit `/v1/publish` with `video_id`.

---

### Option A: 1-Step Direct Publishing (Recommended)

Simply point to your video file on disk:

```bash
curl -X POST http://127.0.0.1:28130/tiktok/v1/publish \
  -H "Content-Type: application/json" \
  -d '{
    "video_path": "/path/to/my_video.mp4",
    "text": "Created with Toolcase Gateway! #ai #tech #creator",
    "visibility": "public"
  }'
```

---

### Option B: Manual 3-Step Lifecycle

#### Step 1: Request Upload Authorization

Request upload authorization tokens from TikTok Creator Studio:

```bash
curl -s http://127.0.0.1:28130/tiktok/v1/upload/auth
```

##### Response
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

#### Step 2: Upload Video Binary

Upload your `.mp4` video binary directly to TikTok's media ingestion cluster using the `video_token_v5` and target region:

```bash
# Example uploading to TikTok media store
curl -X POST "https://tos-${store_region}.tiktok.com/upload/video" \
  -H "Authorization: Bearer ${video_token_v5}" \
  -H "Content-Type: video/mp4" \
  --data-binary "@my_video.mp4"
```

#### Step 3: Publish Video Post with Visibility / Status

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

---

### Request Parameters (`POST /tiktok/v1/publish`)

| Field | Type | Required | Description |
|---|---|---|---|
| `video_path` / `file_path` | `string` | Conditional | Path to local `.mp4` video file for automatic 1-step upload |
| `video_base64` | `string` | Conditional | Base64-encoded video binary for automatic 1-step upload |
| `video_id` | `string` | Conditional | Video token/ID (required if not using `video_path` or `video_base64`) |
| `text` / `title` | `string` | **Yes** | Post description, caption, hashtags (`#tag`), and mentions (`@username`) |
| `schedule_time` | `integer` | No | Unix timestamp (in seconds) for scheduled publication (between 15 min and 30 days ahead) |
| `visibility` / `privacy` | `string` | No | Video visibility: `"only-me"` (private, default), `"public"` (everyone), or `"friends"`. Note: Scheduled posts automatically use `"public"`. |
| `tz_name` | `string` | No | Timezone name for scheduled publication (default: `"UTC"`, e.g. `"America/New_York"`) |
| `allow_comment` | `boolean` | No | Whether to allow comments (default: `true`) |
| `allow_duet` | `boolean` | No | Whether to allow Duets (default: `false`) |
| `allow_stitch` | `boolean` | No | Whether to allow Stitches (default: `false`) |

#### Immediate Publish Response Example
```json
{
  "project_id": "7702355712046828820",
  "item_id": "7702355712046828821",
  "project_status": 1,
  "visibility_type": 1,
  "visibility": "only-me",
  "status_code": 0,
  "status_msg": "",
  "status": "published"
}
```

- `project_id`: Creator Center project ID for the published post.
- `item_id`: TikTok item/video identifier.
- `project_status`: Status indicator (`1` = submitted/processing).
- `visibility_type`: `0` = public, `1` = private ("Only me"), `2` = friends only.
- `status`: `"published"` for immediate posts, `"scheduled"` for scheduled posts.
- `status_code`: `0` indicates success.

---

## 4. Video Scheduling (TikTok Creator Center)

The TikTok Creator Studio adapter supports scheduling video publications in advance on creator accounts with scheduling capabilities.

### How Scheduling Works
1. **Timing Window**: The `schedule_time` parameter must be an integer Unix timestamp between **15 minutes** and **30 days** in the future.
2. **Automatic Storage Acknowledgment**: Before dispatching a scheduled post, the adapter automatically calls TikTok's `/api/v1/post_schedule/ack/?ack_type=2` endpoint to authorize server-side video staging, avoiding upstream `Invalid parameters` (`status_code: 5`) errors.
3. **Visibility Invariant**: TikTok web does not allow private scheduled posts; scheduled videos are automatically published with `visibility_type: 0` (`"public"`).

### Schedule Post Example (`curl`)

```bash
# Calculate timestamp: e.g. 2 days in the future
SCHEDULE_TS=$(($(date +%s) + 172800))

curl -X POST http://127.0.0.1:28130/tiktok/v1/publish \
  -H "Content-Type: application/json" \
  -d "{
    \"video_path\": \"/path/to/my_video.mp4\",
    \"text\": \"Scheduled release for later this week! #ai #future\",
    \"schedule_time\": ${SCHEDULE_TS}
  }"
```

#### Scheduled Response Example
```json
{
  "project_id": "7702784686095111175",
  "item_id": "7702784742617337095",
  "project_status": 1,
  "visibility_type": 0,
  "visibility": "public",
  "status_code": 0,
  "status_msg": "",
  "status": "scheduled",
  "schedule_time": 1793613600
}
```

---

## 5. Verifying Published & Scheduled Posts

Query your Creator Studio library to verify posts, view processing state, and retrieve direct CDN download links:

```bash
curl -s "http://127.0.0.1:28130/tiktok/v1/items?count=5"
```

### Response Example
```json
{
  "status_code": 0,
  "has_more": false,
  "total": 2,
  "items": [
    {
      "id": "7702784742617337095",
      "item_id": "7702784742617337095",
      "desc": "Scheduled release for later this week! #ai #future",
      "create_time": 1793444335,
      "post_time": 1793613600,
      "schedule_time": 1793613600,
      "is_scheduled": true,
      "duration_ms": 10027,
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

- `is_scheduled`: `true` indicates the post is queued for a future release timestamp.
- `schedule_time`: Target future publication timestamp in seconds.
- `post_time`: Publication timestamp (matches `schedule_time` for scheduled posts).

---

## 6. Complete Python Publishing Examples

### Example 1: 1-Step Publishing (Direct File Upload)
```python
import json
import urllib.request

GATEWAY_BASE = "http://127.0.0.1:28130/tiktok"

def post_json(path, payload):
    url = f"{GATEWAY_BASE}{path}"
    headers = {"Content-Type": "application/json"}
    req = urllib.request.Request(url, data=json.dumps(payload).encode("utf-8"), headers=headers, method="POST")
    with urllib.request.urlopen(req) as resp:
        return json.loads(resp.read().decode("utf-8"))

# Publish video directly using local file path
result = post_json("/v1/publish", {
    "video_path": "render.mp4",
    "text": "Created with Toolcase Gateway! #ai #creator #tech",
    "visibility": "public"
})
print("Published post result:", result)
```

### Example 2: Scheduled Publishing
```python
import time
import json
import urllib.request

GATEWAY_BASE = "http://127.0.0.1:28130/tiktok"

# Schedule post for 2 days from now (Unix timestamp in seconds)
schedule_timestamp = int(time.time()) + (2 * 86400)

scheduled_result = post_json("/v1/publish", {
    "video_path": "render.mp4",
    "text": "Scheduled premier! #ai #future",
    "schedule_time": schedule_timestamp
})
print(f"Post scheduled for timestamp {schedule_timestamp}:", scheduled_result)
```

---

## 7. Error Handling & Troubleshooting

| HTTP Status | Error Reason | Resolution |
|---|---|---|
| `401 Unauthorized` | Missing or expired TikTok `sessionid` cookie | Re-import your browser `.har` capture or update the session cookie in Dashboard. |
| `401 Unauthorized` | Missing or mismatched CSRF token | Ensure `tt-csrf-token` / `X-Secsdk-Csrf-Token` is set in module configuration. |
| `400 Bad Request` | Invalid `video_id` format | Ensure `video_id` is a valid identifier from upload authorization. |
| `502 Bad Gateway` | TikTok upstream service rate limit or timeout | Implement exponential backoff retry. Ensure `base_url` points to `https://www.tiktok.com`. |
| `503 Service Unavailable` | Module is disabled | Enable the module in Gateway Dashboard or send `POST /api/v1/dashboard/modules/config` with `{"id": "tiktok-studio", "config": {"enabled": true}}`. |
