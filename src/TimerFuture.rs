use std::{
    sync::{Mutex, Arc}, 
    task::{Waker, Poll, Context}, 
    future::Future, pin::Pin, time::Duration, thread, marker::PhantomPinned,
};
pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>
}

struct SharedState {
    completed: bool,
    // 时间到时调用waker的wake方法
    waker: Option<Waker>,
}

impl TimerFuture {
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        let shared_state_cloned = shared_state.clone();

        thread::spawn(move || {
            thread::sleep(duration);
            println!("时间到");
            // 时间到
            let mut shared_state = shared_state_cloned.lock().unwrap();
            shared_state.completed = true;
            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            }
        });
        TimerFuture { shared_state }
    }
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.completed {
            println!("a future ready!");
            Poll::Ready(())
        } else {
            // TimerFuture有可能会在不同的task之间移动，所以必须每次都执行以下操作：
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}