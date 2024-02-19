use std::{sync::Mutex, thread};

fn main() {
    let s = "xx".to_owned();
    let m = Mutex::new("xxxx".to_owned());
    // 以下代码编译不通过，除非加move
    // thread::spawn(|| {
    //     println!("{}", s);
    //     m.lock();
    // });
    
    thread::scope(|scope| {
        scope.spawn(|| {
            println!("{}", s);
            let guard = m.lock().unwrap();
            println!("{}", &*guard);
        });
    });
}