use std::{future::Future, pin::pin, sync::{Arc, Condvar, Mutex}, task::{Context, Poll, Wake, Waker}};

pub fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);

    let runtime = Runtime {
        park: Condvar::new(),
        worker: Mutex::new(Worker {state: WorkerState::Running}),
    };

    let waker = Arc::new(SimpleWaker {runtime: Arc::new(runtime)});
    let waker = Waker::from(waker);

    // loop {
    //     let mut cx = Context::from_waker(&waker);
    //     match f.as_mut().poll(&mut cx) {
    //         Poll::Ready(value) => return value,
    //         Poll::Pending => {
    //             let mut worker = runtime.worker.lock().unwrap();
    //             if worker.state != WorkerState::Ready {
    //                 worker.state = WorkerState::Parked;
    //             }

    //             let _ = runtime.park.wait(worker);
    //         }
    //     }
    // }

    todo!()
}

struct Runtime {
    park: Condvar,
    worker: Mutex<Worker>,
}

#[derive(PartialEq)]
enum WorkerState {
    Ready,
    Running,
    Parked
}

struct Worker {
    state: WorkerState,
}

struct SimpleWaker {
    runtime: Arc<Runtime>,
}

impl Wake for SimpleWaker {
    fn wake(self: Arc<Self>) {
        let mut worker = self.runtime.worker.lock().unwrap();

        if worker.state == WorkerState::Parked {
            self.runtime.park.notify_one();
        }

        worker.state = WorkerState::Ready;
    }
}

fn main() {
    let (tx, rx) = tokio::sync::oneshot::channel();

    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(2));
        tx.send("hello world").unwrap();
    });

    let value = block_on(async { rx.await.unwrap() });

    println!("{value}");
}
