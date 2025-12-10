# kalloc

Linux kernel memory allocation infrastructure, pulled out as an independent
crate.

## TODO

- [ ] Update docs
- [ ] Fix doctests
- [ ] Add Arc implementation
- [x] Fix unit tests
- [x] Gate nightly features behind `cfg(kernel)`
- [x] Gate pin-init use behind `cfg(feature = "pin-init")`
- [x] Fix build and test integration with `pin-init` feature enabled.
- [x] Remove kernel-specific types (`KBox`, `VBox`, `KVBox`, `KVec`, `VVec`, `KVVec`, `VmallocPageIter`)
- [x] Remove kernel-specific traits (`ForeignOwnable`, `AsPageIter`, `InPlaceInit`)
