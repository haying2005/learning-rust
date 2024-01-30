
use std::cell::UnsafeCell;
use std::pin::Pin;
use std::mem;
use std::sync::{Arc, Mutex, mpsc::{self, Receiver, Sender}};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Instant, Duration};
use futures::Future;
use futures::task::{ArcWake, waker_ref};
use tokio::sync::Notify;

pub struct Delay {
    pub when: Instant,
    waker: Option<Arc<Mutex<Waker>>>
}
impl Delay {
    pub fn new(when: Instant) -> Self {
        Delay {
            when,
            waker: None,
        }
    }
}

impl Future for Delay {
    type Output = &'static str;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<&'static str>
    {
        if Instant::now() >= self.when {
            Poll::Ready("done")
        } else {
            if let Some(waker)  = &self.waker {
                // 更新为新的waker,因为future可以在不同Task之间转移，因此当前poll的
                let mut waker = waker.lock().unwrap();
                
                // 返回true表示两个waker将唤醒同一task(内部的RawWaker是同一个)，此时便无需重新设置waker，减少不必要的clone
                // note: 返回false不一定确保两个waker唤醒的不是同一个task，只是尽量减少不必要的clone，无法做到百分百
                if !waker.will_wake(cx.waker()) {
                    *waker = cx.waker().clone();
                }
            } else {
                let waker = Arc::new(Mutex::new(cx.waker().clone()));
                self.waker = Some(waker.clone());
                let when = self.when;
                thread::spawn(move || {
                    let now = Instant::now();
                    if now < when {
                        thread::sleep(when - now)
                    }
                    let waker = waker.lock().unwrap();
                    waker.wake_by_ref();
                });
            }
            
            Poll::Pending
        }
    }
}


// 更简单的实现future的方式：通过async函数
// 通常这种方式更简单常用，无需深入底层，无需考虑Waker
pub async fn delay(when: Duration) {
    let when = Instant::now() + when;
    let notify = Arc::new(Notify::new());
    let notify_cloned = notify.clone();
    thread::spawn(move || {
        let now = Instant::now();
        if now < when {
            thread::sleep(when - now);
        }
        notify_cloned.notify_one();
    });
    notify.notified().await;
}
pub struct MiniTokio {
    // scheduled paired with sender
    scheduled: Receiver<Arc<Task>>,
    sender: Sender<Arc<Task>>,
}

// ArcWake特征要求Task: Send+Sync, 因为rust要求waker必须是线程安全的(Sync)
// Mutex提供了Sync特征(Mutex<T> is Sync, if T is Send)
struct Task {
    // mutex提供futue的可变引用(内部可变性)，用于对future进行poll调用
    // 实际无需使用mutex，因为不会存在竞争，实际上一个任务只会在一个线程中执行，例如可以采用UnsafeCell替代
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    excutor: Sender<Arc<Task>>,
}

impl Task {
    // wake时调用此方法，把自身重新扔回scheduled队列中等待再次被poll
    fn schedule(self: &Arc<Self>) {
        self.excutor.send(self.clone()).unwrap();
    }
    fn poll(self: Arc<Self>) {
        let waker = waker_ref(&self); // waker_ref调用的前提是 self: ArcWake
        let mut cx = Context::from_waker(&waker);
        // try_lock如果锁被占,则返回Err; 此处不会存在竞争, 所以可以这样用
        let mut future = self.future.try_lock().unwrap();
        let _ = future.as_mut().poll(&mut cx);
    }
    fn spawn<F>(fut: F, sender: &mpsc::Sender<Arc<Task>>)
    where F: Future<Output = ()> + Send + 'static
    {
        let task = Task {
            future: Mutex::new(Box::pin(fut)),
            excutor: sender.clone(),
        };
        // 把自身扔到scheduled队列中进行第一次poll
        sender.send(Arc::new(task)).unwrap()
    }
}

// 实现了ArcWake特征可以让Task转换为[`Waker`] objects, 然后再通过Waker构建[`Context`]去poll一个future对象
impl ArcWake for Task {
    // 此方法由future内部触发
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // 把自身重新扔回scheduled队列中等待再次被poll
        arc_self.schedule();
    }
}

impl MiniTokio {
    pub fn new() -> Self {
        let (sender, recv) = mpsc::channel();
        MiniTokio { scheduled: recv, sender }
    }
    pub fn run(&self) {
        while let Ok(task) = self.scheduled.recv() {
            task.poll();
        }
    }
    pub fn spawn<F>(&self, fut: F)
    where F: Future<Output = ()> + Send + 'static
    {
        Task::spawn(fut, &self.sender);
    }
}

use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let (tx1, rx1) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();

    tokio::spawn(async {
        let _ = tx1.send("one");
    });

    tokio::spawn(async {
        let _ = tx2.send("two");
    });

    tokio::select! {
        val = rx1 => {
            println!("rx1 completed first with {:?}", val);
        }
        val = rx2 => {
            println!("rx2 completed first with {:?}", val);
        }
    }
}



#[cfg(test)]
mod test {
    use std::cell::RefCell;
    use std::process;
    use std::rc::Rc;
    use std::time::{Instant, Duration};

    use crate::Excutor2::delay;

    use super::Delay;
    use super::MiniTokio;



    #[test]
    fn test() {
        let mini_tokio = MiniTokio::new();
        mini_tokio.spawn(async {
            
            let when = Instant::now() + Duration::from_millis(10);
            let future = Delay::new(when);

            let out = future.await;
            assert_eq!(out, "done");
            println!("done...");
            process::exit(0);
        });

        mini_tokio.run();
    }


    #[test]
    fn test1() {
        let mini_tokio = MiniTokio::new();
        mini_tokio.spawn(async {
            let future = delay(Duration::from_millis(1000));
            future.await;
            println!("done 1...");
            process::exit(0);
        });

        mini_tokio.run();
    }
    
}