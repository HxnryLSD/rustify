use crate::app_state::*;
use crate::login_view::login_view;
use crate::ui::RustifyApp;
use crate::views;
use eframe::egui;

/// How often the UI refreshes while something is in flight.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(120);

/// Accent colour used for the active navigation entry and the play button.
const ACCENT: egui::Color32 = egui::Color32::from_rgb(30, 215, 96);

impl eframe::App for RustifyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_responses();
        self.bandwidth_saver.apply(ui.ctx());

        // The login screen takes over the whole window.
        if !self.state.logged_in {
            login_view(self, ui);
            self.render_notice(ui);
            ui.ctx().request_repaint_after(POLL_INTERVAL);
            return;
        }

        egui::Panel::left("nav")
            .exact_size(190.0)
            .resizable(false)
            .show(ui, |ui| {
                self.render_sidebar(ui);
            });
        egui::Panel::right("cover")
            .exact_size(260.0)
            .resizable(false)
            .show(ui, |ui| {
                views::cover_panel(self, ui);
            });
        egui::Panel::bottom("player").show(ui, |ui| {
            self.render_player_bar(ui);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            match self.state.view {
                View::Login => login_view(self, ui),
                View::Home => views::home_view(self, ui),
                View::Search => views::search_view(self, ui),
                View::Library => views::library_view(self, ui),
                View::Playlists => views::playlists_view(self, ui),
                View::Album => views::album_view(self, ui),
                View::Artist => views::artist_view(self, ui),
                View::Playlist => views::playlist_view(self, ui),
                View::Local => views::local_view(self, ui),
                View::Settings => views::settings_view(self, ui),
            }
            self.render_notice(ui);
        });

        // Repaint while requests are in flight so progress is visible.
        if !self.tracker.is_empty() {
            ui.ctx().request_repaint_after(POLL_INTERVAL);
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        // Persist the configuration whenever eframe decides to save.
        let _ = self.state.config.save();
    }

    /// Called once on shutdown, after `save`.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.state.config.save();
    }
}

impl RustifyApp {
    /// Render the navigation sidebar.
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Rustify")
                    .size(20.0)
                    .strong()
                    .color(ACCENT),
            );
        });
        ui.add_space(4.0);
        ui.separator();

        let items: [(View, &str); 7] = [
            (View::Home, "Home"),
            (View::Search, "Search"),
            (View::Library, "Library"),
            (View::Playlists, "Playlists"),
            (View::Local, "Local music"),
            (View::Settings, "Settings"),
            (View::Login, "Account"),
        ];

        for (view, label) in items {
            let selected = self.state.view == view;
            let text = if selected {
                egui::RichText::new(label).strong().color(ACCENT)
            } else {
                egui::RichText::new(label)
            };
            if ui.selectable_label(selected, text).clicked() {
                self.navigate(view);
            }
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.separator();
            if let Some(user) = &self.state.logged_in_user {
                ui.label(egui::RichText::new(user).small());
            }
            if ui.button("Sign out").clicked() {
                self.sign_out();
            }
        });
    }

    /// Render the playback bar at the bottom of the window.
    fn render_player_bar(&mut self, ui: &mut egui::Ui) {
        // The local player knows its own position, so the progress is read
        // from it rather than from the remote device.
        let local_position = if self.state.now_playing.is_local {
            self.audio.as_ref().map(|audio| audio.position())
        } else {
            None
        };

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&self.state.now_playing.title).strong());
            ui.label(
                egui::RichText::new(&self.state.now_playing.artist)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            if self.state.now_playing.source != AudioSource::None {
                ui.label(
                    egui::RichText::new(self.state.now_playing.source.label())
                        .small()
                        .color(egui::Color32::from_rgb(240, 180, 60)),
                );
                if let Some(position) = local_position {
                    ui.label(
                        egui::RichText::new(format!("{:.0}s", position.as_secs_f32()))
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⏭").on_hover_text("Next track").clicked() {
                    self.dispatch(
                        crate::worker::RequestKind::PlaybackCommand,
                        crate::worker::Command::Next,
                    );
                }
                if ui.button("⏮").on_hover_text("Previous track").clicked() {
                    self.dispatch(
                        crate::worker::RequestKind::PlaybackCommand,
                        crate::worker::Command::Previous,
                    );
                }

                let playing = self.state.now_playing.is_playing;
                let label = if playing { "⏸" } else { "▶" };
                let button = egui::Button::new(
                    egui::RichText::new(label)
                        .color(egui::Color32::BLACK)
                        .size(16.0),
                )
                .fill(ACCENT)
                .corner_radius(16.0)
                .min_size(egui::vec2(34.0, 30.0));

                if ui.add(button).on_hover_text("Play or pause").clicked() {
                    if self.state.now_playing.is_local {
                        // Local preview playback is controlled directly.
                        if let Some(audio) = &self.audio {
                            audio.toggle_play();
                        }
                        self.state.now_playing.is_playing = !playing;
                    } else if playing {
                        self.dispatch(
                            crate::worker::RequestKind::PlaybackCommand,
                            crate::worker::Command::Pause,
                        );
                        self.state.now_playing.is_playing = false;
                    } else {
                        self.dispatch(
                            crate::worker::RequestKind::PlaybackCommand,
                            crate::worker::Command::LoadPlaybackState,
                        );
                    }
                }
            });
        });
    }

    /// Render the transient message, if any.
    fn render_notice(&mut self, ui: &mut egui::Ui) {
        let Some(notice) = self.state.notice.clone() else {
            return;
        };
        let color = match notice.kind {
            NoticeKind::Info => egui::Color32::LIGHT_BLUE,
            NoticeKind::Warning => egui::Color32::from_rgb(240, 180, 60),
            NoticeKind::Error => egui::Color32::from_rgb(240, 90, 90),
        };
        ui.horizontal(|ui| {
            ui.colored_label(color, &notice.text);
            if ui.small_button("✕").clicked() {
                self.state.clear_notice();
            }
        });
    }

    /// Forget the session and return to the login screen.
    pub fn sign_out(&mut self) {
        crate::ui::set_access_token(None);
        self.state.logged_in = false;
        self.state.logged_in_user = None;
        self.state.home = Default::default();
        self.state.library = Default::default();
        self.state.playlists = Default::default();
        self.state.search = Default::default();
        self.state.detail = Default::default();
        self.state.now_playing = Default::default();
        self.state.navigate(View::Login);
    }
}
