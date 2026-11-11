use egui::Context;

/// Extreme Bandwidth Saver Mode policy.
///
/// When enabled, image loading is suppressed, animations are frozen, and
/// non-essential network traffic is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandwidthSaver {
    pub enabled: bool,
    pub disable_images: bool,
    pub disable_animations: bool,
}

impl Default for BandwidthSaver {
    fn default() -> Self {
        Self {
            enabled: false,
            disable_images: true,
            disable_animations: true,
        }
    }
}

impl BandwidthSaver {
    /// Build the policy from the persisted configuration.
    pub fn from_config(enabled: bool, disable_images: bool, disable_animations: bool) -> Self {
        Self {
            enabled,
            disable_images,
            disable_animations,
        }
    }

    /// Whether image rendering should be suppressed.
    ///
    /// When this is true no image HTTP request is ever issued.
    pub fn should_disable_images(&self) -> bool {
        self.enabled && self.disable_images
    }

    /// Whether animations should be disabled.
    pub fn should_disable_animations(&self) -> bool {
        self.enabled && self.disable_animations
    }

    /// Whether non-essential functions should be disabled.
    pub fn should_disable_non_essential(&self) -> bool {
        self.enabled
    }

    /// Apply the animation policy to an egui context.
    ///
    /// egui animates via `Style::animation_time`; setting it to zero freezes
    /// every time-based transition (spinners, panel slides, hover fades).
    pub fn apply(&self, ctx: &Context) {
        if self.should_disable_animations() {
            ctx.all_styles_mut(|style| {
                style.animation_time = 0.0;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_mode_allows_everything() {
        let saver = BandwidthSaver::default();
        assert!(!saver.should_disable_images());
        assert!(!saver.should_disable_animations());
        assert!(!saver.should_disable_non_essential());
    }

    #[test]
    fn enabled_mode_suppresses_images_and_animations() {
        let saver = BandwidthSaver::from_config(true, true, true);
        assert!(saver.should_disable_images());
        assert!(saver.should_disable_animations());
        assert!(saver.should_disable_non_essential());
    }

    #[test]
    fn enabled_without_image_flag_keeps_images() {
        let saver = BandwidthSaver::from_config(true, false, true);
        assert!(!saver.should_disable_images());
        assert!(saver.should_disable_animations());
    }
}
