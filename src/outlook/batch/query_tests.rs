//! 用記憶體 IDispatch 模擬空 Exchange 收件匣及 PST 分類樹。
//! 此測試走實際的 COM 參數、遞迴與篩選流程，但不能代替 Outlook 2024 實機驗收。
use super::*;
use std::{cell::RefCell, rc::Rc};
use windows::core::{implement, Error, Result as ComResult};
use windows::Win32::Foundation::{DISP_E_MEMBERNOTFOUND, E_NOTIMPL};

type Handler = Box<dyn Fn(&[VARIANT]) -> ComResult<VARIANT>>;

#[implement(IDispatch)]
struct FakeDispatch {
    members: Vec<(&'static str, Handler)>,
}

#[allow(non_snake_case)]
impl IDispatch_Impl for FakeDispatch_Impl {
    fn GetTypeInfoCount(&self) -> ComResult<u32> {
        Ok(0)
    }
    fn GetTypeInfo(&self, _: u32, _: u32) -> ComResult<ITypeInfo> {
        Err(Error::from_hresult(E_NOTIMPL))
    }
    fn GetIDsOfNames(
        &self,
        _: *const GUID,
        names: *const PCWSTR,
        count: u32,
        _: u32,
        id: *mut i32,
    ) -> ComResult<()> {
        assert_eq!(count, 1);
        let name = unsafe { (*names).to_string()? };
        let index = self
            .members
            .iter()
            .position(|(member, _)| *member == name)
            .ok_or_else(|| Error::from_hresult(DISP_E_MEMBERNOTFOUND))?;
        unsafe {
            *id = index as i32;
        }
        Ok(())
    }
    fn Invoke(
        &self,
        id: i32,
        _: *const GUID,
        _: u32,
        flags: DISPATCH_FLAGS,
        params: *const DISPPARAMS,
        result: *mut VARIANT,
        _: *mut EXCEPINFO,
        _: *mut u32,
    ) -> ComResult<()> {
        assert_eq!(flags, DISPATCH_PROPERTYGET | DISPATCH_METHOD);
        let params = unsafe { &*params };
        let arguments = if params.cArgs == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(params.rgvarg, params.cArgs as usize) }
        };
        let value = (self.members[id as usize].1)(arguments)?;
        if !result.is_null() {
            unsafe {
                *result = value;
            }
        }
        Ok(())
    }
}

fn fixed(name: &'static str, value: impl Into<VARIANT>) -> (&'static str, Handler) {
    let value = value.into();
    (name, Box::new(move |_| Ok(value.clone())))
}

