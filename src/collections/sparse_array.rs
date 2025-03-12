use std::{marker::PhantomData, mem::MaybeUninit, num::NonZero};

use super::handle::Handle;

#[derive(Clone, Copy, Debug)]
struct SparseEntry {
    dense_index: usize,
    dense_cookie: NonZero<u32>,
}

#[derive(Debug)]
pub struct SparseArray<U, W> {
    sparse: Vec<Option<SparseEntry>>,
    dense: Vec<MaybeUninit<W>>,
    dense_to_sparse: Vec<usize>,
    _marker: PhantomData<U>,
}

impl<U, W> Default for SparseArray<U, W> {
    fn default() -> Self {
        Self::new(128)
    }
}

impl<U, W> SparseArray<U, W> {
    pub fn new(capacity: usize) -> Self {
        Self {
            sparse: vec![None; capacity],
            dense: Vec::new(),
            dense_to_sparse: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn contains(&self, handle: Handle<U>) -> bool {
        self.sparse
            .get(handle.index as usize)
            .is_some_and(|h| h.is_some_and(|h| h.dense_cookie == handle.cookie))
    }

    pub fn set(&mut self, handle: Handle<U>, value: W) {
        if self.sparse.len() <= handle.index as usize {
            self.sparse.resize((handle.index + 1) as usize, None);
        }

        if let Some(ref mut h) = self.sparse[handle.index as usize] {
            unsafe {
                self.dense[h.dense_index as usize].assume_init_drop();
            }
            h.dense_cookie = handle.cookie;
            self.dense[h.dense_index as usize] = MaybeUninit::new(value);
        } else {
            let pos = self.dense.len();
            self.dense.push(MaybeUninit::new(value));
            self.dense_to_sparse.push(handle.index as usize);
            self.sparse[handle.index as usize] = Some(SparseEntry {
                dense_index: pos,
                dense_cookie: handle.cookie,
            });
        }
    }

    pub fn get(&self, handle: Handle<U>) -> Option<&W> {
        self.sparse.get(handle.index as usize).and_then(|h| {
            if let Some(h) = h {
                if h.dense_cookie == handle.cookie {
                    unsafe { Some(self.dense[h.dense_index as usize].assume_init_ref()) }
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    pub fn get_mut(&mut self, handle: Handle<U>) -> Option<&mut W> {
        self.sparse.get(handle.index as usize).and_then(|h| {
            if let Some(h) = h {
                if h.dense_cookie == handle.cookie {
                    unsafe { Some(self.dense[h.dense_index as usize].assume_init_mut()) }
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    pub fn remove(&mut self, handle: Handle<U>) -> Option<W> {
        let Some(Some(SparseEntry {
            dense_index,
            dense_cookie,
        })) = self.sparse.get(handle.index as usize).cloned()
        else {
            return None;
        };

        if dense_cookie != handle.cookie {
            return None;
        }

        let value = std::mem::replace(&mut self.dense[dense_index], MaybeUninit::uninit());
        let value = unsafe { value.assume_init() };

        self.dense.swap_remove(dense_index);
        self.dense_to_sparse.swap_remove(dense_index);
        self.sparse[handle.index as usize] = None;

        let Some(Some(handle)) = self.sparse.get_mut(self.dense_to_sparse[dense_index]) else {
            return Some(value);
        };

        handle.dense_index = dense_index;

        Some(value)
    }
}

impl<U, W> Drop for SparseArray<U, W> {
    fn drop(&mut self) {
        for handle in self.sparse.iter_mut() {
            if let Some(handle) = handle.take() {
                unsafe {
                    self.dense[handle.dense_index as usize].assume_init_drop();
                }
            }
        }
    }
}
