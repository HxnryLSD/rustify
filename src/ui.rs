use crate::{
    ad_blocker::AdBlocker,
    auth::{AuthState, SpotifyAuth},
    bandwidth_saver::BandwidthSaver,
    config::Config,
    privacy::Privacy,
    spotify_client::SpotifyClient,
};
use eframe::egui;

/// Application navigation views.
pub enum View {
    Login,
    Home,
    Search,
    Player,
    Settings,
    Library,
}

/// Application state shared between the egui UI and backend services.
pub struct RustifyApp {
    pub config: Config,
    pub auth_state: AuthState,
    pub spotify_client: SpotifyClient,
    pub ad_blocker: AdBlocker,
    pub bandwidth_saver: BandwidthSaver,
    pub privacy: Privacy,

    // UI navigation
    pub current_view: View,
    pub search_query: String,
    pub search_results: Vec<crate::spotify_client::Track>,
    pub is_loading: bool,
    pub error_message: Option<String>,

    // Playback
    pub is_playing: bool,
    pub current_track: Option<crate::spotify_client::Track>,
}

impl RustifyApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config, auth_state: AuthState) -> Self {
        // Apply theme based on config
        if config.ui.theme == "dark" {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
        } else {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
        }

        Self {
            config,
            auth_state,
            spotify_client: SpotifyClient::new().unwrap_or_else(|_| SpotifyClient::default()),
            ad_blocker: AdBlocker::default(),
            bandwidth_saver: BandwidthSaver::default(),
            privacy: Privacy::default(),
            current_view: View::Login,
            search_query: String::new(),
            search_results: Vec::new(),
            is_loading: false,
            error_message: None,
            is_playing: false,
            current_track: None,
        }
    }

    // -----------------------------------------------------------------------
    // Login view
    // -----------------------------------------------------------------------
    fn render_login(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("Log in with Spotify");
            ui.label("Paste your sp_dc cookie value from your browser's Spotify cookies.");
            ui.label("No developer credentials needed.");
            ui.separator();

            ui.text_edit_singleline(&mut self.config.auth.sp_dc_cookie);

            if ui.button("Log in with sp_dc cookie").clicked() {
                let sp_dc = self.config.auth.sp_dc_cookie.trim().to_string();
                if sp_dc.is_empty() {
                    self.error_message = Some("Please enter your sp_dc cookie.".to_string());
                    return;
                }
                self.start_sp_dc_flow(sp_dc);
            }

            if let Some(err) = &self.error_message {
                ui.colored_label(egui::Color32::RED, err);
            }
        });
    }

    /// Begin the sp_dc cookie authentication flow: exchange the sp_dc cookie
    /// for an access token via Spotify's token endpoint.
    fn start_sp_dc_flow(&mut self, sp_dc_cookie: String) {
        self.is_loading = true;
        self.error_message = None;

        let auth_state = self.auth_state.clone();
        let auth = SpotifyAuth::new(&sp_dc_cookie);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(auth.exchange_cookie_for_token());

            match result {
                Ok(tokens) => {
                    auth_state.set_tokens_blocking(tokens.clone());
                    let _ = tokens.save();
                }
                Err(e) => {
                    let _ = e;
                }
            }
        });

        // Check result on next frame via is_loading / error_message
        // (simplified synchronous approach for now)
        let auth = SpotifyAuth::new(&sp_dc_cookie);
        let rt = tokio::runtime::Runtime::new().unwrap();
        match rt.block_on(auth.exchange_cookie_for_token()) {
            Ok(tokens) => {
                self.auth_state.set_tokens_blocking(tokens.clone());
                if let Err(e) = tokens.save() {
                    self.error_message = Some(format!("Failed to cache tokens: {e}"));
                } else {
                    self.error_message = None;
                    self.current_view = View::Home;
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Token exchange failed: {e}"));
            }
        }
        self.is_loading = false;
    }

    // -----------------------------------------------------------------------
    // Home view
    // -----------------------------------------------------------------------
    fn render_home(&mut self, ui: &mut egui::Ui) {
        ui.heading("Home");

        if ui.button("Search").clicked() {
            self.current_view = View::Search;
        }
        if ui.button("Library").clicked() {
            self.current_view = View::Library;
        }
        if ui.button("Player").clicked() {
            self.current_view = View::Player;
        }
        if ui.button("Settings").clicked() {
            self.current_view = View::Settings;
        }
        if ui.button("Logout").clicked() {
            // Clear tokens and return to login
            let auth_state = self.auth_state.clone();
            // Cannot await here synchronously; clear flag only.
            self.current_view = View::Login;
            let _ = auth_state;
        }
    }

    // -----------------------------------------------------------------------
    // Search view
    // -----------------------------------------------------------------------
    fn render_search(&mut self, ui: &mut egui::Ui) {
        ui.heading("Search");
        ui.text_edit_singleline(&mut self.search_query);
        if ui.button("Search").clicked() && !self.search_query.is_empty() {
            let query = self.search_query.clone();
            let tokens = match self.auth_state.get_tokens_blocking() {
                Some(t) => t,
                None => {
                    self.error_message = Some("Not logged in. Please authorize first.".to_string());
                    self.is_loading = false;
                    return;
                }
            };
            self.is_loading = true;
            self.error_message = None;

            // Run the Spotify search on a blocking tokio runtime.
            let client = self.spotify_client.clone();
            let privacy = self.config.privacy.clone();
            match std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(client.search(&tokens.access_token, &privacy, &query, "track", 20))
            })
            .join()
            .unwrap()
            {
                Ok(response) => {
                    self.search_results = response.tracks.map(|t| t.items).unwrap_or_default();
                }
                Err(e) => {
                    self.error_message = Some(format!("Search failed: {e}"));
                }
            }
            self.is_loading = false;
        }
        if self.is_loading {
            ui.label("Loading...");
        }
        for track in &self.search_results {
            ui.label(format!("{} - {}", track.name, track.artists[0].name));
            let track_name = track.name.clone();
            let artist_name = track
                .artists
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default();
            if ui.button("Play").clicked() {
                let allowed = self.ad_blocker.is_ad_track(&track_name, &artist_name);
                if allowed {
                    self.current_track = Some(track.clone());
                    self.is_playing = true;
                } else {
                    self.error_message = Some("Track blocked by ad blocker".to_string());
                }
            }
        }
        if ui.button("Back").clicked() {
            self.current_view = View::Home;
        }
    }

    // -----------------------------------------------------------------------
    // Player view
    // -----------------------------------------------------------------------
    fn render_player(&mut self, ui: &mut egui::Ui) {
        ui.heading("Player");
        if let Some(track) = &self.current_track {
            ui.label(format!("Now playing: {}", track.name));
            ui.label(format!("Artist: {}", track.artists[0].name));
            if self.bandwidth_saver.should_disable_images() {
                ui.label("[Album art disabled - Bandwidth Saver Mode]");
            } else {
                ui.label("[Album art would render here]");
            }
        } else {
            ui.label("No track playing.");
        }
        if self.is_playing {
            if ui.button("Pause").clicked() {
                let tokens = match self.auth_state.get_tokens_blocking() {
                    Some(t) => t,
                    None => {
                        self.error_message = Some("Not logged in.".to_string());
                        return;
                    }
                };
                let client = self.spotify_client.clone();
                let privacy = self.config.privacy.clone();
                match std::thread::spawn(move || {
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(client.pause(&tokens.access_token, &privacy))
                })
                .join()
                .unwrap()
                {
                    Ok(_) => self.is_playing = false,
                    Err(e) => self.error_message = Some(format!("Pause failed: {e}")),
                }
            }
        } else {
            if ui.button("Play").clicked()
                && let Some(track) = &self.current_track
            {
                let allowed = self
                    .ad_blocker
                    .is_ad_track(&track.name, &track.artists[0].name);
                if allowed {
                    let tokens = match self.auth_state.get_tokens_blocking() {
                        Some(t) => t,
                        None => {
                            self.error_message = Some("Not logged in.".to_string());
                            return;
                        }
                    };
                    let uris = vec![format!("spotify:track:{}", track.id)];
                    let client = self.spotify_client.clone();
                    let privacy = self.config.privacy.clone();
                    match std::thread::spawn(move || {
                        tokio::runtime::Runtime::new()
                            .unwrap()
                            .block_on(client.play(&tokens.access_token, &privacy, Some(&uris)))
                    })
                    .join()
                    .unwrap()
                    {
                        Ok(_) => self.is_playing = true,
                        Err(e) => self.error_message = Some(format!("Play failed: {e}")),
                    }
                } else {
                    self.error_message = Some("Track blocked by ad blocker".to_string());
                }
            }
        }
        if ui.button("Next").clicked() {
            let tokens = match self.auth_state.get_tokens_blocking() {
                Some(t) => t,
                None => {
                    self.error_message = Some("Not logged in.".to_string());
                    return;
                }
            };
            let client = self.spotify_client.clone();
            let privacy = self.config.privacy.clone();
            match std::thread::spawn(move || {
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(client.next_track(&tokens.access_token, &privacy))
            })
            .join()
            .unwrap()
            {
                Ok(_) => {}
                Err(e) => self.error_message = Some(format!("Next failed: {e}")),
            }
        }
        if ui.button("Back").clicked() {
            self.current_view = View::Home;
        }
    }

    // -----------------------------------------------------------------------
    // Settings view
    // -----------------------------------------------------------------------
    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");

        ui.collapsing("Privacy", |ui| {
            ui.checkbox(&mut self.config.privacy.no_logging, "No logging (privacy)");
            ui.checkbox(&mut self.config.privacy.limit_telemetry, "Limit telemetry");
            ui.label("Private session: always enforced");
        });

        ui.collapsing("Ad Blocker", |ui| {
            ui.checkbox(&mut self.config.ad_blocker.enabled, "Enable ad blocking");
            ui.checkbox(
                &mut self.config.ad_blocker.block_audio_ads,
                "Block audio ads",
            );
            ui.checkbox(
                &mut self.config.ad_blocker.block_banner_ads,
                "Block banner ads",
            );
            ui.checkbox(
                &mut self.config.ad_blocker.block_video_ads,
                "Block video ads",
            );
        });

        ui.collapsing("Bandwidth Saver", |ui| {
            ui.checkbox(
                &mut self.config.bandwidth_saver.enabled,
                "Extreme Bandwidth Saver Mode",
            );
            if self.config.bandwidth_saver.enabled {
                ui.label("Images disabled: album art shown as placeholders");
                ui.label("Animations disabled");
            }
        });

        ui.separator();
        ui.label(format!("Volume: {:.0}%", self.config.audio.volume * 100.0));
        ui.add(egui::Slider::new(&mut self.config.audio.volume, 0.0..=1.0));

        if ui.button("Save").clicked()
            && let Err(e) = self.config.save()
        {
            self.error_message = Some(format!("Failed to save config: {e}"));
        }

        if ui.button("Back").clicked() {
            self.current_view = View::Home;
        }
    }

    // -----------------------------------------------------------------------
    // Library view
    // -----------------------------------------------------------------------
    fn render_library(&mut self, ui: &mut egui::Ui) {
        ui.heading("Library");
        ui.label("Saved tracks and playlists will appear here.");
        if ui.button("Back").clicked() {
            self.current_view = View::Home;
        }
    }
}

impl eframe::App for RustifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply bandwidth saver: disable animations when enabled
        if self.bandwidth_saver.should_disable_animations() {
            // egui has no global animation toggle; we simply avoid animated widgets.
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Rustify");
            ui.separator();

            match self.current_view {
                View::Login => self.render_login(ui),
                View::Home => self.render_home(ui),
                View::Search => self.render_search(ui),
                View::Player => self.render_player(ui),
                View::Settings => self.render_settings(ui),
                View::Library => self.render_library(ui),
            }

            if let Some(err) = &self.error_message {
                ui.colored_label(egui::Color32::RED, err);
            }
        });

        // Request repaint so loading states animate smoothly
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
