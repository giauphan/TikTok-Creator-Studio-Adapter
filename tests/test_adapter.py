import json
import os
import sys
import unittest
from unittest.mock import patch

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
            "https://v16-webapp-prime.tiktok.com/video/tos/alisg/tos-alisg-pve-0037c001/abc/?a=1988",
        )

    def test_parse_studio_item_list_response(self):
        raw = {
            "status_code": 0,
            "has_more": False,
            "cursor": "1729000000",
            "item_list": [
                {
                    "item_id": "7420000000000000001",
                    "desc": "Creator Studio Video #tiktok",
                    "create_time": "1729000000",
                    "duration": 15000,
                    "cover_url": ["https://p16-sign-sg.tiktokcdn.com/cover.jpeg"],
                    "play_addr": [
                        "https://www.tiktok.com/aweme/v1/play/?video_id=v1",
                        "https://webapp-sg.tiktok.com/video/tos/alisg/direct.mp4",
                    ],
                    "play_count": "1200",
                    "like_count": "85",
                    "comment_count": "10",
                    "share_count": "4",
                }
            ],
        }
        parsed = parse_item_list_response(raw)
        self.assertEqual(parsed["status_code"], 0)
        self.assertEqual(len(parsed["items"]), 1)
        item = parsed["items"][0]
        self.assertEqual(item["id"], "7420000000000000001")
        self.assertEqual(
            item["download_url"],
            "https://webapp-sg.tiktok.com/video/tos/alisg/direct.mp4",
        )
        self.assertEqual(item["stats"]["play_count"], 1200)

    def test_extract_har_credentials(self):
        har = json.dumps(
            {
                "log": {
                    "entries": [
                        {
                            "request": {
                                "url": "https://www.tiktok.com/tiktok/creator/manage/item_list/v1/?aid=1988",
                                "headers": [
                                    {
                                        "name": "Cookie",
                                        "value": "sessionid=abc123456789; sid_tt=abc",
                                    },
                                    {
                                        "name": "x-tt-csrf-token",
                                        "value": "csrf_token_987654321",
                                    },
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
        self.assertIn("tt-cfg-form", ui_resp["body"])

        status_resp = handle_request({"method": "GET", "path": "/v1/status"})
        self.assertEqual(status_resp["status"], 200)
        body = json.loads(status_resp["body"])
        self.assertIn("configured", body)

    @patch("adapter.load_module_config", return_value={"enabled": True, "cookie": "", "base_url": "https://www.tiktok.com/"})
    def test_publish_visibility_mapping(self, _mock_cfg):
        # Without cookie, returns 401 Unauthorized
        resp = handle_request({
            "method": "POST",
            "path": "/v1/publish",
            "body": json.dumps({"text": "test post", "visibility": "only-me"})
        })
        self.assertEqual(resp["status"], 401)

    def test_sigv4_headers_generation(self):
        from tiktok_studio.upload import sigv4_headers
        headers, qs = sigv4_headers(
            sk="test_sk",
            ak="test_ak",
            st="test_st",
            method="GET",
            host="www.tiktok.com",
            path="/top/v1",
            query_params={"Action": "ApplyUploadInner"},
            body_bytes=b"",
        )
        self.assertIn("Authorization", headers)
        self.assertTrue(headers["Authorization"].startswith("AWS4-HMAC-SHA256"))
        self.assertIn("Credential=test_ak", headers["Authorization"])
        self.assertEqual(headers["X-Amz-Security-Token"], "test_st")
        self.assertIn("Action=ApplyUploadInner", qs)

    @patch("tiktok_studio.client.fetch_upstream_json")
    @patch("tiktok_studio.client.TikTokSigner")
    @patch("tiktok_studio.client.create_project", return_value="test_creation_id_123")
    def test_publish_video_post_with_schedule_time(self, _mock_create, mock_signer_cls, mock_fetch):
        from tiktok_studio.client import publish_video_post
        mock_signer = mock_signer_cls.return_value.__enter__.return_value
        mock_signer.sign.return_value = {"x_bogus": "bogus123", "signature": "sig123"}
        mock_fetch.return_value = {
            "status_code": 0,
            "project_id": "proj_999",
            "project_status": 1,
            "single_post_resp_list": [{"item_id": "item_888"}],
        }
        cfg = {"base_url": "https://www.tiktok.com", "cookie": "sessionid=123; msToken=tok456"}
        body = {
            "video_id": "v09044g4000000000000000000000000",
            "text": "Scheduled post test",
            "schedule_time": 1793450000,
        }
        res = publish_video_post(cfg, {}, body)
        self.assertEqual(res["status"], "scheduled")
        self.assertEqual(res["schedule_time"], 1793450000)
        self.assertEqual(res["project_id"], "proj_999")

        call_args = mock_fetch.call_args
        sent_body = json.loads(call_args.kwargs["body"].decode("utf-8"))
        self.assertEqual(sent_body["post_common_info"]["schedule_time"], 1793450000)
        self.assertEqual(sent_body["feature_common_info_list"][0]["schedule_time"], 1793450000)


if __name__ == "__main__":
    unittest.main()
