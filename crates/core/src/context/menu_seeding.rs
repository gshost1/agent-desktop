use super::CommandContext;
use crate::MenuSeed;

impl CommandContext {
    /// Seeds a closed-direction menu wait with a reading taken before the
    /// action it follows; `batch` sets it the way it sets the event baseline.
    pub fn with_menu_seed(mut self, seed: Option<MenuSeed>) -> Self {
        self.options.menu_seed = seed;
        self
    }

    pub fn menu_seed(&self) -> Option<&MenuSeed> {
        self.options.menu_seed.as_ref()
    }
}
