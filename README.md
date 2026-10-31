# TikTok Creator Studio Adapter (`tiktok-studio`)

Self-contained **TikTok Creator Studio** provider module for [Toolcase Gateway](https://github.com/giauphan/toolcase-gateway-rs).

Provides creator post listings, watermark-free CDN video download link resolution (`playAddr`), HAR capture credential extraction (`download-tiktok.har`), video upload authorization, and post publishing—with an automatic schema-driven UI configuration form in the Gateway Dashboard.

---

## 1. Install into Toolcase Gateway

### Via Dashboard UI (1-Click or Custom GitHub Install)
1. Open `http://127.0.0.1:28130/dashboard#module-catalog`.
2. Click **Explore & install** on **TikTok Creator Studio Adapter** (`tiktok-studio`), or paste:
   ```text
   https://github.com/giauphan/TikTok-Creator-Studio-Adapter.git
   ```

### Via REST API
```bash
curl -X POST http://127.0.0.1:28130/api/v1/dashboard/modules/install \
  -H "Content-Type: application/json" \
  -d '{
    "source": "https://github.com/giauphan/TikTok-Creator-Studio-Adapter.git",
    "branch": "main"
  }'
```

---

## 2. Automatic UI Configuration & Menu Positioning

All configuration metadata and `config_schema` live inside [`module.json`](./module.json):
- **Dashboard Provider Settings Tab**: Automatically generated at `/dashboard#provider-settings` (`tiktok-studio`).
- **Standalone Module Config Page**: Automatically served at `/modules/tiktok-studio/config` (and module-owned UI at `/custom/tiktok/ui`).
- **Menu Position Controls**: Can be positioned in `providers`, `sidebar`, `studios`, or `hidden` via `POST /api/v1/dashboard/modules/position`.

---

## 3. Module Endpoints (`/tiktok` & `/custom/tiktok`)

| Method & Path | Description |
|---|---|
| `GET /tiktok/ui` | Self-contained HTML configuration & HAR import UI |
| `GET /tiktok/v1/status` | Module readiness and masked credential status |
| `GET /tiktok/v1/user` | Fetch authenticated Creator profile (`user_id`, `nickname`) |
| `GET /tiktok/v1/config` | Read current masked configuration |
| `POST /tiktok/v1/config` | Update `base_url`, `cookie`, `csrf_token`, `enabled` |
| `POST /tiktok/v1/config/har` | Extract credentials from a DevTools HAR capture (`download-tiktok.har`) |
| `GET /tiktok/v1/items?cursor=0&count=10` | List creator videos with stats and watermark-free `download_url` |
| `GET /tiktok/v1/download?item_id=<id>` | Resolve direct watermark-free CDN download link |
| `GET /tiktok/v1/upload/auth` | Request upload authorization (`video_token_v5`, `store_region`) |
| `POST /tiktok/v1/publish` | Publish video post with caption and hashtags |

---

## 4. Video Publishing Workflow

For complete instructions, request parameters, and Python examples, see the [Video Publishing Guide](./docs/PUBLISHING.md).

### Quick Publish Example

```bash
# 1. Obtain upload authorization token
AUTH=$(curl -s http://127.0.0.1:28130/tiktok/v1/upload/auth)
TOKEN=$(echo "$AUTH" | jq -r '.video_token_v5')

# 2. Upload video binary to TikTok media store
# (Upload using your video asset and the received token)

# 3. Publish post to TikTok Creator Center
curl -X POST http://127.0.0.1:28130/tiktok/v1/publish \
  -H "Content-Type: application/json" \
  -d "{
    \"video_id\": \"$TOKEN\",
    \"text\": \"Created with Toolcase Gateway! #ai #creator #tech\"
  }"
```

---

## 5. Running Tests

```bash
python3 -m unittest discover -s tests
```
