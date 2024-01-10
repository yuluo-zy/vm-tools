use std::intrinsics::fadd_fast;
use std::time::{Duration, Instant};
use crate::event_context::EventLoopContext;

pub struct Timer {
    func: Box<dyn Fn()>,
    expire_time: Instant,
}

impl Timer {
    pub fn new(func: Box<dyn Fn()>, delay: Duration) -> Self {
        Timer {
            func,
            expire_time: Instant::now() + delay,
        }
    }
}

/**
这是一个名为`ClockState`的公共结构体，它记录了虚拟机(VM)的计时器状态。该结构体有四个成员变量：

1. `enable`: 这是一个布尔值，可能表示计时器是否启用。如果`enable`为`true`，则计时器启用；如果为`false`，则计时器禁用。

2. `offset`: 这是一个`Instant`类型的值，表示从某个特定点（如虚拟机启动时刻）到现在的时间间隔。

3. `paused`: 这是一个`Duration`类型的值，可能表示计时器在虚拟机运行期间的暂停时间。

4. `elapsed`: 这也是一个`Duration`类型的值，可能表示从虚拟机启动到现在的已经经过的时间，或者某个特定计时任务的执行时间。

请注意，以上解释是假设性的，实际含义可能会根据代码的上下文和项目的具体需求有所不同。
**/
pub struct ClockState {
    enable: bool,
    offset: Instant,
    paused: Duration,
    elapsed: Duration,
}

impl Default for ClockState {
    fn default() -> Self {
        Self {
            enable: false,
            offset: Instant::now(),
            paused: Duration::default(),
            elapsed: Duration::default()
        }
    }
}

impl ClockState {
    pub fn get_virtual_clock(&mut self) -> Duration {
        let mut time = self.elapsed;
        if self.enable {
            time = self.offset.elapsed() - self.elapsed;
        }
        time
    }

    pub fn enable(&mut self) {
        self.elapsed = self.offset.elapsed() - self.paused;
        self.enable = true;
    }

    pub fn disable(&mut self) {
        self.paused = self.offset.elapsed() - self.elapsed;
        self.enable = false;
    }
}

impl EventLoopContext {

}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use super::ClockState;

    #[test]
    fn test_virtual_clock() {
        let mut clock = ClockState::default();
        clock.enable();
        thread::sleep(Duration::from_secs(5));
        let virtual_clock = clock.get_virtual_clock();
        assert_eq!(virtual_clock.as_secs(), 5);
        clock.disable();
        thread::sleep(Duration::from_secs(10));
        let virtual_clock = clock.get_virtual_clock();
        assert_eq!(virtual_clock.as_secs(), 5);
        clock.enable();
        thread::sleep(Duration::from_secs(5));
        let virtual_clock = clock.get_virtual_clock();
        assert_eq!(virtual_clock.as_secs(), 10);

        clock.disable();
        thread::sleep(Duration::from_secs(10));
        let virtual_clock = clock.get_virtual_clock();
        assert_eq!(virtual_clock.as_secs(), 10);
        clock.enable();
        thread::sleep(Duration::from_secs(5));
        let virtual_clock = clock.get_virtual_clock();
        assert_eq!(virtual_clock.as_secs(), 15);
    }
}