/* 回答重點的表格回歸：使用隨程式交付的同一份 Markdown parser。 */
"use strict";
const assert=require("node:assert/strict"),fs=require("node:fs"),vm=require("node:vm"),path=require("node:path");
const context={window:{}};vm.createContext(context);
vm.runInContext(fs.readFileSync(path.join(__dirname,"../ui/reply-tables.js"),"utf8"),context);
const tables=context.window.ReplyTables,md=require("../ui/vendor/markdown-it.min.js")();
const rows=["| 欄位 | 表頭文字 | 欄寬 |","|---|---|---|","| A | 日期 | 12 |"];
for(const value of [rows,[rows.join("\n")],[rows.join(" ")]]) {
  const original=JSON.stringify(value),grouped=tables.group(value);
  assert.equal(grouped.length,1);const html=md.render(grouped[0]);
  assert.equal((html.match(/<th>/g)||[]).length,3);assert.ok(html.includes("<td>日期</td>"));
  assert.equal(JSON.stringify(value),original);
}
for(const literal of ["正常文字 | A | B |","| A | B | |---|---|---|","```text\n"+rows.join(" ")+"\n```","    "+rows.join(" ")]) {
  assert.equal(tables.normalize(literal),literal);
}
const escaped=["| 名稱 | 值 |","|---|---|","| A \\| B | 2 |"];
assert.ok(md.render(tables.group(escaped)[0]).includes("A | B"));
assert.equal(tables.group(["重點一",...rows,"重點二"]).length,3);
console.log("PASS: key-point tables in multiline, split-row and compact form; escaped pipes, ordinary text, mismatched columns and code remain intact.");
