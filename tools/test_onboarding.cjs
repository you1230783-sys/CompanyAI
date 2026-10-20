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
  for (const id of ["settings-dialog", "updates-replay", "tutorial-catalog", "onboarding-storage-status"]) { const node = new Element("div"); node.id = id; }
  const tutorials = JSON.parse(fs.readFileSync("assets/tutorials.json", "utf8"));
  assert.equal(new Set(tutorials.map(item => item.id)).size, 14);
  const state = {logged_in:false, update_required:false, busy:"none", onboarding:{enabled:true,tutorials,release_notes:[]}, ...overrides};
  const context = {state, window:{SettingsUI:{select:index => assert.equal(index, 3)}}, $:id => nodes.get(id), send:value => sent.push(value),
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
assert.equal(chat.sent[0].topic, "chat.basics");
assert.match(chat.nodes.get("onboarding-title").textContent, /附件/);
chat.nodes.get("onboarding-previous").onclick();
assert.match(chat.nodes.get("onboarding-title").textContent, /一句話/);
chat.nodes.get("onboarding-later").onclick();
chat.nodes.get("settings-dialog").showModal();
chat.nodes.get("tutorial-catalog").children[0].children[1].onclick();
assert.equal(chat.nodes.get("onboarding-dialog").open, true);
chat.nodes.get("onboarding-done").onclick();
assert.equal(chat.nodes.get("settings-dialog").open, true);
chat.nodes.get("updates-replay").onclick();
const acknowledged = chat.sent.length;
chat.nodes.get("onboarding-done").onclick();
assert.equal(chat.sent.length, acknowledged);
chat.nodes.get("tutorial-catalog").children[0].children[1].onclick();
chat.state.logged_in = false; chat.render();
assert.equal(chat.nodes.get("onboarding-dialog").open, false);
const update = fixture({logged_in:true,onboarding:{enabled:true,tutorials:[],version:"0.8.67",release_notes:[{section:"專案",version:"0.8.67",items:["CAD 相容性改善"]}]}});
update.render(); update.flush();
assert.equal(update.nodes.get("onboarding-dialog").open, false);
update.nodes.get("updates-replay").onclick();
assert.match(update.nodes.get("onboarding-intro").textContent, /0.8.67/);
assert.doesNotMatch(update.nodes.get("onboarding-intro").textContent, /更新到|0.8.65/);
update.nodes.get("onboarding-done").onclick();
assert.equal(update.sent.length, 0);
const blocked = fixture({logged_in:true,update_required:true}); blocked.render(); blocked.flush();
assert.equal(blocked.nodes.get("onboarding-dialog").open, false);
console.log("PASS onboarding DOM contracts: login gate, topic sequence, individual replay, settings return, current release dialog, logout, no automatic update notice, required-update gate");
