//! The window's state, and navigating between its pages.

use std::{
    cell::RefCell,
    collections::{BTreeSet, HashSet},
    path::PathBuf,
    rc::Rc,
};

use adw::prelude::*;
use erebus_ipc::frontend::IpcFrontend;

use super::{sidebar, window::Shell};
use crate::{library::Library, runtime::Runtime, watcher::Watcher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Library,
    App(String),
}

pub struct App {
    pub library: RefCell<Library>,
    pub runtime: RefCell<Runtime>,
    pub ipc: Option<IpcFrontend>,
    pub watcher: RefCell<Option<Watcher>>,
    pub on_sources_changed: async_channel::Sender<Vec<PathBuf>>,

    pub selection: RefCell<Selection>,
    pub sidebar_entries: RefCell<Vec<Selection>>,
    /// cache which rows are expanded across pages
    pub expanded: RefCell<HashSet<String>>,
    /// cache library search query across pages
    pub library_search: RefCell<String>,

    pub window: adw::ApplicationWindow,
    pub toasts: adw::ToastOverlay,
    pub split: adw::NavigationSplitView,
    pub sidebar: gtk::ListBox,
    pub content: adw::NavigationPage,
}

impl App {
    pub fn new(
        shell: Shell,
        library: Library,
        ipc: Option<IpcFrontend>,
        on_sources_changed: async_channel::Sender<Vec<PathBuf>>,
    ) -> Self {
        let Shell {
            window,
            toasts,
            split,
            sidebar,
            content,
        } = shell;

        Self {
            library: RefCell::new(library),
            runtime: RefCell::default(),
            ipc,
            watcher: RefCell::default(),
            on_sources_changed,
            selection: RefCell::new(Selection::Library),
            sidebar_entries: RefCell::default(),
            expanded: RefCell::default(),
            library_search: RefCell::default(),
            window,
            toasts,
            split,
            sidebar,
            content,
        }
    }

    pub fn toast(&self, message: &str) {
        self.toasts.add_toast(adw::Toast::new(message));
    }

    pub fn instances(&self, process: &str) -> usize {
        self.runtime.borrow().instances(process).len()
    }

    //
    // MARK: navigation
    //

    pub fn select(self: &Rc<Self>, index: i32) {
        let Some(selection) = self.sidebar_entries.borrow().get(index as usize).cloned() else {
            return;
        };

        if *self.selection.borrow() != selection {
            *self.selection.borrow_mut() = selection;
            self.show_page();
        }
    }

    pub fn show_page(self: &Rc<Self>) {
        let selection = self.selection.borrow().clone();
        let (title, page) = match &selection {
            Selection::Library => ("Theme Library", self.library_page()),
            Selection::App(process) => (process.as_str(), self.app_page(process)),
        };

        self.content.set_title(title);
        self.content.set_child(Some(&page));
    }

    /// Rebuilds the page if it's showing `selection`, e.g. because an app's status changed.
    pub fn refresh(self: &Rc<Self>, selection: &Selection) {
        if *self.selection.borrow() == *selection {
            self.show_page();
        }
    }

    pub fn rebuild_sidebar(self: &Rc<Self>) {
        while let Some(row) = self.sidebar.row_at_index(0) {
            self.sidebar.remove(&row);
        }

        let apps: BTreeSet<String> = {
            let library = self.library.borrow();
            let runtime = self.runtime.borrow();
            library
                .store
                .apps
                .keys()
                .chain(runtime.running())
                .cloned()
                .collect()
        };

        let mut entries = vec![Selection::Library];
        let themes = self.library.borrow().store.themes.len();
        self.sidebar.append(&sidebar::library_row(themes));

        for process in apps {
            self.sidebar
                .append(&sidebar::app_row(&process, self.instances(&process)));
            entries.push(Selection::App(process));
        }

        if entries.len() == 1 {
            self.sidebar.append(&sidebar::no_apps_row());
        }

        // if the selected app is gone, fall back to the library
        let selected = entries
            .iter()
            .position(|entry| *entry == *self.selection.borrow())
            .unwrap_or(0);

        *self.sidebar_entries.borrow_mut() = entries;
        self.sidebar
            .select_row(self.sidebar.row_at_index(selected as i32).as_ref());
    }
}
