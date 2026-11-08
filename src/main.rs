use eframe::egui;
use rustify::{
    auth::AuthState,
    config::{Config, TokenCache},
    ui::RustifyApp,
};

fn main() -> eframe::Result {
    // Load configuration (includes user-provided sp_dc cookie)
    let config = Config::load();

    // Load cached tokens if present and apply them to auth state
    let auth_state = AuthState::new();
    if let Some(tokens) = TokenCache::load()
        && !tokens.is_expired()
    {
        auth_state.set_tokens_blocking(tokens);
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
