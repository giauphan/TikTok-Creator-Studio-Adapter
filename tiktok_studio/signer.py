"""Cryptographic request signer (_signature and X-Bogus) for TikTok Web API."""

from __future__ import annotations

from pathlib import Path
from playwright.sync_api import sync_playwright

_JS_DIR = Path(__file__).resolve().parent / "signer_js"
_SCRIPTS = ("signer.js", "webmssdk.js", "xbogus.js")
PAGE_URL = "https://www.tiktok.com/api/v1/web/project/post/"
VERIFY_FP = "verify_5b161567bda98b6a50c0414d99909d4b"
_STUB_HTML = "<!doctype html><html><head></head><body></body></html>"


class TikTokSigner:
    """Headless signing context using Playwright with local JavaScript stubs.
    
    The browser runs in headless mode against a local stub page; no network requests
    are dispatched outside the local environment.
    """

    def __init__(self, user_agent: str, timeout: float = 30.0):
        self.user_agent = user_agent
        self.timeout = timeout
        self._pw = None
        self._browser = None
        self._context = None
        self._page = None

    def __enter__(self) -> TikTokSigner:
        self.start()
        return self

    def __exit__(self, *args) -> None:
        self.close()

    def start(self) -> None:
        if self._page is not None:
            return
        self._pw = sync_playwright().start()
        self._browser = self._pw.chromium.launch(headless=True)
        self._context = self._browser.new_context(
            user_agent=self.user_agent, bypass_csp=True
        )
        page = self._context.new_page()
        page.route(
            "**/*",
            lambda route: route.fulfill(
                status=200, content_type="text/html", body=_STUB_HTML
            ),
        )
        page.goto(PAGE_URL, wait_until="load", timeout=self.timeout * 1000)
        for s in _SCRIPTS:
            script_path = _JS_DIR / s
            if script_path.is_file():
                page.add_script_tag(path=str(script_path))
        page.wait_for_function(
            "() => !!"
            "(window.byted_acrawler && typeof window.byted_acrawler.sign === 'function'"
            " && typeof window.generateBogus === 'function')",
            timeout=self.timeout * 1000,
        )
        self._page = page

    def sign(self, url: str) -> dict:
        if self._page is None:
            self.start()
        sep = "&" if "?" in url else "?"
        url_fp = f"{url}{sep}verifyFp={VERIFY_FP}"
        token = self._page.evaluate(
            "u => window.byted_acrawler.sign({url: u})", url_fp
        )
        signed = f"{url_fp}&_signature={token}"
        query = self._page.evaluate(
            "u => new URL(u).searchParams.toString()", signed
        )
        bogus = self._page.evaluate(
            "([q, ua]) => window.generateBogus(q, ua)", [query, self.user_agent]
        )
        return {
            "signature": str(token),
            "x_bogus": str(bogus),
            "signed_url": f"{signed}&X-Bogus={bogus}",
        }

    def close(self) -> None:
        if self._browser:
            try:
                self._browser.close()
            except Exception:
                pass
        if self._pw:
            try:
                self._pw.stop()
            except Exception:
                pass
        self._page = self._context = self._browser = self._pw = None
