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
  /** 依跨度取1/2/5刻度，不能依絕對值把90～110擴成50～150。 */
  function niceBounds(min, max, padding = 0) {
    if (![min,max,padding].every(Number.isFinite) || min > max || padding < 0) throw new Error("這個座標軸沒有可用的數值。");
    if (min === max) padding = Math.max(padding, Math.abs(min) * 0.05 || 1);
    const low = min - padding, high = max + padding, target = (high - low) / 5;
    const power = 10 ** Math.floor(Math.log10(target));
    const factor = [1,2,5,10].find(value => value >= target / power);
    const step = power * factor;
    if (!Number.isFinite(step) || step <= 0) throw new Error("數值過大或過小，請手動設定範圍。");
    const clean = value => Number(value.toPrecision(15));
    let lower = clean(Math.floor(low / step) * step), upper = clean(Math.ceil(high / step) * step);
    // 浮點修整不得把實際觀測點裁掉。
    if (lower > min) lower = clean(lower - step);
    if (upper < max) upper = clean(upper + step);
    if (![lower,upper].every(Number.isFinite) || lower >= upper || lower > min || upper < max) throw new Error("無法取得有效範圍，請手動設定上下限。");
    return [lower, upper];
  }
  /** 母體標準差只用作顯示留白，並非信賴區間；以跨度正規化避免平方溢位。 */
  function measurementBounds(values, zeroBaseline = false) {
    if (!values.length) throw new Error("這個座標軸沒有可用的數值。");
    let min=Infinity,max=-Infinity;
    for(const value of values){min=Math.min(min,value);max=Math.max(max,value);}
    const span=max-min;
    let padding=0;
    if(span>0 && Number.isFinite(span)) {
      let count=0,mean=0,m2=0;
      for(const value of values){const x=(value-min)/span,delta=x-mean;count++;mean+=delta/count;m2+=delta*(x-mean);}
      padding=span*Math.max(0.05,Math.min(0.40,2*Math.sqrt(Math.max(0,m2/values.length))));
    }
    const result=niceBounds(min,max,padding);
    // 長條／面積以零為基準，避免用截斷的長度誇大量測差異；手動設定仍由呼叫者優先。
    if(zeroBaseline){result[0]=min>=0?0:Math.min(0,result[0]);result[1]=max<=0?0:Math.max(0,result[1]);}
    return result;
  }
  /** 使用呈現副本：空值／略過點不算，真實0及所有可見系列均保留。 */
  function bounds(data, kind, transform, axis) {
    const shown = view(data, kind, transform);
    if (category(kind, axis, transform)) return [0, shown.x.length > 1 ? shown.x.length - 1 : null];
    const base = kind === "horizontal_bar" ? "y" : "x", values=[];
    for (const series of shown.series) {
      const skipped = new Set(series.skip_indices || []);
      series.values.forEach((value, i) => {
        if (value == null || skipped.has(i)) return;
        const coordinate = axis === base ? shown.x[i] : value;
        if (Number.isFinite(coordinate)) values.push(coordinate);
      });
    }
    if(axis!==base) return measurementBounds(values,["bar","horizontal_bar","area"].includes(kind));
    // 時間／X資料軸只取整跨度，不套量測標準差留白。
    let min=Infinity,max=-Infinity;
    for(const value of values){min=Math.min(min,value);max=Math.max(max,value);}
    return niceBounds(min,max);
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
  return {defaults,settings,category,view,summary,axisLabel,niceBounds,measurementBounds,bounds};
})();
