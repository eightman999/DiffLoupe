//! キャンセルトークン。外部(UI/CLI)から比較処理を中断させる。

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default)]
pub struct CancellationToken {
    cancelled: AtomicBool,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cancelled")
    }
}
impl std::error::Error for Cancelled {}

/// 比較のエントリ数上限超過
#[derive(Debug, Clone)]
pub struct EntryLimitExceeded {
    pub limit: usize,
    pub observed: usize,
}
impl std::fmt::Display for EntryLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Freeは{}ファイルまで比較できます。Proで制限を解除できます。",
            self.limit
        )
    }
}
impl std::error::Error for EntryLimitExceeded {}
