//! ウィンドウプロシージャ(ユーザーコールバック)の保護と診断。
//!
//! - 状態ポインタ(GWLP_USERDATA)が未設定の間に届くメッセージは `DefWindowProcW` へ流す。
//! - 本体を `catch_unwind` で包み、panic が FFI 境界を越えて即死するのを防ぐ
//!   (release は `panic = "unwind"` が前提)。panic したら `DefWindowProcW` の結果を返す。
//! - スモークモードでは、直近のメッセージをリングバッファに記録し、panic / SEH 例外
//!   (アクセス違反など)の直前に何が起きていたかを `smoke.log` に出す。
//!   SEH 例外は `catch_unwind` では捕まえられないため、ベクタ化例外ハンドラでログだけ取る。

use std::collections::VecDeque;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Diagnostics::Debug::{AddVectoredExceptionHandler, EXCEPTION_POINTERS};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GetWindowLongPtrW, GWLP_USERDATA, WM_CREATE,
};

const RING_CAP: usize = 64;

static ENABLED: AtomicBool = AtomicBool::new(false);
static EXC_LOGGED: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
struct Rec {
    proc_name: &'static str,
    hwnd: usize,
    msg: u32,
    wparam: usize,
    lparam: isize,
    depth: u32,
}

static RING: Mutex<VecDeque<Rec>> = Mutex::new(VecDeque::new());
static DEPTH: AtomicU32 = AtomicU32::new(0);

/// スモークモードの診断(リングバッファ記録と例外ハンドラ)を有効にする。
pub fn enable_diagnostics() {
    ENABLED.store(true, Ordering::SeqCst);
    unsafe {
        AddVectoredExceptionHandler(0, Some(veh));
    }
}

fn record(r: Rec) {
    if let Ok(mut q) = RING.lock() {
        if q.len() >= RING_CAP {
            q.pop_front();
        }
        q.push_back(r);
    }
}

/// 直近のメッセージ履歴を smoke.log へ出す。
pub fn dump_ring(reason: &str) {
    let recs: Vec<Rec> = match RING.lock() {
        Ok(q) => q.iter().copied().collect(),
        Err(_) => return,
    };
    super::smoke::log(&format!(
        "--- 直近のメッセージ {} 件 ({}) ---",
        recs.len(),
        reason
    ));
    for r in recs {
        super::smoke::log(&format!(
            "  {}{} hwnd=0x{:X} msg=0x{:04X} w=0x{:X} l=0x{:X}",
            "  ".repeat(r.depth as usize),
            r.proc_name,
            r.hwnd,
            r.msg,
            r.wparam,
            r.lparam
        ));
    }
}

unsafe extern "system" fn veh(info: *mut EXCEPTION_POINTERS) -> i32 {
    const CONTINUE_SEARCH: i32 = 0;
    if info.is_null() || (*info).ExceptionRecord.is_null() {
        return CONTINUE_SEARCH;
    }
    let rec = &*(*info).ExceptionRecord;
    let code = rec.ExceptionCode.0 as u32;
    // 重大な例外のみ(C++ 例外 0xE06D7363 などのデバッグ用通知は除外)
    if code & 0xF000_0000 != 0xC000_0000 || EXC_LOGGED.fetch_add(1, Ordering::SeqCst) >= 3 {
        return CONTINUE_SEARCH;
    }
    let mut msg = format!(
        "SEH 例外 code=0x{:08X} address={:p} params={}",
        code, rec.ExceptionAddress, rec.NumberParameters
    );
    if code == 0xC000_0005 && rec.NumberParameters >= 2 {
        let kind = match rec.ExceptionInformation[0] {
            0 => "read",
            1 => "write",
            8 => "execute",
            _ => "?",
        };
        msg.push_str(&format!(
            " アクセス違反: {} @ 0x{:X}",
            kind, rec.ExceptionInformation[1]
        ));
    }
    super::smoke::log(&msg);
    super::smoke::log(&format!(
        "backtrace:\n{}",
        std::backtrace::Backtrace::force_capture()
    ));
    dump_ring("SEH 例外");
    CONTINUE_SEARCH
}

/// ウィンドウプロシージャ本体 `f` を保護して実行する。
///
/// `state_in_create`: true なら状態ポインタを WM_CREATE で設定する窓(その場合 WM_CREATE のみ素通し)。
/// false なら、状態ポインタが設定されるまでの全メッセージを `DefWindowProcW` に流す。
pub fn run(
    proc_name: &'static str,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    state_in_create: bool,
    f: impl FnOnce() -> LRESULT,
) -> LRESULT {
    let has_state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } != 0;
    if !(has_state || state_in_create && msg == WM_CREATE) {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let diag = ENABLED.load(Ordering::Relaxed);
    if diag {
        record(Rec {
            proc_name,
            hwnd: hwnd.0 as usize,
            msg,
            wparam: wparam.0,
            lparam: lparam.0,
            depth: DEPTH.fetch_add(1, Ordering::SeqCst).min(8),
        });
    }
    let r = catch_unwind(AssertUnwindSafe(f));
    if diag {
        DEPTH.fetch_sub(1, Ordering::SeqCst);
    }
    match r {
        Ok(v) => v,
        Err(_) => {
            // panic の内容は panic hook(スモーク時は smoke.log)に出力済み
            super::smoke::log(&format!(
                "PANIC を wndproc 境界で捕捉 proc={} msg=0x{:04X}",
                proc_name, msg
            ));
            dump_ring("panic");
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
