"use strict";

window.addEventListener("hashchange", () => location.reload());
const element = (id) => document.getElementById(id);
const storageKey = "sqb.cli.handoff";
const code = location.hash.slice(1) || sessionStorage.getItem(storageKey) || "";
history.replaceState(null, "", location.pathname);
const valid = /^[0-9a-f-]{36}\.[0-9a-f]{64}$/i.test(code);
if (valid) sessionStorage.setItem(storageKey, code);
const message = (text) => { element("message").textContent = text; };
const csrf = () => document.cookie.split(";").map((part) => part.trim()).find((part) => part.startsWith("__Host-sqb-csrf="))?.split("=")[1] || "";

async function request(path, body) {
  return fetch(path, { method: "POST", credentials: "same-origin", headers: { "Content-Type": "application/json", "x-sqb-csrf": csrf() }, body: JSON.stringify(body) });
}

async function current() {
  if (document.body.dataset.method === "proxy") {
    element("account").textContent = "Signed in through your organization";
    element("confirmation").hidden = false;
    message("Check that the verification code matches your terminal before authorizing.");
    return;
  }
  const response = await fetch("/api/auth/session", { credentials: "same-origin" });
  if (!response.ok) {
    message("Sign in before authorizing your CLI.");
    element(document.body.dataset.method === "password" ? "password" : "sso").hidden = false;
    return;
  }
  const identity = await response.json();
  for (const id of ["password", "sso"]) element(id).hidden = true;
  element("account").textContent = `Signed in as person ${identity.principal_id}`;
  element("confirmation").hidden = false;
  message("Check that the verification code matches your terminal before authorizing.");
}

element("password").addEventListener("submit", async (event) => {
  event.preventDefault();
  const button = element("password").querySelector("button");
  button.disabled = true;
  try {
    const response = await request("/api/auth/password/login", { username: element("username").value, password: element("password-value").value });
    if (!response.ok) { message("Sign-in was rejected. Check your credentials, or wait before trying again."); return; }
    await current();
  } catch { message("Could not reach SQLBuild. Try signing in again."); }
  finally { element("password-value").value = ""; button.disabled = false; }
});

element("sso").addEventListener("click", () => {
  document.cookie = "__Host-sqb-cli-return=1; Secure; SameSite=Lax; Path=/; Max-Age=300";
  location.assign("/api/auth/oidc/login");
});

async function decide(approve) {
  element("approve").disabled = true;
  element("cancel").disabled = true;
  try {
    const response = await request("/api/auth/cli/approve", { browser_code: code, approve });
    if (!response.ok) { message("This sign-in is no longer available. Start sqb login again."); return; }
    sessionStorage.removeItem(storageKey);
    element("confirmation").hidden = true;
    message(approve ? "CLI authorized. Return to your terminal to finish signing in." : "CLI sign-in cancelled. No CLI credential was issued.");
  } catch { message("Could not confirm the decision. Start sqb login again."); }
}
element("approve").addEventListener("click", () => decide(true));
element("cancel").addEventListener("click", () => decide(false));

if (!valid) {
  sessionStorage.removeItem(storageKey);
  message("Start sqb login in your terminal to open a valid sign-in request.");
} else {
  element("code").textContent = code.split(".")[1].slice(0, 8).toUpperCase();
  current().catch(() => message("Could not reach SQLBuild. Start sqb login again."));
}
