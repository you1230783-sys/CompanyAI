/* 個人預設色票；圖表自身的系列設定優先，原始資料永遠不改。 */
"use strict";
window.ChartAppearance = (() => {
  const defaults = ["#5470c6","#91cc75","#fac858","#ee6666","#73c0de","#3ba272","#fc8452","#9a60b4"];
  let current = [...defaults];
  const valid = colors => Array.isArray(colors) && colors.length === 8 && colors.every(c => typeof c === "string" && /^#[0-9a-f]{6}$/i.test(c));
  function set(colors) { current = valid(colors) ? [...colors] : [...defaults]; }
  return {set, valid, palette:()=>[...current], defaults:()=>[...defaults]};
})();
