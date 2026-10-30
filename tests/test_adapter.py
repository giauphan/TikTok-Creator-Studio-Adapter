import json
import os
import sys
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from tiktok_studio.domain import (
    extract_best_download_url,
    extract_tiktok_config_from_har,
    mask_secret,
    parse_item_list_response,
    validate_count,
    validate_item_id,
)
from adapter import handle_request


class TestTikTokStudioModule(unittest.TestCase):
    def test_extract_best_download_url_prefers_cdn(self):
        urls = [
            "https://www.tiktok.com/aweme/v1/play/?video_id=v1",
            "https://v16-webapp-prime.tiktok.com/video/tos/alisg/tos-alisg-pve-0037c001/abc/?a=1988",
            "https://v16m-default.tiktokcdn.com/abc123/video/tos/alisg/v-1/?a=0",
        ]
        self.assertEqual(
            extract_best_download_url(urls),
            "https://v16m-default.tiktokcdn.com/abc123/video/tos/alisg/v-1/?a=0",
        )

    def test_extract_har_credentials(self):
        har = json.dumps(
            {
                "log": {
                    "entries": [
                        {
                            "request": {
                                "url": "https://www.tiktok.com/api/v1/item/delivery/list?count=10",
                                "headers": [
                                    {"name": "Cookie", "value": "sessionid=abc123456789; sid_tt=abc"},
                                    {"name": "x-tt-csrf-token", "value": "csrf_token_987654321"},
                                ],
                                "cookies": [],
                            }
                        }
                    ]
                }
            }
        )
        cfg = extract_tiktok_config_from_har(har)
        self.assertEqual(cfg["base_url"], "https://www.tiktok.com")
        self.assertIn("sessionid=abc123456789", cfg["cookie"])
        self.assertEqual(cfg["csrf_token"], "csrf_token_987654321")

    def test_validation_and_masking(self):
        self.assertEqual(validate_item_id("7489123456789012345"), "7489123456789012345")
        self.assertEqual(validate_count(10), 10)
        self.assertEqual(mask_secret("sessionid_123456789"), "sess...6789")

    def test_adapter_status_and_ui(self):
        ui_resp = handle_request({"method": "GET", "path": "/ui"})
        self.assertEqual(ui_resp["status"], 200)
        self.assertIn("TikTok Creator Studio Config", ui_resp["body"])

        status_resp = handle_request({"method": "GET", "path": "/v1/status"})
        self.assertEqual(status_resp["status"], 200)
        body = json.loads(status_resp["body"])
        self.assertEqual(body["module"], "tiktok-studio")


if __name__ == "__main__":
    unittest.main()
