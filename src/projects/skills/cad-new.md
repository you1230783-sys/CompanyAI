# 新建CAD圖面
cad_draw(version,name,operations)建立新圖、儲存並重開核對。預設latest；指定版本才先掃描選ProgID。不必先讀目前圖面。
以工具schema直接組計畫：線、圓、文字、尺寸等。一般需求採合理預設，真正幾何衝突才詢問；孔邊與孔心距離要區分。
需要引線／剖面／公式曲線／線型時按需讀cad-geometry；指令讀cad-commands。讀取不會切換技能。不要讀所有CAD指南才開始。
開啟中圖面→load_skill(cad-active)；磁碟DWG副本→cad-copy；表格座標→cad-excel。
