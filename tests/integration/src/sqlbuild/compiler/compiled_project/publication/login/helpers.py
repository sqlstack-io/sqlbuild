"""A real verified HTTPS boundary in front of the installed native control server."""

import base64
import hashlib
import http.client
import json
import ssl
import stat
import subprocess
import threading
import time
import uuid
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from http.cookiejar import CookieJar
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.error import HTTPError
from urllib.parse import SplitResult, parse_qs, urlencode, urlsplit
from urllib.request import HTTPCookieProcessor, HTTPSHandler, Request, build_opener

from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    write_server_config,
)


@contextmanager
def gateway(root: Path) -> Iterator[tuple[str, Path, dict[str, str]]]:
    """Generate disposable keys; forward HTTP unchanged except hop-by-hop transport headers."""
    certificate: Path = root / "localhost.pem"
    key: Path = root / "localhost-key.pem"
    subprocess.run(
        [
            "openssl",
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-keyout",
            str(key),
            "-out",
            str(certificate),
        ],
        check=True,
        capture_output=True,
        timeout=15,
    )
    state: dict[str, str] = {}
    identity_provider: Callable[[BaseHTTPRequestHandler, bytes], None] = provider(key)

    class Forward(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            self.forward()

        def do_POST(self) -> None:
            self.forward()

        def log_message(self, format: str, *args: Any) -> None:
            """Do not log handoff or authentication request paths."""

        def forward(self) -> None:
            body: bytes = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            if self.path.startswith("/issuer/"):
                identity_provider(self, body)
                return
            upstream: http.client.HTTPConnection = http.client.HTTPConnection(
                state["backend"], timeout=10
            )
            try:
                headers: dict[str, str] = dict(self.headers)
                if "proxy-subject" in state:
                    headers = {
                        name: value
                        for name, value in headers.items()
                        if name.lower() != "x-sqb-user"
                    }
                    headers["X-SQB-User"] = state["proxy-subject"]
                upstream.request(self.command, self.path, body, headers)
                response: http.client.HTTPResponse = upstream.getresponse()
                content: bytes = response.read()
                self.send_response(response.status)
                for name, value in response.getheaders():
                    if name.lower() not in {"connection", "transfer-encoding", "content-length"}:
                        self.send_header(name, value)
                self.send_header("Content-Length", str(len(content)))
                self.end_headers()
                self.wfile.write(content)
            finally:
                upstream.close()

    with ThreadingHTTPServer(("127.0.0.1", 0), Forward) as server:
        context: ssl.SSLContext = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(certificate, key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        thread: threading.Thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield f"https://localhost:{server.server_port}", certificate, state
        finally:
            server.shutdown()
            thread.join(timeout=10)


def browser(origin: str, certificate: Path) -> tuple[Any, CookieJar]:
    """Use a real TLS verifier and secure cookie jar, not an insecure browser bypass."""
    cookies: CookieJar = CookieJar()
    return build_opener(
        HTTPSHandler(context=ssl.create_default_context(cafile=str(certificate))),
        HTTPCookieProcessor(cookies),
    ), cookies


def request(
    client: Any, endpoint: str, body: dict[str, Any] | None = None, *, csrf: str = ""
) -> tuple[int, bytes]:
    """Return denials without printing authentication response payloads."""
    origin: str = endpoint.split("/", 3)[0] + "//" + endpoint.split("/", 3)[2]
    call: Request = Request(
        endpoint,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Origin": origin, "Content-Type": "application/json", "x-sqb-csrf": csrf},
    )
    try:
        response: Any = client.open(call, timeout=10)
    except HTTPError as error:
        response = error
    with response:
        status: int | None = response.status
        assert status is not None
        return status, response.read()


def wait_for_handoff(process: subprocess.Popen[bytes], log: Path) -> str:
    """Observe the CLI's public URL while keeping all credential responses out of logs."""
    deadline: float = time.monotonic() + 20
    while "\nVerification code: " not in log.read_text():
        assert process.poll() is None, "CLI exited before offering browser confirmation"
        assert time.monotonic() < deadline, "CLI did not offer browser confirmation"
        time.sleep(0.02)
    return log.read_text().split("Open ", 1)[1].splitlines()[0]


@contextmanager
def pending_login(
    command: list[str], environment: dict[str, str], project: Path, log: Path
) -> Iterator[tuple[subprocess.Popen[bytes], str]]:
    """Reap a real login process even if browser approval or an assertion fails."""
    with log.open("wb") as stderr:
        process: subprocess.Popen[bytes] = subprocess.Popen(
            command,
            cwd=project,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=stderr,
        )
    try:
        yield process, wait_for_handoff(process, log)
    finally:
        process.kill()
        process.wait(timeout=10)


def authentication_arguments(
    mode: str, origin: str, certificate: Path, principal: str
) -> tuple[str, ...]:
    """Use only the product's existing explicit authentication configuration."""
    if mode == "password":
        return "--password-origin", origin
    configuration: dict[str, Any] = {"origin": origin, "identities": {"orders-person": principal}}
    if mode == "oidc":
        configuration.update(
            issuer=origin + "/issuer",
            client_id="orders-control",
            certificate_authority=str(certificate),
        )
        section = "oidc"
    else:
        assert mode == "proxy"
        configuration["trusted_peers"] = ["127.0.0.1"]
        section = "trusted_proxy"
    path: Path = certificate.parent / "server.toml"
    write_server_config(path, {section: configuration})
    return "--config", str(path)


def sign_in(client: Any, origin: str, mode: str, username: str, password: str) -> str:
    """Password and PKCE provider sign-in use real routes; the proxy authenticates each request."""
    if mode == "proxy":
        return ""
    if mode == "password":
        status, payload = request(
            client,
            origin + "/api/auth/password/login",
            {"username": username, "password": password},
        )
    else:
        status, payload = request(client, origin + "/api/auth/oidc/login")
    assert status == 200, f"Installed browser authentication failed with HTTP {status}"
    return json.loads(payload)["csrf_token"]


def assert_private_storage(
    credential: Path, command: list[str], environment: dict[str, str], project: Path
) -> None:
    """Real CLI reload must refuse an unsafe file mode and a symlinked credential."""
    original_mode: int = stat.S_IMODE(credential.stat().st_mode)
    credential.chmod(0o644)
    unsafe: subprocess.CompletedProcess[str] = subprocess.run(
        command,
        cwd=project,
        env=environment,
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    assert unsafe.returncode != 0 and not unsafe.stdout
    credential.chmod(original_mode)
    backup: Path = credential.with_suffix(".backup")
    credential.rename(backup)
    credential.symlink_to(backup)
    symlink: subprocess.CompletedProcess[str] = subprocess.run(
        command,
        cwd=project,
        env=environment,
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    assert symlink.returncode != 0 and not symlink.stdout
    credential.unlink()
    backup.rename(credential)


def encoded(value: bytes) -> str:
    """JOSE uses unpadded URL-safe base64."""
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


def provider(key: Path) -> Callable[[BaseHTTPRequestHandler, bytes], None]:
    """Consume codes once and validate real PKCE before signing the requested nonce."""
    modulus: str = (
        subprocess.run(
            ["openssl", "rsa", "-in", str(key), "-modulus", "-noout"],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        )
        .stdout.strip()
        .split("=", 1)[1]
    )
    jwk: dict[str, str] = {
        "kty": "RSA",
        "use": "sig",
        "alg": "RS256",
        "kid": "orders",
        "n": encoded(bytes.fromhex(modulus)),
        "e": "AQAB",
    }
    codes: dict[str, dict[str, list[str]]] = {}

    def handle(handler: BaseHTTPRequestHandler, body: bytes) -> None:
        assert isinstance(handler.server, ThreadingHTTPServer)
        origin: str = f"https://localhost:{handler.server.server_port}"
        issuer: str = origin + "/issuer"
        path: SplitResult = urlsplit(handler.path)
        payload: dict[str, object]
        if path.path.endswith("/.well-known/openid-configuration"):
            payload = {
                "issuer": issuer,
                "authorization_endpoint": issuer + "/authorize",
                "token_endpoint": issuer + "/token",
                "jwks_uri": issuer + "/jwks",
                "response_types_supported": ["code"],
                "subject_types_supported": ["public"],
                "id_token_signing_alg_values_supported": ["RS256"],
                "code_challenge_methods_supported": ["S256"],
            }
        elif path.path.endswith("/jwks"):
            payload = {"keys": [jwk]}
        elif path.path.endswith("/authorize"):
            parameters: dict[str, list[str]] = parse_qs(path.query)
            assert parameters["client_id"] == ["orders-control"]
            assert parameters["code_challenge_method"] == ["S256"]
            assert parameters["redirect_uri"] == [origin + "/api/auth/oidc/callback"]
            code: str = str(uuid.uuid4())
            codes[code] = parameters
            handler.send_response(303)
            handler.send_header(
                "Location",
                parameters["redirect_uri"][0]
                + "?"
                + urlencode({"code": code, "state": parameters["state"][0]}),
            )
            handler.send_header("Content-Length", "0")
            handler.end_headers()
            return
        elif path.path.endswith("/token"):
            form: dict[str, list[str]] = parse_qs(body.decode())
            parameters = codes.pop(form["code"][0])
            assert form["grant_type"] == ["authorization_code"]
            assert form["redirect_uri"] == parameters["redirect_uri"]
            assert (
                encoded(hashlib.sha256(form["code_verifier"][0].encode()).digest())
                == parameters["code_challenge"][0]
            )
            header: str = encoded(json.dumps({"alg": "RS256", "kid": "orders"}).encode())
            claims: str = encoded(
                json.dumps(
                    {
                        "iss": issuer,
                        "sub": "orders-person",
                        "aud": "orders-control",
                        "iat": int(time.time()),
                        "exp": int(time.time()) + 300,
                        "nonce": parameters["nonce"][0],
                    }
                ).encode()
            )
            signing_input: bytes = f"{header}.{claims}".encode()
            signature: bytes = subprocess.run(
                ["openssl", "dgst", "-sha256", "-sign", str(key)],
                input=signing_input,
                check=True,
                capture_output=True,
                timeout=10,
            ).stdout
            payload = {
                "access_token": "synthetic-orders-access",
                "token_type": "Bearer",
                "expires_in": 300,
                "id_token": f"{header}.{claims}.{encoded(signature)}",
            }
        else:
            raise AssertionError("unexpected identity-provider endpoint")
        content: bytes = json.dumps(payload).encode()
        handler.send_response(200)
        handler.send_header("Content-Type", "application/json")
        handler.send_header("Content-Length", str(len(content)))
        handler.end_headers()
        handler.wfile.write(content)

    return handle
