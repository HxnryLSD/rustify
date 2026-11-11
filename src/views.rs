use crate::app_state::*;
use crate::bandwidth_saver::BandwidthSaver;
use crate::models::{Album, Artist, ImageSize, Playlist, Track, sized_image_url};
use crate::ui::RustifyApp;
use eframe::egui;

/// Image size used for album art in grids.
const GRID_IMAGE_SIZE: ImageSize = ImageSize::Medium;

/// Image size used for the large cover panel.
const COVER_IMAGE_SIZE: ImageSize = ImageSize::Large;

/// Render album art, or a placeholder when bandwidth saver forbids images.
pub fn album_art(
    ui: &mut egui::Ui,
    url: Option<&str>,
    size: f32,
    bandwidth_saver: &BandwidthSaver,
) {
    let blocked = bandwidth_saver.should_disable_images();
    match url {
        Some(url) if !blocked => {
            let sized = sized_image_url(url, GRID_IMAGE_SIZE);
            ui.add(
                egui::Image::from_uri(sized)
                    .fit_to_exact_size(egui::vec2(size, size))
                    .corner_radius(4.0),
            );
        }
        _ => {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
            let color = if blocked {
                egui::Color32::from_gray(55)
            } else {
                egui::Color32::from_gray(42)
            };
            ui.painter().rect_filled(rect, 4.0, color);
            if blocked {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "no images",
                    egui::FontId::proportional(10.0),
                    ui.visuals().weak_text_color(),
                );
            }
        }
    }
}

/// Render a grid of clickable cards.
///
/// The callback returns the detail target the card points at, so the caller
/// can navigate when the card is clicked.
fn card_grid<T>(
    ui: &mut egui::Ui,
    items: &[T],
    mut render: impl FnMut(&mut egui::Ui, &T) -> Option<DetailTarget>,
) -> Option<DetailTarget> {
    let columns = ui.available_width().floor() / 160.0;
    let columns = columns.clamp(1.0, 8.0) as usize;
    let mut selected = None;

    egui::Grid::new("card_grid")
        .spacing([12.0, 12.0])
        .max_col_width(160.0)
        .show(ui, |ui| {
            for (index, item) in items.iter().enumerate() {
                if index % columns == 0 && index > 0 {
                    ui.end_row();
                }
                ui.vertical(|ui| {
                    if let Some(target) = render(ui, item) {
                        selected = Some(target);
                    }
                });
                ui.end_row();
            }
        });

    selected
}

/// The search screen.
pub fn search_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let mut query = app.state.search.query.clone();
        let response = ui.add(
            egui::TextEdit::singleline(&mut query)
                .hint_text("Search tracks, albums, artists, playlists")
                .desired_width(f32::INFINITY),
        );
        if response.changed() {
            app.state.search.query = query;
        }
        if ui.button("Search").clicked() && !app.state.search.query.is_empty() {
            // The web-player API reaches the whole catalogue, also for
            // accounts without Premium.
            app.web_search(&app.state.search.query.clone());
        }
    });

    ui.separator();

    if app.state.search.loading {
        ui.spinner();
        return;
    }
    if let Some(error) = &app.state.search.error {
        ui.colored_label(egui::Color32::RED, error);
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        if !app.state.search.tracks.is_empty() {
            ui.heading("Tracks");
            let tracks = app.state.search.tracks.clone();
            track_table(app, ui, &tracks);
            ui.add_space(8.0);
        }
        if !app.state.search.albums.is_empty() {
            ui.heading("Albums");
            let albums = app.state.search.albums.clone();
            if let Some(target) = card_grid(ui, &albums, |ui, album| {
                album_card(ui, album, &app.bandwidth_saver)
            }) {
                open_detail(app, target);
            }
            ui.add_space(8.0);
        }
        if !app.state.search.artists.is_empty() {
            ui.heading("Artists");
            let artists = app.state.search.artists.clone();
            if let Some(target) = card_grid(ui, &artists, |ui, artist| {
                artist_card(ui, artist, &app.bandwidth_saver)
            }) {
                open_detail(app, target);
            }
            ui.add_space(8.0);
        }
        if !app.state.search.playlists.is_empty() {
            ui.heading("Playlists");
            let playlists = app.state.search.playlists.clone();
            if let Some(target) = card_grid(ui, &playlists, |ui, playlist| {
                playlist_card(ui, playlist, &app.bandwidth_saver)
            }) {
                open_detail(app, target);
            }
        }
    });
}

