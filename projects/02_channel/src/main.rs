// Notes
//
// recv() in std channels is the only function that blocks, blocking in ccoperative concurrency
// block all other execution paths.
// 
// ## Context usage
// - contains waker
// ```rust
//     let waker = cx.waker();
//     waker.wake();
// ```

use std::{collections::VecDeque, future::poll_fn, sync::{Arc, Mutex}, task::Poll, time::{Duration, Instant}};


struct Channel<T> {
    buffer: VecDeque<T>,
    waker: Option<std::task::Waker>,
    sender_count: usize,
    receiver_active: bool,
}

struct Sender<T> {
    channel: Arc<Mutex<Channel<T>>>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let mut channel = self.channel.lock().unwrap();
        channel.sender_count += 1;
        Sender{channel: self.channel.clone()}
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut channel = self.channel.lock().unwrap();
        channel.sender_count -= 1;
        if channel.sender_count == 0 && !channel.receiver_active {
            channel.waker = None;
        }
    }
}

impl<T> Sender<T> {
    fn send(&self, value: T) -> Result<(), ()> {
        if let Ok(mut channel) = self.channel.lock() {
            if channel.receiver_active {
                channel.buffer.push_back(value);

                if let Some(waker) = channel.waker.as_ref() {
                    waker.wake_by_ref();
                }
                if let Some(waker) = channel.waker.as_ref() {
                    waker.wake_by_ref();
                }
                return Ok(());
            }

        }
        
        Err(())
    }
}

struct Receiver<T> {
    channel: Arc<Mutex<Channel<T>>>,
}

impl<T> Receiver<T> {
    async fn recv(&mut self) -> Option<T> {
        return poll_fn(|cx| {
            let mut  channel = self.channel.lock().unwrap();
            if channel.sender_count > 0 {
                if let Some(value) = channel.buffer.pop_front() {
                    return Poll::Ready(Some(value));
                }
    
                let waker = cx.waker().clone();
                channel.waker = Some(waker);
    
                Poll::Pending
            } else {
                channel.receiver_active = false;
                Poll::Ready(None)
            }

        }).await;
    }
}

fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(Mutex::new(Channel::<T>{waker: None, buffer: VecDeque::new(), sender_count: 1, receiver_active: true}));
    (Sender{channel: channel.clone()}, Receiver{channel})
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = channel();
    // let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let tx1 = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        tx1.send(1).expect("channel should be open");
    });

    let tx2 = tx;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        tx2.send(2).expect("channel should be open");
    });

    let now = Instant::now();
    while let Some(x) = rx.recv().await {
        println!("Received msg {x:?} after {dur:?}", dur = now.elapsed());
    }
    println!("Shutting down after {dur:?}", dur = now.elapsed());
}
