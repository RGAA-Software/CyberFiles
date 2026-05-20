pub mod config;

pub const APP_NAME: &str = "CyberFiles";

pub const WINDOW_WIDTH: f32 = 1366.;
pub const WINDOW_HEIGHT: f32 = 768.;

pub use config::{AppConfig, load_config, save_config, window_size};
