/* api.rs
 *
 * Copyright 2026 yusuf
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use crate::models::*;
use reqwest::blocking::Client;
use std::sync::{Arc, Mutex};

/// Audiobookshelf API client
#[derive(Debug, Clone)]
pub struct AudiobookshelfClient {
    inner: Arc<Mutex<ClientInner>>,
}

#[derive(Debug)]
struct ClientInner {
    client: Client,
    base_url: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

impl AudiobookshelfClient {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClientInner {
                client: Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .build()
                    .expect("Failed to create HTTP client"),
                base_url: String::new(),
                access_token: None,
                refresh_token: None,
            })),
        }
    }

    pub fn set_server(&self, url: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.base_url = url.trim_end_matches('/').to_string();
    }

    pub fn server_url(&self) -> String {
        let inner = self.inner.lock().unwrap();
        inner.base_url.clone()
    }

    pub fn set_token(&self, token: &str) {
        self.set_access_token(token);
    }

    pub fn set_access_token(&self, token: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.access_token = if token.is_empty() {
            None
        } else {
            Some(token.to_string())
        };
    }

    pub fn set_refresh_token(&self, token: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.refresh_token = if token.is_empty() {
            None
        } else {
            Some(token.to_string())
        };
    }

    pub fn set_tokens(&self, access_token: &str, refresh_token: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.access_token = if access_token.is_empty() {
            None
        } else {
            Some(access_token.to_string())
        };
        inner.refresh_token = if refresh_token.is_empty() {
            None
        } else {
            Some(refresh_token.to_string())
        };
    }

    pub fn token(&self) -> Option<String> {
        self.access_token()
    }

    pub fn access_token(&self) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner.access_token.clone()
    }

    pub fn refresh_token(&self) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner.refresh_token.clone()
    }

    pub fn is_authenticated(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.access_token.is_some()
    }

    /// Login with username and password
    /// POST /login
    pub fn login(&self, username: &str, password: &str) -> Result<LoginResponse, ApiError> {
        let (client, base_url, _, _) = self.connection_info();
        let url = format!("{}/login?return_tokens=true", base_url);

        let body = serde_json::json!({
            "username": username,
            "password": password,
        });

        let resp = client
            .post(&url)
            .header("x-return-tokens", "true")
            .json(&body)
            .send()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if resp.status().is_success() {
            let login_resp: LoginResponse =
                resp.json().map_err(|e| ApiError::Parse(e.to_string()))?;
            Ok(login_resp)
        } else {
            Err(ApiError::Auth(format!(
                "Login failed: HTTP {}",
                resp.status()
            )))
        }
    }

    /// GET /status — check server status and available auth methods
    pub fn get_status(&self) -> Result<ServerStatus, ApiError> {
        let (client, base_url, _, _) = self.connection_info();
        let url = format!("{}/status", base_url);
        let resp = client
            .get(&url)
            .send()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        if resp.status().is_success() {
            let status: ServerStatus = resp.json().map_err(|e| ApiError::Parse(e.to_string()))?;
            Ok(status)
        } else {
            Err(ApiError::Server(format!("HTTP {}", resp.status())))
        }
    }

    /// GET /api/libraries
    pub fn get_libraries(&self) -> Result<Vec<Library>, ApiError> {
        let resp: serde_json::Value = self.get("/api/libraries")?;
        log::debug!(
            "Libraries response keys: {:?}",
            resp.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
        let libraries_val = resp
            .get("libraries")
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![]));
        let libraries: Vec<Library> = serde_json::from_value(libraries_val).map_err(|e| {
            log::error!("Libraries parse error: {}", e);
            ApiError::Parse(e.to_string())
        })?;
        log::info!("Loaded {} libraries", libraries.len());
        Ok(libraries)
    }

    /// GET /api/libraries/:id/items — fetches all items with pagination
    pub fn get_library_items(&self, library_id: &str) -> Result<Vec<LibraryItem>, ApiError> {
        let mut all_items: Vec<LibraryItem> = Vec::new();
        let page_size = 100;
        let mut offset = 0;

        loop {
            let resp: serde_json::Value = self.get(&format!(
                "/api/libraries/{}/items?limit={}&offset={}&minified=0&include=progress",
                library_id, page_size, offset
            ))?;

            let total = resp.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let results_val = resp
                .get("results")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![]));

            let items: Vec<LibraryItem> = serde_json::from_value(results_val).map_err(|e| {
                log::error!("Library items parse error: {}", e);
                ApiError::Parse(e.to_string())
            })?;

            let count = items.len();
            all_items.extend(items);
            log::debug!(
                "Loaded page: offset={}, got={}, total={}",
                offset,
                count,
                total
            );

            offset += count;
            if count == 0 || offset >= total {
                break;
            }
        }

        log::info!(
            "Loaded {} total items from library {}",
            all_items.len(),
            library_id
        );
        Ok(all_items)
    }

    /// GET /api/items/:id?expanded=1
    pub fn get_library_item(&self, item_id: &str) -> Result<LibraryItemExpanded, ApiError> {
        let resp: LibraryItemExpanded = self.get(&format!(
            "/api/items/{}?expanded=1&include=progress",
            item_id
        ))?;
        Ok(resp)
    }

    /// GET /api/libraries/:id/personalized
    pub fn get_personalized_shelves(
        &self,
        library_id: &str,
    ) -> Result<Vec<PersonalizedShelf>, ApiError> {
        let resp: Vec<PersonalizedShelf> =
            self.get(&format!("/api/libraries/{}/personalized", library_id))?;
        Ok(resp)
    }

    /// POST /api/items/:id/play
    pub fn start_playback(
        &self,
        item_id: &str,
        device_info: &DeviceInfo,
    ) -> Result<PlaybackSession, ApiError> {
        let body = serde_json::json!({
            "deviceInfo": device_info,
            "supportedMimeTypes": ["audio/mpeg", "audio/mp4", "audio/ogg", "audio/flac"],
            "mediaPlayer": "html5",
            "forceDirectPlay": true,
            "forceTranscode": false,
        });
        self.post(&format!("/api/items/{}/play", item_id), &body)
    }

    /// POST /api/session/:id/sync
    pub fn sync_session(
        &self,
        session_id: &str,
        current_time: f64,
        duration: f64,
    ) -> Result<(), ApiError> {
        let body = serde_json::json!({
            "currentTime": current_time,
            "duration": duration,
            "timeListened": 1.0,
        });
        let _: serde_json::Value =
            self.post(&format!("/api/session/{}/sync", session_id), &body)?;
        Ok(())
    }

    /// POST /api/session/:id/close
    pub fn close_session(
        &self,
        session_id: &str,
        current_time: f64,
        duration: f64,
    ) -> Result<(), ApiError> {
        let body = serde_json::json!({
            "currentTime": current_time,
            "duration": duration,
            "timeListened": 0,
        });
        self.execute_empty_post(&format!("/api/session/{}/close", session_id), &body)
    }

    /// GET /api/me/items-in-progress
    pub fn get_items_in_progress(&self) -> Result<Vec<LibraryItem>, ApiError> {
        let body: serde_json::Value = self.get("/api/me/items-in-progress")?;

        // API may return an array directly or { "libraryItems": [...] }
        let arr = if body.is_array() {
            body
        } else {
            body.get("libraryItems")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![]))
        };
        let items: Vec<LibraryItem> =
            serde_json::from_value(arr).map_err(|e| ApiError::Parse(e.to_string()))?;
        log::info!("Items in progress: {}", items.len());
        Ok(items)
    }

    /// GET /api/me/progress/:id
    pub fn get_media_progress(&self, item_id: &str) -> Result<Option<MediaProgress>, ApiError> {
        let mut attempted_refresh = false;

        loop {
            let (client, base_url, access_token, _) = self.connection_info();
            let url = format!("{}/api/me/progress/{}", base_url, item_id);
            let mut req = client.get(&url);
            if let Some(ref t) = access_token {
                req = req.header("Authorization", format!("Bearer {}", t));
            }

            let resp = req.send().map_err(|e| ApiError::Network(e.to_string()))?;
            let status = resp.status();
            if status == 404 {
                return Ok(None);
            }
            if status.is_success() {
                let progress: MediaProgress =
                    resp.json().map_err(|e| ApiError::Parse(e.to_string()))?;
                return Ok(Some(progress));
            }
            if (status.as_u16() == 401 || status.as_u16() == 403) && !attempted_refresh {
                attempted_refresh = true;
                if self.refresh_access_token()? {
                    continue;
                }
                return Err(ApiError::Auth(format!("HTTP {}", status)));
            }
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(ApiError::Auth(format!("HTTP {}", status)));
            }
            return Err(ApiError::Server(format!("HTTP {}", status)));
        }
    }

    /// Helper: extract base_url, token, and client clone from the inner lock
    fn connection_info(&self) -> (Client, String, Option<String>, Option<String>) {
        let inner = self.inner.lock().unwrap();
        (
            inner.client.clone(),
            inner.base_url.clone(),
            inner.access_token.clone(),
            inner.refresh_token.clone(),
        )
    }

    fn apply_refreshed_tokens(&self, login_resp: &LoginResponse) -> bool {
        let access_token = login_resp
            .user
            .access_token
            .as_deref()
            .or(login_resp.user.token.as_deref())
            .unwrap_or_default();
        if access_token.is_empty() {
            return false;
        }

        let refresh_token = login_resp.user.refresh_token.as_deref().unwrap_or_default();
        self.set_tokens(access_token, refresh_token);
        true
    }

    fn refresh_access_token(&self) -> Result<bool, ApiError> {
        let (client, base_url, _, refresh_token) = self.connection_info();
        let Some(refresh_token) = refresh_token else {
            return Ok(false);
        };
        if refresh_token.is_empty() {
            return Ok(false);
        }

        let url = format!("{}/auth/refresh", base_url);
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", refresh_token))
            .header("x-refresh-token", &refresh_token)
            .header("x-return-tokens", "true")
            .send()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if resp.status().is_success() {
            let login_resp: LoginResponse =
                resp.json().map_err(|e| ApiError::Parse(e.to_string()))?;
            Ok(self.apply_refreshed_tokens(&login_resp))
        } else if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
            Ok(false)
        } else {
            Err(ApiError::Server(format!("HTTP {}", resp.status())))
        }
    }

    fn execute_json<T, F>(&self, path: &str, send: F) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
        F: Fn(&Client, &str, Option<&str>) -> Result<reqwest::blocking::Response, reqwest::Error>,
    {
        let mut attempted_refresh = false;

        loop {
            let (client, base_url, access_token, _) = self.connection_info();
            let url = format!("{}{}", base_url, path);
            let resp = send(&client, &url, access_token.as_deref())
                .map_err(|e| ApiError::Network(e.to_string()))?;
            let status = resp.status();

            if status.is_success() {
                let body: T = resp.json().map_err(|e| ApiError::Parse(e.to_string()))?;
                return Ok(body);
            }

            if (status.as_u16() == 401 || status.as_u16() == 403) && !attempted_refresh {
                attempted_refresh = true;
                if self.refresh_access_token()? {
                    continue;
                }
                return Err(ApiError::Auth(format!("HTTP {}", status)));
            }

            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(ApiError::Auth(format!("HTTP {}", status)));
            }

            return Err(ApiError::Server(format!("HTTP {}", status)));
        }
    }

    /// Download cover image bytes
    pub fn download_cover(&self, item_id: &str) -> Result<Vec<u8>, ApiError> {
        self.execute_json_bytes(&format!("/api/items/{}/cover?width=400", item_id))
    }

    /// Build audio stream URL for a track
    pub fn audio_stream_url(&self, content_url: &str) -> String {
        let inner = self.inner.lock().unwrap();
        let token = inner.access_token.as_deref().unwrap_or("");
        let base = if content_url.starts_with("http://") || content_url.starts_with("https://") {
            content_url.to_string()
        } else {
            format!("{}{}", inner.base_url, content_url)
        };

        if token.is_empty() {
            return base;
        }

        let separator = if base.contains('?') { '&' } else { '?' };
        format!("{base}{separator}token={token}")
    }

    /// Generic GET request
    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        self.execute_json(path, |client, url, access_token| {
            let mut req = client.get(url);
            if let Some(token) = access_token {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            req.send()
        })
    }

    /// Generic POST request
    fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, ApiError> {
        self.execute_json(path, |client, url, access_token| {
            let mut req = client.post(url);
            if let Some(token) = access_token {
                req = req.header("Authorization", format!("Bearer {}", token));
            }
            req.json(body).send()
        })
    }

    fn execute_json_bytes(&self, path: &str) -> Result<Vec<u8>, ApiError> {
        let mut attempted_refresh = false;

        loop {
            let (client, base_url, access_token, _) = self.connection_info();
            let url = format!("{}{}", base_url, path);
            let mut req = client.get(&url);
            if let Some(token) = access_token.as_deref() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }

            let resp = req.send().map_err(|e| ApiError::Network(e.to_string()))?;
            let status = resp.status();

            if status.is_success() {
                let bytes = resp.bytes().map_err(|e| ApiError::Network(e.to_string()))?;
                return Ok(bytes.to_vec());
            }

            if (status.as_u16() == 401 || status.as_u16() == 403) && !attempted_refresh {
                attempted_refresh = true;
                if self.refresh_access_token()? {
                    continue;
                }
                return Err(ApiError::Auth(format!("HTTP {}", status)));
            }

            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(ApiError::Auth(format!("HTTP {}", status)));
            }

            return Err(ApiError::Server(format!("HTTP {}", status)));
        }
    }

    fn execute_empty_post(&self, path: &str, body: &serde_json::Value) -> Result<(), ApiError> {
        let mut attempted_refresh = false;

        loop {
            let (client, base_url, access_token, _) = self.connection_info();
            let url = format!("{}{}", base_url, path);
            let mut req = client.post(&url);
            if let Some(token) = access_token.as_deref() {
                req = req.header("Authorization", format!("Bearer {}", token));
            }

            let resp = req
                .json(body)
                .send()
                .map_err(|e| ApiError::Network(e.to_string()))?;
            let status = resp.status();

            if status.is_success() {
                return Ok(());
            }

            if (status.as_u16() == 401 || status.as_u16() == 403) && !attempted_refresh {
                attempted_refresh = true;
                if self.refresh_access_token()? {
                    continue;
                }
                return Err(ApiError::Auth(format!("HTTP {}", status)));
            }

            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(ApiError::Auth(format!("HTTP {}", status)));
            }

            return Err(ApiError::Server(format!("HTTP {}", status)));
        }
    }
}

/// API Error types
#[derive(Debug)]
pub enum ApiError {
    Network(String),
    Auth(String),
    Parse(String),
    Server(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "Network error: {}", e),
            ApiError::Auth(e) => write!(f, "Authentication error: {}", e),
            ApiError::Parse(e) => write!(f, "Parse error: {}", e),
            ApiError::Server(e) => write!(f, "Server error: {}", e),
        }
    }
}
