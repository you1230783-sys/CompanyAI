// 不編譯 EXE 的前端回歸測試。用最小 DOM 替身測登入門檻與 所有對話框背景點擊；
// 真正 WebView2 排版與原生權限檢查仍需下次 Build.ps1 自檢。
"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const root = path.resolve(__dirname, "..");
const elements = new Map();
class Element {
  constructor(id, tag = "button", disabled = false) {
    this.id = id; this.tag = tag; this.hidden = false; this.open = false;
    this.events = new Map(); this.inert = false; this.resetCount = 0;
    if (["button", "input", "select", "textarea"].includes(tag)) this.disabled = disabled;
  }
  closest() { return this; }
  addEventListener(type, handler) {
    if (!this.events.has(type)) this.events.set(type, []);
    this.events.get(type).push(handler);
  }
  dispatch(type, properties = {}) {
    const event = { target: this, clientX: 0, clientY: 0, ...properties };
    for (const handler of this.events.get(type) || []) handler(event);
  }
  getBoundingClientRect() { return {left: 20, right: 200, top: 20, bottom: 200}; }
  dispatchEvent(event) { this.dispatch(event.type); return true; }
  close() { this.open = false; this.dispatch("close"); }
  reset() { this.resetCount++; }
}
function add(id, tag, disabled) {
  const element = new Element(id, tag, disabled); elements.set(id, element); return element;
}
const get = id => elements.get(id) || add(id);
const outlook = add("show-outlook"), settings = add("settings-button"), busy = add("send", "button", true);
const preference = add("dark-mode", "input"), link = add("external", "a");
add("login-panel", "section");
const modal = add("settings-dialog", "dialog"); modal.open = true;
const listeners = new Map(), observers = [], sent = [];
const document = {
  body: {},
  querySelectorAll: selector => selector === "dialog"
    ? [...elements.values()].filter(e => e.tag === "dialog")
    : selector === "dialog[open]"
    ? [...elements.values()].filter(e => e.tag === "dialog" && e.open)
    : [...elements.values()].filter(e => ["button", "input", "select", "textarea", "a", "summary"].includes(e.tag)),
  addEventListener: (type, callback) => listeners.set(type, callback),
};
const context = vm.createContext({
  Event: class { constructor(type) { this.type = type; } },
  window: {}, document, $: get, state: {logged_in: false, busy: "none", config: {}},
  send: command => sent.push(command),
  MutationObserver: class {
    constructor(callback) { this.callback = callback; observers.push(this); }
    disconnect() { this.active = false; }
    observe() { this.active = true; }
    trigger() { if (this.active) this.callback(); }
  },
});
vm.runInContext(fs.readFileSync(path.join(root, "ui/auth-gate.js"), "utf8"), context);
const gate = context.window.AuthUI;
assert.ok(outlook.disabled && settings.disabled && preference.disabled && link.inert);
assert.equal(get("login-entry").disabled, false);
assert.equal(modal.open, false);
for (const type of ["vnc", "mail_batch", "read_mail", "work", "preferences", "behavior", "download", "copy"]) {
  assert.equal(gate.canSend({type}), false, type);
}
for (const type of ["login", "cancel_login", "reopen_login", "ready"]) assert.ok(gate.canSend({type}));
const lateButton = add("late-button"); observers[0].trigger(); assert.ok(lateButton.disabled);
preference.disabled = false; observers[0].trigger(); assert.ok(preference.disabled);
let prevented = false, stopped = false;
listeners.get("click")({target: outlook, preventDefault() { prevented = true; }, stopImmediatePropagation() { stopped = true; }});
assert.ok(prevented && stopped);
prevented = stopped = false;
listeners.get("click")({target: get("login-entry"), preventDefault() { prevented = true; }, stopImmediatePropagation() { stopped = true; }});
assert.ok(!prevented && !stopped);
get("login-entry").onclick(); assert.equal(sent.at(-1).type, "login");
gate.beforeRender(); context.state.busy = "login"; context.state.login_code = "TEST-CODE"; gate.apply();
assert.ok(get("login-entry").disabled && !get("login-entry-cancel").hidden && !get("login-entry-reopen").hidden);
gate.beforeRender(); context.state.logged_in = true; context.state.busy = "none"; gate.apply();
assert.ok(!outlook.disabled && !preference.disabled && !link.inert);
assert.ok(busy.disabled, "login must not unlock an independently disabled control");
assert.ok(get("login-panel").hidden && gate.canSend({type: "vnc"}));
gate.beforeRender(); context.state.logged_in = false; modal.open = true; gate.apply();
assert.ok(settings.disabled && !modal.open && !get("login-panel").hidden);

// 不依賴版面引擎的座標測試：從 HTML 列舉所有對話框，確保沒有漏掉。
const html = fs.readFileSync(path.join(root, "ui/index.html"), "utf8");
const dialogIds = [...html.matchAll(/<dialog id="([^"]+)"/g)].map(match => match[1]);
assert.equal(dialogIds.length, 5);
for (const id of dialogIds) { if (!elements.has(id)) add(id, "dialog"); }
vm.runInContext(fs.readFileSync(path.join(root, "ui/vnc.js"), "utf8"), context);
vm.runInContext(fs.readFileSync(path.join(root, "ui/dialogs.js"), "utf8"), context);
const inside = {clientX: 100, clientY: 100}, outside = {clientX: 10, clientY: 10};
for (const id of dialogIds) {
  const dialog = get(id); dialog.open = true;
  let dismissed = 0; dialog.addEventListener("backdrop-dismiss", () => dismissed++);
  dialog.dispatch("pointerdown", inside); dialog.dispatch("click", inside); assert.ok(dialog.open, id);
  dialog.dispatch("pointerdown", inside); dialog.dispatch("click", outside); assert.ok(dialog.open, id);
  dialog.dispatch("pointerdown", outside); dialog.dispatch("pointercancel"); dialog.dispatch("click", outside); assert.ok(dialog.open, id);
  dialog.dispatch("pointerdown", {...outside, button: 2}); dialog.dispatch("click", outside); assert.ok(dialog.open, id);
  dialog.dispatch("pointerdown", outside); dialog.dispatch("click", outside); assert.equal(dialog.open, false, id);
  assert.equal(dismissed, 1, id);
}
assert.ok(get("vnc-machine-form").resetCount > 0, "close clears unsaved form and password");
const addedDialog = add("future-dialog", "dialog");
observers.at(-1).trigger(); addedDialog.open = true;
addedDialog.dispatch("pointerdown", outside); addedDialog.dispatch("click", outside);
assert.equal(addedDialog.open, false, "future dialogs also receive backdrop behavior");
assert.ok(html.includes('id="login-entry" class="primary-button">登入</button>'));
assert.ok(html.includes("尚未登入時無法使用其他功能"));
console.log("PASS: login lock/lifecycle, dynamic controls, command filter, all 5 dialog backdrops and future dialogs");
