/* 異常值選擇的固定五點示意，不讀使用者資料，也不改變實際圖表或選擇。 */
"use strict";
window.ChartReviewExamples = (() => {
  const values = [10, 20, null, 40, 50];
  const choices = [
    { id: "gap", title: "保留缺值", detail: "第 3 點不畫，折線在此中斷。" },
    { id: "skip", title: "略過此點", detail: "第 2 點直接連到第 4 點；原 X 位置不變。" },
    { id: "zero", title: "設為 0", detail: "在 X＝3 畫出一個 Y＝0 的點。" },
  ];
  function element(tag, className, text) {
    const result = document.createElement(tag);
    result.className = className;
    if (text) result.textContent = text;
    return result;
  }
  function svgElement(tag, attributes, text) {
    const result = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [key, value] of Object.entries(attributes)) result.setAttribute(key, String(value));
    if (text !== undefined) result.textContent = String(text);
    return result;
  }
  // 三張圖使用同一座標及比例。略過僅移除該點，不重新編排剩餘的 X。
  const x = index => 44 + index * 48;
  const y = value => 148 - value * 1.9;
  function plot(choice) {
    const svg = svgElement("svg", { viewBox: "0 0 280 200", role: "img", "aria-label": `${choice.title}：${choice.detail}` });
    svg.append(svgElement("title", {}, `${choice.title}，假設第 3 筆原始值為 NG`));
    for (const value of [0, 30, 60]) {
      svg.append(svgElement("line", { x1: 36, x2: 248, y1: y(value), y2: y(value), class: "example-grid" }));
      svg.append(svgElement("text", { x: 29, y: y(value) + 5, "text-anchor": "end" }, value));
    }
    svg.append(svgElement("line", { x1: x(2), x2: x(2), y1: 24, y2: 156, class: "example-missing" }));
    svg.append(svgElement("text", { x: x(2), y: 16, "text-anchor": "middle" }, "原值：NG"));
    svg.append(svgElement("text", { x: 12, y: 17 }, "Y"));
    values.forEach((_, index) => svg.append(svgElement("text", { x: x(index), y: 174, "text-anchor": "middle" }, index + 1)));
    svg.append(svgElement("text", { x: 140, y: 196, "text-anchor": "middle" }, "X 位置"));
    let path = "", connected = false;
    values.forEach((original, index) => {
      if (original === null && choice.id === "gap") { connected = false; return; }
      if (original === null && choice.id === "skip") return;
      const value = original === null ? 0 : original;
      path += `${connected ? "L" : "M"}${x(index)},${y(value)} `;
      connected = true;
      svg.append(svgElement("circle", { cx: x(index), cy: y(value), r: 4, class: "example-point", "data-x": index + 1, "data-y": value }));
      svg.append(svgElement("text", { x: x(index), y: y(value) - 11, "text-anchor": "middle" }, value));
    });
    svg.append(svgElement("path", { d: path.trim(), class: "example-line" }));
    return svg;
  }
  function create() {
    const section = element("section", "chart-review-examples");
    section.setAttribute("aria-label", "三種異常值處理示意");
    section.append(element("h3", "", "先看三種選擇的差別"));
    section.append(element("p", "chart-example-source", "假設資料：X＝1、2、3、4、5；Y＝10、20、NG、40、50。以下是折線示意，並非您的資料。"));
    const grid = element("div", "chart-example-grid");
    for (const choice of choices) {
      const figure = element("figure", "chart-example");
      figure.dataset.choice = choice.id;
      figure.append(element("h4", "", choice.title), plot(choice), element("figcaption", "", choice.detail));
      grid.append(figure);
    }
    section.append(grid, element("p", "chart-example-note", "略過只是連接前後有效點，不代表補出量測值；三種方式都不改寫原始檔。長條圖／散佈圖沒有折線接續的差別。"));
    return section;
  }
  return { create };
})();
