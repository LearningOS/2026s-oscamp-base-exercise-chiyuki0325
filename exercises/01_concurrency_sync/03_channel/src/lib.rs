//! # Channel Communication
//!
//! In this exercise, you will use `std::sync::mpsc` channels to pass messages between threads.
//!
//! ## Concepts
//! - `mpsc::channel()` creates a multiple producer, single consumer channel
//! - `Sender::send()` sends a message
//! - `Receiver::recv()` receives a message
//! - Multiple producers can be created via `Sender::clone()`

use std::sync::mpsc;
use std::thread;
use std::sync::{Arc, Mutex};

/// Create a producer thread that sends each element from items into the channel.
/// The main thread receives all messages and returns them.
pub fn simple_send_recv(items: Vec<String>) -> Vec<String> {
    // Create channel
    // Spawn thread to send each element in items
    // In main thread, receive all messages and collect into Vec
    // Hint: When all Senders are dropped, recv() returns Err
    let (tx, rx) = mpsc::channel::<String>();

    let handle = thread::spawn(move || {
        items.into_iter().for_each(|it| tx.send(it).unwrap());
    });

    let mut result = Vec::<String>::new();

    while let Ok(el) = rx.recv() {
        result.push(el);
    }

    handle.join().unwrap();

    result
}

/// Create `n_producers` producer threads, each sending a message in format `"msg from {id}"`.
/// Collect all messages, sort them lexicographically, and return.
///
/// Hint: Use `tx.clone()` to create multiple senders. Note that the original tx must also be dropped.
pub fn multi_producer(n_producers: usize) -> Vec<String> {
    // Create channel
    // Clone a sender for each producer
    // Remember to drop the original sender, otherwise receiver won't finish
    // Collect all messages and sort

    let counter = Arc::new(Mutex::<usize>::new(0));
    let (tx, rx) = mpsc::channel::<String>();

    let spawner = |_| {
      let tx = tx.clone();
      let counter = counter.clone();
      thread::spawn(move || {
        let mut guard=counter.lock().unwrap();
        let msg = format!("msg from {}", guard);
        *guard+=1;
        drop(guard);

        _ = tx.send(msg);
      })
    };

    let handles: Vec<thread::JoinHandle<()>>  = (0..n_producers).map(spawner).collect();
    drop(tx);

    let mut results = Vec::<String>::new();

    while let Ok(result) = rx.recv() {
        results.push(result);
    }

    handles.into_iter().for_each(|h| h.join().unwrap());

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_send_recv() {
        let items = vec!["hello".into(), "world".into(), "rust".into()];
        let result = simple_send_recv(items.clone());
        assert_eq!(result, items);
    }

    #[test]
    fn test_simple_empty() {
        let result = simple_send_recv(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_multi_producer() {
        let result = multi_producer(3);
        assert_eq!(
            result,
            vec![
                "msg from 0".to_string(),
                "msg from 1".to_string(),
                "msg from 2".to_string(),
            ]
        );
    }

    #[test]
    fn test_multi_producer_single() {
        let result = multi_producer(1);
        assert_eq!(result, vec!["msg from 0".to_string()]);
    }
}
