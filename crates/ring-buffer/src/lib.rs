#![no_std]
#![no_main]

use core::cmp;

#[derive(Default, Copy, Clone, PartialEq, Eq)]
enum RingBufferState {
    #[default]
    Empty,
    Full,
    Normal,
}

pub struct RingBuffer<const N: usize> {
    arr: [u8; N],
    // NOTE: When and only when `head` equals `tail`, `state` can only be `Full` or `Empty`
    head: usize,
    tail: usize,
    state: RingBufferState,
}

impl<const N: usize> RingBuffer<N> {
    pub fn new() -> Self {
        Self {
            arr: [0; N],
            head: 0,
            tail: 0,
            state: RingBufferState::Empty,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.state == RingBufferState::Empty
    }

    pub fn is_full(&self) -> bool {
        self.state == RingBufferState::Full
    }

    /// Read as much as possible to fill `buf`.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        if self.state == RingBufferState::Empty || buf.is_empty() {
            return 0;
        }

        let ret_len;
        if self.head < self.tail {
            ret_len = cmp::min(self.tail - self.head, buf.len());
            buf[..ret_len].copy_from_slice(&self.arr[self.head..self.head + ret_len]);
        } else {
            // also handles full
            ret_len = cmp::min(N - self.head + self.tail, buf.len());
            if ret_len <= (N - self.head) {
                buf[..ret_len].copy_from_slice(&self.arr[self.head..self.head + ret_len]);
            } else {
                let right_len = N - self.head;
                buf[..right_len].copy_from_slice(&self.arr[self.head..]);
                buf[right_len..ret_len].copy_from_slice(&self.arr[..(ret_len - right_len)]);
            }
        }
        self.head = (self.head + ret_len) % N;

        if self.head == self.tail {
            self.state = RingBufferState::Empty;
        } else {
            self.state = RingBufferState::Normal;
        }

        ret_len
    }

    /// Write as much as possible to fill the ring buffer.
    pub fn write(&mut self, buf: &[u8]) -> usize {
        if self.state == RingBufferState::Full || buf.is_empty() {
            return 0;
        }

        let ret_len;
        if self.head <= self.tail {
            // also handles empty
            ret_len = cmp::min(N - (self.tail - self.head), buf.len());
            if ret_len <= (N - self.tail) {
                self.arr[self.tail..self.tail + ret_len].copy_from_slice(&buf[..ret_len]);
            } else {
                self.arr[self.tail..].copy_from_slice(&buf[..N - self.tail]);
                self.arr[..(ret_len - (N - self.tail))]
                    .copy_from_slice(&buf[N - self.tail..ret_len]);
            }
        } else {
            ret_len = cmp::min(self.head - self.tail, buf.len());
            self.arr[self.tail..self.tail + ret_len].copy_from_slice(&buf[..ret_len]);
        }
        self.tail = (self.tail + ret_len) % N;

        if self.head == self.tail {
            self.state = RingBufferState::Full;
        } else {
            self.state = RingBufferState::Normal;
        }

        ret_len
    }

    pub fn dequeue(&mut self) -> Option<u8> {
        if self.is_empty() {
            return None;
        }

        let c = self.arr[self.head];
        self.head = (self.head + 1) % N;
        if self.head == self.tail {
            self.state = RingBufferState::Empty;
        } else {
            self.state = RingBufferState::Normal;
        }
        Some(c)
    }

    pub fn enqueue(&mut self, byte: u8) -> Option<()> {
        if self.is_full() {
            return None;
        }

        self.arr[self.tail] = byte;
        self.tail = (self.tail + 1) % N;
        if self.head == self.tail {
            self.state = RingBufferState::Full;
        } else {
            self.state = RingBufferState::Normal;
        }
        Some(())
    }
}
