# CAD特殊圖形

- line：start/end；circle：center/radius；text：position/text/height；mtext：position/text/height/width，換行使用 \\P。
- aligned_dimension：start/end/location；diameter_dimension：start/end 為圓的相對端點，leader_length 大於零。
- leader 必填 kind/color/points/text/height，height 是正數文字高度（本版固定 3，請填 3），不能省略。points 三點分別為箭頭、轉折、水平段末端；第二與第三點 y 相同。自動帶箭頭，文字放在水平段左端上方，2008 使用原生註解，2022 使用獨立文字；關聯持久性仍須實測。依文字長度留足水平段。
- hatch：boundary 是有序不自交外框頂點（不重複首點），holes 為外框內互不相交圓孔，seed 在填充區內且不在孔內。圖樣固定 ANSI31；scale 為正值。請自行核對幾何關係，不能以程式未報錯宣稱外觀合格。
- curve：x/y 為 t 的數學公式，t_start < t_end、segments 2–1000。支援 + - * / **、sin/cos/tan/sqrt/exp/log/abs、pi/e。禁止 Python 程式、屬性、索引。結果是取樣直線近似，不是原生 spline，也沒有自動誤差界線；不能宣稱連續曲線精度已保證。
- 正弦例：x="t", y="10*sin(2*pi*t/100)", t_start=0,t_end=200,segments=100。
- 螺旋例：x="(5+2*t)*cos(t)", y="(5+2*t)*sin(t)", t_start=0,t_end=12.566370614359172,segments=128。


## 線型


- 新增 line／circle／curve 可選 line_style={"name":"DASHED","scale":1}；未指定時依當輪Schema處理：若列required且允許null，請填null，程式還原為原工具的省略預設。既有已讀線／圓／弧使用 edits 的 kind="linetype"、value={"name":"CENTER","scale":1}，仍需 handle、revision 與可讀線型屬性。
- 名稱：CONTINUOUS 實線、DASHED 虛線、HIDDEN 隱藏線、CENTER 中心線、DASHDOT 點畫線、DOT 點線、BYLAYER、BYBLOCK；scale 必須介於 0.000001–1000000。
- 不調整全圖 LTSCALE，不修改尺寸／引線／圖塊內部線型。曲線為取樣線段，線型會逐段重新起算；外觀需目視確認，不保證短線段看得到虛線。
- 先沿用已有定義；缺少時只查 CAD 安裝目錄的標準 LIN 檔。找不到則請使用者先在 CAD 載入，不能聲稱已套用或自動改實線。取得失敗後依 recovery 處理。