fn dispatch(members: Vec<(&'static str, Handler)>) -> IDispatch {
    FakeDispatch { members }.into()
}

fn argument_text(value: &VARIANT) -> String {
    // 先驗證型別，再解參考 union，避免測試本身誤讀其他 VARIANT 型別。
    assert_eq!(unsafe { value.Anonymous.Anonymous.vt }, VT_BSTR);
    unsafe { value.Anonymous.Anonymous.Anonymous.bstrVal.to_string() }
}

/// 模擬 Outlook 集合的排序及未讀篩選，驗證反向參數與查詢結果，而非固定回傳測試答案。
fn collection(values: Vec<IDispatch>) -> IDispatch {
    let values = Rc::new(RefCell::new(values));
    let counted = values.clone();
    let indexed = values.clone();
    let sorted = values.clone();
    dispatch(vec![
        (
            "Count",
            Box::new(move |_| Ok(VARIANT::from(counted.borrow().len() as i32))),
        ),
        (
            "Item",
            Box::new(move |args| {
                Ok(VARIANT::from(
                    indexed.borrow()[i32::try_from(&args[0])? as usize - 1].clone(),
                ))
            }),
        ),
        (
            "Sort",
            Box::new(move |args| {
                assert!(bool::try_from(&args[0])?);
                assert_eq!(argument_text(&args[1]), "[ReceivedTime]");
                sorted.borrow_mut().sort_by(|a, b| {
                    received_time(b)
                        .unwrap()
                        .total_cmp(&received_time(a).unwrap())
                });
                Ok(VARIANT::default())
            }),
        ),
        (
            "Restrict",
            Box::new(move |args| {
                assert_eq!(argument_text(&args[0]), "[UnRead] = True");
                let mails = values
                    .borrow()
                    .iter()
                    .filter(|mail| bool::try_from(&get(mail, "UnRead", &mut []).unwrap()).unwrap())
                    .cloned()
                    .collect();
                Ok(VARIANT::from(collection(mails)))
            }),
        ),
    ])
}

fn folder(
    store: &str,
    id: &str,
    children: Vec<IDispatch>,
    mails: Vec<IDispatch>,
    kind: i32,
) -> IDispatch {
    dispatch(vec![
        fixed("StoreID", store),
        fixed("EntryID", id),
        fixed("FolderPath", format!("{store}/{id}").as_str()),
        fixed("Folders", collection(children)),
        fixed("Items", collection(mails)),
        fixed("DefaultItemType", 0i32),
        fixed(
            "PropertyAccessor",
            dispatch(vec![fixed("GetProperty", kind)]),
        ),
    ])
}

fn mail(store: &str, id: &str, received: f64, unread: bool) -> IDispatch {
    let parent = dispatch(vec![
        fixed("StoreID", store),
        fixed("FolderPath", "本機資料檔/分類"),
    ]);
    let mut date = VARIANT::default();
    unsafe {
        VariantChangeType(
            &mut date,
            &VARIANT::from(received),
            VAR_CHANGE_FLAGS(0),
            VT_DATE,
        )
        .unwrap();
    }
    dispatch(vec![
        fixed("Class", 43i32),
        fixed("EntryID", id),
        fixed("ReceivedTime", date),
        fixed("UnRead", unread),
        fixed("Parent", parent),
        fixed("Subject", id),
        fixed("SenderName", "測試寄件者"),
        fixed("To", "測試收件者"),
        fixed("CC", ""),
        // 沒有 Body／SaveAs 成員；預覽如果誤讀正文或匯出，測試就會失敗。
    ])
}

fn app(inbox: IDispatch, current: IDispatch, stores: Vec<IDispatch>) -> IDispatch {
    dispatch(vec![
        fixed(
            "GetNamespace",
            dispatch(vec![
                fixed("GetDefaultFolder", inbox),
                fixed("Stores", collection(stores)),
            ]),
        ),
        fixed(
            "ActiveExplorer",
            dispatch(vec![fixed("CurrentFolder", current)]),
        ),
    ])
}

#[test]
fn date_and_unread_presets_find_moved_pst_mail_but_skip_system_and_search_folders() {
    let base = today()
        .unwrap()
        .signed_duration_since(NaiveDate::from_ymd_opt(1899, 12, 30).unwrap())
        .num_days() as f64;
    let inbox = folder("exchange", "inbox", vec![], vec![], 1);
    let classified = folder(
        "pst",
        "classified",
        vec![],
        vec![
            mail("pst", "old", base - 40.0, true),
            mail("pst", "unread", base + 0.5, true),
            mail("pst", "read", base + 0.7, false),
            mail("pst", "future", base + 2.0, true),
        ],
        1,
    );
    let deleted = folder(
        "pst",
        "deleted",
        vec![],
        vec![mail("pst", "deleted-mail", base + 0.9, true)],
        1,
    );
    let virtual_folder = folder(
        "pst",
        "search",
        vec![],
        vec![mail("pst", "virtual-mail", base + 0.9, true)],
        2,
    );
    let root = folder(
        "pst",
        "root",
        vec![classified.clone(), deleted.clone(), virtual_folder],
        vec![],
        0,
    );
    let stores = vec![
        dispatch(vec![
            fixed("GetRootFolder", inbox.clone()),
            fixed("GetDefaultFolder", VARIANT::default()),
        ]),
        dispatch(vec![
            fixed("GetRootFolder", root),
            fixed("GetDefaultFolder", deleted),
        ]),
    ];
    let app = app(inbox, classified, stores);
    let cancel = AtomicBool::new(false);
    for period in ["today", "three_days", "week"] {
        for unread in [false, true] {
            let result = list(&app, period, unread, SearchScope::AllStores, &cancel).unwrap();
            let ids: Vec<_> = result
                .mails
                .iter()
                .map(|mail| mail.preview.entry_id.as_str())
                .collect();
            assert_eq!(
                ids,
                if unread {
                    vec!["unread"]
                } else {
                    vec!["read", "unread"]
                }
            );
            assert!(!result.truncated, "{}", result.notice);
            assert!(result.mails.iter().all(|mail| mail.preview.body.is_none()));
        }
    }
    assert!(list(&app, "today", false, SearchScope::Inbox, &cancel)
        .unwrap()
        .mails
        .is_empty());
    assert_eq!(
        list(&app, "today", false, SearchScope::CurrentFolder, &cancel)
            .unwrap()
            .mails
            .len(),
        2
    );
    cancel.store(true, Ordering::Relaxed);
    assert!(list(&app, "today", false, SearchScope::AllStores, &cancel).is_err());
}

#[test]
fn failed_folder_is_reported_without_losing_other_folders_and_folder_budget_is_bounded() {
    let bad = dispatch(vec![fixed("StoreID", "pst"), fixed("EntryID", "broken")]);
    let children = (0..510)
        .map(|i| folder("pst", &i.to_string(), vec![], vec![], 1))
        .collect();
    let root = folder("pst", "root", children, vec![], 0);
    let empty = folder("exchange", "inbox", vec![], vec![], 1);
    let app = app(
        empty.clone(),
        empty,
        vec![
            dispatch(vec![
                fixed("GetRootFolder", bad),
                fixed("GetDefaultFolder", VARIANT::default()),
            ]),
            dispatch(vec![
                fixed("GetRootFolder", root),
                fixed("GetDefaultFolder", VARIANT::default()),
            ]),
        ],
    );
    let result = list(
        &app,
        "today",
        false,
        SearchScope::AllStores,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(result.truncated);
    assert!(result.notice.contains("結果不完整"));
    assert!(result.notice.contains("已檢查 500 個資料夾"));
    assert!(result.notice.contains("PropertyAccessor"));
}

#[test]
fn newest_fifty_are_merged_across_stores_and_physical_copies_remain_distinct() {
    let base = today()
        .unwrap()
        .signed_duration_since(NaiveDate::from_ymd_opt(1899, 12, 30).unwrap())
        .num_days() as f64;
    let first = folder(
        "exchange",
        "inbox",
        vec![],
        (0..60)
            .map(|i| mail("exchange", &i.to_string(), base + i as f64 / 1000.0, true))
            .collect(),
        1,
    );
    let duplicate = mail("pst", "copy", base + 0.99, true);
    let second = folder(
        "pst",
        "category",
        vec![],
        (0..60)
            .map(|i| mail("pst", &i.to_string(), base + 0.1 + i as f64 / 1000.0, true))
            .chain([duplicate.clone(), duplicate])
            .collect(),
        1,
    );
    let app = app(
        first.clone(),
        second.clone(),
        vec![
            dispatch(vec![
                fixed("GetRootFolder", first),
                fixed("GetDefaultFolder", VARIANT::default()),
            ]),
            dispatch(vec![
                fixed("GetRootFolder", second),
                fixed("GetDefaultFolder", VARIANT::default()),
            ]),
        ],
    );
    let result = list(
        &app,
        "today",
        true,
        SearchScope::AllStores,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(result.mails.len(), 50);
    assert!(result.truncated);
    assert!(result.mails.iter().all(|mail| mail.store_id == "pst"));
    assert_eq!(result.mails[0].preview.entry_id, "copy");
    assert_eq!(result.mails[49].preview.entry_id, "11");
    assert_eq!(
        result
            .mails
            .iter()
            .filter(|mail| mail.preview.entry_id == "copy")
            .count(),
        1
    );

    // 同 EntryID 在不同 Store 仍是不同實體；不能只用 EntryID 或主旨去重。
    let roots: Vec<_> = ["exchange", "pst"]
        .iter()
        .map(|store| {
            dispatch(vec![
                fixed(
                    "GetRootFolder",
                    folder(
                        store,
                        "classified",
                        vec![],
                        vec![mail(store, "same-id", base + 0.5, false)],
                        1,
                    ),
                ),
                fixed("GetDefaultFolder", VARIANT::default()),
            ])
        })
        .collect();
    let empty = folder("exchange", "empty", vec![], vec![], 1);
    let result = list(
        &self::app(empty.clone(), empty, roots),
        "today",
        false,
        SearchScope::AllStores,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(result.mails.len(), 2);
    assert!(!result.truncated);
}
