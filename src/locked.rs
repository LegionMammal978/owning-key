use crate::{Key, KeyId, Locked};
use core::{
    ffi::CStr,
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit},
    ptr::NonNull,
    slice, str,
};

#[cfg(feature = "alloc")]
use alloc::{
    boxed::Box,
    ffi::CString,
    rc::{self, Rc},
    string::String,
    sync::{self, Arc},
    vec::Vec,
};

#[inline]
fn check_id(key_id: KeyId, value_id: KeyId) {
    if key_id != value_id {
        panic!("locked value accessed with wrong key: expected {value_id:?}, got {key_id:?}");
    }
}

#[derive(Debug)]
pub struct LockedMut<'a, T: ?Sized> {
    ptr: NonNull<T>,
    key_id: KeyId,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T: ?Sized> Locked for LockedMut<'a, T> {
    type Unlocked = &'a mut T;

    #[inline]
    fn key_id(&self) -> KeyId {
        self.key_id
    }

    #[inline]
    unsafe fn raw_lock<K: ?Sized + Key>(r: Self::Unlocked, key: &K) -> Self {
        let key_id = key.id();
        Self {
            ptr: r.into(),
            key_id,
            _marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn raw_unlock<K: ?Sized + Key>(self, key: &mut K) -> Self::Unlocked {
        check_id(key.id(), self.key_id);
        unsafe { { self.ptr }.as_mut() }
    }

    #[inline]
    unsafe fn raw_clone(&self) -> Self {
        Self { ..*self }
    }
}

impl<'a, T: ?Sized> LockedMut<'a, T> {
    #[inline]
    pub fn get<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k T
    where
        'a: 'k,
    {
        check_id(key.id(), self.key_id);
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub fn get_mut<'k, K: ?Sized + Key>(&self, key: &'k mut K) -> &'k mut T
    where
        'a: 'k,
    {
        check_id(key.id(), self.key_id);
        unsafe { &mut *self.ptr.as_ptr() }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (NonNull<T>, KeyId) {
        (self.ptr, self.key_id)
    }

    #[inline]
    pub unsafe fn from_raw_parts(ptr: NonNull<T>, key_id: KeyId) -> Self {
        Self {
            ptr,
            key_id,
            _marker: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct LockedBox<T: ?Sized> {
    ptr: NonNull<T>,
    key_id: KeyId,
}

impl<T: ?Sized> Locked for LockedBox<T> {
    type Unlocked = Box<T>;

    #[inline]
    fn key_id(&self) -> KeyId {
        self.key_id
    }

    #[inline]
    unsafe fn raw_lock<K: ?Sized + Key>(b: Self::Unlocked, key: &K) -> Self {
        let key_id = key.id();
        let ptr = NonNull::new(Box::into_raw(b)).unwrap();
        Self { ptr, key_id }
    }

    #[inline]
    unsafe fn raw_unlock<K: ?Sized + Key>(self, key: &mut K) -> Self::Unlocked {
        check_id(key.id(), self.key_id);
        unsafe { Box::from_raw(self.ptr.as_ptr()) }
    }

    #[inline]
    unsafe fn raw_clone(&self) -> Self {
        Self { ..*self }
    }
}

impl<T: ?Sized> LockedBox<T> {
    #[inline]
    pub fn get<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k T {
        check_id(key.id(), self.key_id);
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub fn get_mut<'k, K: ?Sized + Key>(&self, key: &'k mut K) -> &'k mut T {
        check_id(key.id(), self.key_id);
        unsafe { { self.ptr }.as_mut() }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (NonNull<T>, KeyId) {
        (self.ptr, self.key_id)
    }

    #[inline]
    pub unsafe fn from_raw_parts(ptr: NonNull<T>, key_id: KeyId) -> Self {
        Self { ptr, key_id }
    }
}

#[derive(Debug)]
pub struct LockedVec<T> {
    ptr: NonNull<T>,
    len: usize,
    capacity: usize,
    key_id: KeyId,
}

impl<T> Locked for LockedVec<T> {
    type Unlocked = Vec<T>;

    #[inline]
    fn key_id(&self) -> KeyId {
        self.key_id
    }

    #[inline]
    unsafe fn raw_lock<K: ?Sized + Key>(vec: Self::Unlocked, key: &K) -> Self {
        let key_id = key.id();
        let (len, capacity) = (vec.len(), vec.capacity());
        let ptr = ManuallyDrop::new(vec).as_mut_ptr();
        Self {
            ptr: NonNull::new(ptr).unwrap(),
            len,
            capacity,
            key_id,
        }
    }

    #[inline]
    unsafe fn raw_unlock<K: ?Sized + Key>(self, key: &mut K) -> Self::Unlocked {
        check_id(key.id(), self.key_id);
        unsafe { Vec::from_raw_parts(self.ptr.as_ptr(), self.len, self.capacity) }
    }

    #[inline]
    unsafe fn raw_clone(&self) -> Self {
        Self { ..*self }
    }
}

impl<T> LockedVec<T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn get<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k [T] {
        check_id(key.id(), self.key_id);
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    #[inline]
    pub fn get_mut<'k, K: ?Sized + Key>(&self, key: &'k mut K) -> &'k mut [T] {
        check_id(key.id(), self.key_id);
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    #[inline]
    pub fn get_buf<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k [MaybeUninit<T>] {
        check_id(key.id(), self.key_id);
        unsafe { slice::from_raw_parts(self.ptr.as_ptr().cast(), self.capacity) }
    }

    #[inline]
    pub fn get_buf_mut<'k, K: ?Sized + Key>(&self, key: &'k mut K) -> &'k mut [MaybeUninit<T>] {
        check_id(key.id(), self.key_id);
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr().cast(), self.capacity) }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (NonNull<T>, usize, usize, KeyId) {
        (self.ptr, self.len, self.capacity, self.key_id)
    }

    #[inline]
    pub unsafe fn from_raw_parts(
        ptr: NonNull<T>,
        len: usize,
        capacity: usize,
        key_id: KeyId,
    ) -> Self {
        Self {
            ptr,
            len,
            capacity,
            key_id,
        }
    }
}

#[derive(Debug)]
pub struct LockedString {
    inner: LockedVec<u8>,
}

impl Locked for LockedString {
    type Unlocked = String;

    #[inline]
    fn key_id(&self) -> KeyId {
        self.inner.key_id()
    }

    #[inline]
    unsafe fn raw_lock<K: ?Sized + Key>(s: Self::Unlocked, key: &K) -> Self {
        let vec = s.into_bytes();
        let inner = unsafe { LockedVec::raw_lock(vec, key) };
        Self { inner }
    }

    #[inline]
    unsafe fn raw_unlock<K: ?Sized + Key>(self, key: &mut K) -> Self::Unlocked {
        let vec = unsafe { self.inner.raw_unlock(key) };
        unsafe { String::from_utf8_unchecked(vec) }
    }

    #[inline]
    unsafe fn raw_clone(&self) -> Self {
        let inner = unsafe { self.inner.raw_clone() };
        Self { inner }
    }
}

impl LockedString {
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn get<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k str {
        let bytes = self.inner.get(key);
        unsafe { str::from_utf8_unchecked(bytes) }
    }

    #[inline]
    pub fn get_mut<'k, K: ?Sized + Key>(&self, key: &'k mut K) -> &'k mut str {
        let bytes = self.inner.get_mut(key);
        unsafe { str::from_utf8_unchecked_mut(bytes) }
    }

    #[inline]
    pub fn get_bytes<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k [u8] {
        self.inner.get(key)
    }

    #[inline]
    pub unsafe fn get_bytes_mut<'k, K: ?Sized + Key>(&self, key: &'k mut K) -> &'k mut [u8] {
        self.inner.get_mut(key)
    }

    #[inline]
    pub fn get_bytes_buf<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k [MaybeUninit<u8>] {
        self.inner.get_buf(key)
    }

    #[inline]
    pub unsafe fn get_bytes_buf_mut<'k, K: ?Sized + Key>(
        &self,
        key: &'k mut K,
    ) -> &'k mut [MaybeUninit<u8>] {
        self.inner.get_buf_mut(key)
    }

    #[inline]
    pub fn into_raw_parts(self) -> (NonNull<u8>, usize, usize, KeyId) {
        self.inner.into_raw_parts()
    }

    #[inline]
    pub unsafe fn from_raw_parts(
        ptr: NonNull<u8>,
        len: usize,
        capacity: usize,
        key_id: KeyId,
    ) -> Self {
        let inner = unsafe { LockedVec::from_raw_parts(ptr, len, capacity, key_id) };
        Self { inner }
    }
}

