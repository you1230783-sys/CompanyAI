//! 用記憶體 IDispatch 模擬空 Exchange 收件匣及 PST 分類樹。
//! 此測試走實際的 COM 參數、遞迴與篩選流程，但不能代替 Outlook 2024 實機驗收。
use super::*;
use std::{cell::RefCell, rc::Rc};
use windows::core::{implement, Error, Result as ComResult};
use windows::Win32::Foundation::{DISP_E_MEMBERNOTFOUND, E_NOTIMPL};

type Handler = Box<dyn Fn(&[VARIANT]) -> ComResult<VARIANT>>;

#[test]
fn table_dates_page_in_hundreds_and_fallback_stops_before_start() {
    use crate::outlook::table;
    let start = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
    let base = start
        .signed_duration_since(NaiveDate::from_ymd_opt(1899, 12, 30).unwrap())
        .num_days() as f64;
    // 每日十封，跨整月與上下界，必須經過多次GetArray而不是只看前100封。
    let mails: Vec<_> = (-10..40)
        .flat_map(|day| {
            (0..10).map(move |i| {
                dispatch(vec![
                    fixed("EntryID", format!("{day}-{i}").as_str()),
                    fixed("MessageClass", "IPM.Note"),
                    fixed("Subject", format!("主題{day}-{i}").as_str()),
                    fixed("SentOn", base + day as f64 + i as f64 / 86400.0),
                    fixed("LastModificationTime", base + 50.0),
                ])
            })
        })
        .collect();
    let ns = dispatch(vec![]); // 沒有GetItemFromID或地址簿；短標題必須全部走批次。
    for fallback in [false, true] {
        let values = mails.clone();
        let target = dispatch(vec![
            fixed("StoreID", "s"),
            (
                "GetTable",
                Box::new(move |args| {
                    if fallback && !args.is_empty() {
                        return Err(Error::from_hresult(E_NOTIMPL));
                    }
                    Ok(VARIANT::from(table_fixture(
                        values.clone(),
                        args.first().map(argument_text),
                    )))
                }),
            ),
        ]);
        let rows = table::scan(
            &ns,
            &target,
            "SentOn",
            Some((start, end)),
            false,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(rows.len(), 300);
        assert!(rows
            .iter()
            .all(|r| r.sent.date() >= start && r.sent.date() <= end));
        assert_eq!(rows[0].subject, "主題29-9");
        assert_eq!(rows.last().unwrap().subject, "主題0-0");
    }
}

#[test]
fn hidden_index_revalidates_same_count_changes_and_sent_day_bounds() {
    use crate::outlook::{
        exclusions::Exclusions,
        privacy::{self, Policy},
    };
    let subject = Rc::new(RefCell::new("before".to_string()));
    let changing = subject.clone();
    let item = dispatch(vec![
        (
            "Subject",
            Box::new(move |_| Ok(changing.borrow().as_str().into())),
        ),
        fixed("SentOn", 46000.0),
        fixed("EntryID", "same-id"),
        fixed("LastModificationTime", 46001.0),
    ]);
    let root = folder(
        "s",
        "root",
        vec![folder("s", "hidden", vec![], vec![item], 0)],
        vec![],
        0,
    );
    let app = app(
        root.clone(),
        root.clone(),
        vec![dispatch(vec![fixed("GetRootFolder", root)])],
    );
    let ns = object(&get(&app, "GetNamespace", &mut ["MAPI".into()]).unwrap()).unwrap();
    let policy = Policy {
        configured: true,
        allowed: [privacy::key("s", "root")].into_iter().collect(),
    };
    let day = crate::outlook::project::variant_time(&46000.0.into())
        .unwrap()
        .date();
    let candidate = |name: &str| dispatch(vec![fixed("Subject", name), fixed("SentOn", 46000.0)]);
    let load =
        |range| Exclusions::from_range(&ns, &policy, Some(range), &AtomicBool::new(false)).unwrap();
    assert!(load((day, day)).contains(&candidate("before")).unwrap());
    *subject.borrow_mut() = "after".into();
    let changed = load((day, day));
    assert!(!changed.contains(&candidate("before")).unwrap());
    assert!(changed.contains(&candidate("after")).unwrap());
    assert!(!load((day.succ_opt().unwrap(), day.succ_opt().unwrap()))
        .contains(&candidate("after"))
        .unwrap());
}

#[test]
fn hidden_duplicates_are_removed_across_stores_before_body_access() {
    use crate::outlook::{
        exclusions::Exclusions,
        privacy::{self, Policy},
    };
    // 無Message-ID／PropertyAccessor／Body；Exchange原地址可用，不要求@。
    let identified = |number: usize, copy: bool| {
        dispatch(vec![
            fixed("Class", 43i32),
            fixed(
                "EntryID",
                format!("{}-{number}", if copy { "copy" } else { "original" }).as_str(),
            ),
            fixed("Subject", format!("主旨-{number}").as_str()),
            fixed("SentOn", 46000.0 + number as f64 / 86400.0),
            fixed("SenderEmailAddress", "/o=Company/ou=Exchange/cn=Sender"),
            fixed(
                "Recipients",
                collection(vec![dispatch(vec![
                    fixed("Type", 1i32),
                    fixed("Address", "/o=Company/cn=Recipient"),
                ])]),
            ),
        ])
    };
    let hidden: Vec<_> = (0..10).map(|i| identified(i, true)).collect();
    let visible: Vec<_> = (2..52).map(|i| identified(i, false)).collect();
    let a = folder("pst", "A", vec![], hidden, 0);
    let b = folder("exchange", "B", vec![], visible.clone(), 0);
    let application = app(
        b.clone(),
        b.clone(),
        vec![
            dispatch(vec![fixed(
                "GetRootFolder",
                folder("pst", "root", vec![a], vec![], 0),
            )]),
            dispatch(vec![fixed(
                "GetRootFolder",
                folder("exchange", "root", vec![b], vec![], 0),
            )]),
        ],
    );
    let policy = Policy {
        configured: true,
        allowed: [
            privacy::key("pst", "root"),
            privacy::key("exchange", "root"),
            privacy::key("exchange", "B"),
        ]
        .into_iter()
        .collect(),
    };
    let index = Exclusions::from_app(&application, &policy, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        visible
            .iter()
            .filter(|item| !index.contains(item).unwrap())
            .count(),
        42
    );
    assert!(index.require(&visible[0]).is_err());
    assert!(index.require(&visible[49]).is_ok());
    // 同一份隱藏清單分別套用到寄件備份及收件匣，不互相扣除或耗用。
    for (total, remaining) in [(80, 70), (120, 110)] {
        assert_eq!(
            (0..total)
                .filter(|i| !index.contains(&identified(*i, false)).unwrap())
                .count(),
            remaining
        );
    }
    assert!(Exclusions::from_app(&application, &policy, &AtomicBool::new(true)).is_err());
    // 子資料夾即使保留舊勾選，未勾選祖先仍要排除其郵件。
    let leaf = folder("s", "leaf", vec![], vec![identified(99, true)], 0);
    let root = folder(
        "s",
        "root",
        vec![folder("s", "parent", vec![leaf], vec![], 0)],
        vec![],
        0,
    );
    let application = app(
        root.clone(),
        root.clone(),
        vec![dispatch(vec![fixed("GetRootFolder", root)])],
    );
    let policy = Policy {
        configured: true,
        allowed: [privacy::key("s", "root"), privacy::key("s", "leaf")]
            .into_iter()
            .collect(),
    };
    assert!(
        Exclusions::from_app(&application, &policy, &AtomicBool::new(false))
            .unwrap()
            .contains(&identified(99, false))
            .unwrap()
    );
    assert!(
        !Exclusions::from_app(&application, &Policy::default(), &AtomicBool::new(false))
            .unwrap()
            .contains(&identified(99, false))
            .unwrap()
    );
}

#[test]
fn subject_and_sent_time_need_no_sender_or_recipient_properties() {
    use crate::outlook::{
        exclusions::Exclusions,
        privacy::{self, Policy},
    };
    let partial = dispatch(vec![
        fixed("Class", 43i32),
        fixed("Subject", "隱藏主題"),
        fixed("SentOn", 46000.0),
    ]);
    let index_for = |item: IDispatch| {
        let root = folder(
            "s",
            "root",
            vec![folder("s", "hidden", vec![], vec![item], 0)],
            vec![],
            0,
        );
        let application = app(
            root.clone(),
            root.clone(),
            vec![dispatch(vec![fixed("GetRootFolder", root)])],
        );
        let policy = Policy {
            configured: true,
            allowed: [privacy::key("s", "root")].into_iter().collect(),
        };
        Exclusions::from_app(&application, &policy, &AtomicBool::new(false))
    };
    let index = index_for(partial).unwrap();
    assert!(index
        .contains(&dispatch(vec![
            fixed("Subject", "隱藏主題"),
            fixed("SentOn", 46000.0)
        ]))
        .unwrap());
    for i in 0..31 {
        assert!(!index
            .contains(&dispatch(vec![
                fixed("Subject", format!("其他主題{i}").as_str()),
                fixed("SentOn", 46000.0)
            ]))
            .unwrap());
    }
    // 缺少必要識別欄位必須失敗，不能把未知當作沒有信件。
    assert!(index_for(dispatch(vec![fixed("Class", 43i32)])).is_err());
}

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

/// 真正建立二維SAFEARRAY，測試COM批次欄列方向、日期過濾與排序。
fn table_fixture(mut rows: Vec<IDispatch>, filter: Option<String>) -> IDispatch {
    if let Some(filter) = filter {
        let field = filter.split(']').next().unwrap().trim_start_matches('[');
        let dates: Vec<_> = filter.split('\'').collect();
        let start = crate::outlook::project::variant_time(&VARIANT::from(dates[1])).unwrap();
        let end = crate::outlook::project::variant_time(&VARIANT::from(dates[3])).unwrap();
        rows.retain(|r| {
            get(r, field, &mut [])
                .ok()
                .and_then(|v| crate::outlook::project::variant_time(&v).ok())
                .is_none_or(|d| d >= start && d < end)
        });
    }
    let rows = Rc::new(RefCell::new(rows));
    let sorted = rows.clone();
    let counted = rows.clone();
    let position = Rc::new(RefCell::new(0usize));
    let selected = Rc::new(RefCell::new(Vec::<String>::new()));
    let added = selected.clone();
    dispatch(vec![
        fixed(
            "Columns",
            dispatch(vec![
                fixed("RemoveAll", VARIANT::default()),
                (
                    "Add",
                    Box::new(move |args| {
                        added.borrow_mut().push(argument_text(&args[0]));
                        Ok(VARIANT::default())
                    }),
                ),
            ]),
        ),
        (
            "GetRowCount",
            Box::new(move |_| Ok((counted.borrow().len() as i32).into())),
        ),
        (
            "Sort",
            Box::new(move |args| {
                assert!(bool::try_from(&args[0])?);
                let field = argument_text(&args[1]);
                sorted.borrow_mut().sort_by_key(|r| {
                    std::cmp::Reverse(
                        get(r, &field, &mut [])
                            .ok()
                            .and_then(|v| crate::outlook::project::variant_time(&v).ok()),
                    )
                });
                Ok(VARIANT::default())
            }),
        ),
        (
            "GetArray",
            Box::new(move |args| {
                use windows::Win32::System::Ole::{SafeArrayCreate, SafeArrayPutElement};
                assert_eq!(i32::try_from(&args[0])?, 100);
                let rows = rows.borrow();
                let start = *position.borrow();
                let end = (start + 100).min(rows.len());
                if start == end {
                    return Ok(VARIANT::default());
                }
                let selected = selected.borrow();
                let bounds = [
                    SAFEARRAYBOUND {
                        cElements: selected.len() as u32,
                        lLbound: 0,
                    },
                    SAFEARRAYBOUND {
                        cElements: (end - start) as u32,
                        lLbound: 0,
                    },
                ];
                let array = unsafe { SafeArrayCreate(VT_VARIANT, 2, bounds.as_ptr()) };
                assert!(!array.is_null());
                let mut value = VARIANT::default();
                unsafe {
                    (*value.Anonymous.Anonymous).vt = VARENUM(VT_ARRAY.0 | VT_VARIANT.0);
                    (*value.Anonymous.Anonymous).Anonymous.parray = array;
                }
                for (j, row) in rows[start..end].iter().enumerate() {
                    for (i, name) in selected.iter().enumerate() {
                        let cell =
                            get(row, name, &mut []).unwrap_or_else(|_| match name.as_str() {
                                "MessageClass" => "IPM.Note".into(),
                                "EntryID" => "fixture".into(),
                                "LastModificationTime" => {
                                    get(row, "SentOn", &mut []).unwrap_or_default()
                                }
                                _ => VARIANT::default(),
                            });
                        unsafe {
                            SafeArrayPutElement(
                                array,
                                [i as i32, j as i32].as_ptr(),
                                (&cell as *const VARIANT).cast(),
                            )
                        }?;
                    }
                }
                *position.borrow_mut() = end;
                Ok(value)
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
        fixed("Name", id),
        fixed("Folders", collection(children)),
        fixed("Items", collection(mails.clone())),
        (
            "GetTable",
            Box::new(move |args| {
                Ok(VARIANT::from(table_fixture(
                    mails.clone(),
                    args.first().map(argument_text),
                )))
            }),
        ),
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
fn privacy_checks_real_dispatch_ancestry_before_body_or_export() {
    use crate::outlook::privacy::{self, Policy};
    let namespace = dispatch(vec![fixed("Class", 1i32)]);
    let root = dispatch(vec![
        fixed("Class", 2i32),
        fixed("StoreID", "s"),
        fixed("EntryID", "root"),
        fixed("Parent", namespace),
    ]);
    let blocked = dispatch(vec![
        fixed("Class", 2i32),
        fixed("StoreID", "s"),
        fixed("EntryID", "hidden"),
        fixed("Parent", root.clone()),
    ]);
    let leaf = dispatch(vec![
        fixed("Class", 2i32),
        fixed("StoreID", "s"),
        fixed("EntryID", "leaf"),
        fixed("Parent", blocked),
    ]);
    let policy = Policy {
        configured: true,
        allowed: [privacy::key("s", "root"), privacy::key("s", "leaf")]
            .into_iter()
            .collect(),
    };
    assert!(privacy::folder_allowed(&policy, &root).unwrap());
    // 即使 leaf ID 曾經被允許，移到未勾選祖先下仍須拒絕。
    assert!(!privacy::folder_allowed(&policy, &leaf).unwrap());
    let mail = dispatch(vec![fixed("Parent", leaf)]);
    assert!(privacy::require_item(&policy, &mail).is_err());
    // Fixture 不提供 Body／SaveAs，確認權限檢查完全沒有先讀信或匯出。
}

#[test]
fn local_catalog_lists_names_without_mail_access_and_remembers_selection() {
    use crate::outlook::privacy::{self, Policy};
    let child = dispatch(vec![
        fixed("StoreID", "s"),
        fixed("EntryID", "private"),
        fixed("Name", "私人 <img>"),
        fixed("Folders", collection(vec![])),
    ]);
    let root = dispatch(vec![
        fixed("StoreID", "s"),
        fixed("EntryID", "root"),
        fixed("Name", "信箱"),
        fixed("Folders", collection(vec![child])),
    ]);
    let application = dispatch(vec![fixed(
        "GetNamespace",
        dispatch(vec![fixed(
            "Stores",
            collection(vec![dispatch(vec![fixed("GetRootFolder", root)])]),
        )]),
    )]);
    let cancel = AtomicBool::new(false);
    let initial = privacy::catalog_from_app(&Policy::default(), &cancel, &application).unwrap();
    assert_eq!(initial.len(), 2);
    assert!(initial.iter().all(|c| c.selected));
    assert_eq!(initial[1].parent.as_deref(), Some(initial[0].id.as_str()));
    let policy = Policy::from_selection(&initial, &[initial[0].id.clone()]).unwrap();
    let next = privacy::catalog_from_app(&policy, &cancel, &application).unwrap();
    assert!(next[0].selected);
    assert!(!next[1].selected);
    let encoded = serde_json::to_string(&next).unwrap();
    assert!(!encoded.contains("StoreID"));
    assert!(!encoded.contains("EntryID"));
    assert!(privacy::catalog_from_app(&policy, &AtomicBool::new(true), &application).is_err());
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

#[test]
fn project_online_inbox_uses_inbox_then_lists_children_without_reading_mail() {
    let child = folder("exchange", "rules", vec![], vec![], 1);
    let inbox = folder("exchange", "inbox", vec![child], vec![], 1);
    let sent = folder("exchange", "sent", vec![], vec![], 1);
    let selected = inbox.clone();
    let store = dispatch(vec![
        fixed("ExchangeStoreType", 0i32),
        (
            "GetDefaultFolder",
            Box::new(move |args| match i32::try_from(&args[0])? {
                6 => Ok(VARIANT::from(selected.clone())),
                5 => Ok(VARIANT::from(sent.clone())),
                _ => Err(Error::from_hresult(E_NOTIMPL)),
            }),
        ),
    ]);
    let app = dispatch(vec![fixed(
        "GetNamespace",
        dispatch(vec![
            fixed("Stores", collection(vec![store])),
            fixed("GetFolderFromID", inbox),
        ]),
    )]);
    let cancel = AtomicBool::new(false);
    let (folders, _) =
        crate::outlook::project::folders_from_app(&app, "online_inbox", None, &cancel).unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].name, "inbox");
    assert_eq!(folders[0].scope, "online_inbox");
    let (children, _) =
        crate::outlook::project::folders_from_app(&app, "online_inbox", Some(&folders[0]), &cancel)
            .unwrap();
    assert_eq!(children[0].name, "rules");
    let (sent, _) =
        crate::outlook::project::folders_from_app(&app, "online_sent", None, &cancel).unwrap();
    assert_eq!(sent[0].name, "sent");
}
