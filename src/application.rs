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
            obj.set_accels_for_action("app.preferences", &["<primary>comma"]);
        }
    }

    impl ApplicationImpl for ShelfilyDesktopApplication {
        fn startup(&self) {
            self.parent_startup();
            // libadwaita is initialized by parent_startup(); now StyleManager is safe.
            apply_color_scheme(&load_theme());
        }

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
        let preferences_action = gio::ActionEntry::builder("preferences")
            .activate(move |app: &Self, _, _| app.show_preferences())
            .build();
        self.add_action_entries([
            quit_action,
            about_action,
            logout_action,
            preferences_action,
        ]);
    }

    fn show_preferences(&self) {
        let window = self.active_window();

        let dialog = adw::PreferencesDialog::new();
        dialog.set_title("Preferences");

        let page = adw::PreferencesPage::new();
        page.set_title("General");
        page.set_icon_name(Some("preferences-system-symbolic"));

        let group = adw::PreferencesGroup::new();
        group.set_title("Appearance");

        let model = gtk::StringList::new(&["Follow system", "Light", "Dark"]);
        let combo = adw::ComboRow::new();
        combo.set_title("Theme");
        combo.set_subtitle("Choose the application color scheme");
        combo.set_model(Some(&model));
        combo.set_selected(match load_theme().as_str() {
            "light" => 1,
            "dark" => 2,
            _ => 0,
        });
        combo.connect_selected_notify(|combo| {
            let theme = match combo.selected() {
                1 => "light",
                2 => "dark",
                _ => "system",
            };
            save_theme(theme);
            apply_color_scheme(theme);
        });
        group.add(&combo);

        page.add(&group);
        dialog.add(&page);
        dialog.present(window.as_ref());
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();

        let about = adw::AboutDialog::builder()
            .application_name("Shelfily Desktop")
            .application_icon("io.github.yusyel.ShelfilyDesktop")
            .developer_name("yusyel")
            .version(VERSION)
            .developers(vec!["yusyel"])
            .copyright("© 2026 yusyel")
            .license_type(gtk::License::Gpl30)
            .website("https://github.com/yusyel/shelfily-desktop")
            .comments("Modern GTK4/libadwaita client for Audiobookshelf")
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

fn theme_file_path() -> std::path::PathBuf {
    let mut path = glib::user_config_dir();
    path.push("shelfily-desktop");
    path.push("theme");
    path
}

fn load_theme() -> String {
    std::fs::read_to_string(theme_file_path())
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| matches!(s.as_str(), "light" | "dark" | "system"))
        .unwrap_or_else(|| "system".to_string())
}

fn save_theme(theme: &str) {
    let path = theme_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, theme) {
        log::warn!("Failed to save theme preference: {}", e);
    }
}

fn apply_color_scheme(theme: &str) {
    let scheme = match theme {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}
