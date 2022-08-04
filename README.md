# `typed-arena-any-vec`

[![](https://docs.rs/typed-arena/badge.svg)](https://docs.rs/typed-arena-any-vec/)
[![](https://img.shields.io/crates/v/typed-arena-any-vec.svg)](https://crates.io/crates/typed-arena-any-vec)
[![](https://img.shields.io/crates/d/typed-arena-any-vec.svg)](https://crates.io/crates/typed-arena-any-vec)

**[typed-arena](https://docs.rs/typed-arena)** but supports any type of `Vec` backing, including [`SmallVec`](https://docs.rs/smallvec), [`ArrayVec`](https://docs.rs/arrayvec), and [`SliceVec`](https://docs.rs/slicevec). This means that you can create areans on data which you are mutably borrowing or data which is not stored on the heap.

Do note that if you're putting arenas on the stack, make sure that they're small, as the stack doesn't have much memory.

## `GrowVec`

The backing type is a `GrowVec` provided in this module. This vector only needs to support insertion, as allocated objects are only destroyed all at once when the vec itself is dropped.

`GrowVec` is implemented for these external crates. To use, add the crate's name as a feature:

- [`smallvec`](https://docs.rs/smallvec)
- [`arrayvec`](https://docs.rs/arrayvec)
- [`slicevec`](https://docs.rs/slicevec)

If you have another external trait you want `GrowVec` to support, create a PR.

## Examples (from typed-arena)

```rust
use typed_arena_any_vec::Arena;
use arrayvec::ArrayVec;

struct Monster {
    level: u32,
}

fn fun() {
    let monsters = Arena::new(ArrayVec::<Monster, 4>::new());

    let goku = monsters.alloc(Monster { level: 9001 });
    assert!(goku.level > 9000).unwrap();
}
```

### Safe Cycles

All allocated objects get the same lifetime, so you can safely create cycles
between them. This can be useful for certain data structures, such as graphs
and trees with parent pointers.

```rust
use std::cell::Cell;
use typed_arena_any_vec::Arena;
use slicevec::SliceVec;

struct CycleParticipant<'a> {
    other: Cell<Option<&'a CycleParticipant<'a>>>,
}

struct CapacityError;

fn fun(backing: &mut [CycleParticipant<'_>]) -> Result<(), CapacityError> {
    let arena = Arena::new(SliceVec::new(backing));

    let a = arena.alloc(CycleParticipant { other: Cell::new(None) }).map_err(|_| CapacityError);
    let b = arena.alloc(CycleParticipant { other: Cell::new(None) }).map_err(|_| CapacityError);

    a.other.set(Some(b));
    b.other.set(Some(a));
}
```
