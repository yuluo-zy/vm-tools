use std::collections::BTreeMap;
use std::os::fd::{AsRawFd, RawFd};
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use libc::{EFD_NONBLOCK, eventfd, ftok};
use vmm_sys_util::epoll::{ControlOperation, Epoll, EpollEvent, EventSet};
use vmm_sys_util::eventfd::EventFd;
use crate::event_loop::EventLoopManager;
use crate::event_notifier::{EventNotifier, NotifierCallBack, NotifierOperation};
use crate::timer::{ClockState, Timer};
use crate::utils::read_fd;
use anyhow::{anyhow, Result};
use log::warn;
use crate::utils::UtilError::{BadNotifierOperation, NoParkedFd};

const READY_EVENT_MAX: usize = 256;

#[derive(Debug, PartialOrd, PartialEq)]
pub enum EventStatus {
    Alive = 0,
    Park = 1,
    // 事件已经被删除
    Removed = 2,
}

pub(crate) struct EventLoopContext {
    epoll: Epoll,
    manager: Option<Arc<Mutex<dyn EventLoopManager>>>,
    kick_event: EventFd,
    kick_me: AtomicBool,
    kicked: AtomicBool,
    events: Arc<RwLock<BTreeMap<RawFd, Box<EventNotifier>>>>,
    gc: Arc<RwLock<Vec<Box<EventNotifier>>>>,
    ready_events: Vec<EpollEvent>,
    timers: Arc<Mutex<Vec<Box<Timer>>>>,
    pub clock_state: Arc<Mutex<ClockState>>,
}

unsafe impl Send for EventLoopContext {}

impl EventLoopContext {
    pub fn new() -> Self {
        let mut ctx = EventLoopContext {
            epoll: Epoll::new().unwrap(),
            manager: None,
            kick_event: EventFd::new(EFD_NONBLOCK).unwrap(),
            kick_me: AtomicBool::new(false),
            kicked: AtomicBool::new(false),
            events: Arc::new(RwLock::new(BTreeMap::new())),
            gc: Arc::new(RwLock::new(Vec::new())),
            ready_events: vec![EpollEvent::default(); READY_EVENT_MAX],
            timers: Arc::new(Mutex::new(vec![])),
            clock_state: Arc::new(Mutex::new(ClockState::default())),
        };
        ctx.init_tick();
        ctx
    }

    pub fn init_tick(&mut self) {
        let kick_handler: Rc<NotifierCallBack> = Rc::new(|_, fd| {
            read_fd(fd);
            None
        });
        self.add_event(EventNotifier::new(
            NotifierOperation::AddExclusion,
            self.kick_event.as_raw_fd(),
            None,
            EventSet::IN,
            vec![kick_handler],
        )).unwrap()
    }

    pub fn tick(&mut self) {
        self.kicked.store(true, Ordering::SeqCst);
        if self.kick_me.load(Ordering::SeqCst) {
            if let Err(e) = self.kick_event.write(1){
                warn!("failed to kick eventloop, {:?}", e)
            }
        }
    }

    pub fn update_events(&mut self, notifiers: Vec<EventNotifier>) -> Result<()> {
        for en in notifiers {
            match en.op {
                NotifierOperation::AddExclusion | NotifierOperation::AddShared => {
                    self.add_event(en)?;
                }
                _ => {}
            }
        }
        self.tick();
        Ok(())
    }
    pub fn add_event(&mut self, mut event: EventNotifier) -> Result<()> {
        let mut events_map = self.events.write().unwrap();
        if let Some(notifiier) = events_map.get_mut(&event.raw_fd) {
            if let NotifierOperation::AddExclusion = event.op {
                return Err(anyhow!(BadNotifierOperation));
            }
            if notifiier.event != event.event {
                self.epoll.ctl(
                    ControlOperation::Modify,
                    notifiier.raw_fd,
                    EpollEvent::new(notifiier.event | event.event, &**notifiier as *const _ as u64),
                )?;
                notifiier.event |= event.event;
            }
            notifiier.handlers.append(&mut event.handlers);
            if *notifiier.status.lock().unwrap() == EventStatus::Park {
                warn!("Parked event updated!")
            }
            return Ok(());
        }
        let event = Box::new(event);
        self.epoll.ctl(
            ControlOperation::Add,
            event.raw_fd,
            EpollEvent::new(event.event, &*event as *const _ as u64),
        )?;
        if let Some(parked_fd) = event.parked_fd {
            if let Some(parked) = events_map.get_mut(&parked_fd) {
                self.epoll
                    .ctl(ControlOperation::Delete,
                         parked_fd,
                         EpollEvent::default())?;
                *parked.status.lock().unwrap() = EventStatus::Park;
            } else {
                return Err(anyhow!(NoParkedFd(parked_fd)));
            }
        }
        events_map.insert(event.raw_fd, event);

        Ok(())
    }
}