/// Open a detail screen for the given target.
fn open_detail(app: &mut RustifyApp, target: DetailTarget) {
    app.state.detail_target = Some(target.clone());
    app.state.detail = DetailState::default();
    let view = match target {
        DetailTarget::Album(_) => View::Album,
        DetailTarget::Artist(_) => View::Artist,
        DetailTarget::Playlist(_) => View::Playlist,
    };
    app.navigate(view);
}

/// The home screen.
pub fn home_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    if app.state.home.loading {
        ui.spinner();
        return;
    }
    if let Some(error) = &app.state.home.error {
        ui.colored_label(egui::Color32::RED, error);
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        if !app.state.home.new_releases.is_empty() {
            ui.heading("New releases");
            let albums = app.state.home.new_releases.clone();
            if let Some(target) = card_grid(ui, &albums, |ui, album| {
                album_card(ui, album, &app.bandwidth_saver)
            }) {
                open_detail(app, target);
            }
            ui.add_space(16.0);
        }

        if !app.state.home.featured.is_empty() {
            ui.heading("Featured playlists");
            let playlists = app.state.home.featured.clone();
            if let Some(target) = card_grid(ui, &playlists, |ui, playlist| {
                playlist_card(ui, playlist, &app.bandwidth_saver)
            }) {
                open_detail(app, target);
            }
        }

        if app.state.home.new_releases.is_empty() && app.state.home.featured.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("Nothing to show yet. Try the search screen.");
            });
        }
    });
}

/// The library screen.
pub fn library_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    if app.state.library.loading {
        ui.spinner();
        return;
    }
    if let Some(error) = &app.state.library.error {
        ui.colored_label(egui::Color32::RED, error);
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        if !app.state.library.liked_songs.is_empty() {
            ui.heading("Liked songs");
            let tracks = app.state.library.liked_songs.clone();
            track_table(app, ui, &tracks);
            ui.add_space(16.0);
        } else if app.state.library.liked_loading {
            ui.heading("Liked songs");
            ui.spinner();
            ui.add_space(16.0);
        }
        if !app.state.library.saved_tracks.is_empty() {
            ui.heading("Saved tracks");
            let tracks = app.state.library.saved_tracks.clone();
            track_table(app, ui, &tracks);
            ui.add_space(16.0);
        }
        if !app.state.library.saved_albums.is_empty() {
            ui.heading("Saved albums");
            let albums = app.state.library.saved_albums.clone();
            if let Some(target) = card_grid(ui, &albums, |ui, album| {
                album_card(ui, album, &app.bandwidth_saver)
            }) {
                open_detail(app, target);
            }
            ui.add_space(16.0);
        }
        if !app.state.library.followed_artists.is_empty() {
            ui.heading("Following");
            let artists = app.state.library.followed_artists.clone();
            if let Some(target) = card_grid(ui, &artists, |ui, artist| {
                artist_card(ui, artist, &app.bandwidth_saver)
            }) {
                open_detail(app, target);
            }
        }
    });
}

/// The playlists screen.
pub fn playlists_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    if app.state.playlists.loading {
        ui.spinner();
        return;
    }
    if let Some(error) = &app.state.playlists.error {
        ui.colored_label(egui::Color32::RED, error);
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        let playlists = app.state.playlists.playlists.clone();
        if let Some(target) = card_grid(ui, &playlists, |ui, playlist| {
            playlist_card(ui, playlist, &app.bandwidth_saver)
        }) {
            open_detail(app, target);
        }
    });
}

/// The album detail screen.
pub fn album_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    let Some(album) = app.state.detail.album.clone() else {
        if app.state.detail.loading {
            ui.spinner();
        } else if let Some(error) = &app.state.detail.error {
            ui.colored_label(egui::Color32::RED, error);
        }
        return;
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        album_header(app, ui, &album);
        ui.separator();
        let tracks = app.state.detail.tracks.clone();
        track_table(app, ui, &tracks);
    });
}

/// The artist detail screen.
pub fn artist_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    let Some(artist) = app.state.detail.artist.clone() else {
        if app.state.detail.loading {
            ui.spinner();
        } else if let Some(error) = &app.state.detail.error {
            ui.colored_label(egui::Color32::RED, error);
        }
        return;
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            album_art(
                ui,
                artist.images.first().map(|image| image.url.as_str()),
                140.0,
                &app.bandwidth_saver,
            );
            ui.vertical(|ui| {
                ui.heading(&artist.name);
                if !artist.genres.is_empty() {
                    ui.label(artist.genres.join(" · "));
                }
                if artist.popularity > 0 {
                    ui.label(format!("Popularity {}", artist.popularity));
                }
                if ui.button("Play top tracks").clicked() {
                    app.play_context(format!("spotify:artist:{}", artist.id), None);
                }
            });
        });
        ui.separator();

        ui.heading("Top tracks");
        let tracks = app.state.detail.tracks.clone();
        track_table(app, ui, &tracks);

        if !app.state.detail.related_artists.is_empty() {
            ui.add_space(16.0);
            ui.heading("Fans also like");
            let artists = app.state.detail.related_artists.clone();
            if let Some(target) = card_grid(ui, &artists, |ui, artist| {
                artist_card(ui, artist, &app.bandwidth_saver)
            }) {
                open_detail(app, target);
            }
        }
    });
}

