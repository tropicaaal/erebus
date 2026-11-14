use adw::prelude::*;

use super::sidebar;

pub struct Shell {
    pub window: adw::ApplicationWindow,
    pub toasts: adw::ToastOverlay,
    pub split: adw::NavigationSplitView,
    pub sidebar: gtk::ListBox,
    pub content: adw::NavigationPage,
}

impl Shell {
    pub fn new(application: &adw::Application) -> Self {
        let sidebar = gtk::ListBox::builder()
            .css_classes(["navigation-sidebar"])
            .build();
        sidebar.set_header_func(|row, _| {
            let header = (row.index() == 1).then(sidebar::apps_header);
            row.set_header(header.as_ref());
        });

        let about = gtk::Button::builder()
            .icon_name("help-about-symbolic")
            .tooltip_text("About erebus")
            .build();

        let sidebar_header = adw::HeaderBar::new();
        sidebar_header.pack_end(&about);

        let sidebar_toolbar = adw::ToolbarView::new();
        sidebar_toolbar.add_top_bar(&sidebar_header);
        sidebar_toolbar.set_content(Some(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&sidebar)
                .build(),
        ));

        let split = adw::NavigationSplitView::builder()
            .sidebar(
                &adw::NavigationPage::builder()
                    .title("erebus")
                    .child(&sidebar_toolbar)
                    .build(),
            )
            .content(&adw::NavigationPage::builder().title("erebus").build())
            .build();
        let content = split.content().expect("split view was built with content");

        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&split));

        let window = adw::ApplicationWindow::builder()
            .application(application)
            .title("erebus")
            .default_width(900)
            .default_height(640)
            .width_request(360)
            .height_request(294)
            .content(&toasts)
            .build();

        let narrow = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 600sp").expect("valid breakpoint"),
        );
        narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(narrow);

        about.connect_clicked({
            let window = window.clone();
            move |_| show_about(&window)
        });

        Self {
            window,
            toasts,
            split,
            sidebar,
            content,
        }
    }
}

fn show_about(parent: &adw::ApplicationWindow) {
    let repo = "https://github.com/uwu/erebus";

    adw::AboutDialog::builder()
        .application_name("erebus")
        .comments("Manage themes for GPUI apps running with erebus")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("uwu.network")
        .copyright("© 2026 Tropical")
        .license_type(gtk::License::MitX11)
        .website(repo)
        .issue_url(format!("{repo}/issues"))
        .build()
        .present(Some(parent));
}
