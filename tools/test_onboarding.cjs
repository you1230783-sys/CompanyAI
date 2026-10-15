/* 教學卡互動契約；不取代 WebView2 實際版面與登入驗收。 */
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
function fixture(overrides = {}) {
  const nodes = new Map(), timers = [], sent = [];
  class Element {
    constructor(tag) { this.tagName = tag; this.children = []; this.listeners = {}; this.open = false; this.isConnected = true; }
    set id(value) { this.key = value; nodes.set(value, this); }
    get id() { return this.key; }
    set innerHTML(value) { for (const match of value.matchAll(/id="([^"]+)"/g)) { const child = new Element("div"); child.id = match[1]; } }
    setAttribute() {}
    append(...items) { this.children.push(...items); }
    replaceChildren() { this.children = []; }
    focus() {}
    addEventListener(name, callback) { this.listeners[name] = callback; }
    showModal() { this.open = true; }
    close() { this.open = false; this.listeners.close?.(); }
  }
  for (const id of ["settings-dialog", "onboarding-replay", "onboarding-storage-status"]) { const node = new Element("div"); node.id = id; }
  const state = {logged_in:false, update_required:false, busy:"none", onboarding:{enabled:true,chat_pending:true,updates:[]}, ...overrides};
  const context = {state, window:{}, $:id => nodes.get(id), send:value => sent.push(value),
    setTimeout:callback => { timers.push(callback); return timers.length; },
    document:{createElement:tag => new Element(tag),body:new Element("body"),activeElement:null,
      querySelector:() => [...nodes.values()].find(node => node.open)}};
  vm.runInNewContext(fs.readFileSync("ui/onboarding.js", "utf8"), context);
  return {state,nodes,sent,render:() => context.window.OnboardingUI.render(), flush:() => { while(timers.length) timers.shift()(); }};
}
const chat = fixture();
chat.render(); chat.flush();
assert.equal(chat.nodes.get("onboarding-dialog").open, false);
chat.state.logged_in = true; chat.render(); chat.flush();
assert.equal(chat.nodes.get("onboarding-dialog").open, true);
assert.equal(chat.nodes.get("onboarding-body").children.length, 4);
chat.nodes.get("onboarding-done").onclick();
assert.equal(chat.sent[0].topic, "chat");
chat.nodes.get("onboarding-replay").onclick();
assert.equal(chat.nodes.get("onboarding-dialog").open, true);
chat.state.logged_in = false; chat.render();
assert.equal(chat.nodes.get("onboarding-dialog").open, false);
const update = fixture({logged_in:true,onboarding:{enabled:true,chat_pending:true,from_version:"0.8.64",to_version:"0.8.65",updates:[{section:"專案",version:"0.8.65",items:["技能精簡"]}]}});
update.render(); update.flush();
assert.match(update.nodes.get("onboarding-intro").textContent, /0.8.64.*0.8.65/);
update.nodes.get("onboarding-done").onclick();
assert.equal(update.sent[0].topic, "updates");
const blocked = fixture({logged_in:true,update_required:true}); blocked.render(); blocked.flush();
assert.equal(blocked.nodes.get("onboarding-dialog").open, false);
console.log("PASS onboarding DOM contracts: login gate, one card, replay, logout, update range display, required-update gate");
