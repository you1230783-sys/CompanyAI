/* 從純資料教學目錄產生原生可保存的索引與覆蓋表；不需要 npm 或外部套件。
 * UI 與 native 的 id／revision 應一起更新，避免完成旗標對到不同主題。
 */
"use strict";
const fs = require("node:fs");
const path = require("node:path");
const root = path.resolve(__dirname, "..");
global.window = {};
require(path.join(root, "ui", "tutorial-lessons.js"));
const catalogFile = path.join(root, "assets", "tutorials.json");
const old = JSON.parse(fs.readFileSync(catalogFile, "utf8").replace(/^\uFEFF/, ""));
const core = old.filter(item => item.id.startsWith("projects.practice."));
if (core.length !== 3) throw new Error("The three approved project tutorials must be retained.");
const catalog = core.map(item => ({...item, group:"專案操作"}));
const ids = new Set(core.map(item => item.id));
for (const lesson of window.InteractiveLessons) {
  if (ids.has(lesson.id) || !lesson.title || !lesson.group || !lesson.scenes?.length) throw new Error(`Invalid tutorial: ${lesson.id}`);
  ids.add(lesson.id);
  for (const scene of lesson.scenes) {
    if (!scene.title || !scene.description || !scene.scene?.title || !["text", "table", "conversation", "chart", "flow", "cad", "code", "diff"].includes(scene.scene.kind)
      || !["button", "input", "select", "check"].includes(scene.action?.kind)) throw new Error(`Invalid scene in ${lesson.id}`);
    if (scene.action.kind === "select" && !scene.action.choices?.length) throw new Error(`Missing choices in ${lesson.id}`);
  }
  const previous = old.find(item => item.id === lesson.id);
  catalog.push({id:lesson.id, revision:previous?.interactive ? previous.revision : (previous?.revision || 0) + 1,
    group:lesson.group, context:lesson.id.startsWith("chat.") ? "chat" : lesson.id.startsWith("settings.") ? "settings" : "project",
    title:lesson.title, intro:lesson.description, interactive:true, steps:[]});
}
fs.writeFileSync(catalogFile, JSON.stringify(catalog, null, 2) + "\n", "utf8");
const rows = catalog.map(item => {
  const lesson = window.InteractiveLessons.find(lesson => lesson.id === item.id);
  return `| ${item.group} | ${item.title} | ${lesson?.steps || (item.id.endsWith("request") ? 5 : 6)} | \`${item.id}\` |`;
});
fs.writeFileSync(path.join(root, "docs", "TUTORIAL_COVERAGE.md"),
  `# 互動教學覆蓋表\n\n目前 ${catalog.length} 個主題，全部使用假資料與固定示範，不呼叫 AI 或正式工具。\n每個主題至少包含明確操作與結果核對；教學完成不代表真實服務、模型或外部軟體已驗證。\n\n| 分類 | 主題 | 步數（含完成頁） | 識別碼 |\n|---|---|---:|---|\n${rows.join("\n")}\n`, "utf8");
console.log(`Updated ${catalog.length} interactive tutorials.`);
