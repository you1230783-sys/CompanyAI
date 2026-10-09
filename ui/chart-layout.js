/* 圖表文字的有限排版：資料座標只用來定位引線，拖曳只保存圖框內的比例位置。 */
"use strict";
window.ChartLayout = (() => {
  const empty = () => ({title:null, legend:null, reference_labels:[], annotations:[]});
  const clamp = (value, min, max) => Math.max(min, Math.min(max, value));
  let context;

  function textWidth(text, size, bold = false, family = "sans-serif", italic = false) {
    context ??= document.createElement("canvas").getContext?.("2d");
    if (!context) return [...text].reduce((sum, c) => sum + size * (c.charCodeAt(0)>255 ? 1 : .6), 0);
    context.font = `${italic ? "italic " : ""}${bold ? "bold " : ""}${size}px "${family}"`;
    return context.measureText(text).width;
  }

  // 先明確換行，再讓畫面及PNG繪製同一段文字；不使用HTML，也不裁掉長標籤。
  function wrap(text, width, size, bold = false, family = "sans-serif", italic = false) {
    const lines = [];
    for (const paragraph of String(text).split("\n")) {
      let line = "";
      for (const character of paragraph) {
        if (line && textWidth(line + character, size, bold, family, italic) > width) { lines.push(line); line = ""; }
        line += character;
      }
      lines.push(line);
    }
    return {text:lines.join("\n"), width:Math.max(...lines.map(line => textWidth(line, size, bold, family, italic))), height:lines.length*Math.ceil(size*1.3)};
  }

  function fit(box, frame) {
    return {...box, x:clamp(box.x, frame.x, Math.max(frame.x, frame.x+frame.width-box.width)),
      y:clamp(box.y, frame.y, Math.max(frame.y, frame.y+frame.height-box.height))};
  }
  function overlap(a, b, gap = 5) {
    return Math.max(0, Math.min(a.x+a.width+gap,b.x+b.width)-Math.max(a.x-gap,b.x)) *
      Math.max(0, Math.min(a.y+a.height+gap,b.y+b.height)-Math.max(a.y-gap,b.y));
  }

  // 候選點取既有文字框邊緣，而非無限制反覆位移。空間不足時仍保留文字並回報擁擠。
  function place(preferred, occupied, frame) {
    const start = fit(preferred, frame);
    const xs = [start.x, frame.x, frame.x+frame.width-start.width];
    const ys = [start.y, frame.y, frame.y+frame.height-start.height];
    for (const box of occupied) {
      xs.push(box.x-start.width-6, box.x+box.width+6);
      ys.push(box.y-start.height-6, box.y+box.height+6);
    }
    let best = start, score = Infinity;
    for (const x of xs) for (const y of ys) {
      const candidate = fit({...start,x,y},frame);
      const area = occupied.reduce((sum,box)=>sum+overlap(candidate,box),0);
      const distance = (candidate.x-start.x)**2+(candidate.y-start.y)**2;
      const next = area*1e8+distance;
      if (next < score) { best = candidate; score = next; }
    }
    return {...best, crowded:best.width>frame.width || best.height>frame.height || occupied.some(box=>overlap(best,box)>0)};
  }

  function position(saved, fallback, width, height) {
    return saved && Number.isFinite(saved.x) && Number.isFinite(saved.y)
      ? {x:clamp(saved.x,0,1)*width,y:clamp(saved.y,0,1)*height} : fallback;
  }
  function update(layout, key, point) {
    const result = structuredClone(layout || empty());
    if (key.startsWith("line-")) {
      const index = Number(key.slice(5));
      result.reference_labels ??= [];
      while(result.reference_labels.length<=index) result.reference_labels.push(null);
      result.reference_labels[index] = point;
    } else if (key.startsWith("text-")) {
      const annotation=result.annotations?.[Number(key.slice(5))];
      if(annotation) annotation.position=point;
    } else result[key] = point;
    return result;
  }

  // 只能在setOption建立座標系後呼叫；resize/dataZoom及PNG都走同一排版流程。
  function draw(instance, view, exporting = false, footer = []) {
    const width=instance.getWidth(), height=instance.getHeight(), layout=view.layout || empty();
    // 切換頁面時畫布可能暫時不可見；等待ResizeObserver取得實際大小再定位。
    if(width<200 || height<200) return {boxes:[],crowded:false,width,height};
    const frame={x:8,y:8,width:width-16,height:height-(exporting?165:55)-16};
    const occupied=[], boxes=[], graphic=[...footer];
    const font=exporting?22:18, title=wrap(view.title,Math.min(width-32,exporting?1450:550),font,true);
    const titleBox=fit({key:"title",label:"標題",width:title.width+2,height:title.height+2,
      ...position(layout.title,{x:(width-title.width)/2,y:16},width,height)},frame);
    occupied.push(titleBox);boxes.push(titleBox);
    const titleOption={text:title.text,left:titleBox.x,top:titleBox.y,padding:0,
      textStyle:{fontSize:font,fontFamily:"sans-serif",fontWeight:"bold",width:Math.ceil(title.width)+2,lineHeight:Math.ceil(font*1.3)}};

    let legendOption={show:false}, crowded=titleBox.width>frame.width || titleBox.height>frame.height;
    if(view.legend!=="hidden") {
      // 圖例另留內距與字型取整空間；不能以量到的剛好寬度讓短名稱也變成省略號。
      const vertical=view.legend==="right", nameWidth=Math.min(180,Math.max(100,...view.series.map(s=>Math.ceil(textWidth(s.name,12))+10)));
      const itemWidth=nameWidth+45, columns=vertical?1:Math.max(1,Math.floor((width-40)/itemWidth));
      const legendWidth=Math.min(width-32,Math.min(columns,view.series.length)*itemWidth+10);
      const legendHeight=Math.min(frame.height-80,Math.ceil(view.series.length/columns)*24+14);
      const fallback={x:vertical?width-legendWidth-12:view.legend==="bottom_right"?width-legendWidth-20:(width-legendWidth)/2,
        y:vertical?80:view.legend==="top"?55:height-(exporting?100:65)-legendHeight};
      const legendBox=place({key:"legend",label:"圖例",width:legendWidth,height:legendHeight,
        ...position(layout.legend,fallback,width,height)},occupied,frame);
      occupied.push(legendBox);boxes.push(legendBox);crowded ||= legendBox.crowded;
      legendOption={show:true,left:legendBox.x,top:legendBox.y,right:null,bottom:null,width:legendWidth-10,height:legendHeight-10,
        orient:vertical?"vertical":"horizontal",padding:5,itemWidth:25,itemGap:10,
        textStyle:{fontSize:12,width:nameWidth,overflow:"truncate"},tooltip:{show:true}};
    }

    // 任意文字也是純Canvas圖元，與參考線一起避讓，PNG沿用字型與底線規則。
    for(const [index, annotation] of (layout.annotations || []).entries()) {
      const size=annotation.font_size, family=annotation.font_family;
      const label=wrap(annotation.text,Math.max(80,width*.55),size,annotation.bold,family,annotation.italic);
      const box=place({key:`text-${index}`,label:`文字：${annotation.text}`,width:label.width+12,height:label.height+10,
        ...position(annotation.position,{x:width*.4,y:height*.35},width,height)},occupied,frame);
      crowded ||= box.crowded;occupied.push(box);boxes.push(box);
      const children=[];
      if(annotation.background) children.push({type:"rect",shape:{x:0,y:0,width:box.width,height:box.height,r:3},style:{fill:annotation.background}});
      children.push({type:"text",x:6,y:5,style:{text:label.text,fontSize:size,fontFamily:family,fontWeight:annotation.bold?"bold":"normal",
        fontStyle:annotation.italic?"italic":"normal",lineHeight:Math.ceil(size*1.3),fill:annotation.color,verticalAlign:"top"}});
      // Canvas文字沒有跨後端一致的textDecoration；按相同換行與量測逐行畫底線。
      if(annotation.underline) for(const [lineIndex,line] of label.text.split("\n").entries()) {
        const y=5+(lineIndex+1)*Math.ceil(size*1.3)-2;
        children.push({type:"line",shape:{x1:6,y1:y,x2:6+textWidth(line,size,annotation.bold,family,annotation.italic),y2:y},style:{stroke:annotation.color,lineWidth:Math.max(1,size/18)}});
      }
      graphic.push({id:`chart-text-${index}`,type:"group",x:box.x,y:box.y,z:102,silent:true,children});
    }

    const grid=instance.getOption().grid[0], top=Number(grid.top), bottom=height-Number(grid.bottom), right=width-Number(grid.right);
    const labels=view.lines.map((line,index)=>({line,index,saved:layout.reference_labels?.[index]}))
      .sort((a,b)=>Number(!!b.saved)-Number(!!a.saved));
    for(const {line,index,saved} of labels) {
      if(!line.name.trim()) continue;
      const pixel=instance.convertToPixel(line.axis==="x"?{xAxisIndex:0}:{yAxisIndex:0},line.value);
      const probe=line.axis==="x"?[pixel,(top+bottom)/2]:[(Number(grid.left)+right)/2,pixel];
      // 線在縮放範圍以外時，文字也不冒充出現在圖內；還原縮放後會一起回來。
      if(!Number.isFinite(pixel) || !instance.containPixel({gridIndex:0},probe)) continue;
      const label=wrap(line.name,Math.min(200,Math.max(90,width*.28)),12);
      const anchor=line.axis==="x"?{x:pixel,y:top+8}:{x:right-8,y:pixel};
      const fallback=line.axis==="x"?{x:pixel+5,y:top+8}:{x:right-label.width-20,y:pixel-label.height-10};
      const box=place({key:`line-${index}`,label:`參考線文字：${line.name}`,width:label.width+12,height:label.height+10,
        ...position(saved,fallback,width,height)},occupied,frame);
      crowded ||= box.crowded;occupied.push(box);boxes.push(box);
      const endpoint={x:clamp(anchor.x,box.x,box.x+box.width),y:clamp(anchor.y,box.y,box.y+box.height)};
      graphic.push({id:`reference-link-${index}`,type:"line",silent:true,z:101,
        shape:{x1:anchor.x,y1:anchor.y,x2:endpoint.x,y2:endpoint.y},style:{stroke:line.color,lineWidth:1,lineDash:[3,3],opacity:.7}});
      graphic.push({id:`reference-label-${index}`,type:"group",x:box.x,y:box.y,silent:true,z:102,children:[
        {type:"rect",shape:{x:0,y:0,width:box.width,height:box.height,r:3},style:{fill:"rgba(255,255,255,.94)",stroke:line.color,lineWidth:.6}},
        {type:"text",x:6,y:5,style:{text:label.text,fontSize:12,fontFamily:"sans-serif",lineHeight:16,fill:line.color,verticalAlign:"top"}}
      ]});
    }
    instance.setOption({title:titleOption,legend:legendOption,graphic},{replaceMerge:["graphic"],lazyUpdate:false});
    return {boxes,crowded,width,height};
  }

  // 只有排版模式才加可聚焦拖曳框；平常圖例點選、提示與資料縮放維持原功能。
  function handles(plot, result, moved, editText) {
    const layer=document.createElement("div");layer.className="chart-layout-layer";
    for(const box of result.boxes) {
      const handle=document.createElement("button");handle.type="button";handle.className="chart-layout-handle";
      handle.dataset.layoutKey=box.key;handle.setAttribute("aria-label",`移動${box.label}`);handle.title=`拖曳${box.label}；方向鍵微調，Shift加速`;
      Object.assign(handle.style,{left:`${box.x}px`,top:`${box.y}px`,width:`${box.width}px`,height:`${box.height}px`});
      const commit=(x,y)=>moved(box.key,{x:clamp(x/result.width,0,1),y:clamp(y/result.height,0,1)});
      let drag=null;
      handle.onpointerdown=event=>{
        if(event.button!==0)return;
        event.preventDefault();event.stopPropagation();handle.focus();handle.setPointerCapture(event.pointerId);
        drag={id:event.pointerId,x:event.clientX,y:event.clientY};
      };
      handle.onpointermove=event=>{
        if(!drag)return;
        handle.style.left=`${clamp(box.x+event.clientX-drag.x,0,result.width-box.width)}px`;
        handle.style.top=`${clamp(box.y+event.clientY-drag.y,0,result.height-box.height)}px`;
      };
      handle.onpointerup=event=>{
        if(!drag)return;
        handle.releasePointerCapture(drag.id);drag=null;event.stopPropagation();
        // 單擊不重建按鈕，保留瀏覽器第二次點擊的同一目標，雙擊才能正常開啟文字編輯。
        const x=parseFloat(handle.style.left),y=parseFloat(handle.style.top);
        if(Math.abs(x-box.x)>.5||Math.abs(y-box.y)>.5)commit(x,y);
      };
      handle.onpointercancel=()=>{drag=null;handle.style.left=`${box.x}px`;handle.style.top=`${box.y}px`;};
      handle.ondblclick=event=>{if(box.key.startsWith("text-")){event.stopPropagation();editText?.(Number(box.key.slice(5)));}};
      handle.onkeydown=event=>{
        const direction={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[event.key];
        if(!direction)return;
        event.preventDefault();event.stopPropagation();const step=event.shiftKey?10:1;
        commit(box.x+direction[0]*step,box.y+direction[1]*step);
      };
      layer.append(handle);
    }
    plot.querySelector(".chart-layout-layer")?.remove();plot.append(layer);
  }
  return {empty,update,draw,handles,place,overlap};
})();
