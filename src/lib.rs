// SPDX-License-Identifier: GPL-2.0

//! Implementation of the kernel's memory allocation infrastructure.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]
// `feature(derive_coerce_pointee)` is expected to become stable. Before Rust
// 1.84.0, it did not exist, so enable the predecessor features.
#![cfg_attr(
    all(kernel, CONFIG_RUSTC_HAS_COERCE_POINTEE),
    feature(derive_coerce_pointee)
)]
#![cfg_attr(
    all(kernel, not(CONFIG_RUSTC_HAS_COERCE_POINTEE)),
    feature(coerce_unsized)
)]
#![cfg_attr(
    all(kernel, not(CONFIG_RUSTC_HAS_COERCE_POINTEE)),
    feature(dispatch_from_dyn)
)]
#![cfg_attr(all(kernel, not(CONFIG_RUSTC_HAS_COERCE_POINTEE)), feature(unsize))]

pub mod kbox;
pub mod kvec;
pub mod layout;

pub use self::kbox::Box;

pub use self::kvec::IntoIter;
pub use self::kvec::Vec;

/// Indicates an allocation error.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct AllocError;

use core::{alloc::Layout, ptr::NonNull};

/// Flags accepted by an [`Allocator`].
pub trait AllocatorFlags: Copy {
    /// An empty set of flags.
    fn empty() -> Self;
}

/// The kernel's [`Allocator`] trait.
///
/// An implementation of [`Allocator`] can allocate, re-allocate and free memory buffers described
/// via [`Layout`].
///
/// [`Allocator`] is designed to be implemented as a ZST; [`Allocator`] functions do not operate on
/// an object instance.
///
/// In order to be able to support `#[derive(CoercePointee)]` later on, we need to avoid a design
/// that requires an `Allocator` to be instantiated, hence its functions must not contain any kind
/// of `self` parameter.
///
/// # Safety
///
/// - A memory allocation returned from an allocator must remain valid until it is explicitly freed.
///
/// - Any pointer to a valid memory allocation must be valid to be passed to any other [`Allocator`]
///   function of the same type.
///
/// - Implementers must ensure that all trait functions abide by the guarantees documented in the
///   `# Guarantees` sections.
pub unsafe trait Allocator {
    /// Flags passed to this trait's methods to control allocator behavior.
    type Flags: AllocatorFlags;

    /// The minimum alignment satisfied by all allocations from this allocator.
    ///
    /// # Guarantees
    ///
    /// Any pointer allocated by this allocator is guaranteed to be aligned to `MIN_ALIGN` even if
    /// the requested layout has a smaller alignment.
    const MIN_ALIGN: usize;

