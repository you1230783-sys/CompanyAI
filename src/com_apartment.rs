//! COM 初始化與釋放的共同邊界。
//!
//! 每次成功的 CoInitializeEx（包含 S_FALSE）都必須在同一執行緒平衡一次。
//! 呼叫端先建立本守衛，再建立 COM 物件，讓物件依 Rust 區域變數逆序先釋放；
//! 若物件保存在結構中，仍須自行維持這個釋放順序，不可把物件帶出 apartment。
use std::{marker::PhantomData, rc::Rc};
use windows::{
    core::Result,
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::Com::{
            CoInitializeEx, CoUninitialize, COINIT, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED,
        },
    },
};

/// 只在建立它的執行緒釋放 COM；Rc 標記使守衛不能 Send／Sync。
#[must_use = "COM 守衛必須保留到相關 COM 物件全部釋放之後"]
pub(crate) struct Apartment {
    initialized: bool,
    _thread: PhantomData<Rc<()>>,
}

impl Apartment {
    /// Office、Outlook 與 WebView2 使用 STA；不同 apartment 模式直接回報錯誤。
    pub(crate) fn sta() -> Result<Self> {
        Self::initialize(COINIT_APARTMENTTHREADED, false)
    }

    /// UI Automation 工作執行緒使用 MTA，不默默接受 STA。
    pub(crate) fn mta() -> Result<Self> {
        Self::initialize(COINIT_MULTITHREADED, false)
    }

    /// WIC 可在既有 STA 或新 MTA 執行；借用 STA 時不增加／釋放初始化次數。
    pub(crate) fn for_imaging() -> Result<Self> {
        Self::initialize(COINIT_MULTITHREADED, true)
    }

    fn initialize(mode: COINIT, allow_existing_sta: bool) -> Result<Self> {
        // SAFETY: 保留參數固定為 None；成功後以不能跨執行緒的守衛平衡初始化。
        let status = unsafe { CoInitializeEx(None, mode) };
        let initialized = status.is_ok();
        if !(initialized || allow_existing_sta && status == RPC_E_CHANGED_MODE) {
            status.ok()?;
        }
        Ok(Self {
            initialized,
            _thread: PhantomData,
        })
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: 只有本次初始化成功才呼叫，且 !Send／!Sync 保證仍在原執行緒。
            // COM 物件必須由呼叫端先釋放，詳見模組的生命週期約定。
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{CoGetApartmentType, APTTYPE, APTTYPEQUALIFIER, APTTYPE_MTA};

    fn current() -> Result<APTTYPE> {
        let mut kind = APTTYPE::default();
        let mut qualifier = APTTYPEQUALIFIER::default();
        // SAFETY: 輸出位置有效；只讀取本測試執行緒的 COM 狀態。
        unsafe { CoGetApartmentType(&mut kind, &mut qualifier)? };
        Ok(kind)
    }

    fn nested_initialization_balances_s_false() {
        std::thread::spawn(|| {
            assert!(current().is_err());
            let first = Apartment::sta().unwrap();
            let kind = current().unwrap();
            let second = Apartment::sta().unwrap();
            drop(second);
            assert_eq!(current().unwrap(), kind);
            drop(first);
            assert!(current().is_err());
        })
        .join()
        .unwrap();
    }

    fn rejected_mode_does_not_uninitialize_owner() {
        std::thread::spawn(|| {
            let owner = Apartment::sta().unwrap();
            let kind = current().unwrap();
            assert_eq!(Apartment::mta().err().unwrap().code(), RPC_E_CHANGED_MODE);
            assert_eq!(current().unwrap(), kind);
            drop(owner);
            assert!(current().is_err());
        })
        .join()
        .unwrap();
    }

    fn imaging_borrows_sta_without_releasing_it() {
        std::thread::spawn(|| {
            let owner = Apartment::sta().unwrap();
            let kind = current().unwrap();
            let image = Apartment::for_imaging().unwrap();
            assert!(!image.initialized);
            drop(image);
            assert_eq!(current().unwrap(), kind);
            drop(owner);
            assert!(current().is_err());
        })
        .join()
        .unwrap();
    }

    fn imaging_owns_new_mta_and_balances_existing_mta() {
        std::thread::spawn(|| {
            let owner = Apartment::for_imaging().unwrap();
            assert!(owner.initialized);
            assert_eq!(current().unwrap(), APTTYPE_MTA);
            let image = Apartment::for_imaging().unwrap();
            assert!(image.initialized);
            drop(image);
            assert_eq!(current().unwrap(), APTTYPE_MTA);
            drop(owner);
            assert!(current().is_err());
        })
        .join()
        .unwrap();
    }

    fn unwind_balances_initialization() {
        std::thread::spawn(|| {
            assert!(std::panic::catch_unwind(|| {
                let _apartment = Apartment::sta().unwrap();
                panic!("synthetic COM lifetime test");
            })
            .is_err());
            assert!(current().is_err());
        })
        .join()
        .unwrap();
    }

    #[test]
    fn real_apartment_lifecycle_in_isolated_process() {
        const CHILD: &str = "COMPANY_AI_COM_LIFETIME_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            // 其他測試若仍有 MTA，COM 會為新執行緒提供 implicit MTA；獨立程序
            // 才能確定「最後一次釋放後未初始化」的斷言不受測試並行干擾。
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "com_apartment::tests::real_apartment_lifecycle_in_isolated_process",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        nested_initialization_balances_s_false();
        rejected_mode_does_not_uninitialize_owner();
        imaging_borrows_sta_without_releasing_it();
        imaging_owns_new_mta_and_balances_existing_mta();
        unwind_balances_initialization();
    }
}