// #[cfg(test)]
// mod test {
//     use std::os::unix::io::{AsRawFd, RawFd};
//
//     use vmm_sys_util::{epoll::EventSet, eventfd::EventFd};
//
//     use super::*;
//
//     impl EventLoopContext {
//         fn check_existence(&self, fd: RawFd) -> Option<bool> {
//             let events_map = self.events.read().unwrap();
//             match events_map.get(&fd) {
//                 None => {
//                     return None;
//                 }
//                 Some(notifier) => Some(*notifier.status.lock().unwrap() == EventStatus::Alive),
//             }
//         }
//
//         fn create_event(&mut self) -> i32 {
//             let fd = EventFd::new(EFD_NONBLOCK).unwrap();
//             let result = fd.as_raw_fd();
//             let event = EventNotifier::new(
//                 NotifierOperation::AddShared,
//                 fd.as_raw_fd(),
//                 None,
//                 EventSet::OUT,
//                 Vec::new(),
//             );
//             self.update_events(vec![event]).unwrap();
//             result
//         }
//     }
//
//     fn generate_handler(related_fd: i32) -> Rc<NotifierCallback> {
//         Rc::new(move |_, _| {
//             let mut notifiers = Vec::new();
//             let event = EventNotifier::new(
//                 NotifierOperation::AddShared,
//                 related_fd,
//                 None,
//                 EventSet::IN,
//                 Vec::new(),
//             );
//             notifiers.push(event);
//             Some(notifiers)
//         })
//     }
//
//     #[test]
//     fn basic_test() {
//         let mut mainloop = EventLoopContext::new();
//         let mut notifiers = Vec::new();
//         // eventfd 是 Linux 提供的一个系统调用，用于在用户空间应用程序和内核空间或者用户空间应用程序之间进行事件通知
//         let fd1 = EventFd::new(EFD_NONBLOCK).unwrap();
//         let fd1_related = EventFd::new(EFD_NONBLOCK).unwrap();
//
//         let handler1 = generate_handler(fd1_related.as_raw_fd());
//         let mut handlers = Vec::new();
//         handlers.push(handler1);
//         let event1 = EventNotifier::new(
//             NotifierOperation::AddShared,
//             fd1.as_raw_fd(),
//             None,
//             EventSet::OUT,
//             handlers,
//         );
//
//         notifiers.push(event1);
//         mainloop.update_events(notifiers).unwrap();
//         mainloop.run().unwrap();
//         // Event1 is OUT event, so its handler would be executed immediately.
//         // Event1's handler is to add a fd1_related event, thus checking fd1 and fd1_relate would
//         // make a basic function test.
//         assert!(mainloop.check_existence(fd1.as_raw_fd()).unwrap());
//         assert!(mainloop.check_existence(fd1_related.as_raw_fd()).unwrap());
//     }
//
//     #[test]
//     fn parked_event_test() {
//         let mut mainloop = EventLoopContext::new();
//         let mut notifiers = Vec::new();
//         let fd1 = EventFd::new(EFD_NONBLOCK).unwrap();
//         let fd2 = EventFd::new(EFD_NONBLOCK).unwrap();
//
//         let event1 = EventNotifier::new(
//             NotifierOperation::AddShared,
//             fd1.as_raw_fd(),
//             None,
//             EventSet::OUT,
//             Vec::new(),
//         );
//         let event2 = EventNotifier::new(
//             NotifierOperation::AddShared,
//             fd2.as_raw_fd(),
//             Some(fd1.as_raw_fd()),
//             EventSet::OUT,
//             Vec::new(),
//         );
//
//         notifiers.push(event1);
//         notifiers.push(event2);
//         mainloop.update_events(notifiers).unwrap();
//         mainloop.run().unwrap();
//
//         // For the reason that event1 is the parked event of event2, when event2 added, event1 would
//         // be set to parked.
//         assert!(!mainloop.check_existence(fd1.as_raw_fd()).unwrap());
//         assert!(mainloop.check_existence(fd2.as_raw_fd()).unwrap());
//
//         let event2_remove = EventNotifier::new(
//             NotifierOperation::Delete,
//             fd2.as_raw_fd(),
//             Some(fd1.as_raw_fd()),
//             EventSet::OUT,
//             Vec::new(),
//         );
//         mainloop.update_events(vec![event2_remove]).unwrap();
//
//         // Then we remove event2, event1 will be re-activated and event2 will be deleted (removed
//         // from events_map to gc).
//         assert!(mainloop.check_existence(fd1.as_raw_fd()).unwrap());
//         assert!(mainloop.check_existence(fd2.as_raw_fd()).is_none());
//     }
//
//     #[test]
//     fn event_handler_test() {
//         let mut mainloop = EventLoopContext::new();
//         let mut notifiers = Vec::new();
//         let fd1 = EventFd::new(EFD_NONBLOCK).unwrap();
//         let fd1_related = EventFd::new(EFD_NONBLOCK).unwrap();
//         let fd1_related_update = EventFd::new(EFD_NONBLOCK).unwrap();
//
//         let handler1 = generate_handler(fd1_related.as_raw_fd());
//         let handler1_update = generate_handler(fd1_related_update.as_raw_fd());
//         let event1 = EventNotifier::new(
//             NotifierOperation::AddShared,
//             fd1.as_raw_fd(),
//             None,
//             EventSet::OUT,
//             vec![handler1],
//         );
//
//         let event1_update = EventNotifier::new(
//             NotifierOperation::AddShared,
//             fd1.as_raw_fd(),
//             None,
//             EventSet::OUT,
//             vec![handler1_update],
//         );
//
//         notifiers.push(event1);
//         notifiers.push(event1_update);
//         mainloop.update_events(notifiers).unwrap();
//         mainloop.run().unwrap();
//
//         // Firstly, event1 with handler1 would be added. Then, event1's handlers would append
//         // handler1_update, which would register fd1_related_update in mainloop.
//         assert!(mainloop.check_existence(fd1_related.as_raw_fd()).unwrap());
//         assert!(mainloop
//             .check_existence(fd1_related_update.as_raw_fd())
//             .unwrap());
//     }
//
//     #[test]
//     fn error_operation_test() {
//         let mut mainloop = EventLoopContext::new();
//         let fd1 = EventFd::new(EFD_NONBLOCK).unwrap();
//         let leisure_fd = EventFd::new(EFD_NONBLOCK).unwrap();
//
//         // Delete unexist event
//         let event1 = EventNotifier::new(
//             NotifierOperation::Delete,
//             fd1.as_raw_fd(),
//             None,
//             EventSet::OUT,
//             Vec::new(),
//         );
//         assert!(mainloop.update_events(vec![event1]).is_err());
//
//         // Add event with unexist parked event
//         let event1 = EventNotifier::new(
//             NotifierOperation::AddShared,
//             fd1.as_raw_fd(),
//             Some(leisure_fd.as_raw_fd()),
//             EventSet::OUT,
//             Vec::new(),
//         );
//         assert!(mainloop.update_events(vec![event1]).is_err());
//
//         // Delete event with unexist parked event
//         let event1_delete = EventNotifier::new(
//             NotifierOperation::Delete,
//             fd1.as_raw_fd(),
//             Some(leisure_fd.as_raw_fd()),
//             EventSet::OUT,
//             Vec::new(),
//         );
//         assert!(mainloop.update_events(vec![event1_delete]).is_err());
//     }
//
//     #[test]
//     fn error_parked_operation_test() {
//         let mut mainloop = EventLoopContext::new();
//         let fd1 = EventFd::new(EFD_NONBLOCK).unwrap();
//         let fd2 = EventFd::new(EFD_NONBLOCK).unwrap();
//
//         let event1 = EventNotifier::new(
//             NotifierOperation::AddShared,
//             fd1.as_raw_fd(),
//             None,
//             EventSet::OUT,
//             Vec::new(),
//         );
//         mainloop.update_events(vec![event1]).unwrap();
//
//         let event2 = EventNotifier::new(
//             NotifierOperation::AddShared,
//             fd2.as_raw_fd(),
//             Some(fd1.as_raw_fd()),
//             EventSet::OUT,
//             Vec::new(),
//         );
//         mainloop.update_events(vec![event2]).unwrap();
//
//         // Delete parked event
//         let event1 = EventNotifier::new(
//             NotifierOperation::Delete,
//             fd1.as_raw_fd(),
//             None,
//             EventSet::OUT,
//             Vec::new(),
//         );
//         assert!(mainloop.update_events(vec![event1]).is_ok());
//     }
//
//     #[test]
//     fn fd_released_test() {
//         let mut mainloop = EventLoopContext::new();
//         let fd = mainloop.create_event();
//
//         // In this case, fd is already closed. But program was wrote to ignore the error.
//         let event = EventNotifier::new(
//             NotifierOperation::Delete,
//             fd,
//             None,
//             EventSet::OUT,
//             Vec::new(),
//         );
//
//         assert!(mainloop.update_events(vec![event]).is_ok());
//     }
// }
