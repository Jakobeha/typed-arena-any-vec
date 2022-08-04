use super::*;
#[cfg(any(feature = "arrayvec", feature = "slicevec"))]
use std::cell::Cell;
#[cfg(any(feature = "slicevec"))]
use std::mem::MaybeUninit;
#[cfg(feature = "arrayvec")]
use arrayvec::ArrayVec;
#[cfg(feature = "slicevec")]
use slicevec::SliceVec;

#[derive(Debug, Clone)]
struct DropTracker<'a>(&'a Cell<u32>);

impl<'a> Drop for DropTracker<'a> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[derive(Debug, Clone)]
struct Node<'a, 'b: 'a>(Option<&'a Node<'a, 'b>>, u32, DropTracker<'b>);

#[cfg(feature = "arrayvec")]
#[test]
fn array_arena() {
    let drop_counter = Cell::new(0);
    {
        let arena = Arena::new(ArrayVec::<_, 2>::new());

        let mut node = arena.alloc(Node(None, 1, DropTracker(&drop_counter))).unwrap();
        node = arena.alloc(Node(Some(node), 2, DropTracker(&drop_counter))).unwrap();

        assert_eq!(node.1, 2);
        assert_eq!(node.0.unwrap().1, 1);
        assert!(node.0.unwrap().0.is_none());
        assert_eq!(arena.len(), 2);

        let error = arena.alloc(Node(Some(node), 3, DropTracker(&drop_counter))).unwrap_err();
        let error_elem = error.element();
        assert_eq!(error_elem.1, 3);

        assert_eq!(drop_counter.get(), 0);
        drop(error_elem);
        assert_eq!(drop_counter.get(), 1);

        drop(node);
    }
    assert_eq!(drop_counter.get(), 3);
    drop_counter.set(0);

    {
        let arena = Arena::new(ArrayVec::<_, 25>::new());

        let mut node = arena.alloc(Node(None, 1, DropTracker(&drop_counter))).unwrap();
        node = arena.alloc(Node(Some(node), 2, DropTracker(&drop_counter))).unwrap();
        node = arena.alloc(Node(Some(node), 3, DropTracker(&drop_counter))).unwrap();

        assert_eq!(node.1, 3);
        assert_eq!(node.0.unwrap().1, 2);
        assert_eq!(arena.len(), 3);

        let mut node = arena.alloc(Node(None, 4, DropTracker(&drop_counter))).unwrap();
        node = arena.alloc(Node(Some(node), 5, DropTracker(&drop_counter))).unwrap();

        assert_eq!(drop_counter.get(), 0);
        assert_eq!(node.1, 5);
        assert_eq!(node.0.unwrap().1, 4);
        assert!(node.0.unwrap().0.unwrap().0.is_none());
    }
    assert_eq!(drop_counter.get(), 7);
}

