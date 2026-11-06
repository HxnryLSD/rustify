/// Extreme Bandwidth Saver Mode policy.
///
/// When enabled, most image rendering is disabled, all animations are frozen,
/// and non-essential functions are turned off to minimize bandwidth usage.
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
    pub fn from_config(enabled: bool, disable_images: bool, disable_animations: bool) -> Self {
        Self {
            enabled,
            disable_images,
            disable_animations,
        }
    }

    /// Whether image rendering should be suppressed.
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
}
