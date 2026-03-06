//! # File Descriptor Table
//!
//! Implement a simple file descriptor (fd) table — the core data structure
//! for managing open files in an OS kernel.
//!
//! ## Background
//!
//! In the Linux kernel, each process has an fd table that maps integer fds to kernel file objects.
//! User programs perform read/write/close via fds, and the kernel looks up the corresponding
//! file object through the fd table.
//!
//! ```text
//! fd table:
//!   0 -> Stdin
//!   1 -> Stdout
//!   2 -> Stderr
//!   3 -> File("/etc/passwd")
//!   4 -> (empty)
//!   5 -> Socket(...)
//! ```
//!
//! ## Task
//!
//! Implement the following methods on `FdTable`:
//!
//! - `new()` — create an empty fd table
//! - `alloc(file)` -> `usize` — allocate a new fd, return the fd number
//!   - Prefer reusing the smallest closed fd number
//!   - If no free slot, extend the table
//! - `get(fd)` -> `Option<Arc<dyn File>>` — get the file object for an fd
//! - `close(fd)` -> `bool` — close an fd, return whether it succeeded (false if fd doesn't exist)
//! - `count()` -> `usize` — return the number of currently allocated fds (excluding closed ones)
//!
//! ## Key Concepts
//!
//! - Trait objects: `Arc<dyn File>`
//! - `Vec<Option<T>>` as a sparse table
// Why sparse table? using heap is possible?
//! - fd number reuse strategy (find smallest free slot)
//! - `Arc` reference counting and resource release

use std::sync::Arc;

/// File abstraction trait — all "files" in the kernel (regular files, pipes, sockets) implement this
pub trait File: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> isize;
    fn write(&self, buf: &[u8]) -> isize;
}

struct MinHeap {
    arr: Vec<usize>,
}

impl MinHeap {
    pub fn new() -> MinHeap {
        MinHeap { arr: Vec::new() }
    }

    pub fn left(&self, index: usize) -> usize {
        index * 2 + 1
    }

    pub fn right(&self, index: usize) -> usize {
        index * 2 + 2
    }

    pub fn parent(&self, index: usize) -> Option<usize> {
        if index == 0 {
            None
        } else {
            Some((index - 1) / 2)
        }
    }

    pub fn peek(&self) -> Option<&usize> {
        self.arr.get(0)
    }

    fn swap(&mut self, a: usize, b: usize) {
        self.arr.swap(a, b)
    }

    pub fn len(&self) -> usize {
        self.arr.len()
    }

    fn child_to_swap(&self, i: usize) -> Option<usize> {
        let l = self.left(i);
        let r = self.right(i);
        let n = self.len();

        if l >= n {
            None
        } else if r >= n {
            Some(l)
        } else if self.arr[l] <= self.arr[r] {
            Some(l)
        } else {
            Some(r)
        }
    }

    fn sink_down(&mut self, i: usize) {
        let mut i = i;
        while let Some(mc) = self.child_to_swap(i) {
            if self.arr[mc] > self.arr[i] {
                break;
            }
            self.swap(i, mc);
            i = mc;
        }
    }

    fn float_up(&mut self, i: usize) {
        let mut i = i;
        while let Some(pa) = self.parent(i) {
            if self.arr[pa] <= self.arr[i] {
                break;
            }
            self.swap(i, pa);
            i = pa;
        }
    }

    pub fn push(&mut self, el: usize) {
        self.arr.push(el);
        self.float_up(self.len() - 1);
    }

    pub fn pop(&mut self) -> Option<usize> {
        let len = self.len();
        if len == 0 {
            None
        } else {
            self.swap(0, len - 1);
            let result = self.arr.pop();
            self.sink_down(0);
            result
        }
    }
}

impl From<Vec<usize>> for MinHeap {
    fn from(value: Vec<usize>) -> Self {
        let mut heap = MinHeap { arr: value };
        let mut i = heap.parent(heap.len() - 1).unwrap_or(0);
        loop {
            heap.sink_down(i);
            if i == 0 {
                break;
            }
            i -= 1;
        }
        heap
    }
}

/// File descriptor table
pub struct FdTable {
    // TODO: Design the internal structure
    // Hint: use Vec<Option<Arc<dyn File>>>
    //       the index is the fd number, None means the fd is closed or unallocated
    files: Vec<Option<Arc<dyn File>>>,
    index_heap: MinHeap,
}

