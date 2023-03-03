#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]

use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod key;
mod locked;
pub use key::*;
pub use locked::*;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct KeyId {
    id: usize,
}

impl KeyId {
    #[inline]
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("unique counter for KeyId should not overflow");
        Self { id }
    }
}

/// # Safety
///
/// Consider any two types `K1` and `K2`, not necessarily distinct, which both
/// implement `Key`.
///
/// For any two references `key1: &mut K1` and `key2: &mut K2`, neither of which
/// is a reborrow of the other, if [`Key::id()`] is ever called on both
/// references and returns the same [`KeyId`] for both, the lifetimes of `key1`
/// and `key2` must be disjoint.
///
/// For any two references `key1: &mut K1` and `key2: &K2`, neither of which is
/// a reborrow of the other, if [`Key::id()`] is ever called on both references
/// and returns the same [`KeyId`] for both, the lifetimes of `key1` and `key2`
/// must be disjoint.
///
/// If a type implementing `Key` assigns a unique [`KeyId`] to each value that
/// can be logically borrowed, then these conditions are upheld by the borrow
/// checker.
pub unsafe trait Key {
    fn id(&self) -> KeyId;
}

pub trait Locked {
    /// The type of the value obtained from unlocking this value.
    type Unlocked;

    /// Returns the [`KeyId`] of the key used to create this value.
    fn key_id(&self) -> KeyId;

    /// # Safety
    ///
    /// This function may only be called:
    ///
    /// - in situations where the key type `K` explicitly permits this function
    ///   to be called; or
    /// - during a call to `Locked::raw_lock()` on any other value, using the
    ///   same `key` reference that was provided to that call.
    unsafe fn raw_lock<K: ?Sized + Key>(value: Self::Unlocked, key: &K) -> Self;

    /// # Safety
    ///
    /// This function may only be called:
    ///
    /// - in situations where the key type `K` explicitly permits this function
    ///   to be called; or
    /// - during a call to `Locked::raw_unlock()` on any other value, using the
    ///   same `key` reference that was provided to that call, given that the
    ///   other value resulted from a call to `Locked::raw_lock()`, and this
    ///   value resulted from a call to `Locked::raw_lock()` using the `key`
    ///   reference provided to that call.
    unsafe fn raw_unlock<K: ?Sized + Key>(self, key: &mut K) -> Self::Unlocked;

    /// # Safety
    ///
    /// This function may only be called in situations where the key used to
    /// create this value explicitly permits this function to be called.
    unsafe fn raw_clone(&self) -> Self;
}
