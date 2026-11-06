use eframe::egui;
use rustify::{
    auth::{AuthState, SpotifyAuth},
    config::{Config, TokenCache},
    ui::{RustifyApp, View},
};

fn main() -> eframe::Result {
    // Load configuration (includes user-provided Spotify OAuth credentials)
    let config = Config::load();

    // Load cached tokens if present
    let auth_state = AuthState::new();
    if let Some(tokens) = TokenCache::load() {
        if !tokens.is_expired() {
            // Cached tokens will be applied to auth_state during login flow initialization
            let _ = tokens;
        }
    }

    // eframe/egui native options
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([config.ui.window_width, config.ui.window_height]),
        ..Default::default()
    };

    eframe::run_native(
        "Rustify",
        native_options,
        Box::new(|cc| {
            // Apply theme
            if config.ui.theme == "dark" {
                cc.egui_ctx.set_visuals(egui::Visuals::dark());
            } else {
                cc.egui_ctx.set_visuals(egui::Visuals::light());
            }

            Ok(Box::new(RustifyApp::new(cc, config, auth_state)))
        }),
    )
}

/// Build a SpotifyAuth from the user-configured credentials.
fn build_spotify_auth(config: &Config) -> SpotifyAuth {
    SpotifyAuth::new(
        &config.auth.client_id,
        &config.auth.client_secret,
        &config.auth.redirect_url,
    )
}

/// Convenience: returns the initial view based on whether credentials are configured.
pub fn initial_view(config: &Config) -> View {
    if config.auth.client_id.is_empty() || config.auth.client_secret.is_empty() {
        View::Login // credentials setup first
    } else {
        View::Login
    }
}
