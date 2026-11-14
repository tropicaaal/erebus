//! event handling from IPC

use std::rc::Rc;

use erebus_ipc::frontend::Event;
use gtk::glib;

use super::app::{App, Selection};
use crate::runtime::BackendInstance;

impl App {
    pub fn handle_event(self: &Rc<Self>, event: Event) {
        match event {
            Event::Connected {
                peer,
                process,
                backend,
                pid,
                version,
            } => {
                self.runtime.borrow_mut().connect(
                    process.clone(),
                    BackendInstance {
                        peer,
                        pid,
                        backend,
                        version,
                        connected_at: glib::DateTime::now_local().ok(),
                    },
                );

                let registered = self.library.borrow_mut().register_app(&process);
                if registered {
                    self.save();
                }

                self.rebuild_sidebar();
                self.refresh(&Selection::App(process));
            }

            Event::Disconnected { peer } => {
                let process = self.runtime.borrow_mut().disconnect(peer);
                let Some(process) = process else {
                    return;
                };

                self.rebuild_sidebar();
                self.refresh(&Selection::App(process));
            }

            Event::Log { peer, record } => self.runtime.borrow().log(peer, &record),
        }
    }
}
