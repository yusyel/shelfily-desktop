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

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct StoredSession {
    server_url: String,
    library_id: String,
    #[serde(default)]
    token: String,
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct SecretSessionTokens {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibrarySortMode {
    NewlyAdded,
    AuthorAsc,
    TitleAsc,
    RecentlyPlayed,
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
        pub detail_play_btn: RefCell<Option<gtk::Button>>,
        pub detail_play_item_id: RefCell<Option<String>>,
        pub chapter_indicators: RefCell<Vec<(f64, f64, gtk::Box)>>,
        pub detail_is_finished: Rc<Cell<bool>>,
        pub bookmarks: RefCell<Vec<Bookmark>>,
        pub bookmarks_group: RefCell<Option<adw::PreferencesGroup>>,
        pub bookmarks_section: RefCell<Option<gtk::Box>>,
        pub library_search_entry: RefCell<Option<gtk::SearchEntry>>,
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
        pub toast_overlay: adw::ToastOverlay,
        pub nav_view: RefCell<Option<adw::NavigationView>>,
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
                detail_play_btn: RefCell::new(None),
                detail_play_item_id: RefCell::new(None),
                chapter_indicators: RefCell::new(Vec::new()),
                detail_is_finished: Rc::new(Cell::new(false)),
                bookmarks: RefCell::new(Vec::new()),
                bookmarks_group: RefCell::new(None),
                bookmarks_section: RefCell::new(None),
                library_search_entry: RefCell::new(None),
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
                toast_overlay: adw::ToastOverlay::new(),
                nav_view: RefCell::new(None),
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
    fn preferred_session_token(
        access_token: &str,
        legacy_token: &str,
        refresh_token: &str,
    ) -> String {
        if refresh_token.is_empty() && !legacy_token.is_empty() {
            legacy_token.to_string()
        } else if !access_token.is_empty() {
            access_token.to_string()
        } else {
            legacy_token.to_string()
        }
    }

    fn log_secret_service_error(action: &str, err: &glib::Error) {
        let details = err.to_string();
        if details.contains("org.freedesktop.DBus.Error.ServiceUnknown") {
            log::warn!(
                "Could not {} the session token because no Secret Service is available. \
If this is a Flatpak build, ensure org.freedesktop.secrets is allowed. Original error: {}",
                action,
                details
            );
        } else {
            log::warn!("Could not {} the session token: {}", action, details);
        }
    }

    fn secret_schema() -> libsecret::Schema {
        let mut attrs = HashMap::new();
        attrs.insert("account", libsecret::SchemaAttributeType::String);
        libsecret::Schema::new(
            "io.github.yusyel.ShelfilyDesktop",
            libsecret::SchemaFlags::NONE,
            attrs,
        )
    }

    fn secret_attrs<'a>(account: &'a str) -> HashMap<&'static str, &'a str> {
        let mut attrs = HashMap::new();
        attrs.insert("account", account);
        attrs
    }

    fn legacy_secret_account() -> &'static str {
        "default"
    }

    fn secret_account_for_server(server_url: &str) -> String {
        let trimmed = server_url.trim_end_matches('/');
        if trimmed.is_empty() {
            Self::legacy_secret_account().to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn parse_secret_tokens(raw: &str) -> SecretSessionTokens {
        serde_json::from_str::<SecretSessionTokens>(raw).unwrap_or_else(|_| SecretSessionTokens {
            access_token: raw.to_string(),
            refresh_token: String::new(),
        })
    }

    fn store_secret_tokens(
        &self,
        server_url: &str,
        access_token: &str,
        refresh_token: &str,
    ) -> bool {
        let account = Self::secret_account_for_server(server_url);
        let payload = match serde_json::to_string(&SecretSessionTokens {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
        }) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!("Could not serialize session tokens: {}", err);
                return false;
            }
        };
        let schema = Self::secret_schema();
        match libsecret::password_store_sync(
            Some(&schema),
            Self::secret_attrs(&account),
            Some(libsecret::COLLECTION_DEFAULT.as_str()),
            "Shelfily Desktop Session Tokens",
            &payload,
            gio::Cancellable::NONE,
        ) {
            Ok(_) => true,
            Err(err) => {
                Self::log_secret_service_error("store", &err);
                false
            }
        }
    }

    fn lookup_secret_tokens_for_account(account: &str) -> SecretSessionTokens {
        let schema = Self::secret_schema();
        match libsecret::password_lookup_sync(
            Some(&schema),
            Self::secret_attrs(account),
            gio::Cancellable::NONE,
        ) {
            Ok(Some(tokens)) => Self::parse_secret_tokens(tokens.as_ref()),
            Ok(None) => SecretSessionTokens::default(),
            Err(err) => {
                Self::log_secret_service_error("read", &err);
                SecretSessionTokens::default()
            }
        }
    }

    fn lookup_secret_tokens(&self, server_url: &str) -> SecretSessionTokens {
        let account = Self::secret_account_for_server(server_url);
        let tokens = Self::lookup_secret_tokens_for_account(&account);
        if !tokens.access_token.is_empty() || account == Self::legacy_secret_account() {
            return tokens;
        }

        Self::lookup_secret_tokens_for_account(Self::legacy_secret_account())
    }

    fn clear_secret_tokens_for_account(account: &str) {
        let schema = Self::secret_schema();
        if let Err(err) = libsecret::password_clear_sync(
            Some(&schema),
            Self::secret_attrs(account),
            gio::Cancellable::NONE,
        ) {
            Self::log_secret_service_error("clear", &err);
        }
    }

    fn clear_secret_tokens(&self, server_url: &str) {
        let account = Self::secret_account_for_server(server_url);
        Self::clear_secret_tokens_for_account(&account);
        if account != Self::legacy_secret_account() {
            Self::clear_secret_tokens_for_account(Self::legacy_secret_account());
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
        let mut had_legacy_token = false;
        if let Ok(content) = fs::read_to_string(path) {
            had_legacy_token = content.contains("\"token\":");
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

        if had_legacy_token {
            self.write_stored_session(&session);
        }

        session
    }

    fn clear_stored_session(&self) {
        let saved = self.read_stored_session();
        self.write_stored_session(&StoredSession::default());
        let _ = fs::remove_file(Self::session_file_path());
        self.clear_secret_tokens(&saved.server_url);
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
        let loading_page = self.build_loading_page();
        let library_page = self.build_library_page();

        let nav_view = adw::NavigationView::new();
        let lib_nav_page = adw::NavigationPage::builder()
            .title("Shelfily")
            .tag("library")
            .child(&library_page)
            .build();
        nav_view.add(&lib_nav_page);
        *imp.nav_view.borrow_mut() = Some(nav_view.clone());

        imp.stack.add_named(&login_page, Some("login"));
        imp.stack.add_named(&loading_page, Some("loading"));
        imp.stack.add_named(&nav_view, Some("library"));
        imp.stack.set_visible_child_name(if self.has_saved_session_candidate() {
            "loading"
        } else {
            "login"
        });

        // Build the persistent bottom player bar
        self.build_player_bar();

        // Main layout: stack on top (expanding), player bar pinned at bottom
        let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        main_box.add_css_class("app-root");
        imp.stack.set_vexpand(true);
        main_box.append(&imp.stack);
        main_box.append(&imp.player_bar);

        imp.toast_overlay.set_child(Some(&main_box));
        self.set_content(Some(&imp.toast_overlay));
        self.install_resize_observer();

        // Try auto-login from saved credentials
        self.try_restore_session();
    }

    fn has_saved_session_candidate(&self) -> bool {
        let saved = self.read_stored_session();
        if saved.server_url.is_empty() {
            return false;
        }

        let secret_tokens = self.lookup_secret_tokens(&saved.server_url);
        !secret_tokens.access_token.is_empty()
            || !saved.access_token.is_empty()
            || !saved.token.is_empty()
    }

    fn install_styles(&self) {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            "
            .shelfily-window { background: @window_bg_color; }
            .app-root { }
            .player-bar {
                margin: 6px 12px 12px 12px;
                border-radius: 18px;
                padding: 8px 14px;
                background: @card_bg_color;
                border: 1px solid alpha(@borders, 0.5);
            }
            .book-card {
                padding: 8px;
                border-radius: 14px;
                background: @card_bg_color;
                border: 1px solid alpha(@borders, 0.6);
                transition: border-color 150ms ease;
            }
            .book-card:hover {
                border-color: @accent_color;
            }
            .book-cover-frame { border-radius: 10px; }
            .cover-play-btn {
                transition: opacity 200ms ease;
            }
            .detail-play-btn {
                min-width: 64px;
                min-height: 64px;
            }
            .seek-scale trough {
                min-height: 4px;
                border-radius: 4px;
                transition: min-height 150ms ease;
            }
            .seek-scale:hover trough {
                min-height: 8px;
            }
            .seek-scale slider {
                opacity: 0;
                transition: opacity 150ms ease;
            }
            .seek-scale:hover slider {
                opacity: 1;
            }
            .chapter-indicator {
                min-width: 10px;
                min-height: 10px;
                border-radius: 999px;
                background-color: alpha(@window_fg_color, 0.15);
            }
            .chapter-indicator.completed {
                background-color: @success_color;
            }
            .chapter-indicator.playing {
                background-color: @accent_color;
                box-shadow: 0 0 0 4px alpha(@accent_color, 0.25);
            }
            .chapter-indicator.unplayed {
                background-color: transparent;
                box-shadow: inset 0 0 0 1.5px alpha(@window_fg_color, 0.35);
            }
            .login-surface {
                border-radius: 18px;
                padding: 20px;
                background: @card_bg_color;
                border: 1px solid alpha(@borders, 0.5);
            }
            .compact .player-bar { margin: 4px 6px 8px 6px; border-radius: 12px; padding: 6px 10px; }
            .compact .book-card { padding: 6px; border-radius: 10px; }
            .compact .login-surface { padding: 12px; border-radius: 14px; }
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
        let access_token = imp.client.access_token().unwrap_or_default();
        let refresh_token = imp.client.refresh_token().unwrap_or_default();
        let mut session = StoredSession {
            server_url: imp.client.server_url(),
            library_id: imp.library_id.borrow().clone(),
            token: String::new(),
            access_token: String::new(),
            refresh_token: String::new(),
        };

        if !access_token.is_empty()
            && !self.store_secret_tokens(&session.server_url, &access_token, &refresh_token)
        {
            log::warn!("Falling back to storing the session tokens in the local session file");
            session.access_token = access_token.clone();
            session.refresh_token = refresh_token.clone();
            session.token = access_token;
        }

        self.write_stored_session(&session);
        log::info!("Session credentials saved");
    }

    fn try_restore_session(&self) {
        let saved = self.read_stored_session();
        let server_url = saved.server_url;
        let secret_tokens = self.lookup_secret_tokens(&server_url);
        let access_token_came_from_file = secret_tokens.access_token.is_empty()
            && (!saved.access_token.is_empty() || !saved.token.is_empty());
        let refresh_token_came_from_file =
            secret_tokens.refresh_token.is_empty() && !saved.refresh_token.is_empty();
        let mut refresh_token = secret_tokens.refresh_token;
        if refresh_token.is_empty() {
            refresh_token = saved.refresh_token.clone();
        }
        let mut access_token = if !refresh_token.is_empty() {
            if secret_tokens.access_token.is_empty() {
                saved.access_token.clone()
            } else {
                secret_tokens.access_token.clone()
            }
        } else {
            Self::preferred_session_token(
                if secret_tokens.access_token.is_empty() {
                    &saved.access_token
                } else {
                    &secret_tokens.access_token
                },
                &saved.token,
                &refresh_token,
            )
        };
        if access_token.is_empty() {
            access_token = Self::preferred_session_token(
                &saved.access_token,
                &saved.token,
                &refresh_token,
            );
        }
        let library_id = saved.library_id;

        if server_url.is_empty() || access_token.is_empty() {
            self.imp().stack.set_visible_child_name("login");
            return;
        }

        log::info!("Found saved session, trying login...");
        let imp = self.imp();
        imp.client.set_server(&server_url);
        imp.client.set_tokens(&access_token, &refresh_token);
        if !library_id.is_empty() {
            *imp.library_id.borrow_mut() = library_id;
        }

        // Verify the token is still valid by trying a simple API call
        let client = imp.client.clone();
        let win = self.clone();
        let saved_library_id = imp.library_id.borrow().clone();
        let verification_library_id = saved_library_id.clone();
        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = if verification_library_id.is_empty() {
                    client.get_libraries().map(|_| ())
                } else {
                    client.get_library_items(&verification_library_id).map(|_| ())
                };
                let _ = tx.send_blocking(result);
            });
            match rx.recv().await {
                Ok(Ok(_)) => {
                    log::info!("Saved session is valid, loading library");
                    if access_token_came_from_file || refresh_token_came_from_file {
                        log::info!("Migrating stored session tokens to secret storage");
                    }
                    win.save_credentials();
                    win.imp().stack.set_visible_child_name("library");
                    win.load_library();
                }
                Ok(Err(crate::api::ApiError::Auth(e))) => {
                    log::warn!("Saved token is invalid ({}), clearing credentials", e);
                    win.clear_stored_session();
                    win.imp().stack.set_visible_child_name("login");
                }
                _ => {
                    log::warn!("Could not reach server, keeping saved credentials");
                    win.imp().stack.set_visible_child_name("login");
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
        imp.play_pause_btn.set_tooltip_text(Some("Play / Pause"));
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

        let bookmark_btn = gtk::Button::from_icon_name("bookmark-new-symbolic");
        bookmark_btn.add_css_class("flat");
        bookmark_btn.set_tooltip_text(Some("Bookmark current position"));
        let win = self.clone();
        bookmark_btn.connect_clicked(move |_| {
            win.add_bookmark_at_current_position();
        });
        controls.append(&bookmark_btn);

        center_box.append(&controls);

        // Progress row
        let progress_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        imp.position_label.add_css_class("caption");
        imp.position_label.add_css_class("dim-label");
        imp.position_label.set_width_chars(6);
        imp.duration_label.add_css_class("caption");
        imp.duration_label.add_css_class("dim-label");
        imp.duration_label.set_width_chars(6);

        imp.position_scale.add_css_class("seek-scale");
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
        self.refresh_chapter_indicators(current);
    }

    fn update_play_pause_icon(&self, playing: bool) {
        let icon = if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        };
        self.imp().play_pause_btn.set_icon_name(icon);
    }

    fn apply_chapter_indicators(&self, current_time: f64) {
        let imp = self.imp();
        let force_finished = imp.detail_is_finished.get();
        for (start, end, indicator) in imp.chapter_indicators.borrow().iter() {
            indicator.remove_css_class("completed");
            indicator.remove_css_class("playing");
            indicator.remove_css_class("unplayed");
            if force_finished || current_time >= *end {
                indicator.add_css_class("completed");
            } else if current_time >= *start && current_time < *end {
                indicator.add_css_class("playing");
            } else {
                indicator.add_css_class("unplayed");
            }
        }
    }

    fn show_author_books(&self, author_id: Option<&str>, author_name: &str) {
        let imp = self.imp();
        let nav_view = match imp.nav_view.borrow().clone() {
            Some(v) => v,
            None => return,
        };

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        scrolled.set_vscrollbar_policy(gtk::PolicyType::Automatic);

        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(960);
        clamp.set_margin_top(24);
        clamp.set_margin_bottom(24);
        clamp.set_margin_start(24);
        clamp.set_margin_end(24);

        let container = gtk::Box::new(gtk::Orientation::Vertical, 16);
        clamp.set_child(Some(&container));
        scrolled.set_child(Some(&clamp));

        let toolbar_view = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scrolled));

        let title = format!("Books by {}", author_name);
        let nav_page = adw::NavigationPage::builder()
            .title(&title)
            .child(&toolbar_view)
            .build();
        nav_view.push(&nav_page);

        let loading = adw::StatusPage::new();
        let spinner = adw::Spinner::new();
        spinner.set_size_request(48, 48);
        loading.set_child(Some(&spinner));
        loading.set_title("Loading");
        container.append(&loading);

        if let Some(id) = author_id {
            let client = imp.client.clone();
            let win = self.clone();
            let id_owned = id.to_string();
            let name_owned = author_name.to_string();
            let container_clone = container.clone();
            let loading_clone = loading.clone();
            glib::spawn_future_local(async move {
                let (tx, rx) = async_channel::bounded(1);
                std::thread::spawn(move || {
                    let result = client.get_author_with_items(&id_owned);
                    let _ = tx.send_blocking(result);
                });
                match rx.recv().await {
                    Ok(Ok(author)) => {
                        let items = author.library_items.unwrap_or_default();
                        win.populate_author_books(&container_clone, &loading_clone, &items, &name_owned);
                    }
                    Ok(Err(e)) => {
                        log::warn!("Fetch author items failed: {}, falling back to local filter", e);
                        let items = win.filter_library_items_by_author_name(&name_owned);
                        win.populate_author_books(&container_clone, &loading_clone, &items, &name_owned);
                    }
                    Err(_) => {}
                }
            });
        } else {
            let items = self.filter_library_items_by_author_name(author_name);
            self.populate_author_books(&container, &loading, &items, author_name);
        }
    }

    fn filter_library_items_by_author_name(&self, author_name: &str) -> Vec<LibraryItem> {
        let needle = author_name.to_lowercase();
        self.imp()
            .library_items
            .borrow()
            .iter()
            .filter(|item| {
                let md = item.media.as_ref().and_then(|m| m.metadata.as_ref());
                let an = md
                    .and_then(|m| m.author_name.as_deref())
                    .unwrap_or("")
                    .to_lowercase();
                let lf = md
                    .and_then(|m| m.author_name_lf.as_deref())
                    .unwrap_or("")
                    .to_lowercase();
                an.contains(&needle) || lf.contains(&needle)
            })
            .cloned()
            .collect()
    }

    fn populate_author_books(
        &self,
        container: &gtk::Box,
        loading: &adw::StatusPage,
        items: &[LibraryItem],
        author_name: &str,
    ) {
        container.remove(loading);

        if items.is_empty() {
            let empty = adw::StatusPage::new();
            empty.set_icon_name(Some("user-info-symbolic"));
            empty.set_title("No books found");
            empty.set_description(Some(&format!(
                "No books by {} in your library",
                author_name
            )));
            container.append(&empty);
            return;
        }

        let count_label =
            gtk::Label::new(Some(&format!("{} book(s) by {}", items.len(), author_name)));
        count_label.add_css_class("dim-label");
        count_label.add_css_class("caption");
        count_label.set_halign(gtk::Align::Start);
        container.append(&count_label);

        let flowbox = gtk::FlowBox::new();
        flowbox.set_selection_mode(gtk::SelectionMode::None);
        flowbox.set_homogeneous(true);
        flowbox.set_max_children_per_line(6);
        flowbox.set_min_children_per_line(2);
        flowbox.set_column_spacing(12);
        flowbox.set_row_spacing(12);
        for item in items {
            let card = self.create_book_card(item);
            flowbox.append(&card);
        }
        container.append(&flowbox);
    }

    fn refresh_chapter_indicators(&self, current_time: f64) {
        let imp = self.imp();
        let detail_is_current = imp.detail_play_item_id.borrow().as_deref()
            == imp.current_item_id.borrow().as_deref();
        if !detail_is_current {
            return;
        }
        self.apply_chapter_indicators(current_time);
    }

    // ─── BOOKMARKS ──────────────────────────────────────────────────────────

    fn add_bookmark_at_current_position(&self) {
        let imp = self.imp();
        let item_id = match imp.current_item_id.borrow().clone() {
            Some(id) => id,
            None => {
                let toast = adw::Toast::new("Start playback first to add a bookmark");
                imp.toast_overlay.add_toast(toast);
                return;
            }
        };
        let time = *imp.current_time.borrow();
        let default_title = format!("Bookmark at {}", format_time(time));
        self.prompt_bookmark_dialog(&item_id, time, &default_title, true);
    }

    fn prompt_bookmark_dialog(
        &self,
        item_id: &str,
        time: f64,
        default_title: &str,
        is_new: bool,
    ) {
        let dialog = adw::AlertDialog::new(
            Some(if is_new { "New Bookmark" } else { "Edit Bookmark" }),
            Some(&format!("At {}", format_time(time))),
        );

        let entry = gtk::Entry::new();
        entry.set_text(default_title);
        entry.set_placeholder_text(Some("Bookmark note"));
        entry.set_activates_default(true);
        entry.set_hexpand(true);
        dialog.set_extra_child(Some(&entry));

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("save", if is_new { "Add" } else { "Save" });
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");

        let win = self.clone();
        let item_id_owned = item_id.to_string();
        let entry_clone = entry.clone();
        dialog.connect_response(None, move |dlg, response| {
            if response == "save" {
                let title = entry_clone.text().to_string();
                let title = if title.trim().is_empty() {
                    format!("Bookmark at {}", format_time(time))
                } else {
                    title
                };
                if is_new {
                    win.create_bookmark(&item_id_owned, &title, time);
                } else {
                    win.update_bookmark(&item_id_owned, &title, time);
                }
            }
            dlg.close();
        });

        dialog.present(Some(self));
    }

    fn create_bookmark(&self, item_id: &str, title: &str, time: f64) {
        let client = self.imp().client.clone();
        let item_id_owned = item_id.to_string();
        let title_owned = title.to_string();
        let win = self.clone();
        let (tx, rx) = async_channel::bounded::<Result<Bookmark, String>>(1);
        std::thread::spawn(move || {
            let result = client
                .create_bookmark(&item_id_owned, &title_owned, time)
                .map_err(|e| e.to_string());
            let _ = tx.send_blocking(result);
        });
        let detail_item_id = self.imp().detail_play_item_id.borrow().clone();
        let target_item_id = item_id.to_string();
        glib::spawn_future_local(async move {
            match rx.recv().await {
                Ok(Ok(_)) => {
                    let toast = adw::Toast::new("Bookmark added");
                    win.imp().toast_overlay.add_toast(toast);
                    if detail_item_id.as_deref() == Some(target_item_id.as_str()) {
                        win.load_bookmarks(&target_item_id);
                    }
                }
                Ok(Err(err)) => {
                    log::warn!("Create bookmark failed: {}", err);
                    let toast =
                        adw::Toast::new(&format!("Failed to add bookmark: {}", err));
                    win.imp().toast_overlay.add_toast(toast);
                }
                Err(err) => {
                    log::warn!("Bookmark channel error: {}", err);
                }
            }
        });
    }

    fn update_bookmark(&self, item_id: &str, title: &str, time: f64) {
        let client = self.imp().client.clone();
        let item_id_owned = item_id.to_string();
        let title_owned = title.to_string();
        let target_item_id = item_id.to_string();
        let win = self.clone();
        let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
        std::thread::spawn(move || {
            let result = client
                .update_bookmark(&item_id_owned, &title_owned, time)
                .map_err(|e| e.to_string());
            let _ = tx.send_blocking(result);
        });
        let detail_item_id = self.imp().detail_play_item_id.borrow().clone();
        glib::spawn_future_local(async move {
            match rx.recv().await {
                Ok(Ok(())) => {
                    let toast = adw::Toast::new("Bookmark updated");
                    win.imp().toast_overlay.add_toast(toast);
                    if detail_item_id.as_deref() == Some(target_item_id.as_str()) {
                        win.load_bookmarks(&target_item_id);
                    }
                }
                Ok(Err(err)) => {
                    log::warn!("Update bookmark failed: {}", err);
                    let toast =
                        adw::Toast::new(&format!("Failed to update bookmark: {}", err));
                    win.imp().toast_overlay.add_toast(toast);
                }
                Err(err) => {
                    log::warn!("Bookmark channel error: {}", err);
                }
            }
        });
    }

    fn delete_bookmark(&self, item_id: &str, time: f64) {
        let client = self.imp().client.clone();
        let item_id_owned = item_id.to_string();
        let target_item_id = item_id.to_string();
        let win = self.clone();
        let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
        std::thread::spawn(move || {
            let result = client
                .delete_bookmark(&item_id_owned, time)
                .map_err(|e| e.to_string());
            let _ = tx.send_blocking(result);
        });
        let detail_item_id = self.imp().detail_play_item_id.borrow().clone();
        glib::spawn_future_local(async move {
            match rx.recv().await {
                Ok(Ok(())) => {
                    let toast = adw::Toast::new("Bookmark removed");
                    win.imp().toast_overlay.add_toast(toast);
                    if detail_item_id.as_deref() == Some(target_item_id.as_str()) {
                        win.load_bookmarks(&target_item_id);
                    }
                }
                Ok(Err(err)) => {
                    log::warn!("Delete bookmark failed: {}", err);
                    let toast =
                        adw::Toast::new(&format!("Failed to remove bookmark: {}", err));
                    win.imp().toast_overlay.add_toast(toast);
                }
                Err(err) => {
                    log::warn!("Bookmark channel error: {}", err);
                }
            }
        });
    }

    fn load_bookmarks(&self, item_id: &str) {
        let client = self.imp().client.clone();
        let item_id_owned = item_id.to_string();
        let target_item_id = item_id.to_string();
        let win = self.clone();
        let (tx, rx) = async_channel::bounded::<Result<Vec<Bookmark>, String>>(1);
        std::thread::spawn(move || {
            let result = client
                .get_bookmarks_for_item(&item_id_owned)
                .map_err(|e| e.to_string());
            let _ = tx.send_blocking(result);
        });
        glib::spawn_future_local(async move {
            match rx.recv().await {
                Ok(Ok(bookmarks)) => {
                    let detail_item_id = win.imp().detail_play_item_id.borrow().clone();
                    if detail_item_id.as_deref() == Some(target_item_id.as_str()) {
                        *win.imp().bookmarks.borrow_mut() = bookmarks;
                        win.render_bookmarks(&target_item_id);
                    }
                }
                Ok(Err(err)) => log::warn!("Load bookmarks failed: {}", err),
                Err(err) => log::warn!("Bookmark channel error: {}", err),
            }
        });
    }

    fn render_bookmarks(&self, item_id: &str) {
        let imp = self.imp();
        let section = match imp.bookmarks_section.borrow().clone() {
            Some(s) => s,
            None => return,
        };

        if let Some(old_group) = imp.bookmarks_group.borrow_mut().take() {
            section.remove(&old_group);
        }

        let bookmarks = imp.bookmarks.borrow().clone();
        if bookmarks.is_empty() {
            section.set_visible(false);
            return;
        }
        section.set_visible(true);

        let group = adw::PreferencesGroup::new();

        for bookmark in bookmarks.iter() {
            let time = bookmark.time.unwrap_or(0.0);
            let title = bookmark
                .title
                .clone()
                .unwrap_or_else(|| format!("Bookmark at {}", format_time(time)));

            let row = adw::ActionRow::new();
            row.set_title(&title);
            row.set_subtitle(&format_time(time));

            let icon = gtk::Image::from_icon_name("user-bookmarks-symbolic");
            icon.add_css_class("dim-label");
            row.add_prefix(&icon);

            let play_btn = gtk::Button::from_icon_name("media-playback-start-symbolic");
            play_btn.add_css_class("flat");
            play_btn.add_css_class("circular");
            play_btn.set_valign(gtk::Align::Center);
            play_btn.set_tooltip_text(Some("Play from this position"));
            let win_play = self.clone();
            let item_id_play = item_id.to_string();
            play_btn.connect_clicked(move |_| {
                win_play.start_playback_at(&item_id_play, time);
            });
            row.add_suffix(&play_btn);

            let edit_btn = gtk::Button::from_icon_name("document-edit-symbolic");
            edit_btn.add_css_class("flat");
            edit_btn.add_css_class("circular");
            edit_btn.set_valign(gtk::Align::Center);
            edit_btn.set_tooltip_text(Some("Edit note"));
            let win_edit = self.clone();
            let item_id_edit = item_id.to_string();
            let title_edit = title.clone();
            edit_btn.connect_clicked(move |_| {
                win_edit.prompt_bookmark_dialog(&item_id_edit, time, &title_edit, false);
            });
            row.add_suffix(&edit_btn);

            let del_btn = gtk::Button::from_icon_name("user-trash-symbolic");
            del_btn.add_css_class("flat");
            del_btn.add_css_class("circular");
            del_btn.set_valign(gtk::Align::Center);
            del_btn.set_tooltip_text(Some("Remove bookmark"));
            let win_del = self.clone();
            let item_id_del = item_id.to_string();
            del_btn.connect_clicked(move |_| {
                win_del.delete_bookmark(&item_id_del, time);
            });
            row.add_suffix(&del_btn);

            row.set_activatable_widget(Some(&play_btn));
            group.add(&row);
        }

        section.append(&group);
        *imp.bookmarks_group.borrow_mut() = Some(group);
    }

    fn refresh_detail_play_button(&self) {
        let imp = self.imp();
        let detail_item_id = imp.detail_play_item_id.borrow().clone();
        let current_item_id = imp.current_item_id.borrow().clone();
        let is_current_item = detail_item_id.is_some() && detail_item_id == current_item_id;
        let is_playing_current = is_current_item && imp.is_playing.get();

        if let Some(btn) = imp.detail_play_btn.borrow().as_ref() {
            btn.set_icon_name(if is_playing_current {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            });
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

    fn build_loading_page(&self) -> gtk::Widget {
        let toolbar_view = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new("Shelfily Desktop", "")));
        toolbar_view.add_top_bar(&header);

        let status = adw::StatusPage::new();
        status.set_icon_name(Some("audio-headphones-symbolic"));
        status.set_title("Restoring Session");
        status.set_description(Some("Checking your saved login..."));

        let spinner = adw::Spinner::new();
        spinner.set_size_request(32, 32);
        status.set_child(Some(&spinner));

        toolbar_view.set_content(Some(&status));
        toolbar_view.upcast()
    }

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

                        let legacy_token =
                            login_resp.user.token.as_deref().unwrap_or_default().to_string();
                        let raw_access_token = login_resp
                            .user
                            .access_token
                            .as_deref()
                            .unwrap_or_default()
                            .to_string();
                        let refresh_token = login_resp
                            .user
                            .refresh_token
                            .as_deref()
                            .unwrap_or_default()
                            .to_string();
                        let access_token = login_resp
                            .user
                            .access_token
                            .as_deref()
                            .or(login_resp.user.token.as_deref())
                            .unwrap_or_default();
                        let session_token = Self::preferred_session_token(
                            if raw_access_token.is_empty() {
                                access_token
                            } else {
                                &raw_access_token
                            },
                            &legacy_token,
                            &refresh_token,
                        );
                        let default_lib = login_resp.user_default_library_id.unwrap_or_default();

                        win_c.on_login_success(
                            &server_url,
                            &session_token,
                            &refresh_token,
                            &default_lib,
                        );
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

    fn on_login_success(
        &self,
        server_url: &str,
        access_token: &str,
        refresh_token: &str,
        default_library_id: &str,
    ) {
        let imp = self.imp();
        imp.client.set_server(server_url);
        imp.client.set_tokens(access_token, refresh_token);

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
                log::debug!("WebView URI changed during OAuth flow");

                if let Some(access_token) = extract_access_token(&uri_str) {
                    token_found_c.set(true);
                    log::info!("OAuth token received");
                    let refresh_token = extract_refresh_token(&uri_str).unwrap_or_default();
                    if !refresh_token.is_empty() {
                        dlg.close();
                        win.on_login_success(&srv, &access_token, &refresh_token, "");
                        return;
                    }

                    let win_c = win.clone();
                    let dlg_c = dlg.clone();
                    let srv_c = srv.clone();
                    let access_token_c = access_token.clone();
                    if let Some(cookie_manager) =
                        wv.network_session().and_then(|session| session.cookie_manager())
                    {
                        let cookie_uri = srv_c.clone();
                        cookie_manager.cookies(&srv_c, gio::Cancellable::NONE, move |result| {
                            let refresh_token = match result {
                                Ok(cookies) => cookies
                                    .into_iter()
                                    .find_map(|mut cookie| {
                                        let name = cookie.name()?.to_string();
                                        if name == "refresh_token" {
                                            cookie.value().map(|value| value.to_string())
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_default(),
                                Err(err) => {
                                    log::warn!(
                                        "Could not read OAuth refresh token cookie: {}",
                                        err
                                    );
                                    String::new()
                                }
                            };
                            if refresh_token.is_empty() {
                                log::warn!(
                                    "OAuth login completed without a refresh token; \
the session may expire and require signing in again"
                                );
                            }
                            dlg_c.close();
                            win_c.on_login_success(
                                &cookie_uri,
                                &access_token_c,
                                &refresh_token,
                                "",
                            );
                        });
                    } else {
                        log::warn!(
                            "OAuth cookie manager is unavailable; continuing without a refresh token"
                        );
                        dlg.close();
                        win.on_login_success(&srv, &access_token, "", "");
                    }
                }
            }
        });

        dialog.present();
    }

    pub fn logout(&self) {
        self.stop_playback();
        self.hide_player();

        // Reset NavigationView to library root before switching to login
        if let Some(nav_view) = self.imp().nav_view.borrow().as_ref() {
            while nav_view.pop() {}
        }

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

        let status = adw::StatusPage::new();
        status.set_icon_name(Some("dialog-error-symbolic"));
        status.set_title("Could Not Load Library");
        status.set_description(Some(message));
        status.set_vexpand(true);

        let retry_btn = gtk::Button::with_label("Try Again");
        retry_btn.add_css_class("pill");
        retry_btn.add_css_class("suggested-action");
        let win = self.clone();
        retry_btn.connect_clicked(move |_| {
            win.load_library();
        });
        status.set_child(Some(&retry_btn));

        flowbox.append(&status);
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

        let sort_played_btn = gtk::Button::with_label("Recently Played");
        sort_played_btn.add_css_class("flat");
        let win = self.clone();
        sort_played_btn.connect_clicked(move |_| {
            win.imp()
                .library_sort_mode
                .set(LibrarySortMode::RecentlyPlayed);
            win.render_library();
            win.render_continue_listening();
        });
        sort_box.append(&sort_played_btn);

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
        *self.imp().library_search_entry.borrow_mut() = Some(search_entry.clone());
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

        let continue_page = view_stack.add_titled(&continue_scrolled, Some("continue"), "Continue");
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
        let loading_status = adw::StatusPage::new();
        loading_status.set_title("Loading Books");
        loading_status.set_description(Some("Fetching your library..."));
        let loading_spinner = adw::Spinner::new();
        loading_spinner.set_size_request(32, 32);
        loading_status.set_child(Some(&loading_spinner));

        let content_stack = gtk::Stack::new();
        content_stack.add_named(&loading_status, Some("loading"));
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
                    if win.imp().client.is_authenticated() {
                        win.save_credentials();
                    }
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
            LibrarySortMode::RecentlyPlayed => {
                items.sort_by_key(|item| Reverse(Self::item_last_played_timestamp(item)));
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

    fn item_last_played_timestamp(item: &LibraryItem) -> u64 {
        item.user_media_progress
            .as_ref()
            .and_then(|p| p.last_update)
            .unwrap_or(0)
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
                    log::info!("Continue listening items: {}", items.len());
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
            LibrarySortMode::RecentlyPlayed => {
                items.sort_by_key(|item| Reverse(Self::item_last_played_timestamp(item)));
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
            let status = adw::StatusPage::new();
            status.set_icon_name(Some("audio-headphones-symbolic"));
            status.set_title("No Books in Progress");
            status.set_description(Some("Start listening to a book to see it here"));
            status.set_vexpand(true);
            continue_flowbox.append(&status);
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

        // Gelly tarzı: overlay üzerinde hover'da beliren dairesel play butonu
        let cover_overlay = gtk::Overlay::new();
        cover_overlay.set_child(Some(&cover_box));

        let hover_play = gtk::Button::from_icon_name("media-playback-start-symbolic");
        hover_play.add_css_class("circular");
        hover_play.add_css_class("suggested-action");
        hover_play.add_css_class("cover-play-btn");
        hover_play.set_halign(gtk::Align::Center);
        hover_play.set_valign(gtk::Align::Center);
        hover_play.set_opacity(0.0);
        hover_play.set_size_request(48, 48);
        cover_overlay.add_overlay(&hover_play);

        let motion = gtk::EventControllerMotion::new();
        let btn = hover_play.clone();
        motion.connect_enter(move |_, _, _| btn.set_opacity(1.0));
        let btn = hover_play.clone();
        motion.connect_leave(move |_| btn.set_opacity(0.0));
        cover_overlay.add_controller(motion);

        let item_id_hover = item.id.clone();
        let win_hover = self.clone();
        hover_play.connect_clicked(move |_| {
            win_hover.start_playback(&item_id_hover);
        });

        cover_frame.set_child(Some(&cover_overlay));
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

    fn open_audiobook_detail(&self, item_id: &str) {
        let imp = self.imp();

        let nav_view = match imp.nav_view.borrow().clone() {
            Some(v) => v,
            None => return,
        };

        // Pop back to library root (handles re-opening from detail page)
        while nav_view.pop() {}

        // Build detail content
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        scrolled.set_vscrollbar_policy(gtk::PolicyType::Automatic);

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

        let toolbar_view = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scrolled));

        // Store widget refs before async fetch
        *imp.detail_content.borrow_mut() = Some(detail_box);
        *imp.detail_play_btn.borrow_mut() = None;
        *imp.detail_play_item_id.borrow_mut() = Some(item_id.to_string());
        *imp.detail_top_box.borrow_mut() = None;
        *imp.detail_cover_image.borrow_mut() = None;

        let detail_nav_page = adw::NavigationPage::builder()
            .title("Yükleniyor...")
            .child(&toolbar_view)
            .build();
        let detail_nav_page_ref = detail_nav_page.clone();
        nav_view.push(&detail_nav_page);

        let client = imp.client.clone();
        let win = self.clone();
        let id = item_id.to_string();

        glib::spawn_future_local(async move {
            let (tx, rx) = async_channel::bounded(1);
            let item_id = id.clone();

            std::thread::spawn(move || {
                let result = client.get_library_item(&item_id);
                let _ = tx.send_blocking(result);
            });

            match rx.recv().await {
                Ok(Ok(item)) => {
                    let book_title = item
                        .media
                        .as_ref()
                        .and_then(|m| m.metadata.as_ref())
                        .and_then(|m| m.title.as_deref())
                        .unwrap_or("Kitap Detayı");
                    detail_nav_page_ref.set_title(book_title);
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
        *imp.detail_play_btn.borrow_mut() = None;
        *imp.detail_play_item_id.borrow_mut() = Some(item.id.clone());
        *imp.detail_top_box.borrow_mut() = None;
        *imp.detail_cover_image.borrow_mut() = None;
        imp.chapter_indicators.borrow_mut().clear();
        imp.bookmarks.borrow_mut().clear();
        *imp.bookmarks_group.borrow_mut() = None;
        *imp.bookmarks_section.borrow_mut() = None;

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
            if !authors.is_empty() {
                let authors_flow = gtk::FlowBox::new();
                authors_flow.set_selection_mode(gtk::SelectionMode::None);
                authors_flow.set_column_spacing(6);
                authors_flow.set_row_spacing(6);
                authors_flow.set_max_children_per_line(20);
                authors_flow.set_halign(gtk::Align::Start);
                for author in authors {
                    let btn = gtk::Button::with_label(&author.name);
                    btn.add_css_class("pill");
                    btn.add_css_class("flat");
                    btn.add_css_class("author-chip");
                    btn.set_tooltip_text(Some(&format!("Show books by {}", author.name)));
                    let name = author.name.clone();
                    let id = author.id.clone();
                    let win = self.clone();
                    btn.connect_clicked(move |_| {
                        win.show_author_books(id.as_deref(), &name);
                    });
                    authors_flow.append(&btn);
                }
                info_box.append(&authors_flow);
            }
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

        // Play button (Gelly tarzı dairesel büyük buton)
        let play_button = gtk::Button::from_icon_name("media-playback-start-symbolic");
        play_button.add_css_class("suggested-action");
        play_button.add_css_class("circular");
        play_button.add_css_class("detail-play-btn");
        play_button.set_size_request(64, 64);
        play_button.set_valign(gtk::Align::Center);
        *imp.detail_play_btn.borrow_mut() = Some(play_button.clone());

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

        // Mark as read/unread toggle button
        let is_finished_init = item
            .user_media_progress
            .as_ref()
            .and_then(|p| p.is_finished)
            .unwrap_or(false);
        let saved_progress_time = item
            .user_media_progress
            .as_ref()
            .and_then(|p| p.current_time)
            .unwrap_or(0.0);
        imp.detail_is_finished.set(is_finished_init);

        let mark_button = gtk::Button::new();
        mark_button.add_css_class("pill");
        mark_button.set_valign(gtk::Align::Center);
        let mark_content = adw::ButtonContent::new();
        mark_button.set_child(Some(&mark_content));

        let mark_state = Rc::new(Cell::new(is_finished_init));
        let apply_mark_visual = |state: bool, content: &adw::ButtonContent| {
            if state {
                content.set_icon_name("edit-undo-symbolic");
                content.set_label("Mark as Unfinished");
            } else {
                content.set_icon_name("object-select-symbolic");
                content.set_label("Mark as Finished");
            }
        };
        apply_mark_visual(mark_state.get(), &mark_content);

        let win_mark = self.clone();
        let item_id_mark = item.id.clone();
        let mark_state_cb = mark_state.clone();
        let mark_button_cb = mark_button.clone();
        let mark_content_cb = mark_content.clone();
        mark_button.connect_clicked(move |_| {
            let new_state = !mark_state_cb.get();
            mark_button_cb.set_sensitive(false);
            let (sender, receiver) = async_channel::bounded::<Result<(), String>>(1);
            let client = win_mark.imp().client.clone();
            let item_id = item_id_mark.clone();
            std::thread::spawn(move || {
                let result = client
                    .update_progress(&item_id, new_state)
                    .map_err(|e| e.to_string());
                let _ = sender.send_blocking(result);
            });
            let win_recv = win_mark.clone();
            let mark_button_recv = mark_button_cb.clone();
            let mark_content_recv = mark_content_cb.clone();
            let mark_state_recv = mark_state_cb.clone();
            glib::spawn_future_local(async move {
                match receiver.recv().await {
                    Ok(Ok(())) => {
                        mark_state_recv.set(new_state);
                        win_recv.imp().detail_is_finished.set(new_state);
                        if new_state {
                            mark_content_recv.set_icon_name("edit-undo-symbolic");
                            mark_content_recv.set_label("Mark as Unfinished");
                        } else {
                            mark_content_recv.set_icon_name("object-select-symbolic");
                            mark_content_recv.set_label("Mark as Finished");
                        }
                        let imp_recv = win_recv.imp();
                        let detail_is_current = imp_recv.detail_play_item_id.borrow().as_deref()
                            == imp_recv.current_item_id.borrow().as_deref();
                        let current = if detail_is_current {
                            *imp_recv.current_time.borrow()
                        } else {
                            saved_progress_time
                        };
                        win_recv.apply_chapter_indicators(current);
                        let toast = adw::Toast::new(if new_state {
                            "Marked as finished"
                        } else {
                            "Marked as unfinished"
                        });
                        win_recv.imp().toast_overlay.add_toast(toast);
                    }
                    Ok(Err(err)) => {
                        log::warn!("Update progress error: {}", err);
                        let toast =
                            adw::Toast::new(&format!("Failed to update progress: {}", err));
                        win_recv.imp().toast_overlay.add_toast(toast);
                    }
                    Err(err) => {
                        log::warn!("Update progress channel error: {}", err);
                        let toast = adw::Toast::new("Failed to update progress");
                        win_recv.imp().toast_overlay.add_toast(toast);
                    }
                }
                mark_button_recv.set_sensitive(true);
            });
        });

        let actions_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        actions_row.set_halign(gtk::Align::Start);
        actions_row.append(&play_button);
        actions_row.append(&mark_button);
        detail_box.append(&actions_row);
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

                    let prefix_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                    prefix_box.set_valign(gtk::Align::Center);

                    let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    indicator.add_css_class("chapter-indicator");
                    indicator.set_valign(gtk::Align::Center);
                    indicator.set_halign(gtk::Align::Center);

                    if is_finished || listen_pos >= end {
                        indicator.add_css_class("completed");
                    } else if listen_pos >= start && listen_pos < end {
                        indicator.add_css_class("playing");
                    } else {
                        indicator.add_css_class("unplayed");
                    }

                    let num_label = gtk::Label::new(Some(&format!("{:02}", i + 1)));
                    num_label.add_css_class("dim-label");
                    num_label.add_css_class("monospace");
                    num_label.add_css_class("caption");

                    prefix_box.append(&indicator);
                    prefix_box.append(&num_label);
                    row.add_prefix(&prefix_box);

                    imp.chapter_indicators
                        .borrow_mut()
                        .push((start, end, indicator.clone()));

                    let ch_play = gtk::Button::from_icon_name("media-playback-start-symbolic");
                    ch_play.add_css_class("flat");
                    ch_play.add_css_class("circular");
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

        // Bookmarks section (populated asynchronously)
        let bookmarks_section = gtk::Box::new(gtk::Orientation::Vertical, 12);
        bookmarks_section.set_visible(false);
        let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        bookmarks_section.append(&sep);
        let bm_title = gtk::Label::new(Some("Bookmarks"));
        bm_title.add_css_class("title-4");
        bm_title.set_halign(gtk::Align::Start);
        bookmarks_section.append(&bm_title);
        let bookmarks_group = adw::PreferencesGroup::new();
        bookmarks_section.append(&bookmarks_group);
        detail_box.append(&bookmarks_section);

        *imp.bookmarks_group.borrow_mut() = Some(bookmarks_group);
        *imp.bookmarks_section.borrow_mut() = Some(bookmarks_section);

        self.load_bookmarks(&item.id);
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
                        session.display_title.as_deref().unwrap_or("Unknown Book"),
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

        log::info!("Starting audio stream");

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
                        log::debug!("Buffering: {}%", percent);
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
            .expect("Failed to add bus watch");

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
                        win.refresh_chapter_indicators(secs);
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
                        log::debug!("Session synced: {:.0}s", current)
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

fn extract_refresh_token(url: &str) -> Option<String> {
    extract_url_param(url, "refresh_token").or_else(|| extract_url_param(url, "refreshToken"))
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
