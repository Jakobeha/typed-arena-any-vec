#![feature(ptr_metadata)]
#![feature(exact_size_is_empty)]
#![cfg_attr(all(test, feature = "slicevec"), feature(maybe_uninit_uninit_array))]
#![cfg_attr(all(test, feature = "slicevec"), feature(maybe_uninit_array_assume_init))]
#![cfg_attr(feature = "frozenvec", feature(allocator_api))]
#![doc = include_str!("../README.md")]

#![deny(missing_docs)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]

#[cfg(any(feature = "std", test))]
extern crate core;
#[cfg(any(feature = "arrayvec", feature = "slicevec"))]
extern crate transmute;
#[cfg(feature = "frozenvec")]
extern crate stable_deref_trait;
#[cfg(feature = "arrayvec")]
extern crate arrayvec;
#[cfg(feature = "slicevec")]
extern crate slicevec;

use core::str;
use core::cell::UnsafeCell;
use core::ptr::slice_from_raw_parts_mut;

mod grow_vec;
#[cfg(test)]
mod test;

pub use grow_vec::*;

/// An arena of objects of type `T`.
///
/// ## Example
///
/// ```
/// use typed_arena_any_vec::Arena;
/// use arrayvec::ArrayVec;
///
/// struct Monster {
///     level: u32,
/// }
///
/// fn fun() {
///     let monsters = Arena::new(ArrayVec::<Monster, 5>::new());
///
///     let vegeta = monsters.alloc(Monster { level: 9001 });
///     assert!(vegeta.level > 9000);
/// }
/// ```
pub struct Arena<T, V: GrowVec<T>> {
    // Must be wrapped in UnsafeCell so Rust lets us have active mutable references.
    // while Arena is also active. We can perform actions on V::Raw without actually dereferencing it.
    backing: UnsafeCell<V::Raw>,
}

impl<T, V: GrowVec<T>> Arena<T, V> {
    /// Construct a new arena.
    ///
    /// ## Example
    ///
    /// ```
    /// use typed_arena_any_vec::Arena;
    /// use arrayvec::ArrayVec;
    ///
    /// fn fun() {
    ///    let arena = Arena::new(ArrayVec::<usize, 5>::new());
    /// #  arena.alloc(1).unwrap();
    /// }
    /// ```
    pub fn new(backing: V) -> Arena<T, V> {
        Arena {
            backing: UnsafeCell::new(backing.into_raw()),
        }
    }

    /// Return the size of the arena
    ///
    /// This is useful for using the size of previous typed arenas to build new typed arenas with large enough spaces.
    ///
    /// ## Example
    ///
    /// ```
    /// use typed_arena_any_vec::Arena;
    /// use arrayvec::ArrayVec;
    ///
    /// let arena = Arena::new(ArrayVec::<_, 5>::new());
    /// let a = arena.alloc(1);
    /// let b = arena.alloc(2);
    ///
    /// assert_eq!(arena.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        unsafe { V::len_from_ptr(self.backing.get()) }
    }

    /// Allocates a value in the arena, and returns a mutable reference
    /// to that value. Returns a capacity error if the vector is full.
    ///
    /// ## Example
    ///
    /// ```
    /// use typed_arena_any_vec::Arena;
    /// use arrayvec::ArrayVec;
    ///
    /// let arena = Arena::new(ArrayVec::<_, 5>::new());
    /// let x = arena.alloc(42);
    /// assert_eq!(*x, 42);
    /// ```
    #[inline]
    pub fn alloc(&self, value: T) -> Result<&mut T, V::CapacityError> {
        unsafe {
            V::push_from_ptr(self.backing.get(), value)?;
            Ok(&mut *V::index_mut_from_ptr(self.backing.get(), V::len_from_ptr(self.backing.get()) - 1))
        }
    }

    /// Convert this `Arena` into a `V`.
    ///
    /// Items in the resulting `V` appear in the order that they were
    /// allocated in.
    ///
    /// ## Example
    ///
    /// ```
    /// use typed_arena_any_vec::Arena;
    /// use arrayvec::ArrayVec;
    ///
    /// fn fun() {
    ///   let arena = Arena::new(ArrayVec::<_, 5>::new());
    ///
    ///   arena.alloc("a").unwrap();
    ///   arena.alloc("b").unwrap();
    ///   arena.alloc("c").unwrap();
    ///
    ///   let easy_as_123 = arena.into_vec();
    ///
    ///   assert_eq!(easy_as_123, vec!["a", "b", "c"]);
    /// }
    /// ```
    pub fn into_vec(self) -> V {
        V::from_raw(self.backing.into_inner())
    }

    /// Returns an iterator that allows modifying each value.
    ///
    /// Items are yielded in the order that they were allocated.
    ///
    /// ## Example
    ///
    /// ```
    /// use typed_arena_any_vec::Arena;
    /// use arrayvec::ArrayVec;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Point { x: i32, y: i32 };
    ///
    /// fn fun() {
    ///   let mut arena = Arena::new(ArrayVec::<_, 5>::new());
    ///
    ///   arena.alloc(Point { x: 0, y: 0 }).unwrap();
    ///   arena.alloc(Point { x: 1, y: 1 }).unwrap();
    ///
    ///   for point in arena.iter_mut() {
    ///       point.x += 10;
    ///   }
    ///
    ///   let points = arena.into_vec();
    ///
    ///   assert_eq!(points, vec![Point { x: 10, y: 0 }, Point { x: 11, y: 1 }]);
    /// }
    /// ```
    ///
    /// ## Immutable Iteration
    ///
    /// Note that there is no corresponding `iter` method. Access to the arena's contents
    /// requries mutable access to the arena itself.
    ///
    /// ```compile_fail
    /// use typed_arena_any_vec::Arena;
    /// use arrayvec::ArrayVec;
    ///
    /// fn fun() {
    ///   let mut arena = Arena::new(ArrayVec::<_, 5>::new());
    ///   let x = arena.alloc(1).unwrap();
    ///
    ///   // borrow error!
    ///   for i in arena.iter_mut() {
    ///       println!("i: {}", i);
    ///   }
    ///
    ///   // borrow error!
    ///   *x = 2;
    /// }
    /// ```
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T, V> {
        // Has to explicitly be before we get_mut on backing
        let len = self.len();
        IterMut {
            idx: 0,
            len,
            ptr: self.backing.get_mut(),
        }
    }
}

