use std::{future::Future, task::Poll, time::Duration};

use pin_project_lite::pin_project;

#[derive(Debug)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

// The thought process was to
// 
// 1. go from the entire setup containing the state called `SelectState`,
// an enum of all possible operations
// 2. write the skeleton, then remove the repetition and unused cases
// 3. remove the state enum eventually, since it is not needed
//
// We are creating a daisy-changed chain of futures all the way from main down 
// to the select, the depth matters for performance (advanced runtimes use tricks: jump tables...)
pin_project! {
    struct Select<A,B>{
        #[pin] left: A,
        #[pin] right: B,
    }
}

impl<A,B> Select<A,B> {
    pub fn new(left: A, right: B) -> Self {
        Self{left, right}
    }
}

impl<A,B> Future for Select<A,B>
where A: Future,
      B: Future
{
    type Output = Either<A::Output, B::Output>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if let Poll::Ready(left) = this.left.poll(cx) {
            return Poll::Ready(Either::Left(left));
        }

        if let Poll::Ready(right) = this.right.poll(cx) {
            return Poll::Ready(Either::Right(right));
        }

        Poll::Pending
    }
}

async fn select<A: Future, B: Future>(left: A, right: B) -> Either<A::Output, B::Output> {
    // Assignment: REPLACE ME
    // tokio::select! {
    //     left = left => Either::Left(left),
    //     right = right => Either::Right(right),
    // }

    // Final implementation
    Select::new(left, right).await
}

#[tokio::main]
async fn main() {
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::task::spawn(async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let _ = tx.send(());
    });

    let left = tokio::time::sleep(Duration::from_secs(3));
    let right = rx;

    let res = select(left, right).await;

    println!("raced: {:?}", res);
}
