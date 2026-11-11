use eframe::egui;
use rustify::config::{Config, TokenCache};
use rustify::ui::RustifyApp;

fn main() -> eframe::Result {
    // Load the configuration; the token cache is handled separately so an
    // invalid config file never invalidates a valid session.
    let config = Config::load();

    // Create the config file on first launch so the user can find and edit
    // it without hunting through the menu.
    if let Some(path) = Config::config_path()
        && !path.exists()
    {
        let _ = config.save_to(&path);
    }

    // Restore a cached session when the access token is still usable.
    if let Some(tokens) = TokenCache::load()
        && !tokens.is_expired()
    {
        rustify::ui::set_access_token(Some(tokens.access_token.clone()));
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([config.ui.window_width, config.ui.window_height])
            .with_min_inner_size([820.0, 560.0])
            .with_title("Rustify"),
        ..Default::default()
    };

    eframe::run_native(
        "Rustify",
        native_options,
        Box::new(|cc| {
            // Album art is loaded over HTTP; the loaders are only installed
            // when images are allowed.
            if !config.bandwidth_saver.enabled {
                egui_extras::install_image_loaders(&cc.egui_ctx);
            }

            Ok(Box::new(RustifyApp::new(cc, config)))
        }),
    )
}
