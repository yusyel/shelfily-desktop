/* main.rs
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

mod api;
mod application;
mod config;
mod models;
mod window;

use self::application::ShelfilyDesktopApplication;
use self::window::ShelfilyDesktopWindow;

use config::{GETTEXT_PACKAGE, LOCALEDIR, PKGDATADIR};
use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Initialize GStreamer
    gstreamer::init().expect("GStreamer başlatılamadı");

    // Set up gettext translations
    bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    bind_textdomain_codeset(GETTEXT_PACKAGE, "UTF-8")
        .expect("Unable to set the text domain encoding");
    textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    // Load resources (optional in dev mode)
    let resource_path = PKGDATADIR.to_owned() + "/shelfily-desktop.gresource";
    match gio::Resource::load(&resource_path) {
        Ok(resources) => gio::resources_register(&resources),
        Err(_) => log::warn!(
            "Kaynaklar yüklenemedi ({}), geliştirme modunda çalışıyor",
            resource_path
        ),
    }

    let app = ShelfilyDesktopApplication::new(
        "io.github.yusyel.ShelfilyDesktop",
        &gio::ApplicationFlags::empty(),
    );

    app.run()
}
