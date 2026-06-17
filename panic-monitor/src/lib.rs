use log::*;

#[derive(Debug)]
struct PanicMonitor {
    panic_func: Option<fn()>,
    grace_func: Option<fn()>,
}

impl PanicMonitor {
    fn new(panic_func: Option<fn()>, grace_func: Option<fn()>) -> Self {
        Self {
            panic_func,
            grace_func,
        }
    }
}

impl Drop for PanicMonitor {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(panic_func) = &self.panic_func {
                (panic_func)();
            }
            error!(
                " #### Panicking thread: {:?} ####",
                std::thread::current().name().unwrap_or("unnamed")
            );
            std::process::exit(1);
        } else {
            if let Some(grace_func) = &self.grace_func {
                (grace_func)();
            }
            warn!(
                " #### Graceful exit thread: {:?} ####",
                std::thread::current().name().unwrap_or("unnamed")
            );
        }
    }
}

/// f1은 loop가 포함된 함수
/// f2는 스레드가 종료될 때 실행될 함수
pub fn exit(loop_func: fn(), panic_func: Option<fn()>, grace_func: Option<fn()>) {
    // _만 사용한다면 즉시 드랍된다..
    let _monitor = PanicMonitor::new(panic_func, grace_func);
    loop_func()
}
