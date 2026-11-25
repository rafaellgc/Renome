//! Locks that survive a panic elsewhere.
//!
//! A panic while a `Mutex` is held "poisons" it, and `lock()` then fails for
//! every later caller: one bad file would leave the window unable to do
//! anything. The data behind Renome's locks (caches, the list shown, the
//! stored text) is always left whole between statements, so the next caller
//! may simply carry on with it.

use std::sync::{Mutex, MutexGuard, PoisonError};

pub trait MutexExt<T> {
    /// The lock, taken even when a panic poisoned it.
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_lock_is_still_taken() {
        let m = std::sync::Arc::new(Mutex::new(5));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("holding the lock");
        })
        .join();
        assert!(m.lock().is_err(), "the panic poisoned it");
        assert_eq!(*m.locked(), 5);
    }
}
