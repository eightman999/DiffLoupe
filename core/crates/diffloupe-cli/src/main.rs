mod cli;

use diffloupe_core::cancel::CancellationToken;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(unix)]
extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
}

static SIGNAL_HIT: AtomicBool = AtomicBool::new(false);
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;

extern "C" fn on_signal(_sig: i32) {
    SIGNAL_HIT.store(true, Ordering::SeqCst);
}

fn main() {
    let token = Arc::new(CancellationToken::new());
    #[cfg(unix)]
    unsafe {
        signal(SIGINT, on_signal as usize);
        signal(SIGTERM, on_signal as usize);
        let t = token.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(25));
            if SIGNAL_HIT.load(Ordering::SeqCst) {
                t.cancel();
            }
        });
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = cli::run(&args, &token);
    std::process::exit(code);
}
