import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { once } from "node:events";
import { createServer, request as forwardRequest } from "node:http";
import { test } from "node:test";
import { createControlClient, type components } from "../src/client.ts";

test("generated client preserves native scopes, replay, path encoding, SSE and browser sessions", { timeout: 20000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "sqlbuild-client-"));
  const executable = process.env.SQLBUILD_EXECUTABLE ?? "sqb";
  const env = { ...process.env };
  delete env.SQLBUILD_CONTROL_DATABASE_URL;
  delete env.SQLBUILD_PLATFORM_WEBHOOK_URL;
  const admin = (...args: string[]) => JSON.parse(execFileSync(executable, ["admin", ...args], {
    cwd: directory, env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
  }));
  let baseUrl: string;
  // Exercise exact-origin forwarding without asking fetch to override its forbidden Host header.
  // This HTTP fixture checks the transport contract, not external TLS termination.
  const proxy = createServer((request, response) => {
    const forwarded = forwardRequest(`${baseUrl}${request.url}`, {
      method: request.method, headers: request.headers,
    }, (upstream) => {
      response.writeHead(upstream.statusCode!, upstream.headers);
      upstream.pipe(response);
    });
    forwarded.on("error", () => { response.writeHead(502); response.end(); });
    request.pipe(forwarded);
  });
  let server: ReturnType<typeof spawn> | undefined;
  try {
    proxy.listen(0, "127.0.0.1");
    await once(proxy, "listening");
    const address = proxy.address();
    assert.ok(address && typeof address !== "string");
    const browserUrl = `http://127.0.0.1:${address.port}`;
    const origin = `https://127.0.0.1:${address.port}`;
    const { location_id } = admin("create-location", "orders");
    const other = admin("create-location", "inventory").location_id;
    const { bearer } = admin("issue-location-token", location_id, "--expires-at",
      new Date(Date.now() + 60000).toISOString());
    const { principal_id } = admin("create-principal", "orders-reader");
    const password = "  orders-password-café  ";
    execFileSync(executable, ["admin", "set-password", principal_id], {
      cwd: directory, env, input: `${password}\n`, stdio: ["pipe", "pipe", "pipe"],
    });
    server = spawn(executable, ["server", "--port", "0", "--no-scheduler",
      "--password-origin", origin], {
      cwd: directory, env, stdio: ["ignore", "ignore", "pipe"],
    });
    const running = server;
    // Readiness is emitted on stderr; never log the one-time credential response.
    baseUrl = await new Promise<string>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("API readiness timed out")), 5000);
      let log = "";
      running.once("error", reject);
      running.once("exit", () => { clearTimeout(timer); reject(new Error("API exited before readiness")); });
      running.stderr!.on("data", (chunk: Buffer) => {
        log += chunk.toString();
        const address = /Control API ready: ([^\r\n]+)/.exec(log);
        if (address) { clearTimeout(timer); resolve(`http://${address[1]}`); }
      });
    });
    const client = createControlClient({ baseUrl, headers: { Authorization: `Bearer ${bearer}` } });
    const healthPath = "/api/locations/{location_id}/health";
    const params = { path: { location_id } };
    const body: components["schemas"]["LocationHealthReport"] = {
      event_id: randomUUID(), session_id: randomUUID(), target: "preview #1", health: { kind: "heartbeat" },
    };
    const written = await client.POST(healthPath, { params, body });
    assert.equal(written.response.status, 200);
    assert.deepEqual(written.data, { applied: true });
    const replay = await client.POST(healthPath, { params, body });
    assert.deepEqual(replay.data, { applied: false });
    const read = await client.GET(healthPath, { params });
    assert.equal(read.response.headers.get("cache-control"), "no-store");
    assert.deepEqual(read.data?.map((row) => row.report), [body]);
    const forbidden = await client.GET(healthPath, { params: { path: { location_id: other } } });
    assert.equal(forbidden.response.status, 403);
    assert.equal(forbidden.data, undefined);
    const anonymous = await createControlClient({ baseUrl }).GET(healthPath, { params });
    assert.equal(anonymous.response.status, 401);
    const runs = await client.GET("/api/locations/{location_id}/targets/{target}/runs", {
      params: { path: { location_id, target: "preview #1" }, query: { limit: 1 } },
    });
    assert.equal(runs.response.status, 200);
    assert.deepEqual(runs.data, []);
    const changes = await client.GET("/api/locations/{location_id}/targets/{target}/changes", {
      params: { path: { location_id, target: "preview #1" } }, parseAs: "stream",
      signal: AbortSignal.timeout(3000),
    });
    assert.equal(changes.response.headers.get("content-type"), "text/event-stream");
    const reader = changes.data!.pipeThrough(new TextDecoderStream()).getReader();
    try {
      let event = "";
      while (!event.includes("\n\n")) {
        const chunk = await reader.read();
        assert.equal(chunk.done, false);
        event += chunk.value;
      }
      assert.match(event, /event: changed/);
      assert.match(event, /data: \{"topic":"all"\}/);
    } finally {
      await reader.cancel();
    }
    const browser = createControlClient({ baseUrl: browserUrl, headers: { Origin: origin } });
    const login = await browser.POST("/api/auth/password/login", {
      body: { username: "orders-reader", password },
    });
    assert.equal(login.response.status, 200);
    assert.equal(login.response.headers.get("cache-control"), "no-store");
    assert.equal(login.data?.identity.principal_id, principal_id);
    assert.ok(login.data?.csrf_token);
    assert.equal("bearer" in login.data, false);
    const sessionCookie = login.response.headers.getSetCookie()
      .find((cookie) => cookie.startsWith("__Host-sqb-session="));
    assert.ok(sessionCookie);
    assert.match(sessionCookie, /; Secure;/);
    assert.match(sessionCookie, /; HttpOnly$/);
    const headers = { Cookie: sessionCookie.split(";")[0] };
    // The transport does not persist credentials or turn a person's session into location authority.
    assert.equal((await browser.GET("/api/auth/session")).response.status, 401);
    const session = await browser.GET("/api/auth/session", { headers });
    assert.deepEqual(session.data, login.data.identity);
    const denied = await browser.GET("/api/control/locations/{location_id}/targets/{target}/runs", {
      headers, params: { path: { location_id, target: "development" } },
    });
    assert.equal(denied.response.status, 403);
    assert.equal(denied.response.headers.get("cache-control"), "no-store");
    const decisionId: string | null | undefined = denied.error?.decision_id;
    assert.ok(decisionId);
    assert.deepEqual(denied.error, {
      error: `missing permission in this scope; all requested nodes denied; decision ${decisionId}`,
      decision_id: decisionId,
    });
    const machine = await browser.GET(healthPath, { headers, params });
    assert.equal(machine.response.status, 401);
    const invalidLogout = await browser.POST("/api/auth/logout", {
      headers, params: { header: { "x-sqb-csrf": "incorrect-csrf" } },
    });
    assert.equal(invalidLogout.response.status, 401);
    assert.ok(invalidLogout.error?.request_id);
    assert.equal((await browser.GET("/api/auth/session", { headers })).response.status, 200);
    const logout = await browser.POST("/api/auth/logout", {
      headers, params: { header: { "x-sqb-csrf": login.data.csrf_token } },
    });
    assert.equal(logout.response.status, 204);
    assert.equal((await browser.GET("/api/auth/session", { headers })).response.status, 401);
  } finally {
    if (server && server.exitCode === null) {
      const exited = once(server, "exit");
      server.kill("SIGTERM");
      await exited;
    }
    await new Promise<void>((resolve) => proxy.close(() => resolve()));
    await rm(directory, { recursive: true, force: true });
  }
});
