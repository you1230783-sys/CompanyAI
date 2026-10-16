"""建立法蘭板合成圖面供分頁測試；只新增獨立檔案，不改使用者已開啟的圖面。"""
import argparse
import json
from pathlib import Path
import sys


def operations(segments):
    """一條曲線故意取 600 段，讓原需求形成超過六頁的可重現圖元集合。"""
    result = []
    def add(kind, color, **values):
        result.append(dict(kind=kind, color=color, **values))
    corners = [[20, 0], [200, 0], [220, 20], [220, 120], [200, 140], [20, 140], [0, 120], [0, 20]]
    for start, end in zip(corners, corners[1:] + corners[:1]):
        add("line", 7, start=start, end=end)
    add("circle", 4, center=[110, 70], radius=30)
    for x in (30, 190):
        for y in (25, 115):
            add("circle", 1, center=[x, y], radius=6)
    for start, end in [([-10,70],[230,70]), ([110,-10],[110,150])]:
        add("line", 8, start=start, end=end, line_style={"name":"CENTER","scale":1})
    for start,end,location in [([0,20],[220,20],[110,-25]),([20,0],[20,140],[-25,70]),([30,115],[190,115],[110,165])]:
        add("aligned_dimension", 7, start=start,end=end,location=location)
    add("diameter_dimension", 4,start=[80,70],end=[140,70],leader_length=12)
    add("leader", 3,points=[[190,115],[245,180],[340,180]],text="4處安裝孔 %%c12",height=3)
    add("leader", 3,points=[[210,130],[245,195],[340,195]],text="四角倒角 20×20",height=3)
    add("text", 2,position=[0,215],text="法蘭板綜合測試 / FLANGE TEST",height=5)
    add("mtext", 7,position=[360,180],text="單位 mm\\P孔徑 60 / 12\\P倒角 20×20\\P白外框、紅安裝孔、青中央孔",height=3,width=100)
    add("hatch",8,boundary=[[260,0],[340,0],[340,60],[260,60]],holes=[{"center":[300,30],"radius":10}],seed=[265,5],scale=3)
    add("text",2,position=[260,-12],text="ANSI31 / 比例3 / 孔內不填",height=3)
    add("curve",6,x="30+t",y="-65+12*sin(2*pi*t/160)",t_start=0,t_end=160,segments=segments)
    add("text",2,position=[30,-90],text="正弦：長160 / 振幅12 / 一週期",height=3)
    for index,(name,scale) in enumerate([("CONTINUOUS",1),("DASHED",1),("DASHED",2),("HIDDEN",1),("CENTER",1),("DASHDOT",1)]):
        y=-120-index*12
        add("line",7,start=[20,y],end=[120,y],line_style={"name":name,"scale":scale})
        add("text",2,position=[130,y],text=f"{name} / {scale}",height=3)
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage",type=Path)
    parser.add_argument("output",type=Path)
    parser.add_argument("--segments",type=int,default=600)
    args=parser.parse_args()
    output=args.output.resolve()
    if output.exists():
        raise RuntimeError("合成圖檔已存在，拒絕重畫或覆寫")
    output.parent.mkdir(parents=True,exist_ok=True)
    sys.path.insert(0,str(args.stage.resolve()))
    import worker
    import pythoncom
    import win32com.client
    pythoncom.CoInitialize()
    report={}
    request={"action":"draw","version":"GStarCAD.Application.22","output":str(output),
             "operations":operations(args.segments),"launch_if_missing":False,"wait_for_ready":False}
    output.with_suffix(".plan.json").write_text(json.dumps(request,ensure_ascii=False,indent=2),encoding="utf-8")
    try:
        report.update(worker.execute(request,report))
        # 正式繪圖管線完成存檔重開驗證後會關閉自建文件；再次開啟供目前圖面閱讀。
        app=win32com.client.Dispatch("GStarCAD.Application.22")
        doc=app.Documents.Open(str(output))
        report["fixture_active_document"]=str(doc.FullName)
        print(json.dumps({"status":report["status"],"entities":int(doc.ModelSpace.Count)},ensure_ascii=False))
    finally:
        output.with_suffix(".report.json").write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf-8")
        pythoncom.CoUninitialize()


if __name__=="__main__":
    main()
