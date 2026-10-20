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
  setOption(value,settings){options=settings?.notMerge?value:{...options,...value};},getZr(){return {flush(){}};},
  getWidth(){return 1600;},getHeight(){return 1000;},getOption(){return {grid:[options.grid]};},convertToPixel(){return 500;},containPixel(){return true;},
  getDataURL(){if(fail)throw new Error("renderer failed"); return "data:image/png;base64,test";},
  dispose(){disposed++;}
};}}};
context.window=context;
vm.createContext(context);
for(const name of ["chart-transform.js","chart-appearance.js","chart-layout.js","chart-editor.js","charts.js"]) vm.runInContext(fs.readFileSync(path.join(__dirname,"../ui",name),"utf8"),context);
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
const pair = value => JSON.parse(JSON.stringify(value));
for (const [min,max,expected] of [[12,23,[10,25]],[0.12,0.23,[0.1,0.25]],[-23,-12,[-25,-10]],[-2,3,[-2,3]],[0,0,[-1,1]],[20,20,[19,21]]]) {
  assert.deepEqual(pair(context.ChartTransform.niceBounds(min,max)),expected);
}
for (const [min,max] of [[1e-12,2.3e-12],[-0.23,0.12],[7001,7005],[0.30000000000000004,0.6]]) {
  const [lower,upper]=context.ChartTransform.niceBounds(min,max);
  assert.ok(lower<=min && upper>=max && lower<upper);
}
assert.throws(()=>context.ChartTransform.niceBounds(Infinity,2));
assert.deepEqual(pair(context.ChartTransform.bounds(indexed,"line",style.transform,"y")),[-10,15]);
assert.deepEqual(pair(context.ChartTransform.bounds(indexed,"line",style.transform,"x")),[1,2]);
const horizontal=context.ChartEditor.defaults({...indexed,kind:"horizontal_bar"});
assert.deepEqual(pair(context.ChartTransform.bounds(indexed,horizontal.kind,horizontal.transform,"x")),[0,15]);
assert.deepEqual(pair(context.ChartTransform.bounds(indexed,horizontal.kind,horizontal.transform,"y")),[0,2]);
const missing={...indexed,series:[{name:"missing",values:[null,null,null]}]};
assert.throws(()=>context.ChartTransform.bounds(missing,"scatter",context.ChartTransform.defaults(),"y"));
const skipped={...indexed,series:[{name:"a",values:[10,10000,0],skip_indices:[1]}]};
assert.deepEqual(pair(context.ChartTransform.bounds(skipped,"scatter",context.ChartTransform.defaults(),"y")),[-5,15]);
horizontal.lines=[{axis:"x",value:10,name:"",color:"#ff0000"}];
const horizontalOptions=context.ChartUI.option(indexed,false,horizontal);
const line=horizontalOptions.series[0].markLine.data[0];
assert.equal(line.label.show,false);assert.equal(line.label.formatter(),"");
const tip=horizontalOptions.tooltip.formatter({seriesName:"<img onerror='x'>&",value:[10,2]});
assert.ok(tip.includes("X（數值）：10") && tip.includes("Y（時間）：7003"));
assert.ok(!tip.includes("<img") && tip.includes("&lt;img"));
const transformedOptions=context.ChartUI.option(indexed,false,style);
assert.ok(transformedOptions.tooltip.formatter([{seriesName:"A",value:[2,-2]}]).includes("X（時間）：2"));
assert.ok(transformedOptions.tooltip.formatter({seriesName:"A",value:[1,null]}).includes("Y（數值）：—"));
console.log("PASS: chart bounds (decimals, negatives, constant, empty, transformed and horizontal), X/Y tooltip escaping, blank reference labels, 10000 points / 8 series, PNG options and cleanup.");

// 同一份來源依政策重畫，PNG 與畫面必須使用相同資料，不能把設零後的數字當原值。
const quality={kind:"line",title:"缺值測試",x_label:"x",y_label:"y",source:"fixture",x:[1,2,3,4],
  series:[{name:"a",values:[10,0,null,0]},{name:"b",values:[2,3,4,5]}],data_issues:[{original_value:"NG"},{original_value:null}],
  quality:{cells:[{row:1,series:0,issue:0,blank:false},{row:2,series:0,issue:1,blank:true}]}};
