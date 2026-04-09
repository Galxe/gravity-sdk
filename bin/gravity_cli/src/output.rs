use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable plain text (default)
    #[default]
    Plain,
    /// JSON output for scripting
    Json,
}
