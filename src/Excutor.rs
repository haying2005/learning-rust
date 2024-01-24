use std::{
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex,
    },
    task::Context, cell::UnsafeCell, pin::Pin, rc::Rc,
};

use futures::{
    future::BoxFuture,
    task::{waker_ref, ArcWake},
    Future, FutureExt,
};

pub struct Excutor {
    ready_queue: Receiver<Arc<Task>>,
}

struct Task {
    // pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>
    // a pin and boxed future
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    // a sender end can send arc cloned self
    task_sender: SyncSender<Arc<Task>>,
}

impl Drop for Task {
    fn drop(&mut self) {
        println!("task drop")
    }
}

pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

pub fn new_excutor_and_spwner() -> (Excutor, Spawner) {
    let (task_sender, ready_queue) = sync_channel(10000);
    (Excutor { ready_queue }, Spawner { task_sender })
}

impl Spawner {
    // 创建新任务，并将其扔进队列进行第一次`poll`调用
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        // pin/box a future
        let future: Pin<Box<dyn Future<Output = ()> + Send>> = future.boxed();
        let task = Task {
            /// so we need to use the `Mutex` to prove thread-safety. A production
            /// executor would not need this, and could use `UnsafeCell` instead.
            future: Mutex::new(Some(future)),
            // 队列发送端clone，让task能够把自身扔进队列
            task_sender: self.task_sender.clone(),
        };
        self.task_sender
            .send(Arc::new(task))
            .expect("too many tasks queued");
    }
}

/// the trait make a Task[`Arc<impl ArcWake>`] can get a waker [`WakerRef`] by call function [`waker_ref`]
impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        println!("wake called");
        let cloned = arc_self.clone();
        arc_self
            .task_sender
            .send(cloned)
            .expect("too many tasks queued");
    }
}

impl Excutor {
    pub fn run(&self) {
        // 不停的从task队列中获取状态为[`ready`]的task进行[`poll`]调用
        while let Ok(task) = self.ready_queue.recv() {
            println!("recv a task, ref cnt {}", Arc::strong_count(&task));
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                let waker = waker_ref(&task); // 智能指针，deref-> &Waker
                let context = &mut Context::from_waker(&waker);

                if future.as_mut().poll(context).is_pending() {
                    // 没执行完，将future再放回task中去
                    *future_slot = Some(future);
                }
            }
        }
        println!("channel closed...")
    }
}