impl FdTable {
    /// Create an empty fd table
    pub fn new() -> Self {
        // initial size as 4
        FdTable {
            files: vec![None, None, None, None],
            index_heap: MinHeap::from(vec![0, 1, 2, 3]),
        }
    }

    /// Allocate a new fd, return the fd number.
    ///
    /// Prefers reusing the smallest closed fd number; if no free slot, appends to the end.
    pub fn alloc(&mut self, file: Arc<dyn File>) -> usize {
        if let Some(index) = self.index_heap.peek() {
            let index = index.clone();
            self.files[index] = Some(file);
            self.index_heap.pop();
            index
        } else {
            // extend by one
            let index = self.files.len();
            self.files.push(Some(file));
            index
        }
    }

    /// Get the file object for an fd. Returns None if the fd doesn't exist or is closed.
    pub fn get(&self, fd: usize) -> Option<Arc<dyn File>> {
        if let Some(file) = self.files.get(fd) {
            file.clone()
        } else {
            None
        }
    }

    /// Close an fd. Returns true on success, false if the fd doesn't exist or is already closed.
    pub fn close(&mut self, fd: usize) -> bool {
        if let Some(file) = self.files.get_mut(fd) {
            if file.is_some() {
                *file = None;
                self.index_heap.push(fd);
                return true;
            }
        }
        false
    }

    /// Return the number of currently allocated fds (excluding closed ones)
    pub fn count(&self) -> usize {
        self.files.len()-self.index_heap.len()
    }
}

impl Default for FdTable {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Test File implementation
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockFile {
        id: usize,
        write_log: Mutex<Vec<Vec<u8>>>,
    }

    impl MockFile {
        fn new(id: usize) -> Arc<Self> {
            Arc::new(Self {
                id,
                write_log: Mutex::new(vec![]),
            })
        }
    }

    impl File for MockFile {
        fn read(&self, buf: &mut [u8]) -> isize {
            buf[0] = self.id as u8;
            1
        }
        fn write(&self, buf: &[u8]) -> isize {
            self.write_log.lock().unwrap().push(buf.to_vec());
            buf.len() as isize
        }
    }

    #[test]
    fn test_alloc_basic() {
        let mut table = FdTable::new();
        let fd = table.alloc(MockFile::new(0));
        assert_eq!(fd, 0, "first fd should be 0");
        let fd2 = table.alloc(MockFile::new(1));
        assert_eq!(fd2, 1, "second fd should be 1");
    }

    #[test]
    fn test_get() {
        let mut table = FdTable::new();
        let file = MockFile::new(42);
        let fd = table.alloc(file);
        let got = table.get(fd);
        assert!(got.is_some(), "get should return Some");
        let mut buf = [0u8; 1];
        got.unwrap().read(&mut buf);
        assert_eq!(buf[0], 42);
    }

    #[test]
    fn test_get_invalid() {
        let table = FdTable::new();
        assert!(table.get(0).is_none());
        assert!(table.get(999).is_none());
    }

    #[test]
    fn test_close_and_reuse() {
        let mut table = FdTable::new();
        let fd0 = table.alloc(MockFile::new(0)); // fd=0
        let fd1 = table.alloc(MockFile::new(1)); // fd=1
        let fd2 = table.alloc(MockFile::new(2)); // fd=2

        assert!(table.close(fd1), "closing fd=1 should succeed");
        assert!(
            table.get(fd1).is_none(),
            "get should return None after close"
        );

        // Next allocation should reuse fd=1 (smallest free)
        let fd_new = table.alloc(MockFile::new(99));
        assert_eq!(fd_new, fd1, "should reuse the smallest closed fd");

        let _ = (fd0, fd2);
    }

    #[test]
    fn test_close_invalid() {
        let mut table = FdTable::new();
        assert!(
            !table.close(0),
            "closing non-existent fd should return false"
        );
    }

    #[test]
    fn test_count() {
        let mut table = FdTable::new();
        assert_eq!(table.count(), 0);
        let fd0 = table.alloc(MockFile::new(0));
        let fd1 = table.alloc(MockFile::new(1));
        assert_eq!(table.count(), 2);
        table.close(fd0);
        assert_eq!(table.count(), 1);
        table.close(fd1);
        assert_eq!(table.count(), 0);
    }

    #[test]
    fn test_write_through_fd() {
        let mut table = FdTable::new();
        let file = MockFile::new(0);
        let fd = table.alloc(file);
        let f = table.get(fd).unwrap();
        let n = f.write(b"hello");
        assert_eq!(n, 5);
    }
}
