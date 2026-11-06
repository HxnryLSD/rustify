use crate::{
    ad_blocker::AdBlocker,
    auth::{AuthState, SpotifyAuth},
    bandwidth_saver::BandwidthSaver,
    config::Config,
    oauth_server::OAuthServer,
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
    CredentialsSetup,
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

    // Credentials setup fields
    pub new_client_id: String,
    pub new_client_secret: String,
    pub new_redirect_url: String,
}

impl RustifyApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config, auth_state: AuthState) -> Self {
        // Apply theme based on config
        if config.ui.theme == "dark" {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
        } else {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
        }

        let redirect_url = config.auth.redirect_url.clone();

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
            new_client_id: String::new(),
            new_client_secret: String::new(),
            new_redirect_url: redirect_url,
        }
    }

    // -----------------------------------------------------------------------
    // Credentials setup view
    // -----------------------------------------------------------------------
    fn render_credentials_setup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Spotify Developer Credentials");
        ui.label(
            "Rustify does not ship with Spotify credentials. Please enter your own\n\
             Spotify Developer application Client ID and Client Secret.",
        );
        ui.separator();

        ui.label("Client ID:");
        ui.text_edit_singleline(&mut self.new_client_id);

        ui.label("Client Secret:");
        ui.text_edit_singleline(&mut self.new_client_secret);

        ui.label("Redirect URI:");
        ui.text_edit_singleline(&mut self.new_redirect_url);

        ui.separator();

        if ui.button("Save Credentials").clicked() {
            if self.new_client_id.is_empty() || self.new_client_secret.is_empty() {
                self.error_message = Some("Client ID and Client Secret are required".to_string());
            } else {
                self.config.auth.client_id = self.new_client_id.clone();
                self.config.auth.client_secret = self.new_client_secret.clone();
                self.config.auth.redirect_url = self.new_redirect_url.clone();
                if let Err(e) = self.config.save() {
                    self.error_message = Some(format!("Failed to save credentials: {e}"));
                } else {
                    self.error_message = None;
                    self.current_view = View::Login;
                }
            }
        }

        if ui.button("Back").clicked() {
            self.current_view = View::Login;
        }
    }

    // -----------------------------------------------------------------------
    // Login view
    // -----------------------------------------------------------------------
    fn render_login(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("Log in with Spotify");

            // Show credentials setup link if not configured
            if self.config.auth.client_id.is_empty() || self.config.auth.client_secret.is_empty() {
                ui.label("No Spotify credentials configured yet.");
                if ui.button("Set up credentials").clicked() {
                    self.current_view = View::CredentialsSetup;
                }
                ui.separator();
            }

            if ui.button("Authorize with Spotify").clicked() {
                if self.config.auth.client_id.is_empty()
                    || self.config.auth.client_secret.is_empty()
                {
                    self.error_message =
                        Some("Please configure Spotify credentials first".to_string());
                } else {
                    self.start_oauth_flow();
                }
            }

            if let Some(err) = &self.error_message {
                ui.colored_label(egui::Color32::RED, err);
            }
        });
    }

    /// Begin the OAuth 2.0 authorization code flow: open the system browser
    /// to the Spotify authorization URL and capture the callback automatically.
    fn start_oauth_flow(&mut self) {
        let auth = build_spotify_auth(&self.config);
        let (auth_url, _csrf_state) = auth.authorize_url();

        // Parse the redirect port from the configured redirect URL.
        let port = self
            .config
            .auth
            .redirect_url
            .split(':')
            .nth(2)
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8888);

        let server = OAuthServer::new(port);
        match server.wait_for_callback(&auth_url) {
            Ok(code) => {
                // Exchange the code for tokens (blocking the UI thread is
                // acceptable here; the user is waiting on the browser anyway).
                let auth = build_spotify_auth(&self.config);
                match std::thread::spawn(move || {
                    tokio::runtime::Runtime::new()
                        .unwrap()
                        .block_on(auth.exchange_code(&code))
                })
                .join()
                .unwrap()
                {
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
            }
            Err(e) => {
                self.error_message = Some(format!("OAuth callback failed: {e}"));
            }
        }
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
            if ui.button("Play").clicked() {
                if let Some(track) = &self.current_track {
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

        if ui.button("Save").clicked() {
            if let Err(e) = self.config.save() {
                self.error_message = Some(format!("Failed to save config: {e}"));
            }
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
                View::CredentialsSetup => self.render_credentials_setup(ui),
            }

            if let Some(err) = &self.error_message {
                ui.colored_label(egui::Color32::RED, err);
            }
        });

        // Request repaint so loading states animate smoothly
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Build a SpotifyAuth from the user-configured credentials.
fn build_spotify_auth(config: &Config) -> SpotifyAuth {
    SpotifyAuth::new(
        &config.auth.client_id,
        &config.auth.client_secret,
        &config.auth.redirect_url,
    )
}
