use diffloupe_core::cancel::CancellationToken;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(unix)]
extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
}

// Ctrl+C / Ctrl+Break(Windows) で比較をキャンセルするためのフラグ
static SIGNAL_HIT: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
const SIGINT: i32 = 2;
#[cfg(unix)]
const SIGTERM: i32 = 15;

#[cfg(unix)]
extern "C" fn on_signal(_sig: i32) {
    SIGNAL_HIT.store(true, Ordering::SeqCst);
}

#[cfg(windows)]
extern "system" {
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(ctrl_type: u32) -> i32>,
        add: i32,
    ) -> i32;
}

#[cfg(windows)]
unsafe extern "system" fn on_console_ctrl(_ctrl_type: u32) -> i32 {
    SIGNAL_HIT.store(true, Ordering::SeqCst);
    1
}

fn install_cancel_handler(token: &Arc<CancellationToken>) {
    #[cfg(unix)]
    unsafe {
        signal(SIGINT, on_signal as *const () as usize);
        signal(SIGTERM, on_signal as *const () as usize);
    }
    #[cfg(windows)]
    unsafe {
        SetConsoleCtrlHandler(Some(on_console_ctrl), 1);
    }
    let t = token.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(25));
        if SIGNAL_HIT.load(Ordering::SeqCst) {
            t.cancel();
        }
    });
}

fn main() {
    let token = Arc::new(CancellationToken::new());
    install_cancel_handler(&token);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = diffloupe_cli::run(&args, &token);
    std::process::exit(code);
}
