use std::fmt;
use std::os::fd::RawFd;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use vmm_sys_util::epoll::EventSet;
use crate::event_context::EventStatus;

#[derive(Debug, PartialOrd, PartialEq)]
pub enum NotifierOperation {
    AddExclusion = 1,
    AddShared = 2,
    Modify = 4,
    Delete = 8,
    Park = 16,
    Resume = 32,
}


pub type NotifierCallBack = dyn Fn(EventSet, RawFd) -> Option<Vec<EventNotifier>>;

pub struct EventNotifier {
    pub(crate) raw_fd: RawFd,
    pub(crate) op: NotifierOperation,
    pub(crate) parked_fd: Option<RawFd>,
    pub(crate) event: EventSet,
    pub(crate) handlers: Vec<Rc<NotifierCallBack>>,
    // 预轮询回调
    pub(crate) handler_poll: Option<Box<NotifierCallBack>>,
    pub(crate) status: Arc<Mutex<EventStatus>>,
}

impl EventNotifier {
    pub fn new(
        op: NotifierOperation,
        raw_fd: RawFd,
        parked_fd: Option<RawFd>,
        event: EventSet,
        handlers: Vec<Rc<NotifierCallBack>>,
    ) -> Self {
        EventNotifier {
            raw_fd,
            op,
            parked_fd,
            event,
            handlers,
            handler_poll: None,
            status: Arc::new(Mutex::new(EventStatus::Alive)),
        }
    }
}
impl fmt::Debug for EventNotifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventNotifier")
            .field("raw_fd", &self.raw_fd)
            .field("op", &self.op)
            .field("parked_fd", &self.parked_fd)
            .field("event", &self.event)
            .field("status", &self.status)
            .field("io_poll", &self.handler_poll.is_some())
            .finish()
    }
}

pub trait EventNotifierHelper {
    fn internal_notifiers(ctx: Arc<Mutex<Self>>) -> Option<Vec<EventNotifier>>;
}

pub fn get_notifier_raw_fds(notifiers: &[EventNotifier]) -> Vec<RawFd> {
    let mut raw_fds = Vec::with_capacity(notifiers.len());
    for fd in notifiers {
        raw_fds.push(fd.raw_fd)
    }
    raw_fds
}

pub fn get_delete_notifiers(raw_fds: &[RawFd]) -> Vec<EventNotifier> {
    let mut delete_notifiers = Vec::with_capacity(raw_fds.len());
    for fd in raw_fds {
        delete_notifiers.push(EventNotifier::new(
            NotifierOperation::Delete,
            *fd,
            None,
            EventSet::IN,
            Vec::new()
        ))
    }
    delete_notifiers
}

