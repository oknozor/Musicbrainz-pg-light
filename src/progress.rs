//! Progress reporting utilities with optional indicatif integration
//!
//! When the "progress" feature is enabled, this provides full progress bar functionality.
//! When disabled, all operations become no-ops for minimal overhead.

use indicatif::style::TemplateError;

/// Wrapper around indicatif::ProgressBar that becomes a no-op when progress feature is disabled
#[derive(Debug, Clone)]
pub struct ProgressBar {
    #[cfg(feature = "progress")]
    inner: indicatif::ProgressBar,
}

impl ProgressBar {
    /// Create a new progress bar with the specified length
    #[cfg(feature = "progress")]
    pub fn new(len: u64) -> Self {
        Self {
            inner: indicatif::ProgressBar::new(len),
        }
    }

    #[cfg(not(feature = "progress"))]
    pub fn new(_len: u64) -> Self {
        Self {}
    }

    /// Create a hidden progress bar
    #[cfg(feature = "progress")]
    pub fn hidden() -> Self {
        Self {
            inner: indicatif::ProgressBar::hidden(),
        }
    }

    #[cfg(not(feature = "progress"))]
    pub fn hidden() -> Self {
        Self {}
    }

    /// Set the progress bar style
    #[cfg(feature = "progress")]
    pub fn set_style(&self, style: indicatif::ProgressStyle) -> &Self {
        self.inner.set_style(style);
        self
    }

    #[cfg(not(feature = "progress"))]
    pub fn set_style(&self, _style: ProgressStyle) -> &Self {
        self
    }

    /// Set a message for the progress bar
    #[cfg(feature = "progress")]
    pub fn set_message<S: Into<std::borrow::Cow<'static, str>>>(&self, msg: S) {
        self.inner.set_message(msg);
    }

    #[cfg(not(feature = "progress"))]
    pub fn set_message<S: Into<std::borrow::Cow<'static, str>>>(&self, _msg: S) {}

    /// Increment the progress bar by the specified amount
    #[cfg(feature = "progress")]
    pub fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    #[cfg(not(feature = "progress"))]
    pub fn inc(&self, _delta: u64) {}

    /// Set the position of the progress bar
    #[cfg(feature = "progress")]
    pub fn set_position(&self, pos: u64) {
        self.inner.set_position(pos);
    }

    #[cfg(not(feature = "progress"))]
    pub fn set_position(&self, _pos: u64) {}

    /// Finish the progress bar with a message
    #[cfg(feature = "progress")]
    pub fn finish_with_message<S: Into<std::borrow::Cow<'static, str>>>(&self, msg: S) {
        self.inner.finish_with_message(msg);
    }

    #[cfg(not(feature = "progress"))]
    pub fn finish_with_message<S: Into<std::borrow::Cow<'static, str>>>(&self, _msg: S) {}

    /// Finish the progress bar
    #[cfg(feature = "progress")]
    pub fn finish(&self) {
        self.inner.finish();
    }

    #[cfg(not(feature = "progress"))]
    pub fn finish(&self) {}

    /// Finish and clear the progress bar
    #[cfg(feature = "progress")]
    pub fn finish_and_clear(&self) {
        self.inner.finish_and_clear();
    }

    #[cfg(not(feature = "progress"))]
    pub fn finish_and_clear(&self) {}

    /// Get the underlying indicatif ProgressBar (only available with progress feature)
    #[cfg(feature = "progress")]
    pub fn inner(&self) -> &indicatif::ProgressBar {
        &self.inner
    }
}

/// Wrapper around indicatif::MultiProgress that becomes a no-op when progress feature is disabled
#[derive(Debug, Clone)]
pub struct MultiProgress {
    #[cfg(feature = "progress")]
    inner: indicatif::MultiProgress,
}

impl MultiProgress {
    /// Create a new multi-progress container
    #[cfg(feature = "progress")]
    pub fn new() -> Self {
        Self {
            inner: indicatif::MultiProgress::new(),
        }
    }

    #[cfg(not(feature = "progress"))]
    pub fn new() -> Self {
        Self {}
    }

    /// Add a progress bar to the multi-progress display
    #[cfg(feature = "progress")]
    pub fn add(&self, pb: ProgressBar) -> ProgressBar {
        ProgressBar {
            inner: self.inner.add(pb.inner),
        }
    }

    #[cfg(not(feature = "progress"))]
    pub fn add(&self, pb: ProgressBar) -> ProgressBar {
        pb
    }

    /// Clear the multi-progress display
    #[cfg(feature = "progress")]
    pub fn clear(&self) -> Result<(), std::io::Error> {
        self.inner.clear()
    }

    #[cfg(not(feature = "progress"))]
    pub fn clear(&self) -> Result<(), std::io::Error> {
        Ok(())
    }

    /// Get the underlying indicatif MultiProgress (only available with progress feature)
    #[cfg(feature = "progress")]
    pub fn inner(&self) -> &indicatif::MultiProgress {
        &self.inner
    }
}

impl Default for MultiProgress {
    fn default() -> Self {
        Self::new()
    }
}

/// Progress style wrapper
#[cfg(feature = "progress")]
pub use indicatif::ProgressStyle;

#[cfg(not(feature = "progress"))]
#[derive(Debug, Clone)]
pub struct ProgressStyle;

#[cfg(not(feature = "progress"))]
impl ProgressStyle {
    pub fn default_bar() -> Self {
        Self
    }

    pub fn template(self, _template: &str) -> Result<Self, TemplateError> {
        Ok(self)
    }

    pub fn progress_chars(self, _chars: &str) -> Self {
        self
    }
}

/// Create a progress bar with the default style
pub fn get_progress_bar(len: u64) -> Result<ProgressBar, TemplateError> {
    let pb = ProgressBar::new(len);

    #[cfg(feature = "progress")]
    {
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}")?
                .progress_chars("#>-"),
        );
    }

    Ok(pb)
}
