use tokio::select;
use rdpdk::lib::eal::{Eal, LcoreWorker};
use tokio::time::{sleep, Duration};


#[tokio::main]
async fn main() {
    let mut eal = Eal::init().await.expect("Failed to init EAL");

    let lcores: Vec<_> = eal.lcores_iter_mut().copied().collect();

    for lcore in lcores.clone() {
        eal.rdpdk_set_worker(lcore, Box::new(HelloWorld)).await
    }

    select! {
        _ = sleep(Duration::from_secs(2)) => {
            println!("Timeout reached");
        }
    }
    
    for lcore in lcores {
        eal.rdpdk_set_worker(lcore, Box::new(Hello)).await
    }
}

#[derive(Debug)]
struct HelloWorld;

impl LcoreWorker for HelloWorld {
    fn run(&mut self, core_id: u32) {
        println!("Hello World from core {}", core_id);
    }
}

#[derive(Debug)]
struct Hello;

impl LcoreWorker for Hello {
    fn run(&mut self, core_id: u32) {
        println!("Hello from core {} World", core_id);
    }
}
