use smallvec::SmallVec;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RwcState {
    #[default]
    WaitForWrite,
    WaitForCopy(u64),
    WaitForRead(u64),
}

// Read, write, copy
#[derive(Debug)]
pub struct RwcRingBuffer<T, const N: usize> {
    buffer: SmallVec<[T; N]>,
    size: usize,
    states: SmallVec<[RwcState; N]>,

    pub head: usize,
    pub tail: usize,
}

impl<T, const N: usize> RwcRingBuffer<T, N> {
    pub fn new(buffer: SmallVec<[T; N]>) -> Self {
        let size = buffer.len();
        Self {
            buffer,
            head: 0,
            tail: 0,
            size,
            states: (0..size).map(|_| Default::default()).collect(),
        }
    }

    #[inline]
    pub fn head_state(&self) -> RwcState {
        self.states[self.head]
    }

    #[inline]
    pub fn tail_state(&self) -> RwcState {
        self.states[self.tail]
    }

    #[inline]
    pub fn advance_head(&mut self) {
        self.head = (self.head + 1) % self.size;
    }

    #[inline]
    pub fn advance_tail(&mut self) {
        self.tail = (self.tail + 1) % self.size;
    }

    #[inline]
    pub fn update_head_state(&mut self, state: RwcState) {
        self.states[self.head] = state;
    }

    #[inline]
    pub fn update_tail_state(&mut self, state: RwcState) {
        self.states[self.tail] = state;
    }

    #[inline]
    pub fn head_data(&self) -> &T {
        &self.buffer[self.head]
    }

    #[inline]
    pub fn tail_data(&self) -> &T {
        &self.buffer[self.tail]
    }

    #[inline]
    pub fn tip_index(&self) -> usize {
        if self.tail == 0 {
            self.size - 1
        } else {
            self.tail - 1
        }
    }

    #[inline]
    pub fn tip_data(&self) -> &T {
        &self.buffer[self.tip_index()]
    }
}
