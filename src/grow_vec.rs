#[cfg(feature = "frozenvec")]
use core::alloc::Allocator;
#[cfg(feature = "frozenvec")]
use core::convert::Infallible;
#[cfg(feature = "frozenvec")]
use core::marker::PhantomData;
#[cfg(feature = "arrayvec")]
use core::mem::{MaybeUninit};
#[cfg(any(feature = "arrayvec", feature = "slicevec"))]
use transmute::transmute;
#[cfg(any(feature = "arrayvec", feature = "slicevec"))]
use core::ptr::{addr_of, addr_of_mut};
#[cfg(feature = "frozenvec")]
use stable_deref_trait::StableDeref;
#[cfg(feature = "arrayvec")]
use arrayvec::ArrayVec;
#[cfg(feature = "slicevec")]
use slicevec::SliceVec;

/// A vector which supports mutable indexing and insertion, and you can insert into the vector
/// while indexed values have live references without UB.
///
/// Does not need to support removal.
pub trait GrowVec<T> {
    /// The type which actually supports insertion with active references.
    type Raw;
    /// Error when the vector is full. Use [Infallible] if the vector can grow to the system memory.
    type CapacityError;

    /// Convert from the type which actually supports insertion with active references.
    fn from_raw(raw: Self::Raw) -> Self;

    /// Convert to the type which actually supports insertion with active references.
    fn into_raw(self) -> Self::Raw;

    /// Gets the number of pushed values from a mutable pointer to `Self`.
    ///
    /// SAFETY: the pointer must point to an initialized instance.
    unsafe fn len_from_ptr(this: *const Self::Raw) -> usize;

    /// Gets the mutable pointer of a value at an index a mutable pointer.
    ///
    /// SAFETY: the pointer must point to an initialized instance.
    unsafe fn index_mut_from_ptr(this: *mut Self::Raw, idx: usize) -> *mut T;

    /// Tries to push a value from a mutable pointer to `Self`.
    /// Returns [CapacityError] if the vector is full.
    ///
    /// SAFETY: the pointer must point to an initialized instance.
    unsafe fn push_from_ptr(this: *mut Self::Raw, value: T) -> Result<(), Self::CapacityError>;
}

#[cfg(feature = "frozenvec")]
impl<T: StableDeref, A: Allocator> GrowVec<T> for Vec<T, A> {
    type Raw = Vec<T, A>;
    type CapacityError = Infallible;

    fn from_raw(raw: Self::Raw) -> Self {
        raw
    }

    fn into_raw(self) -> Self::Raw {
        self
    }

    unsafe fn len_from_ptr(this: *const Self::Raw) -> usize {
        // SAFETY: We can freely dereference the entire vec because all values returned from index_mut_from_ptr
        // should be converted into (*mut T::Target)s, so when they get converted into references,
        // they should point to data outside of the vec.
        // Therefore, &mut *this and &mut *this.index_mut_from_ptr(idx) can exist simultaneously.
        let this = &*this;
        this.len()
    }

    unsafe fn index_mut_from_ptr(this: *mut Self::Raw, idx: usize) -> *mut T {
        let this = &mut *this;
        &mut this[idx] as *mut T
    }

    unsafe fn push_from_ptr(this: *mut Self::Raw, value: T) -> Result<(), Self::CapacityError> {
        let this = &mut *this;
        this.push(value);
        Ok(())
    }
}

#[cfg(feature = "arrayvec")]
#[doc(hidden)]
pub struct _ArrayVec<T, const CAP: usize> {
    // the `len` first elements of the array are initialized
    xs: [MaybeUninit<T>; CAP],
    len: u32,
}

#[cfg(feature = "arrayvec")]
impl<T, const CAP: usize> GrowVec<T> for ArrayVec<T, CAP> {
    type Raw = _ArrayVec<T, CAP>;
    type CapacityError = arrayvec::CapacityError<T>;

    fn from_raw(raw: Self::Raw) -> Self {
        // SAFETY: have the same struct definition.
        // Technically this is actually unsafe and UB, and there is no way to access private struct fields.
        // But in practice this is ok
        unsafe { transmute::<_ArrayVec<T, CAP>, ArrayVec<T, CAP>>(raw) }
    }

    fn into_raw(self) -> Self::Raw {
        // SAFETY: have the same struct definition.
        unsafe { transmute::<ArrayVec<T, CAP>, _ArrayVec<T, CAP>>(self) }
    }

    unsafe fn len_from_ptr(this: *const Self::Raw) -> usize {
        addr_of!((*this).len).read() as usize
    }

    unsafe fn index_mut_from_ptr(this: *mut Self::Raw, idx: usize) -> *mut T {
        (addr_of_mut!((*this).xs) as *mut T).add(idx)
    }

    unsafe fn push_from_ptr(this: *mut Self::Raw, value: T) -> Result<(), Self::CapacityError> {
        let len = addr_of!((*this).len).read();
        if len == CAP as u32 {
            Err(arrayvec::CapacityError::new(value))
        } else {
            (addr_of_mut!((*this).xs) as *mut T).add(len as usize).write(value);
            addr_of_mut!((*this).len).write(len + 1);
            Ok(())
        }
    }
}


#[cfg(feature = "slicevec")]
struct _SliceVec<'a, T> {
    storage: &'a mut [T],
    len: usize,
}

#[cfg(feature = "slicevec")]
#[doc(hidden)]
pub struct SliceVecRaw<'a, T> {
    storage: *mut [T],
    len: usize,
    lifetime: PhantomData<&'a ()>
}

#[cfg(feature = "slicevec")]
impl<'a, T> GrowVec<T> for SliceVec<'a, T> {
    type Raw = SliceVecRaw<'a, T>;
    type CapacityError = T;

    fn from_raw(raw: Self::Raw) -> Self {
        // SAFETY: lifetime of raw is stored, just separately
        let raw = _SliceVec {
            storage: unsafe { &mut *raw.storage },
            len: raw.len,
        };
        // SAFETY: have the same struct definition.
        unsafe { transmute::<_SliceVec<T>, SliceVec<T>>(raw) }
    }

    fn into_raw(self) -> Self::Raw {
        // SAFETY: Have the same definition
        let this = unsafe { transmute::<SliceVec<'a, T>, _SliceVec<'a, T>>(self) };
        SliceVecRaw {
            storage: this.storage as *mut _,
            len: this.len,
            lifetime: PhantomData
        }
    }

    unsafe fn len_from_ptr(this: *const Self::Raw) -> usize {
        addr_of!((*this).len).read() as usize
    }

    unsafe fn index_mut_from_ptr(this: *mut Self::Raw, idx: usize) -> *mut T {
        (addr_of!((*this).storage).read() as *mut T).add(idx)
    }

    unsafe fn push_from_ptr(this: *mut Self::Raw, value: T) -> Result<(), Self::CapacityError> {
        let len = addr_of!((*this).len).read();
        let storage = addr_of!((*this).storage).read();
        let (storage, capacity) = storage.to_raw_parts();
        if len == capacity {
            Err(value)
        } else {
            (storage as *mut T).add(len as usize).write(value);
            addr_of_mut!((*this).len).write(len + 1);
            Ok(())
        }
    }
}