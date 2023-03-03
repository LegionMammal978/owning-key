use crate::{Key, KeyId, Locked};
use core::{
    borrow::Borrow,
    cell::RefCell,
    fmt::{self, Debug, Formatter},
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

#[cfg(feature = "std")]
use std::{collections::HashSet, sync::Mutex};

#[derive(Debug)]
pub struct ForgettingKey {
    id: KeyId,
}

unsafe impl Key for ForgettingKey {
    #[inline]
    fn id(&self) -> KeyId {
        self.id
    }
}

impl ForgettingKey {
    #[inline]
    pub fn new() -> Self {
        Self { id: KeyId::new() }
    }

    #[inline]
    pub fn lock<T: Locked>(&self, value: T::Unlocked) -> T {
        unsafe { T::raw_lock(value, self) }
    }

    #[inline]
    pub fn unlock<T: Locked>(&mut self, value: T) -> T::Unlocked {
        unsafe { value.raw_unlock(self) }
    }
}

union DropperInner<T> {
    value: ManuallyDrop<T>,
    _pad: u8,
}

struct Dropper<'a> {
    ptr: NonNull<()>,
    unlock_drop: unsafe fn(NonNull<()>, &mut ForgettingKey),
    _marker: PhantomData<&'a ()>,
}

impl PartialEq for Dropper<'_> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.ptr.eq(&other.ptr)
    }
}

impl Eq for Dropper<'_> {}

impl Debug for Dropper<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dropper")
            .field("ptr", &self.ptr)
            .field("unlock_drop", &(self.unlock_drop as *const ()))
            .field("_marker", &self._marker)
            .finish()
    }
}

impl Hash for Dropper<'_> {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.ptr.hash(state);
    }
}

impl Borrow<NonNull<()>> for Dropper<'_> {
    #[inline]
    fn borrow(&self) -> &NonNull<()> {
        &self.ptr
    }
}

impl<'a> Dropper<'a> {
    #[inline]
    fn new<T: Locked + 'a>(value: T) -> Self {
        let value = ManuallyDrop::new(value);
        let dropper = Box::new(DropperInner { value });
        let ptr = NonNull::new(Box::into_raw(dropper)).unwrap();
        Self {
            ptr: ptr.cast(),
            unlock_drop: |ptr, key| {
                let ptr: NonNull<DropperInner<T>> = ptr.cast();
                let mut dropper = unsafe { Box::from_raw(ptr.as_ptr()) };
                let value = unsafe { ManuallyDrop::take(&mut dropper.value) };
                drop(key.unlock(value));
            },
            _marker: PhantomData,
        }
    }

    #[inline]
    fn unlock_drop(self, key: &mut ForgettingKey) {
        unsafe { (self.unlock_drop)(self.ptr, key) };
    }
}

#[derive(Debug)]
pub struct Dropping<T> {
    value: T,
    ptr: NonNull<()>,
}

impl<T> Deref for Dropping<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Dropping<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

fn unlock_drop_all<'a, I: Iterator<Item = Dropper<'a>>>(droppers: I, key: &mut ForgettingKey) {
    struct DropGuard<'a, 'b, I: Iterator<Item = Dropper<'a>>> {
        droppers: I,
        key: &'b mut ForgettingKey,
    }
    impl<'a, I: Iterator<Item = Dropper<'a>>> Drop for DropGuard<'a, '_, I> {
        fn drop(&mut self) {
            for dropper in &mut self.droppers {
                dropper.unlock_drop(self.key);
            }
        }
    }
    let mut guard = DropGuard { droppers, key };
    for dropper in &mut guard.droppers {
        dropper.unlock_drop(guard.key);
    }
    mem::forget(guard);
}

#[derive(Debug)]
pub struct LocalDroppingKey<'a> {
    inner: ForgettingKey,
    droppers: RefCell<HashSet<Dropper<'a>>>,
}

unsafe impl Key for LocalDroppingKey<'_> {
    #[inline]
    fn id(&self) -> KeyId {
        self.inner.id()
    }
}

impl Drop for LocalDroppingKey<'_> {
    #[inline]
    fn drop(&mut self) {
        let droppers = mem::take(self.droppers.get_mut()).into_iter();
        unlock_drop_all(droppers, &mut self.inner);
    }
}

impl<'a> LocalDroppingKey<'a> {
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: ForgettingKey::new(),
            droppers: RefCell::new(HashSet::new()),
        }
    }

    #[inline]
    pub fn lock<T: Locked + 'a>(&self, value: T::Unlocked) -> Dropping<T> {
        let value: T = self.inner.lock(value);
        let dropper = Dropper::new(unsafe { value.raw_clone() });
        let ptr = dropper.ptr;
        if !self.droppers.borrow_mut().insert(dropper) {
            unreachable!("box address should be unique");
        }
        Dropping { value, ptr }
    }

    #[inline]
    pub fn unlock<T: Locked + 'a>(&mut self, value: Dropping<T>) -> T::Unlocked {
        let ptr = value.ptr;
        let value = self.inner.unlock(value.value);
        if !self.droppers.get_mut().remove(&ptr) {
            unreachable!("value should correspond to dropper");
        }
        value
    }
}

#[derive(Debug)]
pub struct DroppingKey<'a> {
    inner: ForgettingKey,
    droppers: Mutex<HashSet<Dropper<'a>>>,
}

unsafe impl Send for DroppingKey<'_> {}

unsafe impl Sync for DroppingKey<'_> {}

unsafe impl Key for DroppingKey<'_> {
    #[inline]
    fn id(&self) -> KeyId {
        self.inner.id()
    }
}

impl Drop for DroppingKey<'_> {
    #[inline]
    fn drop(&mut self) {
        let droppers = mem::take(self.droppers.get_mut().unwrap()).into_iter();
        unlock_drop_all(droppers, &mut self.inner);
    }
}

impl<'a> DroppingKey<'a> {
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: ForgettingKey::new(),
            droppers: Mutex::new(HashSet::new()),
        }
    }

    #[inline]
    pub fn lock<T: Locked + Send + Sync + 'a>(&self, value: T::Unlocked) -> Dropping<T> {
        let value: T = self.inner.lock(value);
        let dropper = Dropper::new(unsafe { value.raw_clone() });
        let ptr = dropper.ptr;
        if !self.droppers.lock().unwrap().insert(dropper) {
            unreachable!("box address should be unique");
        }
        Dropping { value, ptr }
    }

    #[inline]
    pub fn unlock<T: Locked + Send + Sync + 'a>(&mut self, value: Dropping<T>) -> T::Unlocked {
        let ptr = value.ptr;
        let value = self.inner.unlock(value.value);
        if !self.droppers.get_mut().unwrap().remove(&ptr) {
            unreachable!("value should correspond to dropper");
        }
        value
    }
}
