//! Dependency-free cooperative cancellation, implemented by bc-jobs::CancelToken.

pub trait CancelCheck: Send + Sync {
    fn is_cancelled(&self) -> bool;

    fn check(&self) -> Result<(), crate::CoreError> {
        if self.is_cancelled() {
            Err(crate::CoreError::Cancelled)
        } else {
            Ok(())
        }
    }
}

impl<F: Fn() -> bool + Send + Sync> CancelCheck for F {
    fn is_cancelled(&self) -> bool {
        self()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NeverCancel;
impl CancelCheck for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}