#[derive(Debug)]
pub struct LockedCString {
    inner: LockedVec<u8>,
}

impl Locked for LockedCString {
    type Unlocked = CString;

    #[inline]
    fn key_id(&self) -> KeyId {
        self.inner.key_id()
    }

    #[inline]
    unsafe fn raw_lock<K: ?Sized + Key>(s: Self::Unlocked, key: &K) -> Self {
        let vec = s.into_bytes_with_nul();
        let inner = unsafe { LockedVec::raw_lock(vec, key) };
        Self { inner }
    }

    #[inline]
    unsafe fn raw_unlock<K: ?Sized + Key>(self, key: &mut K) -> Self::Unlocked {
        let vec = unsafe { self.inner.raw_unlock(key) };
        unsafe { CString::from_vec_with_nul_unchecked(vec) }
    }

    #[inline]
    unsafe fn raw_clone(&self) -> Self {
        let inner = unsafe { self.inner.raw_clone() };
        Self { inner }
    }
}

impl LockedCString {
    #[inline]
    pub fn get<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k CStr {
        let bytes = self.inner.get(key);
        unsafe { CStr::from_bytes_with_nul_unchecked(bytes) }
    }

    #[inline]
    pub fn into_raw_parts(self) -> (NonNull<u8>, usize, usize, KeyId) {
        self.inner.into_raw_parts()
    }