/// The playlist detail screen.
pub fn playlist_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    let Some(playlist) = app.state.detail.playlist.clone() else {
        if app.state.detail.loading {
            ui.spinner();
        } else if let Some(error) = &app.state.detail.error {
            ui.colored_label(egui::Color32::RED, error);
        }
        return;
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            album_art(
                ui,
                playlist.images.first().map(|image| image.url.as_str()),
                140.0,
                &app.bandwidth_saver,
            );
            ui.vertical(|ui| {
                ui.heading(&playlist.name);
                if let Some(owner) = &playlist.owner {
                    let name = owner
                        .display_name
                        .clone()
                        .unwrap_or_else(|| owner.id.clone());
                    ui.label(format!("By {name}"));
                }
                if let Some(description) = &playlist.description {
                    ui.label(description);
                }
                if ui.button("Play").clicked() {
                    app.play_context(format!("spotify:playlist:{}", playlist.id), None);
                }
            });
        });
        ui.separator();
        let tracks = app.state.detail.tracks.clone();
        track_table(app, ui, &tracks);
    });
}

/// Header shown at the top of the album screen.
fn album_header(app: &mut RustifyApp, ui: &mut egui::Ui, album: &Album) {
    ui.horizontal(|ui| {
        album_art(
            ui,
            album.images.first().map(|image| image.url.as_str()),
            140.0,
            &app.bandwidth_saver,
        );
        ui.vertical(|ui| {
            ui.heading(&album.name);
            let artists = album
                .artists
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            if !artists.is_empty() {
                ui.label(artists);
            }
            if let Some(date) = &album.release_date {
                ui.label(date);
            }
            if album.total_tracks > 0 {
                ui.label(format!("{} tracks", album.total_tracks));
            }
            if ui.button("Play").clicked() {
                app.play_context(format!("spotify:album:{}", album.id), None);
            }
        });
    });
}

/// A table of tracks with a play button for each row.
fn track_table(app: &mut RustifyApp, ui: &mut egui::Ui, tracks: &[Track]) {
    let mut to_play: Option<Track> = None;
    let mut to_preview: Option<Track> = None;
    let mut to_youtube: Option<Track> = None;

    egui::Grid::new("track_table")
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for track in tracks {
                let is_current = app.state.is_current_track(&track.id);
                let title = if is_current {
                    egui::RichText::new(&track.name)
                        .strong()
                        .color(egui::Color32::from_rgb(30, 215, 96))
                } else {
                    egui::RichText::new(&track.name)
                };

                if ui
                    .button("▶")
                    .on_hover_text("Play on your device")
                    .clicked()
                {
                    to_play = Some(track.clone());
                }
                if track.preview_url.is_some()
                    && ui
                        .button("⤓")
                        .on_hover_text("Play the preview locally")
                        .clicked()
                {
                    to_preview = Some(track.clone());
                }
                if app.state.config.youtube.enabled
                    && ui.button("▶").on_hover_text("Play from YouTube").clicked()
                {
                    to_youtube = Some(track.clone());
                }
                ui.label(title);
                ui.label(track.artist_names());
                ui.label(track.album_name());
                ui.label(track.duration_label());
                ui.end_row();
            }
        });

    if let Some(track) = to_play {
        app.play_track(&track);
    }
    if let Some(track) = to_preview {
        app.play_preview(&track);
    }
    if let Some(track) = to_youtube {
        app.play_on_youtube(&track);
    }
}

