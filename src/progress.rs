use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle, style::TemplateError};

pub fn get_progress_bar(len: u64) -> Result<ProgressBar, TemplateError> {
    let pb = ProgressBar::new(len);
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}")?
            .progress_chars("#>-"),
    );
    Ok(pb)
}
