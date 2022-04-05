use crate::{Update, ProgressTracker, StageId, ProgressReport};
use crossbeam_channel::{bounded, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};


pub fn print_reporter(interval: Duration) -> impl Update {
    let (tx, rx) = bounded(100);
    
    std::thread::spawn(move || {

        let mut tracker = ProgressTracker::default();
        let mut last_time;

        'outer: loop {
            tracker.print();
            // wait for at least one update
            match rx.recv() {
                Ok((id, update)) => tracker.update(id, update),
                Err(_) => break,
            }
            last_time = Instant::now();

            // process more updates until the interval elapsed
            let mut delta = Duration::ZERO;
            loop {
                match rx.recv_timeout(delta) {
                    Ok((id, update)) => {
                        tracker.update(id, update);
                    }
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => break 'outer,
                }
                delta = last_time.elapsed();
                if delta >= interval {
                    break;
                }
            }
        }
    });

    struct Updater {
        tx: Sender<(StageId, ProgressReport)>
    }
    impl Update for Updater {
        fn update(&self, id: StageId, progress: ProgressReport) {
            self.tx.send((id, progress)).unwrap();
        }
    }
    Updater { tx }
}
