use std::{time::{Instant, Duration}, process};

use hello_world::Excutor2::{MiniTokio, Delay};

fn main() {
    let mut mini_tokio = MiniTokio::new();

    mini_tokio.spawn(async {
        let when = Instant::now() + Duration::from_millis(1000);
        let future = Delay::new(when);

        let out = future.await;
        assert_eq!(out, "done");

        let when = Instant::now() + Duration::from_millis(1000);
        let future = Delay::new(when);
        future.await;

        let when = Instant::now() + Duration::from_millis(1000);
        let future = Delay::new(when);
        future.await;

        let when = Instant::now() + Duration::from_millis(1000);
        let future = Delay::new(when);
        future.await;

        let when = Instant::now() + Duration::from_millis(1000);
        let future = Delay::new(when);
        future.await;

        println!("finish...");


        process::exit(0);
    });

    mini_tokio.run();
}