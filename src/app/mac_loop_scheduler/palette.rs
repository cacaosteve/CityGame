//! Timing and buffer selection for Mac CODE 6 exports 129 and 130.
//!
//! The original advances two palette buffers independently. Rendering their
//! color tables and accounting for Toolbox calls that reuse the process stack
//! are separate work; this state alone does not determine route-search costs.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct MacPaletteTrack {
    pub(in crate::app) deadline: u32,
    /// True after an odd number of native current/alternate swaps.
    pub(in crate::app) alternate: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct MacPaletteSchedule {
    pub(in crate::app) fast: MacPaletteTrack,
    pub(in crate::app) slow: MacPaletteTrack,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct MacPaletteUpdate {
    pub(in crate::app) fast_swapped: bool,
    pub(in crate::app) slow_swapped: bool,
    pub(in crate::app) fast_animated: bool,
    pub(in crate::app) slow_animated: bool,
}

impl MacPaletteSchedule {
    /// CODE 6 export 129 samples the slow deadline before the fast deadline.
    pub(in crate::app) fn initialize(mut tick: impl FnMut() -> u32) -> Self {
        let slow = tick();
        let fast = tick();
        Self {
            fast: MacPaletteTrack {
                deadline: fast,
                alternate: false,
            },
            slow: MacPaletteTrack {
                deadline: slow,
                alternate: false,
            },
        }
    }

    /// CODE 6 export 130 reads TickCount separately for each check and again
    /// for each due track. No front window suppresses only AnimatePalette;
    /// the deadline and the buffer swap still occur.
    pub(in crate::app) fn update(
        &mut self,
        speed: i16,
        mut tick: impl FnMut() -> u32,
        mut front_window: impl FnMut() -> bool,
    ) -> MacPaletteUpdate {
        if speed <= 1 {
            return MacPaletteUpdate::default();
        }
        let mut result = MacPaletteUpdate::default();
        if tick() >= self.fast.deadline {
            self.fast.deadline = tick().wrapping_add(12);
            self.fast.alternate = !self.fast.alternate;
            result.fast_swapped = true;
            result.fast_animated = front_window();
        }
        if tick() >= self.slow.deadline {
            self.slow.deadline = tick().wrapping_add(90);
            self.slow.alternate = !self.slow.alternate;
            result.slow_swapped = true;
            result.slow_animated = front_window();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_palette_deadline_boundaries_and_front_window() {
        let schedule = MacPaletteSchedule::initialize({
            let mut ticks = [200, 100].into_iter();
            move || ticks.next().unwrap()
        });
        assert_eq!((schedule.fast.deadline, schedule.slow.deadline), (100, 200));
        for (speed, ticks, front, deadlines, swaps, animations) in [
            (1, vec![], true, (100, 200), (false, false), (false, false)),
            (
                2,
                vec![99, 199],
                true,
                (100, 200),
                (false, false),
                (false, false),
            ),
            (
                2,
                vec![100, 103, 200, 205],
                true,
                (115, 295),
                (true, true),
                (true, true),
            ),
            (
                2,
                vec![100, 103, 200, 205],
                false,
                (115, 295),
                (true, true),
                (false, false),
            ),
        ] {
            let mut current = schedule;
            let mut ticks = ticks.into_iter();
            let result = current.update(speed, || ticks.next().unwrap(), || front);
            assert_eq!(ticks.next(), None);
            assert_eq!((current.fast.deadline, current.slow.deadline), deadlines);
            assert_eq!((result.fast_swapped, result.slow_swapped), swaps);
            assert_eq!((result.fast_animated, result.slow_animated), animations);
            assert_eq!((current.fast.alternate, current.slow.alternate), swaps);
        }
    }

    #[test]
    fn native_palette_uses_unsigned_check_and_wrapping_refresh() {
        let mut schedule = MacPaletteSchedule::initialize({
            let mut ticks = [200, 0xfffffff0].into_iter();
            move || ticks.next().unwrap()
        });
        let mut ticks = [2, 199].into_iter();
        assert_eq!(
            schedule.update(2, || ticks.next().unwrap(), || true),
            MacPaletteUpdate::default()
        );
        let mut ticks = [0xfffffff0, 0xfffffff8, 199].into_iter();
        let result = schedule.update(2, || ticks.next().unwrap(), || true);
        assert_eq!(schedule.fast.deadline, 4);
        assert!(result.fast_swapped && result.fast_animated && !result.slow_swapped);
    }
}
