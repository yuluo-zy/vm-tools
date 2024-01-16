use std::collections::HashMap;
use std::iter::Map;
use std::sync::{Arc, Mutex};
use std::thread;
use crate::event_context::EventLoopContext;
use anyhow::{bail, Result};
use log::info;
use crate::event_notifier::EventNotifier;

pub(crate) struct EventLoop {
    pub main_loop: EventLoopContext,
    pub io_thread: HashMap<String, EventLoopContext>,
}

static mut GLOBAL_EVENT_LOOP: Option<EventLoop> = None;

impl EventLoop {
    pub fn loop_init(iothreads: &Option<Vec<String>>) -> Result<()> {
        let mut io_threads = HashMap::new();
        if let Some(thr) = iothreads {
            for item in thr {
                io_threads.insert(item.clone(), EventLoopContext::new());
            }
        }
        unsafe {
            if GLOBAL_EVENT_LOOP.is_none() {
                GLOBAL_EVENT_LOOP = Some(EventLoop {
                    main_loop: EventLoopContext::new(),
                    io_thread: io_threads,
                });

                if let Some(event_loop) = GLOBAL_EVENT_LOOP.as_mut() {
                    for (id, ctx) in &mut event_loop.io_thread {
                        thread::Builder::new().name(id.to_string()).spawn(move || {
                            // 添加 iothread_info
                            while let Ok(ret) = ctx.io_thread_run() {
                                if !ret {
                                    break;
                                }
                            }
                        })?;
                    }
                } else {
                    bail!("Global Event Loop have not been initialized.")
                }
            }
        }

        Ok(())
    }

    pub fn set_manager(manager: Arc<Mutex<dyn EventLoopManager>>, name: Option<&String>) {
        if let Some(ctx) = Self::get_ctx(name) {
            ctx.set_manager(manager)
        }
    }

    pub fn update_event(notifiers: Vec<EventNotifier>, name: Option<&String>) -> Result<()> {
        if let Some(ctx) = Self::get_ctx(name) {
            ctx.update_events(notifiers)
        } else {
            bail!("Loop Context not found in EventLoop.")
        }
    }

    pub fn get_ctx(name: Option<&String>) -> Option<&mut EventLoopContext> {
        unsafe {
            if let Some(event_loop) = GLOBAL_EVENT_LOOP.as_mut() {
                if let Some(name) = name {
                    return event_loop.io_thread.get_mut(name);
                }

                return Some(&mut event_loop.main_loop);
            }
        }

        panic!("Global Event Loop have not been initialized.");
    }

    pub fn loop_run() -> Result<()> {
        // SAFETY: the main_loop ctx is dedicated for main thread, thus no concurrent
        // accessing.
        unsafe {
            if let Some(event_loop) = GLOBAL_EVENT_LOOP.as_mut() {
                loop {
                    if !event_loop.main_loop.run()? {
                        info!("MainLoop exits due to guest internal operation.");
                        return Ok(());
                    }
                }
            } else {
                bail!("Global Event Loop have not been initialized.")
            }
        }
    }
}

pub trait EventLoopManager: Send + Sync {
    fn loop_should_exit(&self) -> bool;
    fn loop_cleanup(&self) -> Result<()>;
}

