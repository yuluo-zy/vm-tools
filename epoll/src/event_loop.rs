use std::iter::Map;
use crate::event_context::EventLoopContext;

pub(crate) struct EventLoop {
    pub main_loop: EventLoopContext,
    pub io_thread: Map<String, EventLoopContext>
}
static mut GLOBAL_EVENT_LOOP: Option<EventLoop> = None;

impl EventLoop {
    pub fn loop_init(){}

    pub fn set_manager(){}

    pub fn update_event(){}

    pub fn get_ctx(){}

    pub fn loop_run(){}
}

pub trait EventLoopManager: Send + Sync {
    fn loop_should_exit(&self) -> bool;
    fn loop_cleanup(&self) -> Result<(), Err>;
}

