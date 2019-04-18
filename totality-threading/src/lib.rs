extern crate proc_macro;
pub mod killable_thread;

#[macro_export]
macro_rules! create_killable_thread {
    ( $type:ty, $name:literal, {$($head:tt)*}, {$($body:tt)*}, {$($tail:tt)*} ) => {
        {
            let (tx, rx) = std::sync::mpsc::channel();
            killable_thread::KillableThread::new(tx, $name.to_string(), move || -> $type {
                $($head)*
                loop {
                    $($body)*
                    match rx.try_recv() {
                        // Cannot handle messages
                        Ok(_) => panic!("Unexpected input into thread control channel."),
                        // No input means continue
                        Err(::std::sync::mpsc::TryRecvError::Empty) => (),
                        // Outside was dropped, so stop this thread
                        Err(::std::sync::mpsc::TryRecvError::Disconnected) => {
                            info!("Completed");
                            break
                        },
                    };
                }
                $($tail)*
            })
        }
    }
}

#[test]
pub fn run_test() {
    let mut some_fun = vec![0, 1, 2, 3, 4];
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    match create_killable_thread!(String, ":", {
        let mut hello = Vec::new();
        hello.extend(some_fun);
        let mut count = 0;
    }, {
        info!("We're looping!! We've reached this point {:?} times.", count);
        match tx.send(count.clone()) {
            Ok(()) => info!("Successfully sent integer!"),
            Err(e) => info!("Unexpected behavior on iteration {:?}!", count),
        }
        count += 1;
        if (count == 10) {
            drop(tx);
            break
        }
    }, {
        info!("Welp, guess we're through here. Here's the vec, but the way: {:?}", hello);
        info!("We're early terminating to check if break works as expected.");
        "A random string is your reward!".to_string()
    }) {
        Ok(kt) => {
            for i in 0..10 {
                match rx.recv() {
                    Ok(r) => assert_eq!(r, i, "Oh crap, we received things in a weird way!"),
                    Err(e) => panic!("Not like this!!!! We've failed due to {:?}!", e),
                }
            }
            match rx.try_recv() {
                Ok(v) => panic!("We received a value ({:?}) after it should've been dropped...", v),
                Err(std::sync::mpsc::TryRecvError::Empty) => panic!("The channel should've dropped by now!"),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => info!("Successfully disconnected from the thread."),
            }
            match kt.finish() {
                Some(res) =>{
                        match res {
                        Ok(s) => assert_eq!("A random string is your reward!", s, "We should have gotten this string back..."),
                        Err(e) => panic!("Oh shit! We failed somewhere to join! Here's the error: {:?}", e),
                    }
                },
                None => panic!("We should have gotten something back!"),
            }
        },
        Err(e) => panic!("Unexpected error!"),
    }
    info!("No error!");
}
