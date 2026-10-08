/* 只修復能核對欄數與分隔列的表格，保留原始 payload／歷史文字，不猜一般直線符號。 */
"use strict";
window.ReplyTables = (() => {
  function cells(line) {
    const text=line.trim();
    if(!text.startsWith("|") || !text.endsWith("|")) return null;
    const result=[];let start=1,slashes=0;
    for(let i=1;i<text.length;i++) {
      const character=text[i];
      if(character==="|" && slashes%2===0) {result.push(text.slice(start,i).trim());start=i+1;}
      slashes=character==="\\"?slashes+1:0;
    }
    return start===text.length?result:null;
  }
  const row=text=>!text.includes("\n") && !text.startsWith("    ") && !text.startsWith("\t") && cells(text);
  function valid(lines) {
    if(lines.length<2) return false;
    const rows=lines.map(cells),columns=rows[0]?.length;
    return !!columns && rows.every(cells=>cells?.length===columns) && rows[1].every(cell=>/^:?-{3,}:?$/.test(cell));
  }
  function compact(line) {
    if(!row(line) || line.includes("```") || line.includes("~~~")) return line;
    // 相鄰兩列被壓成「... | | ...」時，只在完整表頭／分隔列與所有資料欄數一致才展開。
    const parts=line.split(/\|[ \t]+\|/);
    if(parts.length<2) return line;
    const lines=parts.map((part,index)=>(index?"|":"")+part+(index<parts.length-1?"|":""));
    return valid(lines)?lines.join("\n"):line;
  }
  function normalize(text) {
    let fence=null;
    return text.split("\n").map(line=>{
      const marker=line.trimStart().match(/^(`{3,}|~{3,})/);
      if(marker) {
        if(!fence) fence=marker[1];
        else if(marker[1][0]===fence[0] && marker[1].length>=fence.length) fence=null;
        return line;
      }
      return fence?line:compact(line);
    }).join("\n");
  }
  function group(values) {
    const items=values.map(normalize),result=[];
    for(let i=0;i<items.length;i++) {
      if(row(items[i]) && row(items[i+1]||"") && valid([items[i],items[i+1]])) {
        const lines=[items[i],items[++i]],columns=cells(lines[0]).length;
        while(i+1<items.length && row(items[i+1]) && cells(items[i+1]).length===columns) lines.push(items[++i]);
        result.push(lines.join("\n"));
      } else result.push(items[i]);
    }
    return result;
  }
  return {normalize,group};
})();