impl<V: GrowVec<u8>> Arena<u8, V> {
    /// Allocates a string slice and returns a mutable reference to it.
    ///
    /// This is on `Arena<u8>`, because string slices use byte slices (`[u8]`) as their backing
    /// storage.
    ///
    /// # Example
    ///
    /// ```
    /// use typed_arena_any_vec::Arena;
    /// use arrayvec::ArrayVec;
    ///
    /// fn fun() {
    ///   let arena: Arena<u8, ArrayVec<u8, 25>> = Arena::new(ArrayVec::new());
    ///   let hello = arena.alloc_str("Hello world");
    ///   assert_eq!("Hello world", hello);
    /// }
    /// ```
    #[inline]
    pub fn alloc_str(&self, s: &str) -> Result<&mut str, V::CapacityError> {
        // TODO: optimize if the compiler doesn't
        let start_idx = self.len();
        let bytes = s.bytes();
        let len = bytes.len();
        for byte in bytes {
            self.alloc(byte)?;
        }
        let buffer = unsafe {
            &mut *slice_from_raw_parts_mut(
                V::index_mut_from_ptr(self.backing.get(), start_idx),
                len
            )
        };
        // SAFETY: can't fail because we got from utf8
        Ok(unsafe { str::from_utf8_unchecked_mut(buffer) })
    }
}

impl<T, V: GrowVec<T> + Default> Default for Arena<T, V> {
    fn default() -> Self {
        Self::new(V::default())
    }
}

/// Mutable arena iterator.
///
/// This struct is created by the [`iter_mut`](struct.Arena.html#method.iter_mut) method on [Arenas](struct.Arena.html).
pub struct IterMut<'a, T: 'a, V: GrowVec<T> + 'a> {
    idx: usize,
    len: usize,
    // We could store *mut V::Raw and lifetime separately, because we're only using *mut V::Raw
    ptr: &'a mut V::Raw,
}

impl<'a, T: 'a, V: GrowVec<T> + 'a> Iterator for IterMut<'a, T, V> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        if self.idx == self.len {
            None
        } else {
            Some(unsafe { &mut *V::index_mut_from_ptr(self.ptr as *mut V::Raw, self.idx) })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.idx;
        (remaining, Some(remaining))
    }
}

impl<'a, T: 'a, V: GrowVec<T> + 'a> ExactSizeIterator for IterMut<'a, T, V> {
    fn len(&self) -> usize {
        self.len - self.idx
    }

    fn is_empty(&self) -> bool {
        self.len == self.idx
    }
}