    /// Allocate memory based on `layout`, `flags` and `nid`.
    ///
    /// On success, returns a buffer represented as `NonNull<[u8]>` that satisfies the layout
    /// constraints (i.e. minimum size and alignment as specified by `layout`).
    ///
    /// This function is equivalent to `realloc` when called with `None`.
    ///
    /// # Guarantees
    ///
    /// When the return value is `Ok(ptr)`, then `ptr` is
    /// - valid for reads and writes for `layout.size()` bytes, until it is passed to
    ///   [`Allocator::free`] or [`Allocator::realloc`],
    /// - aligned to `layout.align()`,
    ///
    /// Additionally, `Flags` are honored as documented in
    /// <https://docs.kernel.org/core-api/mm-api.html#mm-api-gfp-flags>.
    fn alloc(layout: Layout, flags: Self::Flags) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: Passing `None` to `realloc` is valid by its safety requirements and asks for a
        // new memory allocation.
        unsafe { Self::realloc(None, layout, Layout::new::<()>(), flags) }
    }

    /// Re-allocate an existing memory allocation to satisfy the requested `layout` and
    /// a specific NUMA node request to allocate the memory for.
    ///
    /// Systems employing a Non Uniform Memory Access (NUMA) architecture contain collections of
    /// hardware resources including processors, memory, and I/O buses, that comprise what is
    /// commonly known as a NUMA node.
    ///
    /// `nid` stands for NUMA id, i. e. NUMA node identifier, which is a non-negative integer
    /// if a node needs to be specified, or [`NumaNode::NO_NODE`] if the caller doesn't care.
    ///
    /// If the requested size is zero, `realloc` behaves equivalent to `free`.
    ///
    /// If the requested size is larger than the size of the existing allocation, a successful call
    /// to `realloc` guarantees that the new or grown buffer has at least `Layout::size` bytes, but
    /// may also be larger.
    ///
    /// If the requested size is smaller than the size of the existing allocation, `realloc` may or
    /// may not shrink the buffer; this is implementation specific to the allocator.
    ///
    /// On allocation failure, the existing buffer, if any, remains valid.
    ///
    /// The buffer is represented as `NonNull<[u8]>`.
    ///
    /// # Safety
    ///
    /// - If `ptr == Some(p)`, then `p` must point to an existing and valid memory allocation
    ///   created by this [`Allocator`]; if `old_layout` is zero-sized `p` does not need to be a
    ///   pointer returned by this [`Allocator`].
    /// - `ptr` is allowed to be `None`; in this case a new memory allocation is created and
    ///   `old_layout` is ignored.
    /// - `old_layout` must match the `Layout` the allocation has been created with.
    ///
    /// # Guarantees
    ///
    /// This function has the same guarantees as [`Allocator::alloc`]. When `ptr == Some(p)`, then
    /// it additionally guarantees that:
    /// - the contents of the memory pointed to by `p` are preserved up to the lesser of the new
    ///   and old size, i.e. `ret_ptr[0..min(layout.size(), old_layout.size())] ==
    ///   p[0..min(layout.size(), old_layout.size())]`.
    /// - when the return value is `Err(AllocError)`, then `ptr` is still valid.
    unsafe fn realloc(
        ptr: Option<NonNull<u8>>,
        layout: Layout,
        old_layout: Layout,
        flags: Self::Flags,
    ) -> Result<NonNull<[u8]>, AllocError>;

    /// Free an existing memory allocation.
    ///
    /// # Safety
    ///
    /// - `ptr` must point to an existing and valid memory allocation created by this [`Allocator`];
    ///   if `old_layout` is zero-sized `p` does not need to be a pointer returned by this
    ///   [`Allocator`].
    /// - `layout` must match the `Layout` the allocation has been created with.
    /// - The memory allocation at `ptr` must never again be read from or written to.
    unsafe fn free(ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: The caller guarantees that `ptr` points at a valid allocation created by this
        // allocator. We are passing a `Layout` with the smallest possible alignment, so it is
        // smaller than or equal to the alignment previously used with this allocation.
        let _ =
            unsafe { Self::realloc(Some(ptr), Layout::new::<()>(), layout, Self::Flags::empty()) };
    }
}

#[cfg(all(test, not(kernel)))]
mod test {
    use super::*;
    use std::alloc::{GlobalAlloc, System};

    /// Returns a properly aligned dangling pointer from the given `layout`.
    fn dangling_from_layout(layout: Layout) -> NonNull<u8> {
        let ptr = layout.align() as *mut u8;

        // SAFETY: `layout.align()` (and hence `ptr`) is guaranteed to be non-zero.
        unsafe { NonNull::new_unchecked(ptr) }
    }

    impl AllocatorFlags for () {
        fn empty() -> Self {
            ()
        }
    }

    unsafe impl Allocator for System {
        type Flags = ();
        const MIN_ALIGN: usize = 32;

        unsafe fn realloc(
            ptr: Option<NonNull<u8>>,
            layout: Layout,
            old_layout: Layout,
            _: Self::Flags,
        ) -> Result<NonNull<[u8]>, AllocError> {
            let out = match ptr {
                Some(ptr) => match old_layout.size() {
                    0 => unsafe { NonNull::new(System.alloc(layout)).ok_or(AllocError) },
                    _ => match layout.size() {
                        0 => unsafe {
                            System.dealloc(ptr.as_ptr(), old_layout);
                            Ok(dangling_from_layout(layout))
                        },
                        size => unsafe {
                            NonNull::new(System.realloc(ptr.as_ptr(), old_layout, size))
                                .ok_or(AllocError)
                        },
                    },
                },
                None => match layout.size() {
                    0 => Ok(dangling_from_layout(layout)),
                    _ => unsafe { NonNull::new(System.alloc(layout)).ok_or(AllocError) },
                },
            }?;

            Ok(NonNull::slice_from_raw_parts(out, layout.size()))
        }
    }
}
