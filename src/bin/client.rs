use std::time::Duration;

use bytes::Bytes;
use tokio::{sync::{ mpsc, oneshot }, time::sleep};
use mini_redis::{ Connection, Frame, client };

#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        value: Bytes,
        resp: Responder<()>,
    }
}

type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<Command>(32);
    let tx1 = tx.clone();

    tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(Command::Get { key: "hello".to_owned(), resp: resp_tx }).await.unwrap();
        let res = resp_rx.await.unwrap();
        println!("{:?}", res);
    });
    
    tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        tx1.send(Command::Set { key: "hello".to_owned(), value: "world!".into(), resp: resp_tx }).await.unwrap();
        let res = resp_rx.await.unwrap();
        println!("{:?}", res);
    });

    let j = tokio::spawn(async move {
        let mut client = client::connect("127.0.0.1:6377").await.unwrap();
        while let Some(cmd) = rx.recv().await {
            // sleep(Duration::from_millis(100)).await;
            match cmd {
                Command::Get { key , resp} => {
                    println!("Recv Get Comamnd with key {}", key);
                    let res = client.get(&key).await;
                    resp.send(res).unwrap();
                },
                Command::Set { key, value , resp} => {
                    println!("Recv Set Command with key {key} and value {:?}", value);
                    let res = client.set(&key, value).await;
                    resp.send(res).unwrap();
                }
            }
        }
    
        
        // let _ = client.set("hello", "world".into()).await;
        // let handle = tokio::spawn(async move {
        //     let res: Option<Bytes> = client.get("hello").await.unwrap();
        //     println!("{:?}", res);
        // });
        // handle.await.unwrap();
    });

    j.await.unwrap();


    
}