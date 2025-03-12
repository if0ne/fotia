use std::{marker::PhantomData, num::NonZero};

const _OPT_HANDLE_SIZE: () = if size_of::<Option<Handle<u32>>>() != size_of::<Handle<u32>>() {
    panic!("size of Option<Handle<T>> not equal to size of Handle<T>");
} else {
};

pub struct Handle<T> {
    pub(super) index: u32,
    pub(super) cookie: NonZero<u32>,
    _marker: PhantomData<T>,
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.cookie == other.cookie
    }
}

impl<T> Eq for Handle<T> {}

impl<T> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceHandle")
            .field("index", &self.index)
            .field("gen", &self.cookie())
            .finish()
    }
}

impl<T> Handle<T> {
    pub fn new(index: u32, cookie: u32) -> Self {
        Self {
            index,
            cookie: NonZero::new(cookie).expect("internal bug, wrong cookie"),
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn idx(&self) -> u32 {
        self.index
    }

    #[inline]
    pub fn cookie(&self) -> u32 {
        self.cookie.get()
    }
}

#[derive(Debug)]
pub struct HandleAllocator<T> {
    gens: Vec<u32>,
    free_list: Vec<u32>,
    _marker: PhantomData<T>,
}

impl<T> HandleAllocator<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            gens: Vec::new(),
            free_list: Vec::new(),
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn allocate(&mut self) -> Handle<T> {
        if let Some(idx) = self.free_list.pop() {
            Handle::new(idx, self.gens[idx as usize])
        } else {
            let idx = self.gens.len();
            self.gens.push(1);

            Handle::new(idx as u32, 1)
        }
    }

    #[inline]
    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.gens
            .get(handle.index as usize)
            .is_some_and(|h| *h == handle.cookie.get())
    }

    #[inline]
    pub fn free(&mut self, handle: Handle<T>) {
        if let Some(cookie) = self.gens.get_mut(handle.index as usize) {
            *cookie += 1;
            self.free_list.push(handle.index);
        }
    }
}
