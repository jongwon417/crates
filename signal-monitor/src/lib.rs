use std::{
    thread::{sleep, Builder},
    time::Duration,
};

use log::error;
use signal_hook::iterator::Signals;
use signal_hook::{consts::*, low_level};

const SIGS: [i32; 2] = [SIGINT, SIGTERM];

fn term_process() {
    error!("Process terminated");
    std::process::exit(0);
}

pub fn signal_monitor<A>(action: A)
where
    A: Fn() + Send + Copy + 'static,
{
    let mut sigs = Signals::new(&SIGS).unwrap();
    let catch_sig = move || loop {
        for sig in sigs.forever() {
            error!("got {}({})", low_level::signal_name(sig).unwrap_or("Unknown"), sig);
            match sig {
                SIGINT => {
                    action();
                    term_process();
                }
                // SIGKILL은 catch 되지 않음
                // SIGKILL => {
                //     warn!("got SIGKILL({})", sig);
                //     action();
                //     term_process();
                // },
                SIGTERM => {
                    action();
                    term_process();
                }
                _ => {
                    error!("Unhandled signal");
                }
            }
        }
        sleep(Duration::from_secs(1));
    };

    Builder::new()
        .name("signal_monitor".to_string())
        .spawn(catch_sig)
        .unwrap();
}
