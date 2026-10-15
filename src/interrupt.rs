//! Ctrl+C: erstes Drücken bricht die laufende Antwort sauber ab (Chat wird
//! gespeichert), zweites Drücken beendet sofort.

use std::sync::atomic::{AtomicBool, Ordering};

static REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn install() {
    let _ = ctrlc::set_handler(|| {
        if REQUESTED.swap(true, Ordering::SeqCst) {
            eprintln!();
            std::process::exit(130);
        }
    });
}

pub fn requested() -> bool {
    REQUESTED.load(Ordering::SeqCst)
}