    #[inline]
    pub unsafe fn from_raw_parts(
        ptr: NonNull<u8>,
        len: usize,
        capacity: usize,
        key_id: KeyId,
    ) -> Self {
        let inner = unsafe { LockedVec::from_raw_parts(ptr, len, capacity, key_id) };
        Self { inner }
    }
}

#[derive(Debug)]
pub struct LockedRc<T: ?Sized> {
    ptr: NonNull<T>,
    key_id: KeyId,
}

impl<T: ?Sized> Locked for LockedRc<T> {
    type Unlocked = Rc<T>;

    #[inline]
    fn key_id(&self) -> KeyId {
        self.key_id
    }

    #[inline]
    unsafe fn raw_lock<K: ?Sized + Key>(rc: Self::Unlocked, key: &K) -> Self {
        let key_id = key.id();
        let ptr = NonNull::new(Rc::into_raw(rc) as *mut T).unwrap();
        Self { ptr, key_id }
    }

    #[inline]
    unsafe fn raw_unlock<K: ?Sized + Key>(self, key: &mut K) -> Self::Unlocked {
        check_id(key.id(), self.key_id);
        unsafe { Rc::from_raw(self.ptr.as_ptr()) }
    }

    #[inline]
    unsafe fn raw_clone(&self) -> Self {
        Self { ..*self }
    }
}

impl<T: ?Sized> LockedRc<T> {
    #[inline]
    pub fn get<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k T {
        check_id(key.id(), self.key_id);
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub fn get_mut<'k, K: ?Sized + Key>(&self, key: &'k mut K) -> Option<&'k mut T> {
        check_id(key.id(), self.key_id);
        let mut rc = ManuallyDrop::new(unsafe { Rc::from_raw(self.ptr.as_ptr()) });
        let ptr: *mut T = Rc::get_mut(&mut rc)?;
        unsafe { Some(&mut *ptr) }
    }

    #[inline]
    pub fn clone<K: ?Sized + Key>(&self, key: &K) -> Rc<T> {
        check_id(key.id(), self.key_id);
        unsafe {
            Rc::increment_strong_count(self.ptr.as_ptr());
            Rc::from_raw(self.ptr.as_ptr())
        }
    }

    #[inline]
    pub fn downgrade<K: ?Sized + Key>(&self, key: &K) -> rc::Weak<T> {
        check_id(key.id(), self.key_id);
        let rc = ManuallyDrop::new(unsafe { Rc::from_raw(self.ptr.as_ptr()) });
        Rc::downgrade(&rc)
    }
}

#[derive(Debug)]
pub struct LockedArc<T: ?Sized> {
    ptr: NonNull<T>,
    key_id: KeyId,
}

impl<T: ?Sized> Locked for LockedArc<T> {
    type Unlocked = Arc<T>;

    #[inline]
    fn key_id(&self) -> KeyId {
        self.key_id
    }

    #[inline]
    unsafe fn raw_lock<K: ?Sized + Key>(arc: Self::Unlocked, key: &K) -> Self {
        let key_id = key.id();
        let ptr = NonNull::new(Arc::into_raw(arc) as *mut T).unwrap();
        Self { ptr, key_id }
    }

    #[inline]
    unsafe fn raw_unlock<K: ?Sized + Key>(self, key: &mut K) -> Self::Unlocked {
        check_id(key.id(), self.key_id);
        unsafe { Arc::from_raw(self.ptr.as_ptr()) }
    }

    #[inline]
    unsafe fn raw_clone(&self) -> Self {
        Self { ..*self }
    }
}

impl<T: ?Sized> LockedArc<T> {
    #[inline]
    pub fn get<'k, K: ?Sized + Key>(&self, key: &'k K) -> &'k T {
        check_id(key.id(), self.key_id);
        unsafe { self.ptr.as_ref() }
    }

    #[inline]
    pub fn get_mut<'k, K: ?Sized + Key>(&self, key: &'k mut K) -> Option<&'k mut T> {
        check_id(key.id(), self.key_id);
        let mut arc = ManuallyDrop::new(unsafe { Arc::from_raw(self.ptr.as_ptr()) });
        let ptr: *mut T = Arc::get_mut(&mut arc)?;
        unsafe { Some(&mut *ptr) }
    }

    #[inline]
    pub fn clone<K: ?Sized + Key>(&self, key: &K) -> Arc<T> {
        check_id(key.id(), self.key_id);
        unsafe {
            Arc::increment_strong_count(self.ptr.as_ptr());
            Arc::from_raw(self.ptr.as_ptr())
        }
    }

    #[inline]
    pub fn downgrade<K: ?Sized + Key>(&self, key: &K) -> sync::Weak<T> {
        check_id(key.id(), self.key_id);
        let arc = ManuallyDrop::new(unsafe { Arc::from_raw(self.ptr.as_ptr()) });
        Arc::downgrade(&arc)
    }
}
