import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import openapiTS, { astToString } from "openapi-typescript";

const destination = new URL("../src/schema.d.ts", import.meta.url);
const document = execFileSync(process.env.SQLBUILD_EXECUTABLE ?? "sqb", ["server", "--openapi"], {
  encoding: "utf8",
  maxBuffer: 16 * 1024 * 1024,
});
const generated = "// Generated from sqb server --openapi. Run npm run generate; do not edit.\n"
  + astToString(await openapiTS(JSON.parse(document)));
if (process.argv.includes("--check")) {
  if (await readFile(destination, "utf8") !== generated) {
    throw new Error("Control client is stale; rebuild sqb and run npm run generate.");
  }
  console.log("Control client matches the native OpenAPI contract.");
} else {
  await writeFile(destination, generated);
  console.log("Control client generated from the native OpenAPI contract.");
}
