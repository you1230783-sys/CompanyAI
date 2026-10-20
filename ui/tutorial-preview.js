/* 只連接純前端練習；刻意不載入 app.js 或正式原生命令處理。 */
"use strict";
(() => {
  const completed = new Set();
  const start = () => window.ProjectTutorial.open({
    completed: [...completed],
    onComplete: id => {
      completed.add(id);
      document.getElementById("preview-status").textContent = `本次預覽已完成 ${completed.size} / ${window.ProjectTutorial.catalog.length} 個教學。正式介面會保存各主題的完成紀錄。`;
    },
  });
  document.getElementById("preview-start").onclick = start;
  start();
})();
