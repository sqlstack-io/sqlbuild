"""Exercise installed-server browser sessions over a disposable local HTTP transport."""

from __future__ import annotations

import json
from email.message import Message
from http.client import HTTPResponse
from http.cookies import SimpleCookie
from typing import Any
from urllib.error import HTTPError
from urllib.request import Request, urlopen


def request(
    endpoint: str, body: bytes | None = None, session: tuple[str, str] = ("", "")
) -> tuple[int | None, Message[str, str], dict[str, Any]]:
    """Return denials as ordinary results; never print credentials or response payloads."""
    headers: dict[str, str] = {
        "Host": "control.example.com",
        "Origin": "https://control.example.com",
        "Content-Type": "application/json",
        "Cookie": session[0],
        "x-sqb-csrf": session[1],
    }
    call: Request = Request(endpoint, data=body, headers=headers)
    response: HTTPResponse | HTTPError
    try:
        response = urlopen(call, timeout=5)
    except HTTPError as error:
        response = error
    with response:
        return response.status, response.headers, json.load(response)


def login(url: str, username: str, password: str) -> tuple[str, str]:
    """Authenticate with the real Argon2/password route and retain its host-only cookie."""
    status, headers, body = request(
        f"{url}/api/auth/password/login",
        json.dumps({"username": username, "password": password}).encode("utf-8"),
    )
    assert status == 200, f"Password sign-in failed with HTTP {status}"
    cookies: SimpleCookie[str] = SimpleCookie("\n".join(headers.get_all("Set-Cookie", [])))
    cookie: str = "__Host-sqb-session=" + cookies["__Host-sqb-session"].value
    return cookie, body["csrf_token"]
