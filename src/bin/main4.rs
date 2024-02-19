use tokio::runtime::{Builder, Handle};
use tokio::time::{sleep, Duration};
use tracing::{debug, info, instrument};

fn main() {
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let runtime = Builder::new_current_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    let mut handles = Vec::with_capacity(10);
    for i in 0..10 {
        handles.push(runtime.spawn(my_bg_task(i)));
    }

    // Do something time-consuming while the background tasks execute.
    std::thread::sleep(Duration::from_millis(750));
    let d = Duration::from_secs(12);
    let st = "Finished time-consuming task.";
    info!(ssss= 123, ?d, st);

    // Wait for all of them to complete.
    for handle in handles {
        // The `spawn` method returns a `JoinHandle`. A `JoinHandle` is
        // a future, so we can wait for it using `block_on`.
        runtime.block_on(handle).unwrap();
    }
}

#[instrument]
async fn my_bg_task(i: u64) {
    // By subtracting, the tasks with larger values of i sleep for a
    // shorter duration.
    let millis = 1000 - 50 * i;
    info!("Task {} sleeping for {} ms.", i, millis);

    sleep(Duration::from_millis(millis)).await;

    info!("Task {} stopping.", i);
    info!{%i, "sdfsdf"};
    info!(?i, "sdfsdf");
    info!(?i, %i, i, "sdfsdf");
}