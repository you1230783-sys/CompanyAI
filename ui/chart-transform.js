/* 與 Rust charts/transform.rs 同一契約：先篩列，再以共同列序號轉換實體 X／Y。
 * 原始 Chart 永不修改；畫面、編輯預覽及 PNG 共用此處的呈現用副本。 */
"use strict";
window.ChartTransform = (() => {
  const axisDefault = () => ({mode:"original",offset:0,start:1,step:1});
  const defaults = () => ({x:axisDefault(),y:axisDefault(),drop_empty:false});
  function settings(data, style) {
    return structuredClone(style?.transform ?? data.transform ?? defaults());
  }
  function category(kind, axis, transform=defaults()) {
    const base=kind==="horizontal_bar"?"y":"x";
    return axis===base && kind!=="scatter" &&
      (["bar","horizontal_bar"].includes(kind) || transform[axis].mode==="original");
  }
  function validate(axis) {
    if (!["original","offset","index"].includes(axis.mode) ||
        ![axis.offset,axis.start,axis.step].every(Number.isFinite) || (axis.mode==="index" && axis.step===0)) {
      throw new Error("座標轉換需使用有限數字，重新編號的間距不可為 0。");
    }
  }
  function number(axis,value,row) {
    const result=axis.mode==="index" ? axis.start+axis.step*row : axis.mode==="offset" ? value+axis.offset : value;
    if (!Number.isFinite(result)) throw new Error("座標轉換結果超出有限數值範圍。");
    return result;
  }
  function view(data,kind,transform=defaults()) {
    validate(transform.x);validate(transform.y);
    const indices=data.x.flatMap((_,i)=>!transform.drop_empty || data.series.some(s=>s.values[i]!=null)?[i]:[]);
    if(!indices.length) throw new Error("移除無值位置後沒有資料，請保留缺值或重新選取來源。");
    const [base,measure]=kind==="horizontal_bar"?[transform.y,transform.x]:[transform.x,transform.y];
    const x=indices.map((i,row)=>{
      if(base.mode==="original") return data.x[i];
      if(base.mode==="offset" && typeof data.x[i]!=="number") throw new Error("文字／日期類別不能直接數值平移；請選重新編號或保留原值。");
      return number(base,base.mode==="index"?0:data.x[i],row);
    });
    if(kind==="scatter" && x.some(v=>typeof v!=="number")) throw new Error("散佈圖需要數值座標；文字類別可先選擇重新編號。");
    const series=data.series.map(s=>{
      const skipped=new Set(s.skip_indices || []);
      return {...s,values:indices.map((i,row)=>s.values[i]==null?null:number(measure,s.values[i],row)),
        skip_indices:indices.flatMap((i,row)=>skipped.has(i)?[row]:[])};
    });
    return {...data,x,series,source_indices:indices};
  }
  function axisLabel(label,axis) {
    return axis.mode==="index"?`${label}（重新編號）`:axis.mode==="offset"?`${label}（位移 ${axis.offset}）`:label;
  }
  function summary(data,kind,transform) {
    const rendered=view(data,kind,transform), notes=[];
    if(transform.drop_empty) notes.push(`移除全系列無值位置 ${data.x.length-rendered.x.length} 筆，保留 ${rendered.x.length} 筆`);
    for(const name of ["x","y"]){const axis=transform[name];
      if(axis.mode==="offset") notes.push(`${name.toUpperCase()} 位移 ${axis.offset}`);
      if(axis.mode==="index") notes.push(`${name.toUpperCase()} 重新編號：起點 ${axis.start}、間距 ${axis.step}`);
    }
    return notes.length?`${notes.join("；")}。原始資料保留。`:"";
  }
  return {defaults,settings,category,view,summary,axisLabel};
})();