const qstyle=context.ChartEditor.defaults(quality);qstyle.quality_policy={blank:"skip",invalid:"gap"};
const qview=context.ChartQuality.view(quality,qstyle.quality_policy);
assert.deepEqual(pair(qview.series[0].values),[10,null,null,0]);
assert.deepEqual(pair(qview.series[0].skip_indices),[2]);assert.equal(quality.series[0].values[1],0);
assert.deepEqual(pair(context.ChartUI.option(quality,false,qstyle).series[0].data),[[0,10],[1,null],[3,0]]);
assert.deepEqual(pair(context.ChartUI.option(quality,true,qstyle).series[0].data),[[0,10],[1,null],[3,0]]);
assert.deepEqual(pair(qview.series[1].values),[2,3,4,5]);
console.log("PASS: reversible missing-value policies preserve real zero, other series, source issues and matching display/PNG points.");

// AI 參考線沿用實體座標；畫面、PNG 及編輯器採相同預設，使用者覆寫不改原值。
const annotated = {kind:"scatter",title:"參數切換",x_label:"秒",y_label:"量測",source:"fixture",
  x:[0,200,500,700],series:[{name:"Y",values:[1,2,3,4]}],reference_lines:[
    {axis:"x",value:200,name:"參數1→2",color:"#d62728"},
    {axis:"x",value:500,name:"參數2→3",color:"#5470c6"}]};
const beforeAnnotations = JSON.stringify(annotated);
const markLines = context.ChartUI.option(annotated).series[0].markLine.data;
assert.deepEqual(pair(markLines.map(line=>line.xAxis)),[200,500]);
assert.equal(markLines[0].name,"參數1→2");
fail=false;context.ChartUI.exportPng(annotated);
assert.deepEqual(pair(options.series[0].markLine.data.map(line=>line.xAxis)),[200,500]);
const customLines=context.ChartEditor.defaults(annotated);customLines.lines[0].value=250;
assert.equal(context.ChartUI.option(annotated,false,customLines).series[0].markLine.data[0].xAxis,250);
assert.equal(JSON.stringify(annotated),beforeAnnotations);
console.log("AI reference lines and PNG options: PASS");

// 使用者兩個例子：明確的留白與依跨度取整。實際算法另核對常數、離群值及零。
assert.deepEqual(pair(context.ChartTransform.niceBounds(90,110,6)),[80,120]);
assert.deepEqual(pair(context.ChartTransform.niceBounds(.25,.35,.04)),[.2,.4]);
assert.deepEqual(pair(context.ChartTransform.measurementBounds([90,95,100,105,110])),[80,120]);
assert.deepEqual(pair(context.ChartTransform.measurementBounds([.25,.28,.3,.32,.35])),[.2,.4]);
assert.deepEqual(pair(context.ChartTransform.measurementBounds([100,100])),[94,106]);
assert.deepEqual(pair(context.ChartTransform.measurementBounds([0])),[-1,1]);
const tight={kind:"scatter",title:"T",x_label:"x",y_label:"y",source:"fixture",x:[0,1,2,3,4],series:[{name:"T",values:[90,95,100,105,110]}]};
const auto=context.ChartUI.option(tight), png=context.ChartUI.option(tight,true);
assert.equal(auto.yAxis.min,80);assert.equal(auto.yAxis.max,120);
assert.equal(png.yAxis.min,80);assert.equal(png.yAxis.max,120);
const manual=context.ChartEditor.defaults(tight);manual.y_min=85;manual.y_max=115;
assert.equal(context.ChartUI.option(tight,false,manual).yAxis.min,85);
assert.equal(context.ChartUI.option(tight,false,manual).yAxis.max,115);
const outliers=[...Array(999).fill(100),1000];
const [lo,hi]=context.ChartTransform.measurementBounds(outliers);
assert.ok(lo<=100 && hi>=1000); // 不以sigma裁掉離群點。
console.log("PASS: adaptive sigma padding, manual bounds, real zero, constants, outliers and matching PNG limits.");
