/* application.rs
 *
 * Copyright 2026 yusuf
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::config::VERSION;
use crate::ShelfilyDesktopWindow;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct ShelfilyDesktopApplication {}

    #[glib::object_subclass]
    impl ObjectSubclass for ShelfilyDesktopApplication {
        const NAME: &'static str = "ShelfilyDesktopApplication";
        type Type = super::ShelfilyDesktopApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for ShelfilyDesktopApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_gactions();
            obj.set_accels_for_action("app.quit", &["<primary>q"]);
        }
    }

    impl ApplicationImpl for ShelfilyDesktopApplication {
        fn activate(&self) {
            let application = self.obj();
            let window = application.active_window().unwrap_or_else(|| {
                let window = ShelfilyDesktopWindow::new(&*application);
                window.upcast()
            });
            window.present();
        }
    }

    impl GtkApplicationImpl for ShelfilyDesktopApplication {}
    impl AdwApplicationImpl for ShelfilyDesktopApplication {}
}

glib::wrapper! {
    pub struct ShelfilyDesktopApplication(ObjectSubclass<imp::ShelfilyDesktopApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl ShelfilyDesktopApplication {
    pub fn new(application_id: &str, flags: &gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .build()
    }

    fn setup_gactions(&self) {
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(move |app: &Self, _, _| app.quit())
            .build();
        let about_action = gio::ActionEntry::builder("about")
            .activate(move |app: &Self, _, _| app.show_about())
            .build();
        let logout_action = gio::ActionEntry::builder("logout")
            .activate(move |app: &Self, _, _| app.do_logout())
            .build();
        self.add_action_entries([quit_action, about_action, logout_action]);
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();

        let about = adw::AboutDialog::builder()
            .application_name("Shelfily Desktop")
            .application_icon("audio-headphones-symbolic")
            .developer_name("yusuf")
            .version(VERSION)
            .developers(vec!["yusuf"])
            .copyright("© 2026 yusuf")
            .license_type(gtk::License::Gpl30)
            .website("https://github.com/yusyel/shelfily-desktop")
            .comments("Audiobookshelf için modern GTK4/libadwaita istemcisi")
            .build();

        about.present(Some(&window));
    }

    fn do_logout(&self) {
        if let Some(window) = self.active_window() {
            if let Some(win) = window.downcast_ref::<ShelfilyDesktopWindow>() {
                win.logout();
            }
        }
    }
}
