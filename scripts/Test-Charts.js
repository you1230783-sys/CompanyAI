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
const pair = value => JSON.parse(JSON.stringify(value));
for (const [min,max,expected] of [[12,23,[10,25]],[0.12,0.23,[0.1,0.25]],[-23,-12,[-25,-10]],[-2,3,[-2,3]],[0,0,[-5,5]],[20,20,[15,25]]]) {
  assert.deepEqual(pair(context.ChartTransform.niceBounds(min,max)),expected);
}
for (const [min,max] of [[1e-12,2.3e-12],[-0.23,0.12],[7001,7005],[0.30000000000000004,0.6]]) {
  const [lower,upper]=context.ChartTransform.niceBounds(min,max);
  assert.ok(lower<=min && upper>=max && lower<upper);
}
assert.throws(()=>context.ChartTransform.niceBounds(Infinity,2));
assert.deepEqual(pair(context.ChartTransform.bounds(indexed,"line",style.transform,"y")),[-2,8]);
assert.deepEqual(pair(context.ChartTransform.bounds(indexed,"line",style.transform,"x")),[1,2]);
const horizontal=context.ChartEditor.defaults({...indexed,kind:"horizontal_bar"});
assert.deepEqual(pair(context.ChartTransform.bounds(indexed,horizontal.kind,horizontal.transform,"x")),[0,10]);
assert.deepEqual(pair(context.ChartTransform.bounds(indexed,horizontal.kind,horizontal.transform,"y")),[0,2]);
const missing={...indexed,series:[{name:"missing",values:[null,null,null]}]};
assert.throws(()=>context.ChartTransform.bounds(missing,"scatter",context.ChartTransform.defaults(),"y"));
const skipped={...indexed,series:[{name:"a",values:[10,10000,0],skip_indices:[1]}]};
assert.deepEqual(pair(context.ChartTransform.bounds(skipped,"scatter",context.ChartTransform.defaults(),"y")),[0,10]);
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
