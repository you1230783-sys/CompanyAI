//! 產生不含外部資料的 PDF fixture，經真實 AppContainer 與 broker 驗證。
use company_ai::{
    projects::{files::Broker, sandbox::Worker, Project, Tool},
    AppResult,
};
use pdf_extract::{dictionary, Document, Object, Stream};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool};

fn fixture(text: &str, padding: usize) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages = doc.new_object_id();
    if padding > 0 {
        doc.add_object(Stream::new(dictionary! {}, vec![b' '; padding]));
    }
    let font = doc.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier" },
    );
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let content = doc.add_object(Stream::new(
        dictionary! {},
        format!("BT /F1 12 Tf 30 700 Td ({text}) Tj ET").into_bytes(),
    ));
    let page =
        doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Contents" => content });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1, "Resources" => resources, "MediaBox" => vec![0.into(),0.into(),595.into(),842.into()] }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

pub fn verify(exe: &Path, root: &Path) -> AppResult<()> {
    let cancel = AtomicBool::new(false);
    let mut worker = Worker::start(exe, &cancel)?;
    let bytes = fixture("CompanyAI PDF text W40", 0);
    std::fs::write(root.join("source.pdf"), &bytes).map_err(|e| e.to_string())?;
    println!("PDF basic fixture...");
    assert!(worker
        .extract_pdf(&bytes, &cancel)?
        .contains("CompanyAI PDF text W40"));
    assert!(worker
        .extract_pdf(&fixture("", 0), &cancel)
        .unwrap_err()
        .contains("OCR"));
    assert!(worker.extract_pdf(b"damaged pdf", &cancel).is_err());
    // 超過管線容量仍須完整傳送；不能死鎖或把尾端資料當成下一個操作。
    println!("PDF large transfer fixture...");
    let large = fixture("CompanyAI PDF text W40", 2_000_000);
    assert!(worker.extract_pdf(&large, &cancel)?.contains("W40"));
    std::fs::write(root.join("source.pdf"), &bytes).map_err(|e| e.to_string())?;
    let mut broker = Broker::new(
        Project {
            id: "pdf-test".into(),
            name: "PDF".into(),
            root: root.into(),
            imports: BTreeMap::new(),
        },
        "pdf-task".into(),
    )?;
    let mut call = |id: &str, tool: Tool| -> AppResult<serde_json::Value> {
        let result = broker.execute(id, &tool, &mut worker, &cancel)?;
        if result["ok"] != true {
            return Err(result.to_string());
        }
        Ok(result["result"].clone())
    };
    let read = call(
        "read",
        Tool::ReadFile {
            path: "source.pdf".into(),
            offset: 0,
        },
    )?;
    assert!(read["text"].as_str().unwrap().contains("[第 1 頁]"));
    assert!(call(
        "bad-copy",
        Tool::CreateWorkingCopy {
            source: Some("source.pdf".into()),
            name: "copy.pdf".into()
        }
    )
    .is_err());
    let copy = call(
        "copy",
        Tool::CreateWorkingCopy {
            source: Some("source.pdf".into()),
            name: "PDF文字.txt".into(),
        },
    )?;
    let id = copy["copy_id"].as_str().unwrap().to_owned();
    let saved = call(
        "save",
        Tool::SaveCopy {
            copy_id: id.clone(),
            revision: copy["revision"].as_str().unwrap().into(),
        },
    )?;
    assert!(saved["path"].as_str().unwrap().ends_with("PDF文字.txt"));
    assert_eq!(broker.finish(&[id])?.len(), 1);
    assert_eq!(
        std::fs::read(root.join("source.pdf")).map_err(|e| e.to_string())?,
        bytes
    );
    println!("PASS: PDF/AppContainer extraction, page label, blank/corrupt rejection, 2 MB transfer, TXT output and source preservation.");
    Ok(())
}
