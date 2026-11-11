use crate::app_state::Notice;
use crate::ui::RustifyApp;
use eframe::egui;

/// The login screen.
///
/// Two sign-in paths are offered: the Spotify web-player cookie exchange and
/// the official OAuth 2.0 authorization-code flow with PKCE.
pub fn login_view(app: &mut RustifyApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(24.0);
        ui.heading(egui::RichText::new("Rustify").size(28.0));
        ui.label("A lightweight, privacy-focused unofficial Spotify client");
        ui.add_space(16.0);

        if app.state.logging_in {
            ui.spinner();
            ui.label("Signing in…");
            return;
        }

        ui.group(|ui| {
            ui.set_width(460.0);
            ui.label(egui::RichText::new("Sign in with the web player").strong());
            ui.label(
                egui::RichText::new(
                    "Paste the sp_dc cookie from your browser's Spotify cookies. \
                     No developer account is needed.",
                )
                .small()
                .color(ui.visuals().weak_text_color()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.sp_dc_input)
                    .hint_text("sp_dc cookie value")
                    .desired_width(f32::INFINITY),
            );
            if ui.button("Sign in with cookie").clicked() {
                let cookie = app.sp_dc_input.trim().to_string();
                if cookie.is_empty() {
                    app.state
                        .notify(Notice::error("Paste your sp_dc cookie first"));
                } else {
                    app.begin_cookie_login(cookie);
                }
            }
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.set_width(460.0);
            ui.label(egui::RichText::new("Sign in with OAuth").strong());
            ui.label(
                egui::RichText::new(
                    "Use a Spotify application client id. Rustify opens your browser \
                     and receives the callback on a local loopback port.",
                )
                .small()
                .color(ui.visuals().weak_text_color()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.client_id_input)
                    .hint_text("Spotify application client id")
                    .desired_width(f32::INFINITY),
            );
            if ui.button("Sign in with OAuth").clicked() {
                let client_id = app.client_id_input.trim().to_string();
                if client_id.is_empty() {
                    app.state
                        .notify(Notice::error("Enter your application client id first"));
                } else {
                    app.begin_oauth_login(client_id);
                }
            }
            if app.oauth_pending
                && let Some(url) = app.oauth_url.clone()
            {
                ui.add_space(6.0);
                ui.label("Waiting for the browser callback…");
                let _ = ui.selectable_label(false, url.clone());
                if ui.button("Open the browser again").clicked() {
                    open_in_browser(&url);
                }
            }
        });

        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(
                "Rustify stores your access token in a file only you can read, \
                 sends no analytics, and keeps no listening history.",
            )
            .small()
            .color(ui.visuals().weak_text_color()),
        );
    });
}

/// Open a URL in the user's default browser.
pub fn open_in_browser(url: &str) {
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}
