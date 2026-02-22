/* window.rs
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
use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::rc::Rc;
use webkit6::prelude::WebViewExt;

use crate::api::AudiobookshelfClient;
use crate::models::*;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct StoredSession {
    server_url: String,
    library_id: String,
    #[serde(default)]
    token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibrarySortMode {
    NewlyAdded,
    AuthorAsc,
    TitleAsc,
}

mod imp {
    use super::*;

    #[derive(Debug)]
    pub struct ShelfilyDesktopWindow {
        pub stack: gtk::Stack,
        pub client: AudiobookshelfClient,
        // Library
        pub library_flowbox: RefCell<Option<gtk::FlowBox>>,
        pub library_content_stack: RefCell<Option<gtk::Stack>>,
        pub library_id: RefCell<String>,
        pub continue_flowbox: RefCell<Option<gtk::FlowBox>>,
        pub library_switcher_bar: RefCell<Option<adw::ViewSwitcherBar>>,
        pub library_items: RefCell<Vec<LibraryItem>>,
        pub continue_items: RefCell<Vec<LibraryItem>>,
        pub library_sort_mode: Cell<LibrarySortMode>,
        pub library_search_query: RefCell<String>,
        // Detail
        pub detail_content: RefCell<Option<gtk::Box>>,
        pub detail_top_box: RefCell<Option<gtk::Box>>,
        pub detail_cover_image: RefCell<Option<gtk::Image>>,
        pub detail_play_content: RefCell<Option<adw::ButtonContent>>,
        pub detail_play_item_id: RefCell<Option<String>>,
        pub detail_play_default_label: RefCell<String>,
        // Persistent bottom player bar
        pub player_bar: gtk::ActionBar,
        pub player_title: gtk::Label,
        pub player_author: gtk::Label,
        pub player_cover: gtk::Image,
        pub play_pause_btn: gtk::Button,
        pub position_scale: gtk::Scale,
        pub position_label: gtk::Label,
        pub duration_label: gtk::Label,
        // Audio/playback state
        pub pipeline: RefCell<Option<gstreamer::Element>>,
        pub bus_guard: RefCell<Option<gstreamer::bus::BusWatchGuard>>,
        pub is_playing: Rc<Cell<bool>>,
        pub session_id: RefCell<Option<String>>,
        pub current_time: RefCell<f64>,
        pub duration: RefCell<f64>,
        pub current_item_id: RefCell<Option<String>>,
        pub updating_slider: Rc<Cell<bool>>,
        pub sync_source: RefCell<Option<glib::SourceId>>,
        pub progress_source: RefCell<Option<glib::SourceId>>,
        // OAuth
        pub oauth_consumed: RefCell<bool>,
        pub compact_mode: Cell<bool>,
    }

    impl Default for ShelfilyDesktopWindow {
        fn default() -> Self {
            let position_scale =
                gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
            position_scale.set_draw_value(false);
            position_scale.set_hexpand(true);

            let player_bar = gtk::ActionBar::new();
            player_bar.set_revealed(false);

            Self {
                stack: gtk::Stack::new(),
                client: AudiobookshelfClient::new(),
                library_flowbox: RefCell::new(None),
                library_content_stack: RefCell::new(None),
                library_id: RefCell::new(String::new()),
                continue_flowbox: RefCell::new(None),
                library_switcher_bar: RefCell::new(None),
                library_items: RefCell::new(Vec::new()),
                continue_items: RefCell::new(Vec::new()),
                library_sort_mode: Cell::new(LibrarySortMode::NewlyAdded),
                library_search_query: RefCell::new(String::new()),
                detail_content: RefCell::new(None),
                detail_top_box: RefCell::new(None),
                detail_cover_image: RefCell::new(None),
                detail_play_content: RefCell::new(None),
                detail_play_item_id: RefCell::new(None),
                detail_play_default_label: RefCell::new(String::from("Start Listening")),
                player_bar,
                player_title: gtk::Label::new(None),
                player_author: gtk::Label::new(None),
                player_cover: gtk::Image::from_icon_name("audio-x-generic-symbolic"),
                play_pause_btn: gtk::Button::from_icon_name("media-playback-start-symbolic"),
                position_scale,
                position_label: gtk::Label::new(Some("0:00")),
                duration_label: gtk::Label::new(Some("0:00")),
                pipeline: RefCell::new(None),
                bus_guard: RefCell::new(None),
                is_playing: Rc::new(Cell::new(false)),
                session_id: RefCell::new(None),
                current_time: RefCell::new(0.0),
                duration: RefCell::new(0.0),
                current_item_id: RefCell::new(None),
                updating_slider: Rc::new(Cell::new(false)),
                sync_source: RefCell::new(None),
                progress_source: RefCell::new(None),
                oauth_consumed: RefCell::new(false),
                compact_mode: Cell::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ShelfilyDesktopWindow {
        const NAME: &'static str = "ShelfilyDesktopWindow";
        type Type = super::ShelfilyDesktopWindow;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for ShelfilyDesktopWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_ui();
        }
    }

    impl WidgetImpl for ShelfilyDesktopWindow {}
    impl WindowImpl for ShelfilyDesktopWindow {}
    impl ApplicationWindowImpl for ShelfilyDesktopWindow {}
    impl AdwApplicationWindowImpl for ShelfilyDesktopWindow {}
}

glib::wrapper! {
    pub struct ShelfilyDesktopWindow(ObjectSubclass<imp::ShelfilyDesktopWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl ShelfilyDesktopWindow {
    fn secret_schema() -> libsecret::Schema {
        let mut attrs = HashMap::new();
        attrs.insert("account", libsecret::SchemaAttributeType::String);
        libsecret::Schema::new(
            "io.github.yusyel.ShelfilyDesktop",
            libsecret::SchemaFlags::NONE,
            attrs,
        )
    }

    fn secret_attrs() -> HashMap<&'static str, &'static str> {
        let mut attrs = HashMap::new();
        attrs.insert("account", "default");
        attrs
    }

    fn store_secret_token(&self, token: &str) {
        let schema = Self::secret_schema();
        if let Err(err) = libsecret::password_store_sync(
            Some(&schema),
            Self::secret_attrs(),
            Some(libsecret::COLLECTION_DEFAULT.as_str()),
            "Shelfily Desktop Session Token",
            token,
            gio::Cancellable::NONE,
        ) {
            log::warn!("Token libsecret'e kaydedilemedi: {}", err);
        }
    }

    fn lookup_secret_token(&self) -> String {
        let schema = Self::secret_schema();
        match libsecret::password_lookup_sync(
            Some(&schema),
            Self::secret_attrs(),
            gio::Cancellable::NONE,
        ) {
            Ok(Some(token)) => token.to_string(),
            Ok(None) => String::new(),
            Err(err) => {
                log::warn!("Failed to read token from libsecret: {}", err);
                String::new()
            }
        }
    }

    fn clear_secret_token(&self) {
        let schema = Self::secret_schema();
        if let Err(err) = libsecret::password_clear_sync(
            Some(&schema),
            Self::secret_attrs(),
            gio::Cancellable::NONE,
        ) {
            log::warn!("Token libsecret'ten silinemedi: {}", err);
        }
    }

    fn has_gsettings_schema() -> bool {
        gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup("io.github.yusyel.ShelfilyDesktop", true))
            .is_some()
    }

    fn session_file_path() -> std::path::PathBuf {
        let mut path = glib::user_config_dir();
        path.push("shelfily-desktop");
        path.push("session.json");
        path
    }

    fn write_stored_session(&self, session: &StoredSession) {
        if Self::has_gsettings_schema() {
            let settings = gio::Settings::new("io.github.yusyel.ShelfilyDesktop");
            let _ = settings.set_string("server-url", &session.server_url);
            let _ = settings.set_string("library-id", &session.library_id);
        }

        let path = Self::session_file_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(session) {
            let _ = fs::write(path, json);
        }
    }

    fn read_stored_session(&self) -> StoredSession {
        let mut session = StoredSession::default();
        let path = Self::session_file_path();
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(file_session) = serde_json::from_str::<StoredSession>(&content) {
                session = file_session;
            }
        }

        if Self::has_gsettings_schema() {
            let settings = gio::Settings::new("io.github.yusyel.ShelfilyDesktop");
            let server_url = settings.string("server-url").to_string();
            let library_id = settings.string("library-id").to_string();
            if !server_url.is_empty() {
                session.server_url = server_url;
            }
            if !library_id.is_empty() {
                session.library_id = library_id;
            }
        }

        session
    }

    fn clear_stored_session(&self) {
        self.write_stored_session(&StoredSession::default());
        let _ = fs::remove_file(Self::session_file_path());
        self.clear_secret_token();
    }

    pub fn new<P: IsA<gtk::Application>>(application: &P) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }

    fn setup_ui(&self) {
        self.set_title(Some("Shelfily Desktop"));
        self.set_icon_name(Some("io.github.yusyel.ShelfilyDesktop"));
        self.set_default_width(1000);
        self.set_default_height(700);
        self.set_size_request(360, 540);
        self.add_css_class("shelfily-window");
        self.install_styles();

        let imp = self.imp();

        imp.stack
            .set_transition_type(gtk::StackTransitionType::SlideLeftRight);
        imp.stack.set_transition_duration(200);

        let login_page = self.build_login_page();
        let library_page = self.build_library_page();
        let detail_page = self.build_detail_page();

        imp.stack.add_named(&login_page, Some("login"));
        imp.stack.add_named(&library_page, Some("library"));
        imp.stack.add_named(&detail_page, Some("detail"));
        imp.stack.set_visible_child_name("login");

        // Build the persistent bottom player bar
        self.build_player_bar();

        // Main layout: stack on top (expanding), player bar pinned at bottom
        let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        main_box.add_css_class("app-root");
        imp.stack.set_vexpand(true);
        main_box.append(&imp.stack);
        main_box.append(&imp.player_bar);

        self.set_content(Some(&main_box));
        self.install_resize_observer();

        // Try auto-login from saved credentials
        self.try_restore_session();
    }

    fn install_styles(&self) {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            "
            .shelfily-window { background: @window_bg_color; }
            .app-root { padding: 6px; }
            .player-bar {
                margin: 8px 10px 10px 10px;
                border-radius: 14px;
                padding: 8px 10px;
                background: @card_bg_color;
                border: 1px solid @borders;
            }
            .book-card {
                padding: 10px;
                border-radius: 12px;
                background: @card_bg_color;
                border: 1px solid @borders;
            }
            .book-card:hover {
                background: @card_bg_color;
                border-color: @accent_bg_color;
            }
            .book-cover-frame { border-radius: 10px; }
            .login-surface {
                border-radius: 16px;
                padding: 18px;
                background: @card_bg_color;
                border: 1px solid @borders;
            }
            .compact .app-root { padding: 2px; }
            .compact .player-bar { margin: 4px 4px 6px 4px; border-radius: 10px; padding: 6px; }
            .compact .book-card { padding: 6px; }
            .compact .login-surface { padding: 10px; border-radius: 12px; }
            ",
        );
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("Display yok"),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    fn install_resize_observer(&self) {
        let win = self.clone();
        self.add_tick_callback(move |widget, _| {
            win.apply_responsive_layout(widget.width());
            glib::ControlFlow::Continue
        });
    }

    fn resize_flowbox(flowbox: &gtk::FlowBox, width: i32) {
        let (min_cols, max_cols, spacing, card_width) = if width < 680 {
            (1, 2, 10, 126)
        } else if width < 980 {
            (2, 4, 12, 142)
        } else {
            (2, 6, 16, 160)
        };
        flowbox.set_min_children_per_line(min_cols);
        flowbox.set_max_children_per_line(max_cols);
        flowbox.set_column_spacing(spacing);
        flowbox.set_row_spacing(spacing);

        let mut child_opt = flowbox.first_child();
        while let Some(child) = child_opt {
            if let Some(inner) = child.first_child() {
                inner.set_width_request(card_width);
            }
            child_opt = child.next_sibling();
        }
    }

    fn apply_responsive_layout(&self, width: i32) {
        let compact = width < 820;
        let imp = self.imp();
        if imp.compact_mode.get() == compact {
            if let Some(flowbox) = imp.library_flowbox.borrow().as_ref() {
                Self::resize_flowbox(flowbox, width);
            }
            if let Some(flowbox) = imp.continue_flowbox.borrow().as_ref() {
                Self::resize_flowbox(flowbox, width);
            }
            return;
        }

        imp.compact_mode.set(compact);
        if compact {
            self.add_css_class("compact");
        } else {
            self.remove_css_class("compact");
        }

        if let Some(flowbox) = imp.library_flowbox.borrow().as_ref() {
            Self::resize_flowbox(flowbox, width);
        }
        if let Some(flowbox) = imp.continue_flowbox.borrow().as_ref() {
            Self::resize_flowbox(flowbox, width);
        }
        if let Some(switcher_bar) = imp.library_switcher_bar.borrow().as_ref() {
            switcher_bar.set_reveal(compact);
        }

        if let Some(top_box) = imp.detail_top_box.borrow().as_ref() {
            top_box.set_orientation(if compact {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            });
        }
        if let Some(img) = imp.detail_cover_image.borrow().as_ref() {
            let size = if compact { 160 } else { 220 };
            img.set_pixel_size(size);
            img.set_size_request(size, size);
        }
    }

    fn save_credentials(&self) {
        let imp = self.imp();
        let token = imp.client.token().unwrap_or_default();
        self.write_stored_session(&StoredSession {
            server_url: imp.client.server_url(),
            library_id: imp.library_id.borrow().clone(),
            token: token.clone(),
        });
        self.store_secret_token(&token);
        log::info!("Oturum bilgileri kaydedildi");
    }

    fn try_restore_session(&self) {
        let saved = self.read_stored_session();
        let server_url = saved.server_url;
        let token = {
            let secret_token = self.lookup_secret_token();
            if secret_token.is_empty() {
                saved.token
            } else {
                secret_token
            }
        };
        let library_id = saved.library_id;

        if server_url.is_empty() || token.is_empty() {
            return;
        }

        log::info!("Found saved session, trying login...");
        let imp = self.imp();
        imp.client.set_server(&server_url);
        imp.client.set_token(&token);
        if !library_id.is_empty() {
            *imp.library_id.borrow_mut() = library_id;
        }

        // Verify the token is still valid by trying a simple API call
        let client = imp.client.clone();
        let win = self.clone();
        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = client.get_libraries();
                let _ = tx.send_blocking(result);
            });
            match rx.recv().await {
                Ok(Ok(_)) => {
                    log::info!("Saved session is valid, loading library");
                    win.imp().stack.set_visible_child_name("library");
                    win.load_library();
                }
                _ => {
                    log::warn!("Saved session is invalid, showing login screen");
                    win.clear_stored_session();
                }
            }
        });
    }

    // ─── PERSISTENT BOTTOM PLAYER BAR (Gelly-style) ────────────────────────

    fn build_player_bar(&self) {
        let imp = self.imp();
        imp.player_bar.add_css_class("player-bar");

        // Cover image (small)
        imp.player_cover.set_pixel_size(48);
        imp.player_cover.set_size_request(48, 48);
        imp.player_cover.add_css_class("card");

        // Info column
        imp.player_title
            .set_ellipsize(gtk::pango::EllipsizeMode::End);
        imp.player_title.set_max_width_chars(30);
        imp.player_title.add_css_class("heading");
        imp.player_title.set_halign(gtk::Align::Start);

        imp.player_author
            .set_ellipsize(gtk::pango::EllipsizeMode::End);
        imp.player_author.set_max_width_chars(30);
        imp.player_author.add_css_class("dim-label");
        imp.player_author.add_css_class("caption");
        imp.player_author.set_halign(gtk::Align::Start);

        let info_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        info_box.append(&imp.player_title);
        info_box.append(&imp.player_author);

        let start_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        start_box.append(&imp.player_cover);
        start_box.append(&info_box);
        imp.player_bar.pack_start(&start_box);

        // Center: controls + progress
        let center_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        center_box.set_hexpand(true);

        // Controls row
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        controls.set_halign(gtk::Align::Center);

        let back_btn = gtk::Button::from_icon_name("media-skip-backward-symbolic");
        back_btn.add_css_class("flat");
        back_btn.set_tooltip_text(Some("Back 30 seconds"));
        let win = self.clone();
        back_btn.connect_clicked(move |_| {
            win.seek_relative(-30);
        });
        controls.append(&back_btn);

        imp.play_pause_btn.add_css_class("circular");
        imp.play_pause_btn.add_css_class("suggested-action");
        imp.play_pause_btn.set_width_request(40);
        imp.play_pause_btn.set_height_request(40);
        imp.play_pause_btn
            .set_tooltip_text(Some("Play / Pause"));
        let win = self.clone();
        imp.play_pause_btn.connect_clicked(move |_| {
            win.toggle_play_pause();
        });
        controls.append(&imp.play_pause_btn);

        let fwd_btn = gtk::Button::from_icon_name("media-skip-forward-symbolic");
        fwd_btn.add_css_class("flat");
        fwd_btn.set_tooltip_text(Some("Forward 30 seconds"));
        let win = self.clone();
        fwd_btn.connect_clicked(move |_| {
            win.seek_relative(30);
        });
        controls.append(&fwd_btn);

        center_box.append(&controls);

        // Progress row
        let progress_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        imp.position_label.add_css_class("caption");
        imp.position_label.add_css_class("dim-label");
        imp.position_label.set_width_chars(6);
        imp.duration_label.add_css_class("caption");
        imp.duration_label.add_css_class("dim-label");
        imp.duration_label.set_width_chars(6);

        progress_row.append(&imp.position_label);
        progress_row.append(&imp.position_scale);
        progress_row.append(&imp.duration_label);
        center_box.append(&progress_row);

        imp.player_bar.pack_end(&center_box);

        // Connect seek handler (user-only)
        let win = self.clone();
        imp.position_scale
            .connect_change_value(move |_scale, _scroll, value| {
                if win.imp().updating_slider.get() {
                    return glib::Propagation::Proceed;
                }
                win.seek_to(value);
                glib::Propagation::Proceed
            });
    }

    fn reveal_player(&self) {
        self.imp().player_bar.set_revealed(true);
    }

    fn hide_player(&self) {
        self.imp().player_bar.set_revealed(false);
    }

    fn update_player_info(&self, title: &str, author: &str, duration: f64, current: f64) {
        let imp = self.imp();
        imp.player_title.set_text(title);
        imp.player_author.set_text(author);
        imp.duration_label.set_text(&format_time(duration));
        imp.position_label.set_text(&format_time(current));
        imp.position_scale.set_range(0.0, duration);
        imp.updating_slider.set(true);
        imp.position_scale.set_value(current);
        imp.updating_slider.set(false);
        *imp.duration.borrow_mut() = duration;
        *imp.current_time.borrow_mut() = current;
    }

    fn update_play_pause_icon(&self, playing: bool) {
        let icon = if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        self.imp().play_pause_btn.set_icon_name(icon);
    }

    fn refresh_detail_play_button(&self) {
        let imp = self.imp();
        let detail_item_id = imp.detail_play_item_id.borrow().clone();
        let current_item_id = imp.current_item_id.borrow().clone();
        let is_current_item = detail_item_id.is_some() && detail_item_id == current_item_id;
        let is_playing_current = is_current_item && imp.is_playing.get();
        let default_label = imp.detail_play_default_label.borrow().clone();

        if let Some(content) = imp.detail_play_content.borrow().as_ref() {
            if is_playing_current {
                content.set_icon_name("media-playback-pause-symbolic");
                content.set_label("Durdur");
            } else {
                content.set_icon_name("media-playback-start-symbolic");
                content.set_label(&default_label);
            }
        }
    }

    fn toggle_play_pause(&self) {
        use gstreamer::prelude::ElementExt;
        let imp = self.imp();
        if let Some(pipeline) = imp.pipeline.borrow().as_ref() {
            let playing = imp.is_playing.get();
            if playing {
                let _ = pipeline.set_state(gstreamer::State::Paused);
                imp.is_playing.set(false);
                self.update_play_pause_icon(false);
                self.refresh_detail_play_button();
            } else {
                let _ = pipeline.set_state(gstreamer::State::Playing);
                imp.is_playing.set(true);
                self.update_play_pause_icon(true);
                self.refresh_detail_play_button();
            }
        }
    }

    fn seek_relative(&self, delta_secs: i64) {
        use gstreamer::prelude::*;
        let imp = self.imp();
        if let Some(pipeline) = imp.pipeline.borrow().as_ref() {
            if let Some(pos) = pipeline.query_position::<gstreamer::ClockTime>() {
                let new_pos = if delta_secs < 0 {
                    pos.saturating_sub(gstreamer::ClockTime::from_seconds((-delta_secs) as u64))
                } else {
                    pos + gstreamer::ClockTime::from_seconds(delta_secs as u64)
                };
                let _ = pipeline.seek_simple(
                    gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT,
                    new_pos,
                );
            }
        }
    }

    fn seek_to(&self, seconds: f64) {
        use gstreamer::prelude::*;
        let imp = self.imp();
        if let Some(pipeline) = imp.pipeline.borrow().as_ref() {
            let position = gstreamer::ClockTime::from_seconds(seconds as u64);
            let _ = pipeline.seek_simple(
                gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT,
                position,
            );
        }
    }

    // ─── LOGIN PAGE ────────────────────────────────────────────────────────

    fn build_login_page(&self) -> gtk::Widget {
        let toolbar_view = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new("Shelfily Desktop", "")));
        toolbar_view.add_top_bar(&header);

        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(400);

        let main_box = gtk::Box::new(gtk::Orientation::Vertical, 20);
        main_box.add_css_class("login-surface");
        main_box.set_valign(gtk::Align::Center);
        main_box.set_margin_start(24);
        main_box.set_margin_end(24);
        main_box.set_margin_top(24);
        main_box.set_margin_bottom(24);

        // Icon
        let icon = gtk::Image::from_icon_name("audio-headphones-symbolic");
        icon.set_pixel_size(80);
        icon.add_css_class("dim-label");
        main_box.append(&icon);

        let title = gtk::Label::new(Some("Shelfily Desktop"));
        title.add_css_class("title-1");
        main_box.append(&title);

        let subtitle = gtk::Label::new(Some("Connect to your Audiobookshelf server"));
        subtitle.add_css_class("dim-label");
        main_box.append(&subtitle);

        // Form
        let group = adw::PreferencesGroup::new();

        let server_row = adw::EntryRow::new();
        server_row.set_title("Server URL");
        server_row.set_text("http://");
        group.add(&server_row);

        let username_row = adw::EntryRow::new();
        username_row.set_title("Username");
        group.add(&username_row);

        let password_row = adw::PasswordEntryRow::new();
        password_row.set_title("Password");
        group.add(&password_row);

        main_box.append(&group);

        let status_label = gtk::Label::new(None);
        status_label.add_css_class("error");
        status_label.set_visible(false);
        status_label.set_wrap(true);
        main_box.append(&status_label);

        let spinner = gtk::Spinner::new();
        spinner.set_visible(false);
        main_box.append(&spinner);

        let login_btn = gtk::Button::with_label("Sign In");
        login_btn.add_css_class("suggested-action");
        login_btn.add_css_class("pill");
        login_btn.set_height_request(42);
        main_box.append(&login_btn);

        // OAuth separator
        let or_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let left_sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        left_sep.set_hexpand(true);
        left_sep.set_valign(gtk::Align::Center);
        let or_label = gtk::Label::new(Some("or"));
        or_label.add_css_class("dim-label");
        or_label.add_css_class("caption");
        let right_sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        right_sep.set_hexpand(true);
        right_sep.set_valign(gtk::Align::Center);
        or_box.append(&left_sep);
        or_box.append(&or_label);
        or_box.append(&right_sep);
        main_box.append(&or_box);

        // OAuth button
        let oauth_btn = gtk::Button::with_label("Sign In with OpenID");
        oauth_btn.add_css_class("pill");
        oauth_btn.set_height_request(42);
        main_box.append(&oauth_btn);

        clamp.set_child(Some(&main_box));
        toolbar_view.set_content(Some(&clamp));

        // Login logic
        let win = self.clone();
        let srv = server_row.clone();
        let usr = username_row.clone();
        let pwd = password_row.clone();
        let lbl = status_label.clone();
        let spn = spinner.clone();
        let btn = login_btn.clone();

        login_btn.connect_clicked(move |_| {
            let server_url = srv.text().to_string().trim().to_string();
            let username = usr.text().to_string().trim().to_string();
            let password = pwd.text().to_string();

            if server_url.len() <= 7 || username.is_empty() {
                lbl.set_text("Please fill in all fields");
                lbl.set_visible(true);
                return;
            }

            lbl.set_visible(false);
            spn.set_visible(true);
            spn.set_spinning(true);
            btn.set_sensitive(false);

            let win_c = win.clone();
            let lbl_c = lbl.clone();
            let spn_c = spn.clone();
            let btn_c = btn.clone();

            glib::spawn_future_local(async move {
                let (tx, rx) = async_channel::bounded(1);
                let server = server_url.clone();
                let user = username.clone();
                let pass = password.clone();

                std::thread::spawn(move || {
                    let client = AudiobookshelfClient::new();
                    client.set_server(&server);
                    let result = client.login(&user, &pass);
                    let _ = tx.send_blocking((result, server));
                });

                match rx.recv().await {
                    Ok((Ok(login_resp), server_url)) => {
                        spn_c.set_spinning(false);
                        spn_c.set_visible(false);
                        btn_c.set_sensitive(true);

                        let token = login_resp.user.token.unwrap_or_default();
                        let default_lib = login_resp.user_default_library_id.unwrap_or_default();

                        win_c.on_login_success(&server_url, &token, &default_lib);
                    }
                    Ok((Err(e), _)) => {
                        spn_c.set_spinning(false);
                        spn_c.set_visible(false);
                        btn_c.set_sensitive(true);
                        lbl_c.set_text(&format!("Login failed: {}", e));
                        lbl_c.set_visible(true);
                    }
                    Err(_) => {
                        spn_c.set_spinning(false);
                        spn_c.set_visible(false);
                        btn_c.set_sensitive(true);
                        lbl_c.set_text("Connection error");
                        lbl_c.set_visible(true);
                    }
                }
            });
        });

        let btn_c = login_btn.clone();
        password_row.connect_apply(move |_| {
            btn_c.emit_clicked();
        });

        // OAuth button
        let win = self.clone();
        let srv = server_row.clone();
        let lbl = status_label.clone();
        let spn = spinner.clone();

        oauth_btn.connect_clicked(move |btn| {
            *win.imp().oauth_consumed.borrow_mut() = false;
            let server_url = srv.text().to_string().trim().to_string();

            if server_url.len() <= 7 {
                lbl.set_text("Please enter the server URL");
                lbl.set_visible(true);
                return;
            }

            lbl.set_visible(false);
            spn.set_visible(true);
            spn.set_spinning(true);
            btn.set_sensitive(false);

            let win_c = win.clone();
            let lbl_c = lbl.clone();
            let spn_c = spn.clone();
            let btn_c = btn.clone();
            let server = server_url.clone();

            glib::spawn_future_local(async move {
                let (tx, rx) = async_channel::bounded(1);
                let srv = server.clone();

                std::thread::spawn(move || {
                    let client = AudiobookshelfClient::new();
                    client.set_server(&srv);
                    let result = client.get_status();
                    let _ = tx.send_blocking(result);
                });

                spn_c.set_spinning(false);
                spn_c.set_visible(false);
                btn_c.set_sensitive(true);

                match rx.recv().await {
                    Ok(Ok(status)) => {
                        let has_openid = status
                            .auth_methods
                            .as_ref()
                            .map(|m| m.iter().any(|a| a == "openid"))
                            .unwrap_or(false);

                        if has_openid {
                            let button_text = status
                                .auth_form_data
                                .as_ref()
                                .and_then(|d| d.auth_openid_button_text.clone());
                            win_c.show_oauth_webview(&server, button_text.as_deref());
                        } else {
                            lbl_c.set_text("OpenID is not configured on this server");
                            lbl_c.set_visible(true);
                        }
                    }
                    Ok(Err(e)) => {
                        lbl_c.set_text(&format!("Failed to fetch server status: {}", e));
                        lbl_c.set_visible(true);
                    }
                    Err(_) => {
                        lbl_c.set_text("Connection error");
                        lbl_c.set_visible(true);
                    }
                }
            });
        });

        toolbar_view.upcast()
    }

    fn on_login_success(&self, server_url: &str, token: &str, default_library_id: &str) {
        let imp = self.imp();
        imp.client.set_server(server_url);
        imp.client.set_token(token);

        if !default_library_id.is_empty() {
            *imp.library_id.borrow_mut() = default_library_id.to_string();
        }

        // Persist credentials
        self.save_credentials();

        imp.stack.set_visible_child_name("library");
        self.load_library();
    }

    // ─── OAUTH WEBVIEW ─────────────────────────────────────────────────────

    fn show_oauth_webview(&self, server_url: &str, button_text: Option<&str>) {
        let dialog = adw::Window::new();
        let title = button_text.unwrap_or("OpenID Login");
        dialog.set_title(Some(title));
        dialog.set_default_width(500);
        dialog.set_default_height(700);
        dialog.set_transient_for(Some(self));
        dialog.set_modal(true);

        let toolbar_view = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new(title, server_url)));
        toolbar_view.add_top_bar(&header);

        let loading_bar = gtk::ProgressBar::new();
        loading_bar.add_css_class("osd");
        loading_bar.set_visible(false);
        toolbar_view.add_top_bar(&loading_bar);

        let webview = webkit6::WebView::new();
        webview.set_vexpand(true);
        webview.set_hexpand(true);

        let auth_url = format!("{}/login?autoLaunch=1", server_url.trim_end_matches('/'));
        log::info!("OAuth URL: {}", auth_url);
        webview.load_uri(&auth_url);

        toolbar_view.set_content(Some(&webview));
        dialog.set_content(Some(&toolbar_view));

        let lb = loading_bar.clone();
        webview.connect_estimated_load_progress_notify(move |wv: &webkit6::WebView| {
            let progress = wv.estimated_load_progress();
            lb.set_fraction(progress);
            lb.set_visible(progress < 1.0);
        });

        let win = self.clone();
        let dlg = dialog.clone();
        let srv = server_url.to_string();
        let token_found = Rc::new(Cell::new(false));
        let token_found_c = token_found.clone();

        webview.connect_uri_notify(move |wv: &webkit6::WebView| {
            if token_found_c.get() {
                return;
            }
            if let Some(uri) = wv.uri() {
                let uri_str: String = uri.into();
                log::debug!("WebView URI: {}", uri_str);

                if let Some(token) = extract_access_token(&uri_str) {
                    token_found_c.set(true);
                    log::info!("OAuth token received");
                    dlg.close();
                    win.on_login_success(&srv, &token, "");
                }
            }
        });

        dialog.present();
    }

    pub fn logout(&self) {
        self.stop_playback();
        self.hide_player();

        self.clear_stored_session();

        self.imp().stack.set_visible_child_name("login");
    }

    fn show_library_error(&self, message: &str) {
        let imp = self.imp();
        let flowbox = imp.library_flowbox.borrow();
        let flowbox = flowbox.as_ref().unwrap();

        while let Some(child) = flowbox.first_child() {
            flowbox.remove(&child);
        }

        let error_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        error_box.set_valign(gtk::Align::Center);
        error_box.set_halign(gtk::Align::Center);
        error_box.set_vexpand(true);

        let icon = gtk::Image::from_icon_name("dialog-error-symbolic");
        icon.set_pixel_size(48);
        icon.add_css_class("dim-label");
        error_box.append(&icon);

        let label = gtk::Label::new(Some(message));
        label.add_css_class("dim-label");
        label.set_wrap(true);
        error_box.append(&label);

        let retry_btn = gtk::Button::with_label("Try Again");
        retry_btn.add_css_class("pill");
        let win = self.clone();
        retry_btn.connect_clicked(move |_| {
            win.load_library();
        });
        error_box.append(&retry_btn);

        flowbox.append(&error_box);
        self.set_library_loading(false);
    }

    // ─── LIBRARY PAGE ──────────────────────────────────────────────────────

    fn build_library_page(&self) -> gtk::Widget {
        let toolbar_view = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();

        let refresh_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.set_tooltip_text(Some("Refresh"));
        let win = self.clone();
        refresh_btn.connect_clicked(move |_| {
            win.load_library();
        });

        let sort_btn = gtk::MenuButton::new();
        sort_btn.set_icon_name("view-sort-ascending-symbolic");
        sort_btn.set_tooltip_text(Some("Sort All Books"));
        sort_btn.add_css_class("flat");

        let sort_popover = gtk::Popover::new();
        let sort_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        sort_box.set_margin_top(8);
        sort_box.set_margin_bottom(8);
        sort_box.set_margin_start(8);
        sort_box.set_margin_end(8);

        let sort_new_btn = gtk::Button::with_label("Newly Added");
        sort_new_btn.add_css_class("flat");
        let win = self.clone();
        sort_new_btn.connect_clicked(move |_| {
            win.imp().library_sort_mode.set(LibrarySortMode::NewlyAdded);
            win.render_library();
            win.render_continue_listening();
        });
        sort_box.append(&sort_new_btn);

        let sort_author_btn = gtk::Button::with_label("Author (A-Z)");
        sort_author_btn.add_css_class("flat");
        let win = self.clone();
        sort_author_btn.connect_clicked(move |_| {
            win.imp().library_sort_mode.set(LibrarySortMode::AuthorAsc);
            win.render_library();
            win.render_continue_listening();
        });
        sort_box.append(&sort_author_btn);

        let sort_title_btn = gtk::Button::with_label("Title (A-Z)");
        sort_title_btn.add_css_class("flat");
        let win = self.clone();
        sort_title_btn.connect_clicked(move |_| {
            win.imp().library_sort_mode.set(LibrarySortMode::TitleAsc);
            win.render_library();
            win.render_continue_listening();
        });
        sort_box.append(&sort_title_btn);

        sort_popover.set_child(Some(&sort_box));
        sort_btn.set_popover(Some(&sort_popover));

        let search_btn = gtk::MenuButton::new();
        search_btn.set_icon_name("system-search-symbolic");
        search_btn.set_tooltip_text(Some("Search in All Books"));
        search_btn.add_css_class("flat");

        let search_popover = gtk::Popover::new();
        let search_popover_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        search_popover_box.set_margin_top(8);
        search_popover_box.set_margin_bottom(8);
        search_popover_box.set_margin_start(8);
        search_popover_box.set_margin_end(8);
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some("Search books or authors"));
        search_entry.set_hexpand(true);
        let win = self.clone();
        search_entry.connect_search_changed(move |entry| {
            *win.imp().library_search_query.borrow_mut() = entry.text().to_string();
            win.render_library();
            win.render_continue_listening();
        });
        search_popover_box.append(&search_entry);
        search_popover.set_child(Some(&search_popover_box));
        search_btn.set_popover(Some(&search_popover));

        let menu_button = gtk::MenuButton::new();
        menu_button.set_primary(true);
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_tooltip_text(Some("Menu"));

        let menu = gio::Menu::new();
        menu.append(Some("About"), Some("app.about"));
        menu.append(Some("Log Out"), Some("app.logout"));
        menu.append(Some("Quit"), Some("app.quit"));
        menu_button.set_menu_model(Some(&menu));

        // Right-side order: hamburger (far right), search, sort, refresh.
        header.pack_end(&menu_button);
        header.pack_end(&search_btn);
        header.pack_end(&sort_btn);
        header.pack_end(&refresh_btn);

        // ── ViewStack with two tabs ──
        let view_stack = adw::ViewStack::new();

        // Tab 1: Continue
        let continue_scrolled = gtk::ScrolledWindow::new();
        continue_scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        continue_scrolled.set_vscrollbar_policy(gtk::PolicyType::Automatic);

        let continue_clamp = adw::Clamp::new();
        continue_clamp.set_maximum_size(1200);
        continue_clamp.set_margin_top(16);
        continue_clamp.set_margin_bottom(16);
        continue_clamp.set_margin_start(16);
        continue_clamp.set_margin_end(16);

        let continue_flowbox = gtk::FlowBox::new();
        continue_flowbox.set_valign(gtk::Align::Start);
        continue_flowbox.set_max_children_per_line(6);
        continue_flowbox.set_min_children_per_line(2);
        continue_flowbox.set_column_spacing(16);
        continue_flowbox.set_row_spacing(16);
        continue_flowbox.set_homogeneous(true);
        continue_flowbox.set_selection_mode(gtk::SelectionMode::None);

        continue_clamp.set_child(Some(&continue_flowbox));
        continue_scrolled.set_child(Some(&continue_clamp));

        let continue_page =
            view_stack.add_titled(&continue_scrolled, Some("continue"), "Continue");
        continue_page.set_icon_name(Some("media-playback-start-symbolic"));

        // Tab 2: All Books
        let library_scrolled = gtk::ScrolledWindow::new();
        library_scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        library_scrolled.set_vscrollbar_policy(gtk::PolicyType::Automatic);

        let library_clamp = adw::Clamp::new();
        library_clamp.set_maximum_size(1200);
        library_clamp.set_margin_top(16);
        library_clamp.set_margin_bottom(16);
        library_clamp.set_margin_start(16);
        library_clamp.set_margin_end(16);

        let all_books_box = gtk::Box::new(gtk::Orientation::Vertical, 12);

        let flowbox = gtk::FlowBox::new();
        flowbox.set_valign(gtk::Align::Start);
        flowbox.set_max_children_per_line(6);
        flowbox.set_min_children_per_line(2);
        flowbox.set_column_spacing(16);
        flowbox.set_row_spacing(16);
        flowbox.set_homogeneous(true);
        flowbox.set_selection_mode(gtk::SelectionMode::None);

        all_books_box.append(&flowbox);
        library_clamp.set_child(Some(&all_books_box));
        library_scrolled.set_child(Some(&library_clamp));

        let library_page = view_stack.add_titled(&library_scrolled, Some("all"), "All Books");
        library_page.set_icon_name(Some("view-grid-symbolic"));

        // ViewSwitcher in the header
        let switcher = adw::ViewSwitcher::new();
        switcher.set_stack(Some(&view_stack));
        switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
        header.set_title_widget(Some(&switcher));

        // Modern GTK4/adwaita pattern: show bottom switcher on narrow widths
        let switcher_bar = adw::ViewSwitcherBar::new();
        switcher_bar.set_stack(Some(&view_stack));
        switcher_bar.set_reveal(false);

        toolbar_view.add_top_bar(&header);
        toolbar_view.add_bottom_bar(&switcher_bar);

        // Loading overlay
        let loading_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        loading_box.set_valign(gtk::Align::Center);
        loading_box.set_halign(gtk::Align::Center);
        loading_box.set_vexpand(true);
        let loading_spinner = gtk::Spinner::new();
        loading_spinner.set_spinning(true);
        loading_spinner.set_width_request(32);
        loading_spinner.set_height_request(32);
        loading_box.append(&loading_spinner);
        let loading_label = gtk::Label::new(Some("Loading books..."));
        loading_label.add_css_class("dim-label");
        loading_box.append(&loading_label);

        let content_stack = gtk::Stack::new();
        content_stack.add_named(&loading_box, Some("loading"));
        content_stack.add_named(&view_stack, Some("content"));
        content_stack.set_visible_child_name("loading");

        toolbar_view.set_content(Some(&content_stack));

        *self.imp().library_flowbox.borrow_mut() = Some(flowbox);
        *self.imp().library_content_stack.borrow_mut() = Some(content_stack);
        *self.imp().continue_flowbox.borrow_mut() = Some(continue_flowbox);
        *self.imp().library_switcher_bar.borrow_mut() = Some(switcher_bar);

        toolbar_view.upcast()
    }

    fn load_library(&self) {
        let imp = self.imp();
        let client = imp.client.clone();
        let library_id = imp.library_id.borrow().clone();
        let win = self.clone();

        self.set_library_loading(true);

        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            let lib_id = library_id.clone();

            std::thread::spawn(move || {
                let result = if lib_id.is_empty() {
                    match client.get_libraries() {
                        Ok(libs) => {
                            if let Some(lib) = libs.first() {
                                let first_id = lib.id.clone();
                                match client.get_library_items(&first_id) {
                                    Ok(items) => Ok((first_id, items)),
                                    Err(e) => Err(e),
                                }
                            } else {
                                Ok((String::new(), vec![]))
                            }
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    match client.get_library_items(&lib_id) {
                        Ok(items) => Ok((lib_id, items)),
                        Err(e) => Err(e),
                    }
                };
                let _ = tx.send_blocking(result);
            });

            match rx.recv().await {
                Ok(Ok((lib_id, items))) => {
                    log::info!("Library loaded: {} books", items.len());
                    *win.imp().library_id.borrow_mut() = lib_id;
                    win.populate_library(&items);
                    win.set_library_loading(false);
                }
                Ok(Err(e)) => {
                    log::error!("Failed to load library: {}", e);
                    win.set_library_loading(false);
                    win.show_library_error(&format!("Failed to load library: {}", e));
                }
                Err(_) => {
                    log::error!("Channel error while loading library");
                    win.set_library_loading(false);
                    win.show_library_error("Connection error");
                }
            }
        });
    }

    fn set_library_loading(&self, loading: bool) {
        let imp = self.imp();
        if let Some(ref cs) = *imp.library_content_stack.borrow() {
            cs.set_visible_child_name(if loading { "loading" } else { "content" });
        }
    }

    fn populate_library(&self, items: &[LibraryItem]) {
        let mut seen = HashSet::new();
        let deduped: Vec<LibraryItem> = items
            .iter()
            .filter(|item| seen.insert(item.id.clone()))
            .cloned()
            .collect();

        let mut cached = self.imp().library_items.borrow_mut();
        cached.clear();
        cached.extend(deduped);
        drop(cached);

        self.render_library();

        // Fetch "Continue" items from the server
        self.load_continue_listening();
    }

    fn render_library(&self) {
        let imp = self.imp();
        let flowbox = imp.library_flowbox.borrow();
        let flowbox = flowbox.as_ref().unwrap();

        while let Some(child) = flowbox.first_child() {
            flowbox.remove(&child);
        }

        let mut items = imp.library_items.borrow().clone();
        match imp.library_sort_mode.get() {
            LibrarySortMode::NewlyAdded => {
                items.sort_by_key(|item| Reverse(Self::item_added_timestamp(item)));
            }
            LibrarySortMode::AuthorAsc => {
                items.sort_by_key(|item| {
                    (
                        Self::item_author_for_sort(item).to_lowercase(),
                        Self::item_title_for_sort(item).to_lowercase(),
                    )
                });
            }
            LibrarySortMode::TitleAsc => {
                items.sort_by_key(|item| {
                    (
                        Self::item_title_for_sort(item).to_lowercase(),
                        Self::item_author_for_sort(item).to_lowercase(),
                    )
                });
            }
        }

        let query = imp.library_search_query.borrow().trim().to_lowercase();
        if !query.is_empty() {
            items.retain(|item| {
                let title = Self::item_title_for_sort(item).to_lowercase();
                let author = Self::item_author_for_sort(item).to_lowercase();
                title.contains(&query) || author.contains(&query)
            });
        }

        for item in &items {
            let card = self.create_book_card(item);
            flowbox.append(&card);
        }
    }

    fn item_title_for_sort(item: &LibraryItem) -> &str {
        item.media
            .as_ref()
            .and_then(|m| m.metadata.as_ref())
            .and_then(|md| md.title.as_deref())
            .unwrap_or("")
    }

    fn item_author_for_sort(item: &LibraryItem) -> &str {
        item.media
            .as_ref()
            .and_then(|m| m.metadata.as_ref())
            .and_then(|md| md.author_name_lf.as_deref().or(md.author_name.as_deref()))
            .unwrap_or("")
    }

    fn item_added_timestamp(item: &LibraryItem) -> u64 {
        item.extra
            .as_ref()
            .and_then(|v| v.get("addedAt").and_then(|x| x.as_u64()))
            .or_else(|| {
                item.extra
                    .as_ref()
                    .and_then(|v| v.get("createdAt").and_then(|x| x.as_u64()))
            })
            .or_else(|| {
                item.extra
                    .as_ref()
                    .and_then(|v| v.get("updatedAt").and_then(|x| x.as_u64()))
            })
            .unwrap_or(0)
    }

    fn load_continue_listening(&self) {
        let imp = self.imp();
        let client = imp.client.clone();
        let win = self.clone();

        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = client.get_items_in_progress();
                let _ = tx.send_blocking(result);
            });

            match rx.recv().await {
                Ok(Ok(items)) => {
                    log::info!("Devam eden kitaplar: {}", items.len());
                    let mut seen = HashSet::new();
                    let deduped: Vec<LibraryItem> = items
                        .iter()
                        .filter(|item| seen.insert(item.id.clone()))
                        .cloned()
                        .collect();
                    *win.imp().continue_items.borrow_mut() = deduped;
                    win.render_continue_listening();
                }
                Ok(Err(e)) => {
                    log::warn!("Failed to load continue listening books: {}", e);
                }
                Err(_) => {
                    log::warn!("Channel error while loading continue listening books");
                }
            }
        });
    }

    fn render_continue_listening(&self) {
        let imp = self.imp();
        let continue_flowbox = imp.continue_flowbox.borrow();
        let continue_flowbox = continue_flowbox.as_ref().unwrap();
        while let Some(child) = continue_flowbox.first_child() {
            continue_flowbox.remove(&child);
        }

        let mut items = imp.continue_items.borrow().clone();
        match imp.library_sort_mode.get() {
            LibrarySortMode::NewlyAdded => {
                items.sort_by_key(|item| Reverse(Self::item_added_timestamp(item)));
            }
            LibrarySortMode::AuthorAsc => {
                items.sort_by_key(|item| {
                    (
                        Self::item_author_for_sort(item).to_lowercase(),
                        Self::item_title_for_sort(item).to_lowercase(),
                    )
                });
            }
            LibrarySortMode::TitleAsc => {
                items.sort_by_key(|item| {
                    (
                        Self::item_title_for_sort(item).to_lowercase(),
                        Self::item_author_for_sort(item).to_lowercase(),
                    )
                });
            }
        }

        let query = imp.library_search_query.borrow().trim().to_lowercase();
        if !query.is_empty() {
            items.retain(|item| {
                let title = Self::item_title_for_sort(item).to_lowercase();
                let author = Self::item_author_for_sort(item).to_lowercase();
                title.contains(&query) || author.contains(&query)
            });
        }

        for item in &items {
            let card = self.create_book_card(item);
            continue_flowbox.append(&card);
        }

        if items.is_empty() {
            let empty_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
            empty_box.set_valign(gtk::Align::Center);
            empty_box.set_halign(gtk::Align::Center);
            empty_box.set_vexpand(true);
            let icon = gtk::Image::from_icon_name("audio-headphones-symbolic");
            icon.set_pixel_size(48);
            icon.add_css_class("dim-label");
            empty_box.append(&icon);
            let label = gtk::Label::new(Some("No books in progress yet"));
            label.add_css_class("dim-label");
            empty_box.append(&label);
            continue_flowbox.append(&empty_box);
        }
    }

    fn create_book_card(&self, item: &LibraryItem) -> gtk::Widget {
        let card_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card_box.add_css_class("book-card");
        card_box.set_width_request(160);
        card_box.set_halign(gtk::Align::Center);

        let cover_frame = gtk::Frame::new(None);
        cover_frame.set_halign(gtk::Align::Center);
        cover_frame.add_css_class("card");
        cover_frame.add_css_class("book-cover-frame");

        let cover_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let cover_image = gtk::Image::from_icon_name("audio-x-generic-symbolic");
        cover_image.set_pixel_size(160);
        cover_image.set_size_request(160, 160);
        cover_image.add_css_class("dim-label");
        cover_box.append(&cover_image);

        let progress_val = item
            .user_media_progress
            .as_ref()
            .and_then(|p| p.progress)
            .unwrap_or(0.0);
        let is_finished = item
            .user_media_progress
            .as_ref()
            .and_then(|p| p.is_finished)
            .unwrap_or(false);

        // Red progress bar under the cover
        let bar_track = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar_track.set_height_request(4);
        bar_track.set_hexpand(true);

        if progress_val > 0.0 || is_finished {
            let frac = if is_finished { 1.0 } else { progress_val };

            let bar_fill = gtk::DrawingArea::new();
            bar_fill.set_height_request(4);
            bar_fill.set_hexpand(true);
            let frac_c = frac;
            bar_fill.set_draw_func(move |_area, cr, width, height| {
                // Gray background
                cr.set_source_rgb(0.3, 0.3, 0.3);
                let _ = cr.paint();
                // Red fill
                cr.set_source_rgb(0.9, 0.2, 0.2);
                cr.rectangle(0.0, 0.0, width as f64 * frac_c, height as f64);
                let _ = cr.fill();
            });
            bar_track.append(&bar_fill);
            bar_track.set_visible(true);
        } else {
            bar_track.set_visible(false);
        }

        cover_box.append(&bar_track);
        cover_frame.set_child(Some(&cover_box));
        card_box.append(&cover_frame);

        let title = item
            .media
            .as_ref()
            .and_then(|m| m.metadata.as_ref())
            .and_then(|md| md.title.as_deref())
            .unwrap_or("Unknown Book");

        let title_label = gtk::Label::new(Some(title));
        title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title_label.set_max_width_chars(20);
        title_label.set_lines(2);
        title_label.set_wrap(true);
        title_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        title_label.add_css_class("heading");
        title_label.set_halign(gtk::Align::Start);
        card_box.append(&title_label);

        let author = item
            .media
            .as_ref()
            .and_then(|m| m.metadata.as_ref())
            .and_then(|md| md.author_name.as_deref())
            .unwrap_or("Unknown Author");

        let author_label = gtk::Label::new(Some(author));
        author_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        author_label.set_max_width_chars(20);
        author_label.add_css_class("dim-label");
        author_label.add_css_class("caption");
        author_label.set_halign(gtk::Align::Start);
        card_box.append(&author_label);

        if let Some(duration) = item.media.as_ref().and_then(|m| m.duration) {
            let hours = (duration / 3600.0) as u32;
            let mins = ((duration % 3600.0) / 60.0) as u32;
            let dur_text = if hours > 0 {
                format!("{} h {} min", hours, mins)
            } else {
                format!("{} min", mins)
            };
            let dur_label = gtk::Label::new(Some(&dur_text));
            dur_label.add_css_class("dim-label");
            dur_label.add_css_class("caption");
            dur_label.set_halign(gtk::Align::Start);
            card_box.append(&dur_label);
        }

        let gesture = gtk::GestureClick::new();
        let item_id = item.id.clone();
        let win = self.clone();
        gesture.connect_released(move |_, _, _, _| {
            win.open_audiobook_detail(&item_id);
        });
        card_box.add_controller(gesture);
        card_box.set_cursor_from_name(Some("pointer"));

        // Load cover
        let client = self.imp().client.clone();
        let item_id = item.id.clone();
        let img = cover_image.clone();

        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            let id = item_id.clone();
            std::thread::spawn(move || {
                let result = client.download_cover(&id);
                let _ = tx.send_blocking(result);
            });
            if let Ok(Ok(bytes)) = rx.recv().await {
                let gbytes = glib::Bytes::from(&bytes);
                let stream = gio::MemoryInputStream::from_bytes(&gbytes);
                if let Ok(pixbuf) =
                    gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE)
                {
                    let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
                    img.set_paintable(Some(&texture));
                    img.set_size_request(160, -1);
                    img.remove_css_class("dim-label");
                }
            }
        });

        card_box.upcast()
    }

    // ─── DETAIL PAGE ───────────────────────────────────────────────────────

    fn build_detail_page(&self) -> gtk::Widget {
        let toolbar_view = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();

        let back_btn = gtk::Button::from_icon_name("go-previous-symbolic");
        back_btn.set_tooltip_text(Some("Back to Library"));
        let win = self.clone();
        back_btn.connect_clicked(move |_| {
            win.imp().stack.set_visible_child_name("library");
        });
        header.pack_start(&back_btn);
        header.set_title_widget(Some(&adw::WindowTitle::new("Book Details", "")));

        toolbar_view.add_top_bar(&header);

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);

        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(800);
        clamp.set_margin_top(24);
        clamp.set_margin_bottom(24);
        clamp.set_margin_start(24);
        clamp.set_margin_end(24);

        let detail_box = gtk::Box::new(gtk::Orientation::Vertical, 24);
        detail_box.set_valign(gtk::Align::Start);

        clamp.set_child(Some(&detail_box));
        scrolled.set_child(Some(&clamp));
        toolbar_view.set_content(Some(&scrolled));

        *self.imp().detail_content.borrow_mut() = Some(detail_box);

        toolbar_view.upcast()
    }

    fn open_audiobook_detail(&self, item_id: &str) {
        let imp = self.imp();
        let client = imp.client.clone();
        let win = self.clone();
        let id = item_id.to_string();

        imp.stack.set_visible_child_name("detail");

        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            let item_id = id.clone();

            std::thread::spawn(move || {
                let result = client.get_library_item(&item_id);
                let _ = tx.send_blocking(result);
            });

            match rx.recv().await {
                Ok(Ok(item)) => {
                    win.populate_detail(&item);
                }
                Ok(Err(e)) => {
                    log::error!("Failed to load book: {}", e);
                }
                Err(_) => {
                    log::error!("Channel error");
                }
            }
        });
    }

    fn populate_detail(&self, item: &LibraryItemExpanded) {
        let imp = self.imp();
        let detail_box = imp.detail_content.borrow();
        let detail_box = detail_box.as_ref().unwrap();

        while let Some(child) = detail_box.first_child() {
            detail_box.remove(&child);
        }
        *imp.detail_play_content.borrow_mut() = None;
        *imp.detail_play_item_id.borrow_mut() = Some(item.id.clone());
        *imp.detail_top_box.borrow_mut() = None;
        *imp.detail_cover_image.borrow_mut() = None;

        let metadata = item.media.as_ref().and_then(|m| m.metadata.as_ref());

        // Top: cover + info
        let top_box = gtk::Box::new(gtk::Orientation::Horizontal, 24);

        let cover_frame = gtk::Frame::new(None);
        cover_frame.add_css_class("card");
        let cover_image = gtk::Image::from_icon_name("audio-x-generic-symbolic");
        cover_image.set_pixel_size(220);
        cover_image.set_size_request(220, 220);
        cover_frame.set_child(Some(&cover_image));
        top_box.append(&cover_frame);
        *imp.detail_top_box.borrow_mut() = Some(top_box.clone());
        *imp.detail_cover_image.borrow_mut() = Some(cover_image.clone());

        // Load cover
        let client = imp.client.clone();
        let item_id = item.id.clone();
        let img = cover_image.clone();
        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = client.download_cover(&item_id);
                let _ = tx.send_blocking(result);
            });
            if let Ok(Ok(bytes)) = rx.recv().await {
                let gbytes = glib::Bytes::from(&bytes);
                let stream = gio::MemoryInputStream::from_bytes(&gbytes);
                if let Ok(pixbuf) =
                    gtk::gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE)
                {
                    let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
                    img.set_paintable(Some(&texture));
                    img.set_size_request(220, -1);
                }
            }
        });

        let info_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        info_box.set_valign(gtk::Align::Center);
        info_box.set_hexpand(true);

        let title = metadata
            .and_then(|m| m.title.as_deref())
            .unwrap_or("Unknown Book");
        let title_label = gtk::Label::new(Some(title));
        title_label.add_css_class("title-1");
        title_label.set_halign(gtk::Align::Start);
        title_label.set_wrap(true);
        info_box.append(&title_label);

        if let Some(subtitle) = metadata.and_then(|m| m.subtitle.as_deref()) {
            let sub_label = gtk::Label::new(Some(subtitle));
            sub_label.add_css_class("title-3");
            sub_label.add_css_class("dim-label");
            sub_label.set_halign(gtk::Align::Start);
            sub_label.set_wrap(true);
            info_box.append(&sub_label);
        }

        if let Some(authors) = metadata.and_then(|m| m.authors.as_ref()) {
            let author_names: Vec<&str> = authors.iter().map(|a| a.name.as_str()).collect();
            let author_text = author_names.join(", ");
            let author_label = gtk::Label::new(Some(&author_text));
            author_label.add_css_class("title-4");
            author_label.set_halign(gtk::Align::Start);
            author_label.set_wrap(true);
            info_box.append(&author_label);
        }

        if let Some(narrators) = metadata.and_then(|m| m.narrators.as_ref()) {
            if !narrators.is_empty() {
                let narrator_label =
                    gtk::Label::new(Some(&format!("Narrated by: {}", narrators.join(", "))));
                narrator_label.add_css_class("dim-label");
                narrator_label.set_halign(gtk::Align::Start);
                narrator_label.set_wrap(true);
                info_box.append(&narrator_label);
            }
        }

        if let Some(duration) = item.media.as_ref().and_then(|m| m.duration) {
            let hours = (duration / 3600.0) as u32;
            let mins = ((duration % 3600.0) / 60.0) as u32;
            let dur_label = gtk::Label::new(Some(&format!("Duration: {} h {} min", hours, mins)));
            dur_label.add_css_class("dim-label");
            dur_label.set_halign(gtk::Align::Start);
            info_box.append(&dur_label);
        }

        if let Some(series) = metadata.and_then(|m| m.series.as_ref()) {
            for s in series {
                let mut text = s.name.clone().unwrap_or_default();
                if let Some(seq) = &s.sequence {
                    text = format!("{} #{}", text, seq);
                }
                if !text.is_empty() {
                    let series_label = gtk::Label::new(Some(&format!("Series: {}", text)));
                    series_label.add_css_class("dim-label");
                    series_label.set_halign(gtk::Align::Start);
                    info_box.append(&series_label);
                }
            }
        }

        if let Some(genres) = metadata.and_then(|m| m.genres.as_ref()) {
            if !genres.is_empty() {
                let genre_flow = gtk::FlowBox::new();
                genre_flow.set_selection_mode(gtk::SelectionMode::None);
                genre_flow.set_max_children_per_line(10);
                genre_flow.set_column_spacing(4);
                genre_flow.set_row_spacing(4);
                for genre in genres {
                    let badge = gtk::Label::new(Some(genre));
                    badge.add_css_class("caption");
                    badge.add_css_class("card");
                    badge.set_margin_start(4);
                    badge.set_margin_end(4);
                    badge.set_margin_top(2);
                    badge.set_margin_bottom(2);
                    genre_flow.append(&badge);
                }
                info_box.append(&genre_flow);
            }
        }

        top_box.append(&info_box);
        detail_box.append(&top_box);
        self.apply_responsive_layout(self.width());

        // Play button
        let has_progress = item
            .user_media_progress
            .as_ref()
            .and_then(|p| p.progress)
            .unwrap_or(0.0)
            > 0.0;

        let play_button = gtk::Button::new();
        let play_content = adw::ButtonContent::new();
        play_content.set_icon_name("media-playback-start-symbolic");
        let default_play_label = if has_progress {
            "Continue"
        } else {
            "Start Listening"
        };
        play_content.set_label(default_play_label);
        *imp.detail_play_default_label.borrow_mut() = default_play_label.to_string();
        *imp.detail_play_content.borrow_mut() = Some(play_content.clone());
        play_button.set_child(Some(&play_content));
        play_button.add_css_class("suggested-action");
        play_button.add_css_class("pill");
        play_button.set_height_request(48);
        play_button.set_halign(gtk::Align::Start);

        let item_id = item.id.clone();
        let win = self.clone();
        play_button.connect_clicked(move |_| {
            let imp = win.imp();
            let same_item = imp.current_item_id.borrow().as_deref() == Some(item_id.as_str());
            if same_item && imp.is_playing.get() {
                win.stop_playback();
                win.hide_player();
                win.refresh_detail_play_button();
            } else {
                win.start_playback(&item_id);
            }
        });
        detail_box.append(&play_button);
        self.refresh_detail_play_button();

        // Description
        if let Some(desc) = metadata.and_then(|m| m.description.as_deref()) {
            if !desc.is_empty() {
                let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
                detail_box.append(&sep);

                let desc_title = gtk::Label::new(Some("Description"));
                desc_title.add_css_class("title-4");
                desc_title.set_halign(gtk::Align::Start);
                detail_box.append(&desc_title);

                let desc_label = gtk::Label::new(Some(desc));
                desc_label.set_wrap(true);
                desc_label.set_halign(gtk::Align::Start);
                desc_label.set_selectable(true);
                desc_label.add_css_class("body");
                detail_box.append(&desc_label);
            }
        }

        // Chapters
        if let Some(chapters) = item.media.as_ref().and_then(|m| m.chapters.as_ref()) {
            if !chapters.is_empty() {
                let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
                detail_box.append(&sep);

                let ch_title = gtk::Label::new(Some("Chapters"));
                ch_title.add_css_class("title-4");
                ch_title.set_halign(gtk::Align::Start);
                detail_box.append(&ch_title);

                // Get current listening position
                let listen_pos = item
                    .user_media_progress
                    .as_ref()
                    .and_then(|p| p.current_time)
                    .unwrap_or(0.0);
                let is_finished = item
                    .user_media_progress
                    .as_ref()
                    .and_then(|p| p.is_finished)
                    .unwrap_or(false);

                let chapters_group = adw::PreferencesGroup::new();
                for (i, chapter) in chapters.iter().enumerate() {
                    let ch_name = chapter.title.as_deref().unwrap_or("Chapter");
                    let start = chapter.start.unwrap_or(0.0);
                    let end = chapter.end.unwrap_or(0.0);
                    let dur = end - start;
                    let mins = (dur / 60.0) as u32;
                    let secs = (dur % 60.0) as u32;

                    let row = adw::ActionRow::new();
                    row.set_title(ch_name);
                    row.set_subtitle(&format!("{} min {} sec", mins, secs));
                    row.add_prefix(&gtk::Label::new(Some(&format!("{}", i + 1))));

                    // Color based on progress
                    if is_finished || listen_pos >= end {
                        // Completed chapter — light green
                        row.add_css_class("success");
                    } else if listen_pos < start {
                        // Not yet reached — light red
                        row.add_css_class("error");
                    }
                    // Currently playing chapter (start <= listen_pos < end) — default style

                    let ch_play = gtk::Button::from_icon_name("media-playback-start-symbolic");
                    ch_play.add_css_class("flat");
                    ch_play.set_valign(gtk::Align::Center);
                    let ch_start = start;
                    let item_id = item.id.clone();
                    let win = self.clone();
                    ch_play.connect_clicked(move |_| {
                        win.start_playback_at(&item_id, ch_start);
                    });
                    row.add_suffix(&ch_play);
                    row.set_activatable_widget(Some(&ch_play));

                    chapters_group.add(&row);
                }
                detail_box.append(&chapters_group);
            }
        }
    }

    // ─── PLAYBACK ──────────────────────────────────────────────────────────

    fn start_playback(&self, item_id: &str) {
        self.start_playback_at(item_id, -1.0);
    }

    fn start_playback_at(&self, item_id: &str, seek_override: f64) {
        // Stop existing playback, close old session
        self.stop_playback();

        let imp = self.imp();
        let client = imp.client.clone();
        let id = item_id.to_string();
        let win = self.clone();

        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            let item_id = id.clone();

            std::thread::spawn(move || {
                let device = DeviceInfo::default();
                let result = client.start_playback(&item_id, &device);
                let _ = tx.send_blocking(result);
            });

            match rx.recv().await {
                Ok(Ok(session)) => {
                    log::info!(
                        "Playback session started: {} - {}",
                        session.display_title.as_deref().unwrap_or(""),
                        session.id
                    );

                    let session_duration = session.duration.unwrap_or(0.0);
                    let session_current = if seek_override >= 0.0 {
                        seek_override
                    } else {
                        session.current_time.unwrap_or(0.0)
                    };

                    *win.imp().session_id.borrow_mut() = Some(session.id.clone());
                    *win.imp().current_item_id.borrow_mut() = Some(id.clone());

                    // Update player bar info
                    win.update_player_info(
                        session
                            .display_title
                            .as_deref()
                            .unwrap_or("Unknown Book"),
                        session.display_author.as_deref().unwrap_or(""),
                        session_duration,
                        session_current,
                    );

                    // Load cover into player bar
                    let client = win.imp().client.clone();
                    let player_cover = win.imp().player_cover.clone();
                    let item_id = id.clone();
                    glib::spawn_future_local(async move {
                        let (tx, rx) = async_channel::bounded(1);
                        std::thread::spawn(move || {
                            let result = client.download_cover(&item_id);
                            let _ = tx.send_blocking(result);
                        });
                        if let Ok(Ok(bytes)) = rx.recv().await {
                            let gbytes = glib::Bytes::from(&bytes);
                            let stream = gio::MemoryInputStream::from_bytes(&gbytes);
                            if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_stream(
                                &stream,
                                gio::Cancellable::NONE,
                            ) {
                                let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
                                player_cover.set_paintable(Some(&texture));
                            }
                        }
                    });

                    win.reveal_player();
                    win.play_audio(&session, session_current);
                    win.start_progress_timer();
                    win.start_sync_timer();
                }
                Ok(Err(e)) => {
                    log::error!("Failed to start playback: {}", e);
                }
                Err(_) => {
                    log::error!("Channel error");
                }
            }
        });
    }

    fn stop_playback(&self) {
        use gstreamer::prelude::ElementExt;
        let imp = self.imp();

        // Stop timers
        if let Some(id) = imp.progress_source.borrow_mut().take() {
            id.remove();
        }
        if let Some(id) = imp.sync_source.borrow_mut().take() {
            id.remove();
        }

        // Close session on server
        if let Some(session_id) = imp.session_id.borrow().as_ref() {
            let client = imp.client.clone();
            let sid = session_id.clone();
            let ct = *imp.current_time.borrow();
            let dur = *imp.duration.borrow();
            std::thread::spawn(move || {
                let _ = client.close_session(&sid, ct, dur);
            });
        }
        *imp.session_id.borrow_mut() = None;

        // Stop GStreamer
        if let Some(pipeline) = imp.pipeline.borrow().as_ref() {
            let _ = pipeline.set_state(gstreamer::State::Null);
        }
        *imp.bus_guard.borrow_mut() = None;
        *imp.pipeline.borrow_mut() = None;
        imp.is_playing.set(false);
        *imp.current_item_id.borrow_mut() = None;
        self.update_play_pause_icon(false);
        self.refresh_detail_play_button();
    }

    fn play_audio(&self, session: &PlaybackSession, start_position: f64) {
        use gstreamer::prelude::*;
        let imp = self.imp();

        let stream_url = session
            .audio_tracks
            .as_ref()
            .and_then(|tracks| tracks.first())
            .and_then(|t| t.content_url.as_ref())
            .map(|url| imp.client.audio_stream_url(url));

        let stream_url = match stream_url {
            Some(url) => url,
            None => {
                log::error!("Audio stream URL not found");
                return;
            }
        };

        log::info!("Starting audio stream: {}", stream_url);

        let playbin = gstreamer::ElementFactory::make("playbin3")
            .property("uri", &stream_url)
            .build()
            .or_else(|_| {
                gstreamer::ElementFactory::make("playbin")
                    .property("uri", &stream_url)
                    .build()
            })
            .expect("Failed to create playbin");

        // Buffering configuration
        if playbin.find_property("buffer-duration").is_some() {
            playbin.set_property("buffer-duration", 15_000_000_000i64);
        }
        if playbin.find_property("buffer-size").is_some() {
            playbin.set_property("buffer-size", 10 * 1024 * 1024i32);
        }

        let bus = playbin.bus().unwrap();
        let pipeline_weak = playbin.downgrade();
        let seek_target = if start_position > 1.0 {
            Some(start_position)
        } else {
            None
        };
        let initial_seek_done = Rc::new(Cell::new(false));
        let initial_seek_clone = initial_seek_done.clone();
        let is_buffering = Rc::new(Cell::new(false));
        let is_buffering_clone = is_buffering.clone();
        let is_playing = imp.is_playing.clone();
        let win_weak = self.downgrade();

        let guard = bus
            .add_watch_local(move |_, msg| {
                use gstreamer::MessageView;
                match msg.view() {
                    MessageView::AsyncDone(_) => {
                        if !initial_seek_clone.get() {
                            initial_seek_clone.set(true);
                            if let Some(pipeline) = pipeline_weak.upgrade() {
                                if let Some(pos) = seek_target {
                                    let clock_pos = gstreamer::ClockTime::from_seconds(pos as u64);
                                    let _ = pipeline.seek_simple(
                                        gstreamer::SeekFlags::FLUSH
                                            | gstreamer::SeekFlags::KEY_UNIT,
                                        clock_pos,
                                    );
                                    log::info!("Seeked to position: {:.0}s", pos);
                                }
                            }
                        }
                        glib::ControlFlow::Continue
                    }
                    MessageView::Buffering(buffering) => {
                        let percent = buffering.percent();
                        log::debug!("Tamponlama: {}%", percent);
                        if let Some(pipeline) = pipeline_weak.upgrade() {
                            if percent < 100 {
                                if !is_buffering_clone.get() {
                                    let _ = pipeline.set_state(gstreamer::State::Paused);
                                    is_buffering_clone.set(true);
                                }
                            } else if is_buffering_clone.get() {
                                let _ = pipeline.set_state(gstreamer::State::Playing);
                                is_buffering_clone.set(false);
                            }
                        }
                        glib::ControlFlow::Continue
                    }
                    MessageView::Error(err) => {
                        log::error!("GStreamer error: {} - {:?}", err.error(), err.debug());
                        glib::ControlFlow::Break
                    }
                    MessageView::Eos(_) => {
                        log::info!("Audio stream ended");
                        is_playing.set(false);
                        if let Some(win) = win_weak.upgrade() {
                            win.update_play_pause_icon(false);
                            win.refresh_detail_play_button();
                        }
                        glib::ControlFlow::Break
                    }
                    _ => glib::ControlFlow::Continue,
                }
            })
            .expect("Bus watch eklenemedi");

        *imp.bus_guard.borrow_mut() = Some(guard);

        let _ = playbin.set_state(gstreamer::State::Playing);
        imp.is_playing.set(true);
        self.update_play_pause_icon(true);
        self.refresh_detail_play_button();
        *imp.pipeline.borrow_mut() = Some(playbin);
    }

    // ─── PROGRESS & SYNC TIMERS ────────────────────────────────────────────

    fn start_progress_timer(&self) {
        let imp = self.imp();
        // Remove old timer
        if let Some(id) = imp.progress_source.borrow_mut().take() {
            id.remove();
        }

        let win = self.clone();
        let source_id = glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
            use gstreamer::prelude::*;
            let imp = win.imp();
            if let Some(pipeline) = imp.pipeline.borrow().as_ref() {
                if imp.is_playing.get() {
                    if let Some(pos) = pipeline.query_position::<gstreamer::ClockTime>() {
                        let secs = pos.seconds() as f64;
                        imp.updating_slider.set(true);
                        imp.position_scale.set_value(secs);
                        imp.updating_slider.set(false);
                        imp.position_label.set_text(&format_time(secs));
                        *imp.current_time.borrow_mut() = secs;
                    }
                }
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });
        *imp.progress_source.borrow_mut() = Some(source_id);
    }

    fn start_sync_timer(&self) {
        let imp = self.imp();
        // Remove old timer
        if let Some(id) = imp.sync_source.borrow_mut().take() {
            id.remove();
        }

        let win = self.clone();
        let source_id = glib::timeout_add_local(std::time::Duration::from_secs(15), move || {
            let imp = win.imp();
            if imp.pipeline.borrow().is_none() {
                return glib::ControlFlow::Break;
            }

            let session_id = imp.session_id.borrow().clone();
            if let Some(sid) = session_id {
                let current = *imp.current_time.borrow();
                let duration = *imp.duration.borrow();
                let client = imp.client.clone();

                std::thread::spawn(move || match client.sync_session(&sid, current, duration) {
                    Ok(_) => {
                        log::debug!("Oturum senkronize edildi: {:.0}s", current)
                    }
                    Err(e) => {
                        log::warn!("Sync error: {}", e)
                    }
                });
            }
            glib::ControlFlow::Continue
        });
        *imp.sync_source.borrow_mut() = Some(source_id);
    }
}

fn format_time(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}

fn extract_access_token(url: &str) -> Option<String> {
    extract_url_param(url, "access_token")
        .or_else(|| extract_url_param(url, "accessToken"))
        .or_else(|| extract_url_param(url, "token"))
}

fn extract_url_param(url: &str, key: &str) -> Option<String> {
    for segment in [url.split('?').nth(1), url.split('#').nth(1)]
        .into_iter()
        .flatten()
    {
        for param in segment.split('&') {
            if let Some((k, v)) = param.split_once('=') {
                if k == key && !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}
