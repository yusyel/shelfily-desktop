/* models.rs
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

use serde::{Deserialize, Serialize};

// ─── Server Status ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ServerStatus {
    #[serde(rename = "isInit")]
    pub is_init: Option<bool>,
    #[serde(rename = "authMethods")]
    pub auth_methods: Option<Vec<String>>,
    #[serde(rename = "authFormData")]
    pub auth_form_data: Option<AuthFormData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthFormData {
    #[serde(rename = "authOpenIDButtonText")]
    pub auth_openid_button_text: Option<String>,
    #[serde(rename = "authOpenIDAutoLaunch")]
    pub auth_openid_auto_launch: Option<bool>,
    #[serde(rename = "authLoginCustomMessage")]
    pub auth_login_custom_message: Option<String>,
}

// ─── Authentication ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct LoginResponse {
    pub user: User,
    #[serde(rename = "userDefaultLibraryId")]
    pub user_default_library_id: Option<String>,
    #[serde(rename = "serverSettings")]
    pub server_settings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub token: Option<String>,
    #[serde(rename = "type")]
    pub user_type: Option<String>,
    #[serde(rename = "mediaProgress")]
    pub media_progress: Option<Vec<MediaProgress>>,
}

// ─── Libraries ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Library {
    pub id: String,
    pub name: String,
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
    pub icon: Option<String>,
    #[serde(flatten)]
    pub extra: Option<serde_json::Value>,
}

// ─── Library Items ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryItem {
    pub id: String,
    pub ino: Option<String>,
    #[serde(rename = "libraryId")]
    pub library_id: Option<String>,
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
    pub media: Option<Media>,
    #[serde(rename = "numFiles")]
    pub num_files: Option<u32>,
    pub size: Option<serde_json::Value>,
    #[serde(rename = "userMediaProgress")]
    pub user_media_progress: Option<MediaProgress>,
    #[serde(flatten)]
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryItemExpanded {
    pub id: String,
    #[serde(rename = "libraryId")]
    pub library_id: Option<String>,
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
    pub media: Option<MediaExpanded>,
    #[serde(rename = "userMediaProgress")]
    pub user_media_progress: Option<MediaProgress>,
    #[serde(rename = "libraryFiles")]
    pub library_files: Option<Vec<serde_json::Value>>,
}

// ─── Media ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Media {
    pub metadata: Option<Metadata>,
    #[serde(rename = "coverPath")]
    pub cover_path: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "numTracks")]
    pub num_tracks: Option<u32>,
    #[serde(rename = "numAudioFiles")]
    pub num_audio_files: Option<u32>,
    #[serde(rename = "numChapters")]
    pub num_chapters: Option<u32>,
    pub duration: Option<f64>,
    pub size: Option<serde_json::Value>,
    #[serde(rename = "ebookFormat")]
    pub ebook_format: Option<String>,
    #[serde(flatten)]
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MediaExpanded {
    pub metadata: Option<MetadataExpanded>,
    #[serde(rename = "coverPath")]
    pub cover_path: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "audioFiles")]
    pub audio_files: Option<Vec<AudioFile>>,
    pub chapters: Option<Vec<Chapter>>,
    pub tracks: Option<Vec<AudioTrack>>,
    pub duration: Option<f64>,
    pub size: Option<u64>,
    #[serde(flatten)]
    pub _extra: Option<serde_json::Value>,
}

// ─── Metadata ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Metadata {
    pub title: Option<String>,
    #[serde(rename = "titleIgnorePrefix")]
    pub title_ignore_prefix: Option<String>,
    pub subtitle: Option<String>,
    #[serde(rename = "authorName")]
    pub author_name: Option<String>,
    #[serde(rename = "authorNameLF")]
    pub author_name_lf: Option<String>,
    #[serde(rename = "narratorName")]
    pub narrator_name: Option<String>,
    #[serde(rename = "seriesName")]
    pub series_name: Option<String>,
    pub genres: Option<Vec<String>>,
    #[serde(rename = "publishedYear")]
    pub published_year: Option<String>,
    pub description: Option<String>,
    pub isbn: Option<String>,
    pub asin: Option<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    #[serde(flatten)]
    pub _extra: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetadataExpanded {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Option<Vec<Author>>,
    pub narrators: Option<Vec<String>>,
    pub series: Option<Vec<SeriesItem>>,
    pub genres: Option<Vec<String>>,
    #[serde(rename = "publishedYear")]
    pub published_year: Option<String>,
    pub description: Option<String>,
    pub isbn: Option<String>,
    pub asin: Option<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    #[serde(flatten)]
    pub _extra: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Author {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SeriesItem {
    pub id: Option<String>,
    pub name: Option<String>,
    pub sequence: Option<String>,
}

// ─── Audio ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct AudioFile {
    pub index: Option<u32>,
    pub ino: Option<String>,
    pub metadata: Option<AudioFileMetadata>,
    #[serde(rename = "addedAt")]
    pub added_at: Option<u64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<u64>,
    #[serde(rename = "trackNumFromMeta")]
    pub track_num: Option<u32>,
    pub duration: Option<f64>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AudioFileMetadata {
    pub filename: Option<String>,
    pub ext: Option<String>,
    pub path: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AudioTrack {
    pub index: Option<u32>,
    #[serde(rename = "startOffset")]
    pub start_offset: Option<f64>,
    pub duration: Option<f64>,
    pub title: Option<String>,
    #[serde(rename = "contentUrl")]
    pub content_url: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Chapter {
    pub id: Option<u32>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub title: Option<String>,
}

// ─── Playback ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct PlaybackSession {
    pub id: String,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "libraryItemId")]
    pub library_item_id: Option<String>,
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,
    #[serde(rename = "mediaMetadata")]
    pub media_metadata: Option<serde_json::Value>,
    pub chapters: Option<Vec<Chapter>>,
    #[serde(rename = "displayTitle")]
    pub display_title: Option<String>,
    #[serde(rename = "displayAuthor")]
    pub display_author: Option<String>,
    #[serde(rename = "coverPath")]
    pub cover_path: Option<String>,
    pub duration: Option<f64>,
    #[serde(rename = "playMethod")]
    pub play_method: Option<u32>,
    #[serde(rename = "startTime")]
    pub start_time: Option<f64>,
    #[serde(rename = "currentTime")]
    pub current_time: Option<f64>,
    #[serde(rename = "audioTracks")]
    pub audio_tracks: Option<Vec<AudioTrack>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeviceInfo {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    #[serde(rename = "clientName")]
    pub client_name: String,
    #[serde(rename = "clientVersion")]
    pub client_version: String,
    #[serde(rename = "deviceName")]
    pub device_name: String,
    #[serde(rename = "deviceType")]
    pub device_type: String,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            device_id: "shelfily-desktop-gtk".to_string(),
            client_name: "Shelfily Desktop GTK".to_string(),
            client_version: "0.1.0".to_string(),
            device_name: hostname(),
            device_type: "desktop".to_string(),
        }
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "linux-desktop".to_string())
}

// ─── Media Progress ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct MediaProgress {
    pub id: Option<String>,
    #[serde(rename = "libraryItemId")]
    pub library_item_id: Option<String>,
    #[serde(rename = "episodeId")]
    pub episode_id: Option<String>,
    pub duration: Option<f64>,
    pub progress: Option<f64>,
    #[serde(rename = "currentTime")]
    pub current_time: Option<f64>,
    #[serde(rename = "isFinished")]
    pub is_finished: Option<bool>,
    #[serde(rename = "lastUpdate")]
    pub last_update: Option<u64>,
    #[serde(rename = "startedAt")]
    pub started_at: Option<u64>,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<u64>,
}

// ─── Personalized Shelves ───────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct PersonalizedShelf {
    pub id: Option<String>,
    pub label: Option<String>,
    #[serde(rename = "labelStringKey")]
    pub label_string_key: Option<String>,
    #[serde(rename = "type")]
    pub shelf_type: Option<String>,
    pub entities: Option<Vec<serde_json::Value>>,
    pub total: Option<u32>,
}