/// A card for an album.
fn album_card(
    ui: &mut egui::Ui,
    album: &Album,
    bandwidth_saver: &BandwidthSaver,
) -> Option<DetailTarget> {
    let response = ui.vertical(|ui| {
        album_art(
            ui,
            album.images.first().map(|image| image.url.as_str()),
            140.0,
            bandwidth_saver,
        );
        ui.label(egui::RichText::new(&album.name).strong().size(12.0));
        let artists = album
            .artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        ui.label(
            egui::RichText::new(artists)
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    });
    response
        .response
        .clicked()
        .then(|| DetailTarget::Album(album.id.clone()))
}

/// A card for an artist.
fn artist_card(
    ui: &mut egui::Ui,
    artist: &Artist,
    bandwidth_saver: &BandwidthSaver,
) -> Option<DetailTarget> {
    let response = ui.vertical(|ui| {
        album_art(
            ui,
            artist.images.first().map(|image| image.url.as_str()),
            140.0,
            bandwidth_saver,
        );
        ui.label(egui::RichText::new(&artist.name).strong().size(12.0));
        ui.label(
            egui::RichText::new("Artist")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    });
    response
        .response
        .clicked()
        .then(|| DetailTarget::Artist(artist.id.clone()))
}

/// A card for a playlist.
fn playlist_card(
    ui: &mut egui::Ui,
    playlist: &Playlist,
    bandwidth_saver: &BandwidthSaver,
) -> Option<DetailTarget> {
    let response = ui.vertical(|ui| {
        album_art(
            ui,
            playlist.images.first().map(|image| image.url.as_str()),
            140.0,
            bandwidth_saver,
        );
        ui.label(egui::RichText::new(&playlist.name).strong().size(12.0));
        ui.label(
            egui::RichText::new("Playlist")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    });
    response
        .response
        .clicked()
        .then(|| DetailTarget::Playlist(playlist.id.clone()))
}

/// The local music screen.
pub fn local_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Local music");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Rescan").clicked() {
                app.rescan_local_library();
            }
        });
    });

    let folder = app.state.config.library.local_folder.clone();
    if folder.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label("No folder is set. Choose one in the settings.");
        });
        return;
    }
    ui.label(
        egui::RichText::new(&folder)
            .small()
            .color(ui.visuals().weak_text_color()),
    );

    if app.state.local_tracks.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label("No audio files were found in that folder.");
        });
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for track in app.state.local_tracks.clone() {
            ui.horizontal(|ui| {
                if ui.button("▶").clicked() {
                    app.play_local_track(&track);
                }
                ui.label(&track.title);
                ui.label(
                    egui::RichText::new(&track.artist)
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        }
    });
}

