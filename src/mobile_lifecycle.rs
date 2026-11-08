//! Pause/resume when the OS backgrounds the app (Android / iPadOS).
use crate::traffic::GameSpeed;

#[derive(Debug, Default)]
pub struct AppLifecycle {
    suspended: bool,
    /// Speed to restore after foreground (None → leave current).
    resume_speed: Option<GameSpeed>,
}

impl AppLifecycle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended
    }

    /// Enter background: remember speed and request pause.
    pub fn enter_background(&mut self, current_speed: GameSpeed) -> GameSpeed {
        if !self.suspended {
            self.resume_speed = Some(current_speed);
            self.suspended = true;
            log::info!("app entered background; pausing simulation");
        }
        GameSpeed::Paused
    }

    /// Leave background: restore prior speed (or Llama if none).
    pub fn enter_foreground(&mut self) -> Option<GameSpeed> {
        if !self.suspended {
            return None;
        }
        self.suspended = false;
        let speed = self.resume_speed.take().unwrap_or(GameSpeed::Llama);
        log::info!("app entered foreground; restoring speed {speed:?}");
        Some(speed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_pauses_and_foreground_restores() {
        let mut life = AppLifecycle::new();
        assert_eq!(life.enter_background(GameSpeed::Cheetah), GameSpeed::Paused);
        assert!(life.is_suspended());
        assert_eq!(life.enter_foreground(), Some(GameSpeed::Cheetah));
        assert!(!life.is_suspended());
    }
}
