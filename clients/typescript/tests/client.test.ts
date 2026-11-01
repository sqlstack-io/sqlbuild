import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { once } from "node:events";
import { test } from "node:test";
import { createControlClient, type components } from "../src/client.ts";

test("generated client preserves native HTTP scopes, replay, path encoding and SSE", { timeout: 20000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "sqlbuild-client-"));
  const executable = process.env.SQLBUILD_EXECUTABLE ?? "sqb";
  const env = { ...process.env };
  delete env.SQLBUILD_CONTROL_DATABASE_URL;
  delete env.SQLBUILD_PLATFORM_WEBHOOK_URL;
  const admin = (...args: string[]) => JSON.parse(execFileSync(executable, ["admin", ...args], {
    cwd: directory, env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
  }));
  let server: ReturnType<typeof spawn> | undefined;
  try {
    const { location_id } = admin("create-location", "orders");
    const other = admin("create-location", "inventory").location_id;
    const { bearer } = admin("issue-location-token", location_id, "--expires-at",
      new Date(Date.now() + 60000).toISOString());
    server = spawn(executable, ["server", "--port", "0", "--no-scheduler"], {
      cwd: directory, env, stdio: ["ignore", "ignore", "pipe"],
    });
    const running = server;
    // Readiness is emitted on stderr; never log the one-time credential response.
    const baseUrl = await new Promise<string>((resolve, reject) => {
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
  } finally {
    if (server && server.exitCode === null) {
      const exited = once(server, "exit");
      server.kill("SIGTERM");
      await exited;
    }
    await rm(directory, { recursive: true, force: true });
  }
});