/// The settings screen.
pub fn settings_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Privacy");
        ui.checkbox(
            &mut app.state.config.privacy.private_session,
            "Private session",
        );
        ui.checkbox(&mut app.state.config.privacy.no_logging, "No logging");
        ui.checkbox(
            &mut app.state.config.privacy.limit_telemetry,
            "Limit telemetry",
        );
        ui.checkbox(
            &mut app.state.config.privacy.no_local_history,
            "No local listening history",
        );
        ui.label(
            egui::RichText::new(
                "Rustify sends no analytics and keeps no listening history on disk.",
            )
            .small()
            .color(ui.visuals().weak_text_color()),
        );
        ui.separator();

        ui.heading("Ad blocker");
        ui.checkbox(
            &mut app.state.config.ad_blocker.enabled,
            "Enable ad blocking",
        );
        ui.checkbox(
            &mut app.state.config.ad_blocker.block_audio_ads,
            "Block audio ads",
        );
        ui.checkbox(
            &mut app.state.config.ad_blocker.block_banner_ads,
            "Block banner ads",
        );
        ui.checkbox(
            &mut app.state.config.ad_blocker.block_video_ads,
            "Block video ads",
        );
        ui.separator();

        ui.heading("Bandwidth saver");
        ui.checkbox(
            &mut app.state.config.bandwidth_saver.enabled,
            "Extreme Bandwidth Saver Mode",
        );
        ui.checkbox(
            &mut app.state.config.bandwidth_saver.disable_images,
            "Disable images",
        );
        ui.checkbox(
            &mut app.state.config.bandwidth_saver.disable_animations,
            "Disable animations",
        );
        if app.state.config.bandwidth_saver.enabled {
            ui.label(
                egui::RichText::new("No image is downloaded while this mode is on.")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        }
        ui.separator();

        ui.heading("Audio");
        ui.label(format!(
            "Volume: {:.0}%",
            app.state.config.audio.volume * 100.0
        ));
        ui.add(egui::Slider::new(
            &mut app.state.config.audio.volume,
            0.0..=1.0,
        ));
        ui.separator();

        ui.heading("Where the audio comes from");
        egui::ComboBox::from_label("Audio source")
            .selected_text(match app.state.config.library.audio_source {
                crate::config::AudioSourcePreference::Automatic => "Automatic",
                crate::config::AudioSourcePreference::Device => "Your Spotify device",
                crate::config::AudioSourcePreference::Preview => "Preview clip",
                crate::config::AudioSourcePreference::YouTube => "YouTube",
                crate::config::AudioSourcePreference::Local => "Local files",
            })
            .show_ui(ui, |ui| {
                use crate::config::AudioSourcePreference as Preference;
                for (value, label) in [
                    (Preference::Automatic, "Automatic"),
                    (Preference::Device, "Your Spotify device"),
                    (Preference::Preview, "Preview clip"),
                    (Preference::YouTube, "YouTube"),
                    (Preference::Local, "Local files"),
                ] {
                    ui.selectable_value(&mut app.state.config.library.audio_source, value, label);
                }
            });
        ui.label(
            egui::RichText::new(
                "Automatic plays the preview, tries YouTube when it is set up, \
                 and falls back so a button is never dead.",
            )
            .small()
            .color(ui.visuals().weak_text_color()),
        );
        ui.separator();

        ui.heading("YouTube");
        ui.checkbox(
            &mut app.state.config.youtube.enabled,
            "Use YouTube as an audio source",
        );
        ui.label(
            egui::RichText::new(
                "Rustify finds the track on YouTube and plays it locally. \
                 Spotify's audio ads are not part of it.",
            )
            .small()
            .color(ui.visuals().weak_text_color()),
        );
        ui.add(
            egui::TextEdit::singleline(&mut app.state.config.youtube.invidious_instance)
                .hint_text("Invidious instance, for example https://inv.example.org")
                .desired_width(f32::INFINITY),
        );
        ui.add(
            egui::TextEdit::singleline(&mut app.state.config.youtube.yt_dlp_path)
                .hint_text("Path to yt-dlp, for example /usr/bin/yt-dlp")
                .desired_width(f32::INFINITY),
        );
        ui.separator();

        ui.heading("Local music");
        ui.add(
            egui::TextEdit::singleline(&mut app.state.config.library.local_folder)
                .hint_text("Folder with your music")
                .desired_width(f32::INFINITY),
        );
        if ui.button("Choose a folder").clicked() {
            app.choose_local_folder();
        }
        ui.separator();

        ui.heading("Appearance");
        egui::ComboBox::from_label("Theme")
            .selected_text(app.state.config.ui.theme.clone())
            .show_ui(ui, |ui| {
                for theme in ["dark", "light"] {
                    ui.selectable_value(&mut app.state.config.ui.theme, theme.to_string(), theme);
                }
            });
        ui.checkbox(
            &mut app.state.config.ui.show_cover_panel,
            "Show the cover panel",
        );
        ui.label(format!(
            "Search results: {}",
            app.state.config.ui.search_limit
        ));
        ui.add(egui::Slider::new(
            &mut app.state.config.ui.search_limit,
            5..=50,
        ));
        ui.separator();

        if ui.button("Save settings").clicked() {
            app.save_config();
        }
    });
}

/// The right-hand cover panel.
pub fn cover_panel(app: &mut RustifyApp, ui: &mut egui::Ui) {
    if !app.state.config.ui.show_cover_panel {
        return;
    }

    ui.vertical_centered(|ui| {
        let url = app.state.now_playing.art_url.clone();
        let blocked = app.bandwidth_saver.should_disable_images();
        match url {
            Some(url) if !blocked => {
                let sized = sized_image_url(&url, COVER_IMAGE_SIZE);
                ui.add(
                    egui::Image::from_uri(sized)
                        .fit_to_exact_size(egui::vec2(220.0, 220.0))
                        .corner_radius(6.0),
                );
            }
            _ => {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(220.0, 220.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 6.0, egui::Color32::from_gray(50));
            }
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new(&app.state.now_playing.title).strong());
        ui.label(
            egui::RichText::new(&app.state.now_playing.artist)
                .small()
                .color(ui.visuals().weak_text_color()),
        );

        if !app.state.device.devices.is_empty() {
            ui.add_space(8.0);
            ui.label("Devices");
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .show(ui, |ui| {
                    for device in app.state.device.devices.clone() {
                        let label = device.name.clone().unwrap_or_else(|| "Unknown".into());
                        let active = device.is_active;
                        if ui.selectable_label(active, label).clicked()
                            && !active
                            && let Some(id) = device.id.clone()
                        {
                            app.dispatch(
                                crate::worker::RequestKind::PlaybackCommand,
                                crate::worker::Command::TransferPlayback { device_id: id },
                            );
                        }
                    }
                });
        }
    });
}
