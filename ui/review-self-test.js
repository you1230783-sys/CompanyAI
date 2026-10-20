/* 相同案例由正式 WebView2 --self-check 與獨立瀏覽器測試執行。 */
"use strict";
window.runReviewTests = async (check) => {
  const old=LMUI.getState(), oldSend=send, commands=[], savedPrompt=$("prompt").value;
  const frame=()=>new Promise(resolve=>setTimeout(resolve,30));
  const block=(text,label="第1行",format=null)=>({text,label,format});
  const sample={name:"報告.txt",path:"_AI_Output/test/報告.txt",before_revision:"v1",after_revision:"v2",office:false,
    before:{blocks:[block("標題"),block("原文😀 <img src=x>"),block("刪除行"),block("尾端")]},
    after:{blocks:[block("標題"),block("修訂😀 <script>"),block("尾端"),block("新增行")]}};
  try {
    send=message=>commands.push(message);
    LMUI.receive({...old,logged_in:true,can_send:true,update_required:false,active_id:"review-chat",messages:[],
      conversations:[{id:"review-chat",title:"內容審閱",project_id:"review-project"}],
      projects:{items:[],running:false}});
    await frame();
    const changed=ReviewUI.buildRows(sample).rows;
    check(changed.filter(r=>r.kind!=="same").length===3,"comparison preserves changed/deleted/added Unicode lines");
    const office=ReviewUI.buildRows({office:true,before:{scope:"Excel",blocks:[block("42","A!B2",{bold:false})]},after:{scope:"Excel",blocks:[block("42","A!B2",{bold:true})]}});
    check(office.rows[0].kind==="changed","Office format-only change remains visible");
    const large=ReviewUI.align(Array(800).fill("old"),Array(800).fill("new"));
    check(large.coarse&&large.pairs.length===800,"large comparison is bounded without dropping rows");
    ReviewUI.open(sample);await frame();
    const dialog=$("review-window");
    check(dialog.open && dialog.querySelector("del") && dialog.querySelector("ins"),"comparison opens red deletion and green insertion sides");
    check(!dialog.querySelector("img,script")&&dialog.textContent.includes("<script>"),"comparison treats source HTML as plain text");
    const cells=dialog.querySelector(".review-row[data-kind]").children;
    check(cells[0].getBoundingClientRect().right<=cells[1].getBoundingClientRect().left+1,"before and after remain side by side");
    const search=dialog.querySelector('input[type="search"]');search.value="新增行";search.dispatchEvent(new Event("input"));
    check(dialog.querySelectorAll(".review-row[data-kind]").length===1,"comparison search filters both sides");
    search.value="";search.dispatchEvent(new Event("input"));
    const toggle=dialog.querySelector('input[type="checkbox"]');toggle.checked=true;toggle.dispatchEvent(new Event("change"));
    check(!dialog.querySelector('[data-kind="same"]'),"only changes hides unchanged rows");
    dialog.querySelector("select").value="24";dialog.querySelector("select").dispatchEvent(new Event("change"));
    check(dialog.querySelector(".review-body").style.fontSize==="24px","comparison font size is adjustable");
    dialog.querySelector(".review-header button").click();check(dialog.classList.contains("review-maximized"),"comparison maximizes within app");
    dialog.querySelector(".review-header button").click();dialog.close();await frame();
    ReviewUI.open({...sample,before:null});
    check(dialog.textContent.includes("沒有保存修改前基準"),"legacy artifact never invents a baseline");
    dialog.close();await frame();
    const holder=document.createElement("div"),link=document.createElement("button");link.className="project-artifact";link.dataset.path=sample.path;holder.append(link);
    ReviewUI.attach(holder,{conversation:"review-chat",message_index:1,request_id:"review-run"});holder.querySelector(".review-open").click();
    const request=commands.at(-1).command;
    check(request.action==="review"&&request.path===sample.path&&request.request_id==="review-run","review request binds artifact to conversation message and run");
    ReviewUI.receive({type:"project_review",conversation:"other-chat",request_id:"review-run",path:sample.path,data:sample});
    check(!dialog.open,"foreign conversation response cannot open review");
    ReviewUI.receive({type:"project_review",conversation:"review-chat",request_id:"review-run",path:sample.path,data:sample});
    check(dialog.open,"matching response opens fixed snapshot");
    LMUI.receive({...LMUI.getState(),active_id:"other-chat"});await frame();
    check(!dialog.open,"conversation switch closes private review");
    LMUI.receive({...LMUI.getState(),active_id:"review-chat"});
    ReviewUI.receive({type:"project_methods",conversation:"review-chat",methods:[{title:"分析方法：配對",revision:"3",body:"先核對來源"}]});await frame();
    const methods=document.querySelector(".source-window[open]"),fields=methods.querySelectorAll("textarea");
    fields[0].value="核對欄位與樣本";fields[1].value="A.log，10:00–11:00，機台A";
    const before=commands.length;[...methods.querySelectorAll("button")].find(b=>b.textContent==="加入提問草稿").click();await frame();
    check($("prompt").value.includes("核對欄位與樣本")&&$("prompt").value.includes("A.log"),"method supports editable steps and new input conditions");
    // 草稿輸入有 250ms 防抖；等它真正送到測試攔截器才恢復原生 send，
    // 避免殘留的計時器在下一項測試中改寫原生狀態與畫面捲動位置。
    const draftDeadline=performance.now()+5000;
    while(commands.length===before && performance.now()<draftDeadline)await frame();
    const methodCommands=commands.slice(before);
    check(methodCommands.length===1 && methodCommands[0].type==="draft" && methodCommands[0].text===$("prompt").value,"method reuse saves the actual draft without executing or sending a chat");
    const panel=document.createElement("div");
    AnalysisUI.render(panel,{completion:{files:1,scopes:2,counts:{partial:1,failed:1},unresolved_scopes:2,failed_checks:1,notice:"實際紀錄"},coverage:{},
      timeline:[{group:"A",label:"開始",date:null,time_millis:null,path:"A.log",line:5,text:"<script>事件",revision:"r1",pair_status:"uncertain"}],
      sources:[{path:"A.log",line:5,revision:"r1",excerpt:"<img>來源"}],sources_truncated:true},true);
    check(panel.textContent.includes("2 個範圍待處理")&&panel.textContent.includes("1 項筆數核對未通過"),"completion keeps partial failures and failed checks visible");
    check(panel.textContent.includes("日期未知")&&panel.textContent.includes("時間／分組待確認"),"timeline preserves uncertainty");
    check(panel.textContent.includes("200處上限")&&!panel.querySelector("img,script"),"source cap is explicit and preview text is inert");
    check(!panel.querySelector('.analysis-disclosure').open,"completed analysis defaults to collapsed");

    // 同一份分析由執行中轉成完成時收起；之後由使用者決定展開，更新不搶走狀態。
    const analysis={coverage:{one:{path:"A.log",scope:"LOG 原文",status:"partial",detail:"已讀取 2 行"}}};
    AnalysisUI.render(panel,analysis);
    check(panel.querySelector('.analysis-disclosure').open,"running analysis starts expanded");
    panel.querySelector('.analysis-disclosure > summary').click();
    AnalysisUI.render(panel,{...analysis,review_required:true});
    check(!panel.querySelector('.analysis-disclosure').open,"running analysis respects manual collapse on update");
    panel.querySelector('.analysis-disclosure > summary').click();
    AnalysisUI.render(panel,analysis,true);
    check(!panel.querySelector('.analysis-disclosure').open,"finishing analysis collapses previously expanded content");
    panel.querySelector('.analysis-disclosure > summary').click();
    AnalysisUI.render(panel,{...analysis,review_required:true},true);
    check(panel.querySelector('.analysis-disclosure').open,"completed analysis preserves manual expansion on update");

    // 訊息整區重建也要保留目前閱讀狀態；切換對話則重新採用完成後收起的預設。
    const reply={role:"assistant",content:"完成答案",project_analysis:analysis};
    LMUI.receive({...LMUI.getState(),active_id:"review-chat",messages:[reply]});
    const disclosure=document.querySelector('#messages .analysis-disclosure');
    check(!disclosure.open,"saved answer analysis starts collapsed");
    disclosure.querySelector('summary').click();
    document.querySelector('#messages details[data-key="coverage"]').open=true;
    LMUI.receive({...LMUI.getState(),messages:[{...reply,content:"完成答案更新"}]});
    check(document.querySelector('#messages .analysis-disclosure').open && document.querySelector('#messages details[data-key="coverage"]').open,"message redraw preserves analysis and evidence expansion");
    LMUI.receive({...LMUI.getState(),active_id:"another-chat",messages:[reply]});
    check(!document.querySelector('#messages .analysis-disclosure').open,"another conversation defaults to collapsed analysis");
  } finally {
    clearTimeout(draftTimer);
    document.querySelectorAll(".review-window[open],.source-window[open]").forEach(d=>d.close());await frame();
    send=oldSend;LMUI.receive(old);$("prompt").value=savedPrompt;resizePrompt();
  }
};
