use bytes::Bytes;
use std::collections::hash_map::{DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::net::{TcpListener, TcpStream};

use mini_redis::{Command, Connection, Frame};

// type Db = Arc<Mutex<HashMap<String, Bytes>>>;

// 分片
type ShardedDb = Arc<Vec<Mutex<HashMap<String, Bytes>>>>;

fn new_sharded_db() -> ShardedDb {
    let num_sharded = 5;
    let mut v = Vec::with_capacity(num_sharded);
    for _ in 0..num_sharded {
        v.push(Mutex::new(HashMap::new()));
    }
    Arc::new(v)
}
#[tokio::main]
async fn main() {
    let port = 6377;
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .unwrap();
    // 根据经验来说，只要锁竞争比较弱，且不会跨await(across await)持有锁，则可以使用同步mutex(std mutex);
    // 如果必须跨await持有锁，则只能使用tokio::sync::Mutex(其内部也是使用同步mutex, 因此也会阻塞线程);
    // 尽量避免使用tokio::sync::Mutex(异步mutex), 因为其性能损耗相对于常规mutex来说比较大
    let db = new_sharded_db();
    println!("listening port {}", port);

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let db = db.clone();

        println!("new connection...");
        tokio::spawn(async {
            process(socket, db).await;
        });
    }
}

fn hash<T: Hash>(v: T) -> u64 {
    let mut hasher = DefaultHasher::new();
    v.hash(&mut hasher);
    hasher.finish()
}

async fn process(socket: TcpStream, db: ShardedDb) {
    println!("processing...");

    let mut conn = Connection::new(socket);

    while let Some(frame) = conn.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Command::Get(cmd) => {
                let shard = &db[hash(cmd.key()) as usize % db.len()];
                let db = shard.lock().unwrap();
                if let Some(x) = db.get(cmd.key()) {
                    Frame::Bulk(x.clone())
                } else {
                    Frame::Null
                }
            }
            Command::Set(cmd) => {
                let shard = &db[hash(cmd.key()) as usize % db.len()];
                let mut db = shard.lock().unwrap();
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            cmd => panic!("unimplemented {:?}", cmd),
        };

        conn.write_frame(&response).await.unwrap();
    }
}

async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    {
        let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
        *lock += 1;
    }

    do_something_async().await;
}

async fn do_something_async() {}
