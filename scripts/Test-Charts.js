/* 開發用純 JS 檢查；不編譯 EXE，也不取代 WebView2 的實際 PNG／視覺驗收。 */
"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
let options, disposed = 0, removed = 0, fail = false;
const context = {structuredClone, ResizeObserver:class {}, document:{
  body:{append(){}}, createElement(){return {style:{}, remove(){removed++;}};}
}, echarts:{init(){return {
  setOption(value){options=value;},getZr(){return {flush(){}};},
  getDataURL(){if(fail)throw new Error("renderer failed"); return "data:image/png;base64,test";},
  dispose(){disposed++;}
};}}};
context.window=context;
vm.createContext(context);
for(const name of ["chart-transform.js","chart-editor.js","charts.js"]) vm.runInContext(fs.readFileSync(path.join(__dirname,"../ui",name),"utf8"),context);
const chart = {kind:"line",title:"<script>只是標題</script>",x_label:"時間",y_label:"數值",source:"測試",
  x:Array.from({length:10000},(_,i)=>i),series:Array.from({length:8},(_,s)=>({name:`s${s}`,values:Array.from({length:10000},(_,i)=>i===3000?null:i)}))};
assert.equal(context.window.ChartUI.exportPng(chart),"data:image/png;base64,test");
assert.equal(options.xAxis.data.length,10000);
assert.equal(options.series.length,8);
assert.equal(options.series[0].data.length,10000);
assert.equal(options.series[0].data[3000][1],null);
assert.equal(options.series[0].progressive,0);
assert.equal(options.dataZoom.length,0);
assert.equal(options.title.text,chart.title);
assert.equal(disposed,1);assert.equal(removed,1);
assert.throws(()=>context.window.ChartUI.exportPng({...chart,x:[...chart.x,10000]}));
fail=true;assert.throws(()=>context.window.ChartUI.exportPng(chart));
assert.equal(disposed,2);assert.equal(removed,2);
const indexed={...chart,x:[7001,7002,7003],series:[{name:"a",values:[10,null,0]}]};
const style=context.ChartEditor.defaults(indexed);style.transform.x.mode="index";style.transform.y.mode="offset";style.transform.y.offset=-2;style.transform.drop_empty=true;
assert.equal(JSON.stringify(context.ChartUI.option(indexed,false,style).series[0].data),"[[1,8],[2,-2]]");
assert.equal(indexed.x[0],7001);assert.equal(indexed.series[0].values[0],10);
console.log("PASS: 10000 points / 8 series, gaps, transformed X/Y, original preservation, full-range PNG options, bounds and renderer cleanup (mock renderer).");