#[cfg(feature = "slicevec")]
#[test]
fn slice_arena() {
    let drop_counter_buffer2 = Cell::new(0);
    let mut buffer2 = MaybeUninit::uninit_array::<25>();
    for elem in buffer2.iter_mut() {
        elem.write(Node(None, 100, DropTracker(&drop_counter_buffer2)));
    }
    // let mut buffer2 = unsafe { MaybeUninit::array_assume_init(buffer2) };

    let drop_counter = Cell::new(0);
    {
        let mut buffer1 = MaybeUninit::uninit_array::<2>();
        for elem in buffer1.iter_mut() {
            elem.write(Node(None, 10, DropTracker(&drop_counter)));
        }
        let mut buffer1 = unsafe { MaybeUninit::array_assume_init(buffer1) };        let arena = Arena::new(SliceVec::new(&mut buffer1));

        let mut node = arena.alloc(Node(None, 1, DropTracker(&drop_counter))).unwrap();
        node = arena.alloc(Node(Some(node), 2, DropTracker(&drop_counter))).unwrap();

        assert_eq!(node.1, 2);
        assert_eq!(node.0.unwrap().1, 1);
        assert!(node.0.unwrap().0.is_none());
        assert_eq!(arena.len(), 2);

        let error = arena.alloc(Node(Some(node), 3, DropTracker(&drop_counter))).unwrap_err();
        assert_eq!(error.1, 3);

        assert_eq!(drop_counter.get(), 2);
    }

    assert_eq!(drop_counter.get(), 5);
    drop_counter.set(0);

    /* {
        let arena = Arena::new(SliceVec::new(&mut buffer2));

        let node1 = arena.alloc(Node(None, 1, DropTracker(&drop_counter))).unwrap();
        let node2 = arena.alloc(Node(Some(node1), 2, DropTracker(&drop_counter))).unwrap();
        let node3 = arena.alloc(Node(Some(node2), 3, DropTracker(&drop_counter))).unwrap();

        assert_eq!(node3.1, 3);
        assert_eq!(node3.0.unwrap().1, 2);
        assert_eq!(node2.1, 2);
        assert_eq!(arena.len(), 3);

        let node4 = arena.alloc(Node(None, 4, DropTracker(&drop_counter))).unwrap();
        let node5 = arena.alloc(Node(Some(node4), 5, DropTracker(&drop_counter))).unwrap();

        assert_eq!(drop_counter.get(), 0);
        assert_eq!(node5.1, 5);
        assert_eq!(node5.0.unwrap().1, 4);
        assert!(node5.0.unwrap().0.unwrap().0.is_none());

        assert_eq!(drop_counter_buffer2.get(), 7);
    }
    assert_eq!(drop_counter.get(), 0);
    assert_eq!(drop_counter_buffer2.get(), 7);

    drop(buffer2);

    assert_eq!(drop_counter.get(), 7);
    assert_eq!(drop_counter_buffer2.get(), 100); */
}

#[test]
#[cfg(feature = "stable_deref_trait")]
fn ensure_into_vec_maintains_order_of_allocation() {
    let arena = Arena::new(Vec::new());
    for &s in &["t", "e", "s", "t"] {
        arena.alloc(String::from(s)).unwrap();
    }
    let vec = arena.into_vec();
    assert_eq!(vec, vec!["t", "e", "s", "t"]);
}

#[test]
#[cfg(feature = "arrayvec")]
fn test_is_send() {
    fn assert_is_send<T: Send>(_: T) {}

    // If `T` is `Send`, ...
    assert_is_send(42_u32);

    // Then `Arena<T>` is also `Send`.
    let arena: Arena<u32, ArrayVec<u32, 5>> = Arena::new(ArrayVec::new());
    assert_is_send(arena);
}

#[derive(Debug, PartialEq, Eq)]
struct NonCopy(usize);

#[test]
#[cfg(feature = "arrayvec")]
fn iter_mut_full_capacity() {
    const MAX: usize = 1000;

    let mut arena = Arena::new(ArrayVec::<_, 1000>::new());
    for i in 0..MAX {
        arena.alloc(NonCopy(i)).unwrap();
    }

    let mut iter = arena.iter_mut();
    assert_eq!(iter.len(), MAX);

    for i in 0..MAX {
        assert_eq!(Some(&mut NonCopy(i)), iter.next());
    }

    assert!(iter.is_empty());
    assert_eq!(None, iter.next());
}

#[test]
#[cfg(feature = "arrayvec")]
fn iter_mut_not_full_capacity() {
    const MAX: usize = 1000;

    let mut arena = Arena::new(ArrayVec::<_, 2000>::new());
    for i in 0..MAX {
        arena.alloc(NonCopy(i)).unwrap();
    }

    let mut iter = arena.iter_mut();
    assert_eq!(iter.len(), MAX);

    for i in 0..MAX {
        assert_eq!(Some(&mut NonCopy(i)), iter.next());
    }

    assert!(iter.is_empty());
    assert_eq!(None, iter.next());
}