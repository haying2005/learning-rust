use std::{
    sync::{Mutex, Arc}, 
    task::{Waker, Poll, Context}, 
    future::Future, pin::Pin, time::Duration, thread,
};

use futures::join;
use hello_world::TimerFuture::TimerFuture;
use hello_world::Excutor;
fn main() {
    let (excutor, spawner ) = Excutor::new_excutor_and_spwner();
    let f1 = TimerFuture::new(Duration::from_secs(5));
    let f2 = TimerFuture::new(Duration::from_secs(1));
    let s = "".to_string();
    let fut = async move {
        // println!("howdy!");
        join!(f1, f2);
        println!("{}", s);
        // println!("done");
    };
    spawner.spawn(fut);
    // spawner.spawn(async move {
    //     // println!("howdy!");
    //     join!(f1, f2);
    //     println!("{}", s);
    //     // println!("done");
    // });
    // drop(spawner);
    excutor.run();
}

