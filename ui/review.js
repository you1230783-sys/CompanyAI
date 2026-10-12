/* 本機唯讀審閱：所有文字使用 textContent；快照來自原生歷史，不使用模型生成的 HTML。 */
"use strict";
window.ReviewUI = (() => {
  const make = (tag, text = "", className = "") => {
    const node = document.createElement(tag); node.textContent = text; node.className = className; return node;
  };
  let owner = null;
  let dialog, pending, rows = [], cursor = -1, limit = 300;
  let body, summary, query, changedOnly, font, activeData;

  // 先移除相同首尾，小區段使用 LCS；超大變更保留全部內容並明示使用區塊比較。
  // 記憶體上限與內容長度無平方關係；不為了避免成本而默默省略資料。
  function align(left, right, key = value => value) {
    let prefix = 0, endLeft = left.length, endRight = right.length;
    const result = []; let coarse = false;
    while (prefix < endLeft && prefix < endRight && key(left[prefix]) === key(right[prefix])) {
      result.push([left[prefix], right[prefix]]); prefix++;
    }
    while (endLeft > prefix && endRight > prefix && key(left[endLeft-1]) === key(right[endRight-1])) {endLeft--;endRight--;}
    const a = left.slice(prefix,endLeft), b = right.slice(prefix,endRight);
    if (a.length * b.length <= 500000) {
      const width = b.length + 1, table = new Uint32Array((a.length+1)*width);
      for(let i=a.length-1;i>=0;i--) for(let j=b.length-1;j>=0;j--)
        table[i*width+j] = key(a[i]) === key(b[j]) ? 1+table[(i+1)*width+j+1] : Math.max(table[(i+1)*width+j],table[i*width+j+1]);
      let i=0,j=0, deleted=[],added=[];
      const flush=()=>{for(let n=0;n<Math.max(deleted.length,added.length);n++)result.push([deleted[n]??null,added[n]??null]);deleted=[];added=[];};
      while(i<a.length || j<b.length) {
        if(i<a.length&&j<b.length&&key(a[i])===key(b[j])) {flush();result.push([a[i++],b[j++]]);}
        else if(i<a.length&&(j===b.length||table[(i+1)*width+j]>=table[i*width+j+1])) deleted.push(a[i++]);
        else added.push(b[j++]);
      }
      flush();
    } else {
      coarse=true;
      for(let i=0;i<Math.max(a.length,b.length);i++)result.push([a[i]??null,b[i]??null]);
    }
    for(let i=endLeft,j=endRight;i<left.length;i++,j++)result.push([left[i],right[j]]);
    return {pairs:result,coarse};
  }
  function same(a,b) {return a&&b&&a.text===b.text&&JSON.stringify(a.format)===JSON.stringify(b.format)&&a.kind===b.kind;}
  function buildRows(data) {
    // Excel 與投影片使用來源位置配對，避免相同數值被對齊到另一張表／投影片。
    const positional = /^(Excel|PowerPoint)/.test(data.after?.scope || data.before?.scope || "");
    const comparison=align(data.before?.blocks||[],data.after?.blocks||[],block=>positional?block.label:block.text);
    return {coarse:comparison.coarse,rows:comparison.pairs.map(([before,after])=>({before,after,
      kind:!before?"added":!after?"deleted":same(before,after)?"same":"changed"}))};
  }
  function ensure() {
    if(dialog)return;
    dialog=make("dialog","","review-window");dialog.id="review-window";
    const header=make("div","","review-header"), title=make("strong","內容比較");
    const maximize=make("button","放大／還原"),close=make("button","關閉");
    maximize.onclick=()=>dialog.classList.toggle("review-maximized");close.onclick=()=>dialog.close();
    header.append(title,maximize,close);
    const tools=make("div","","review-toolbar");
    query=make("input");query.type="search";query.placeholder="搜尋兩側文字";query.setAttribute("aria-label","搜尋兩側文字");
    changedOnly=make("input");changedOnly.type="checkbox";
    const label=make("label","只看修改處 ");label.append(changedOnly);
    font=make("select");font.setAttribute("aria-label","比較文字大小");
    for(const size of [14,16,18,20,24]) {const option=make("option",size+" px");option.value=size;option.selected=size===16;font.append(option);}
    const previous=make("button","上一處"), next=make("button","下一處");
    previous.onclick=()=>jump(-1);next.onclick=()=>jump(1);
    query.oninput=changedOnly.onchange=()=>{limit=300;cursor=-1;render();};
    font.onchange=()=>body.style.fontSize=font.value+"px";
    tools.append(query,label,previous,next,font);
    summary=make("div","","review-summary");body=make("div","","review-body");
    dialog.append(header,tools,summary,body);document.body.append(dialog);
    dialog.addEventListener("close",()=>{pending=null;activeData=null;rows=[];body.replaceChildren();});
  }
  function highlight(parent,text,other,removed) {
    const chars=Array.from(text), otherChars=Array.from(other);
    // 長段落保留共同首尾，較短文字以字元LCS凸顯多處細節。
    const parts=removed?align(chars,otherChars):align(otherChars,chars);
    let buffer="", marked=null;
    const flush=()=>{if(!buffer)return;const span=make(marked?(removed?"del":"ins"):"span",buffer);parent.append(span);buffer="";};
    for(const [a,b] of parts.pairs) {
      const value=removed?a:b;if(value===null)continue;
      const changed=a!==b;if(marked!==changed){flush();marked=changed;}buffer+=value;
    }
    flush();
  }
  function cell(block,other,kind,removed) {
    const node=make("div","","review-cell "+(kind==="same"?"":removed?"review-removed":"review-added"));
    if(!block){node.append(make("small","此側無內容"));return node;}
    node.append(make("small",block.label));const text=make("div","","review-text");
    if(kind!=="same")highlight(text,block.text,other?.text||"",removed);else text.textContent=block.text;
    node.append(text);
    if(other&&JSON.stringify(block.format)!==JSON.stringify(other.format)) {
      const detail=make("details");detail.append(make("summary","格式變更"),make("pre",JSON.stringify(block.format,null,2)));node.append(detail);
    }
    return node;
  }
  function visibleRows() {
    const term=query.value.toLocaleLowerCase();
    return rows.map((row,index)=>({row,index})).filter(({row})=>(!changedOnly.checked||row.kind!=="same")&&(!term||
      [row.before?.text,row.after?.text,row.before?.label,row.after?.label].some(s=>s?.toLocaleLowerCase().includes(term))));
  }
  function render() {
    if(!activeData)return;
    body.replaceChildren();summary.replaceChildren();
    if(activeData.before===null) {
      summary.append(make("p","此成果沒有保存修改前基準，無法比較；以下僅顯示修改後快照。"));
    } else {
      const count=kind=>rows.filter(r=>r.kind===kind).length;
      summary.append(make("p",`${activeData.name} · 新增 ${count("added")}、刪除 ${count("deleted")}、修改 ${count("changed")} 個${activeData.office?"內容區塊":"文字行"}`));
    }
    if(activeData.draft)summary.append(make("p","此側為尚未交付的草稿，內容可能未完成驗證。"));
    summary.append(make("small",activeData.notice||"唯讀內容快照"));
    if(activeData.coarse)summary.append(make("p","變更範圍較大，部分採區塊對照；內容未省略，細部對齊可能較粗略。"));
    const metadata=make("details");metadata.append(make("summary","版本與結構變更"),make("pre",`修改前：${activeData.before_revision||"未保存"}\n修改後：${activeData.after_revision}\n${activeData.path}`));
    if(JSON.stringify(activeData.before?.structure)!==JSON.stringify(activeData.after?.structure)) {
      metadata.append(make("p","文件結構有變更（不等同實際版面預覽）。"),make("pre",JSON.stringify({before:activeData.before?.structure,after:activeData.after?.structure},null,2)));
    }
    summary.append(metadata);
    const heading=make("div","","review-row review-heading");heading.append(make("strong","修改前 · 刪除為紅色 −"),make("strong","修改後 · 新增為綠色 ＋"));body.append(heading);
    const visible=visibleRows();
    for(const {row,index} of visible.slice(0,limit)) {
      const item=make("div","","review-row");item.dataset.reviewIndex=index;item.dataset.kind=row.kind;
      item.append(cell(row.before,row.after,row.kind,true),cell(row.after,row.before,row.kind,false));body.append(item);
    }
    body.append(make("p",`顯示 ${Math.min(limit,visible.length)}／${visible.length} 個符合條件的區塊。`));
    if(visible.length>limit){const more=make("button","顯示接下來300個區塊");more.onclick=()=>{const position=body.scrollTop;limit+=300;render();body.scrollTop=position;};body.append(more);}
  }
  function jump(direction) {
    const changes=visibleRows().filter(({row})=>row.kind!=="same");if(!changes.length)return;
    let position=changes.findIndex(({index})=>index===cursor);
    position=position<0?(direction>0?0:changes.length-1):(position+direction+changes.length)%changes.length;cursor=changes[position].index;
    const visible=visibleRows(), target=visible.findIndex(({index})=>index===cursor);
    if(target>=limit){limit=target+1;render();}
    body.querySelector(`[data-review-index="${cursor}"]`)?.scrollIntoView({block:"center"});
  }
  function open(data) {
    ensure();owner=state.active_id;activeData=data;const compared=buildRows(data);rows=compared.rows;activeData.coarse=compared.coarse;
    cursor=-1;limit=300;query.value="";changedOnly.checked=false;render();if(!dialog.open)dialog.showModal();
  }
  function attach(container,target) {
    const paths=new Set();
    for(const link of container.querySelectorAll(".project-artifact")) {
      const path=link.dataset.path;if(paths.has(path)||!path||/\.png$/i.test(path))continue;paths.add(path);
      const button=make("button","查看修改","review-open");
      button.onclick=()=>{ensure();owner=state.active_id;pending={...target,path};send({type:"project",command:{action:"review",...pending}});};
      link.after(button);
    }
  }
  function source(value,title="來源快照") {
    owner=state.active_id;
    const modal=make("dialog","","source-window");const close=make("button","關閉");close.onclick=()=>modal.close();
    modal.append(make("h3",title),close,make("p","工具取得當時的來源快照；目前原檔可能已有變更。"));
    if(value.path)modal.append(make("p",value.path+" · "+(value.label||"")+(value.line?` · 第 ${value.line} 行`:"")));
    if(value.revision)modal.append(make("small","來源版本："+value.revision));
    modal.append(make("pre",typeof value.excerpt==="string"?value.excerpt:window.AnalysisUI.excerpt(value.excerpt??value.text??value)));
    if(value.excerpt_truncated)modal.append(make("p","此處為節錄；完整內容需以原閱讀工具查回。"));
    modal.addEventListener("close",()=>modal.remove());document.body.append(modal);modal.showModal();
  }
  function methods(items) {
    owner=state.active_id;
    const modal=make("dialog","","source-window"),close=make("button","關閉");close.onclick=()=>modal.close();
    modal.append(make("h3","重用專案分析方法"),close,make("p","選擇過往方法後，指定本次資料與條件。套用只填入提問草稿；執行前仍須重新核對新檔案。"));
    if(!items.length)modal.append(make("p","尚無已保存的方法。完成分析時可請助理保存適用條件、步驟、驗證及限制。"));
    for(const method of items) {
      const detail=make("details"),content=make("textarea");content.value=method.body;content.rows=8;content.setAttribute("aria-label","方法與步驟");
      const inputs=make("textarea");inputs.placeholder="本次來源檔案、日期／時段、機台、欄位與希望產出的成果";inputs.rows=3;inputs.setAttribute("aria-label","本次資料與條件");
      const use=make("button","加入提問草稿");use.onclick=()=>{
        if(!inputs.value.trim()){inputs.focus();return;}
        const prompt=document.getElementById("prompt");
        const text=`請依以下方法分析本次資料。先重新核對來源版本、格式、欄位及樣本；不適用時說明原因，不直接重播舊程式。\n方法：${method.title}（版本 ${method.revision}）\n${content.value}\n本次資料與條件：${inputs.value}`;
        prompt.value=(prompt.value.trim()?prompt.value+"\n\n":"")+text;
        prompt.dispatchEvent(new Event("input",{bubbles:true}));send({type:"draft",text:prompt.value});modal.close();prompt.focus();
      };
      detail.append(make("summary",method.title),content,inputs,use);modal.append(detail);
    }
    modal.addEventListener("close",()=>modal.remove());document.body.append(modal);modal.showModal();
  }
  function receive(message) {
    if(message.type==="project_review"&&pending&&message.conversation===state.active_id&&message.request_id===pending.request_id&&message.path===pending.path) {
      if(message.error){toast(message.error);pending=null;}else open(message.data);
    }
    if(message.type==="project_methods"&&state.logged_in&&message.conversation===state.active_id)methods(message.methods||[]);
  }
  function sync() {
    if(!state.logged_in || (owner!==null && owner!==state.active_id)) {
      document.querySelectorAll(".review-window[open],.source-window[open]").forEach(d=>d.close());
      pending=null;owner=null;
    }
  }
  return {attach,receive,source,open,buildRows,align,sync};
})();
