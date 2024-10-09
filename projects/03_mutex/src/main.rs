use std::{
    cell::UnsafeCell, collections::BTreeMap, future::Future, ops::{Deref, DerefMut}, sync::Arc, task::{Poll, Waker}, time::Duration
};

struct WakerQueue {
    counter: u64,
    unlocked: bool,
    wakers: BTreeMap<u64,Option<Waker>>
}

impl WakerQueue {
    fn new() -> Self {
        Self {
            unlocked: true,
            counter: 0,
            wakers: BTreeMap::new()
        }
    }
}

struct AsyncMutex<T> {
    data: UnsafeCell<T>,
    wakers: std::sync::Mutex<WakerQueue>
}

impl<T> AsyncMutex<T> {
    pub fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
            wakers: std::sync::Mutex::new(WakerQueue::new())
        }
    }

    pub async fn lock(&self) -> AsyncMutexGuard<'_, T> {
        let key = {
            let mut queue = self.wakers.lock().unwrap();

            if queue.unlocked {
                queue.unlocked = false;
                return AsyncMutexGuard { mutex: &self };
            }

            let key = queue.counter;
            queue.counter += 1;
            queue.wakers.insert(key, None);
            
            // bug in Rust, must be scope dropped
            // drop(queue);
            key
        };

        Acquire { mutex: self, key, done: false }.await
    }
}

struct Acquire<'a, T> {
    key: u64,
    mutex: &'a AsyncMutex<T>,
    done: bool
}

impl<'a, T> Future for Acquire<'a, T> {
    type Output = AsyncMutexGuard<'a, T>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut queue = self.mutex.wakers.lock().unwrap();

        let Some(waker) = queue.wakers.get_mut(&self.key) else {
            self.done = true;
            return Poll::Ready(AsyncMutexGuard { mutex: self.mutex });
        };

        *waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl<'a, T> Drop for Acquire<'a, T> {
    fn drop(&mut self) {
        if self.done {
            return ;
        }

        let mut queue = self.mutex.wakers.lock().unwrap();
        if queue.wakers.remove(&self.key).is_none() {
            drop(AsyncMutexGuard { mutex: self.mutex });
        }
    }
}

struct AsyncMutexGuard<'a, T> {
    mutex: &'a AsyncMutex<T>,
}

impl<'a, T> Drop for AsyncMutexGuard<'a, T> {
    fn drop(&mut self) {
        let mut queue = self.mutex.wakers.lock().unwrap();
        if let Some((_, waker)) = queue.wakers.pop_first() {
            if let Some(waker) = waker {
                waker.wake();
            }
        } else {
            queue.unlocked = true;
        }
    }
}

// Safety: We are a mutex :)
unsafe impl<T: Send> Send for AsyncMutex<T> {}
unsafe impl<T: Send> Sync for AsyncMutex<T> {}
unsafe impl<T: Sync + Send> Sync for AsyncMutexGuard<'_, T> {}

impl<T> Deref for AsyncMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.data.get() }
    }
}
impl<T> DerefMut for AsyncMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.data.get() }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mutex = Arc::new(AsyncMutex::new(0));
    // let mutex = Arc::new(tokio::sync::Mutex::new(0));

    let mutex1 = mutex.clone();
    tokio::spawn(async move {
        println!("task 1 acquiring the lock");
        let mut lock = mutex1.lock().await;
        println!("task 1 acquired the lock");

        tokio::time::sleep(Duration::from_millis(2000)).await;

        *lock += 1;
        println!("task 1 releasing the lock");
    });

    let mutex2 = mutex.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1000)).await;

        println!("task 2 acquiring the lock");
        let mut lock = mutex2.lock().await;
        println!("task 2 acquired the lock");

        tokio::time::sleep(Duration::from_millis(2000)).await;

        *lock += 1;
        println!("task 2 releasing the lock");
    });

    tokio::time::sleep(Duration::from_millis(1500)).await;

    println!("task 0 acquiring the lock");
    let val = *mutex.lock().await;
    println!("task 0 acquired the lock");

    println!("Shutting down with value {val}");
}
