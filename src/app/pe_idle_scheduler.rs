use super::*;

/// Requested by the frame's WM_CREATE path at PE 0x40a2a0..0x40a2ac.
const PE_TIMER_PERIOD: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PeIdleScheduler {
    elapsed: Duration,
    /// DWORD 0x4e63d8; native resets nonzero multiples of eight to zero.
    cycle: u32,
    /// WORD 0x4e63dc; incremented only when the mode-2 pending gate is tested.
    disaster_parity: u16,
    /// App +0x30c is a latch, not a count of calendar days to catch up.
    pending: u32,
    palette_314: u32,
    palette_318: u32,
    modal: PeModalPause,
    continuation: Option<PeIdleContinuation>,
    day_zero: Option<PeDayZeroContinuation>,
}

/// Admitted body of PE 0x4815e0. Rejected/waiting dialog admission belongs to
/// the caller; the existing UI routes here only after accepting a dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PeModalPause {
    depth: u32,
    flag: u32,
    saved_speed: Option<u16>,
}

impl PeModalPause {
    fn enter(&mut self, speed: &mut u16) {
        if self.depth == 0 {
            self.saved_speed = Some(*speed);
            self.flag = 1;
            *speed = 1;
        }
        self.depth = self.depth.wrapping_add(1);
    }

    fn leave(&mut self, speed: &mut u16) {
        assert_ne!(self.depth, 0, "unbalanced PE modal return");
        self.depth = self.depth.wrapping_sub(1);
        if self.depth == 0 {
            self.flag = 0;
            *speed = self.saved_speed.take().expect("outer modal speed snapshot");
        }
    }

    fn active(self) -> bool {
        self.depth != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeIdleContinuation {
    AdmitDisaster,
    TickDisaster { started: bool },
    PublishDisasterEnd,
    Finish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PeDayZeroContext {
    year: u16,
    month: u8,
    day: u8,
    year_rolled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeDayZeroContinuation {
    AfterBudget(PeDayZeroContext),
    AfterAnnual(PeDayZeroContext),
    MonthlyRows(PeDayZeroContext),
    Subscription(PeDayZeroContext),
    Publish(PeDayZeroContext),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeIdleDispatch {
    Calendar,
    Disaster,
}

fn native_speed(speed: GameSpeed) -> u16 {
    match speed {
        GameSpeed::Paused => 1,
        GameSpeed::Turtle => 2,
        GameSpeed::Llama => 3,
        GameSpeed::Cheetah => 4,
        GameSpeed::AfricanSwallow => 5,
    }
}

impl PeIdleScheduler {
    /// Integer/memory branch of PE 0x40c100. Palette callbacks and the separate
    /// countdown are excluded; neither is a calendar update. The native x86
    /// fixture tests these exact exclusions, including DWORD wrap and latching.
    fn timer_pulse(&mut self, speed: u16, city_mode: u16) {
        self.cycle = self.cycle.wrapping_add(1);
        if self.cycle != 0 {
            if self.cycle % 8 == 0 {
                self.cycle = 0;
                self.palette_314 = 1;
            }
            self.palette_318 = 1;
        }
        let admits = match speed {
            1 => self.cycle == 0,
            2 => self.cycle % 4 == 0,
            3 => self.cycle % 2 == 0,
            4 | 5 => true,
            _ => false,
        };
        if admits {
            self.pending = 1;
        }
        if self.pending != 0 && city_mode == 2 && speed != 5 {
            let previous = self.disaster_parity;
            self.disaster_parity = previous.wrapping_add(1);
            if previous & 1 != 0 {
                self.pending = 0;
            }
        }
    }

    fn advance_timers(&mut self, elapsed: Duration, speed: GameSpeed, city_mode: u16) {
        self.elapsed += elapsed;
        // Model queued timer deliveries before an idle opportunity. A delayed
        // render does not turn the one-bit latch into a backlog of city days.
        while self.elapsed >= PE_TIMER_PERIOD {
            self.elapsed -= PE_TIMER_PERIOD;
            self.timer_pulse(native_speed(speed), city_mode);
        }
    }

    fn take_things_opportunity(
        &mut self,
        speed: GameSpeed,
        city_mode: u16,
        input_busy: bool,
    ) -> bool {
        // PE 0x405ab0 consumes +0x318 before the busy/speed gates, then offers
        // one Things-walker check through 0x407490. Keep that one-shot separate
        // from the calendar's +0x30c pending latch.
        let signaled = self.palette_318 != 0;
        self.palette_314 = 0;
        self.palette_318 = 0;
        signaled && speed != GameSpeed::Paused && matches!(city_mode, 1 | 2) && !input_busy
    }

    fn idle_dispatch(
        &mut self,
        speed: GameSpeed,
        city_mode: u16,
        input_busy: bool,
    ) -> Option<PeIdleDispatch> {
        // Direct calendar-only callers still consume any unclaimed palette
        // signals here. This does not reschedule the road renderer.
        self.palette_314 = 0;
        self.palette_318 = 0;
        // PE 0x405b7e tests app +0xf8 after palette consumption. Input loops
        // still dispatch timer messages, so this must not pause timer_pulse,
        // reset its phase, or clear pending on behalf of a held consumer.
        if input_busy || speed == GameSpeed::Paused || self.pending == 0 {
            return None;
        }
        match city_mode {
            1 => Some(PeIdleDispatch::Calendar),
            2 => Some(PeIdleDispatch::Disaster),
            _ => None,
        }
    }

    fn finish_idle(&mut self, speed: GameSpeed) {
        // PE reads speed again AFTER dispatch. A newly started disaster may
        // have reduced speed 5 to 4, so the entry speed cannot decide this.
        if speed != GameSpeed::AfricanSwallow {
            self.pending = 0;
        }
    }
}

impl ViewerState {
    pub(super) fn pe_dialog_owns_input(&self) -> bool {
        self.pe_modal_active()
            || (self.engine_fidelity.uses_win95()
                && self.city_info_dialog == Some(CityInfoDialog::FiscalCrisis))
    }

    pub(super) fn pe_modal_active(&self) -> bool {
        self.engine_fidelity.uses_win95() && self.pe_idle_scheduler.modal.active()
    }

    fn set_pe_modal_runtime_speed(&mut self, raw: u16) {
        let speed = match raw {
            1 => GameSpeed::Paused,
            2 => GameSpeed::Turtle,
            3 => GameSpeed::Llama,
            4 => GameSpeed::Cheetah,
            5 => GameSpeed::AfricanSwallow,
            _ => unreachable!("live modal snapshots come from GameSpeed"),
        };
        // This is app state, not a saved-city speed edit. Preserve traffic
        // density, redraw/animation phase, RNG, and shared speed multipliers.
        self.traffic.set_speed(speed);
        self.menu_checks.speed = speed;
        mac_menu::set_game_speed(speed);
    }

    pub(super) fn sync_pe_city_info_modal_state(&mut self) {
        if !self.engine_fidelity.uses_win95() {
            return;
        }
        let admitted = matches!(
            self.city_info_dialog,
            Some(CityInfoDialog::Budget | CityInfoDialog::Newspaper(_))
        );
        let mut raw = native_speed(self.traffic.speed());
        match (self.pe_idle_scheduler.modal.active(), admitted) {
            (false, true) => {
                self.pe_idle_scheduler.modal.enter(&mut raw);
                self.set_pe_modal_runtime_speed(raw);
            }
            (true, false) => {
                self.pe_idle_scheduler.modal.leave(&mut raw);
                self.set_pe_modal_runtime_speed(raw);
            }
            _ => {}
        }
    }

    pub(super) fn cancel_pe_suspended_city_work(&mut self) {
        // Timer phase/pending are app owned and survive a new document. Stack
        // continuations refer to the old city and must never run in the new one.
        self.pe_idle_scheduler.continuation = None;
        self.pe_idle_scheduler.day_zero = None;
    }

    fn begin_live_pe_day_zero(&mut self) {
        let at = self.game_clock;
        let context = PeDayZeroContext {
            year: at.year(),
            month: at.month(),
            day: at.day(),
            year_rolled: self.city.saved_date().is_some_and(|old| {
                old.month == 12 && at.month() == 1 && at.year() == old.year.saturating_add(1)
            }),
        };
        // Native 0x413520 publishes the new age/month/year before phase 0;
        // retain the previous-date rollover decision across the modal return.
        self.city.set_saved_date_and_day(
            CityDate {
                year: context.year,
                month: context.month,
            },
            context.day,
        );
        self.query_status_pinned = false;
        sc2kfix_pe_recalculate_city_value(&mut self.city);
        self.pe_idle_scheduler.day_zero = Some(PeDayZeroContinuation::AfterBudget(context));
        if context.year_rolled && !auto_budget_enabled_for_engine(&self.city, self.engine_fidelity)
        {
            self.open_january_budget_review();
            return;
        }
        self.resume_live_pe_day_zero();
    }

    fn resume_live_pe_day_zero(&mut self) {
        while !self.pe_dialog_owns_input() {
            let Some(stage) = self.pe_idle_scheduler.day_zero.take() else {
                break;
            };
            match stage {
                PeDayZeroContinuation::AfterBudget(at) => {
                    self.pe_idle_scheduler.day_zero = Some(PeDayZeroContinuation::AfterAnnual(at));
                    if at.year_rolled {
                        self.apply_budget_year_end_settlement();
                        self.clear_budget_year_history();
                        self.sim_calendar_yearly_updates();
                    }
                }
                PeDayZeroContinuation::AfterAnnual(at) => {
                    self.pe_idle_scheduler.day_zero = Some(PeDayZeroContinuation::MonthlyRows(at));
                    if at.year_rolled {
                        self.open_pe_negative_auto_budget_notice();
                    }
                }
                PeDayZeroContinuation::MonthlyRows(at) => {
                    self.pe_idle_scheduler.day_zero = Some(PeDayZeroContinuation::Subscription(at));
                    self.refresh_monthly_budget_rows(population_stats_for_city_with_fidelity(
                        &self.city,
                        self.engine_fidelity,
                    ));
                }
                PeDayZeroContinuation::Subscription(at) => {
                    self.pe_idle_scheduler.day_zero = Some(PeDayZeroContinuation::Publish(at));
                    if matches!(at.month, 4 | 8)
                        && self.city.kind != CityFileKind::MacCity
                        && newspaper_subscription_enabled_for_city(&self.city)
                    {
                        self.open_city_info_dialog(CityInfoDialog::Newspaper(
                            NewspaperEdition::Subscription,
                        ));
                        self.set_sim_status_announcement("Newspaper: subscription delivery");
                    }
                }
                PeDayZeroContinuation::Publish(at) => {
                    self.finish_sim_calendar_day0_publication(at.year, at.month, at.day);
                }
            }
        }
    }

    fn resume_pe_idle_invocation(&mut self) {
        while !self.pe_dialog_owns_input() {
            let Some(stage) = self.pe_idle_scheduler.continuation.take() else {
                break;
            };
            match stage {
                PeIdleContinuation::AdmitDisaster => {
                    let started = self.consume_pe_trigger_disaster();
                    self.pe_idle_scheduler.continuation =
                        Some(PeIdleContinuation::TickDisaster { started });
                }
                PeIdleContinuation::TickDisaster { started } => {
                    self.pe_idle_scheduler.continuation =
                        Some(PeIdleContinuation::PublishDisasterEnd);
                    if started || self.city.has_active_disaster() {
                        self.tick_pe_disaster_clock();
                    }
                }
                PeIdleContinuation::PublishDisasterEnd => {
                    self.pe_idle_scheduler.continuation = Some(PeIdleContinuation::Finish);
                    if !self.city.has_active_disaster()
                        && self.city.pe_w_current_disaster_type() != 0
                    {
                        self.persist_cleared_disaster_misc();
                    }
                }
                PeIdleContinuation::Finish => {
                    self.pe_idle_scheduler.finish_idle(self.traffic.speed());
                }
            }
        }
    }

    pub(super) fn advance_frame_simulation(&mut self, elapsed: Duration) -> GameClockAdvance {
        if !self.engine_fidelity.uses_win95() {
            return self.advance_mac_frame_simulation(elapsed);
        }

        // These are runtime mode/actors. A Mac save's opaque MISC +4 is not a
        // Win95 scheduler selector, and must not choose the simulation branch.
        let mode = pe_city_mode_misc_value(self.mode, self.city.has_active_disaster()) as u16;
        let timer_speed = self.traffic.speed();
        self.pe_idle_scheduler
            .advance_timers(elapsed, timer_speed, mode);
        if self.mode != AppMode::CityView {
            self.cancel_pe_suspended_city_work();
        }
        if self.pe_dialog_owns_input() {
            return GameClockAdvance::default();
        }
        if self.pe_idle_scheduler.day_zero.is_some()
            || self.pe_idle_scheduler.continuation.is_some()
        {
            // Return into the interrupted native invocation before offering a
            // new idle opportunity, even if a modal timer cleared pending.
            self.resume_live_pe_day_zero();
            if !self.pe_dialog_owns_input() {
                self.resume_pe_idle_invocation();
            }
            return GameClockAdvance::default();
        }
        // Native rectangle/network/highway input workers bracket app busy.
        // Their SDL counterparts own input until finish/cancel takes the
        // drag. A view's this+0xf8 and merely selected tools are unrelated.
        let input_busy = self.zone_drag.is_some()
            || self.network_drag.is_some()
            || self.underground_drag.is_some()
            || self.highway_drag.is_some();

        // In the ordinary PE route each 200 ms timer pulse raises +0x318. The
        // idle controller consumes that signal before calendar pending work,
        // and calls the selected XTHG walk only when speed/mode/busy gates
        // admit it. The native 30/45 ms deadline is due by the next ordinary
        // 200 ms pulse; direct command 0x1fd and exact GetTickCount phase are
        // outside this portable adapter.
        if self
            .pe_idle_scheduler
            .take_things_opportunity(timer_speed, mode, input_busy)
        {
            self.tick_pe_disaster_xthg_actors_in_slot_order();
        }
        let speed = self.traffic.speed();
        let mode = pe_city_mode_misc_value(self.mode, self.city.has_active_disaster()) as u16;
        let Some(dispatch) = self
            .pe_idle_scheduler
            .idle_dispatch(speed, mode, input_busy)
        else {
            return GameClockAdvance::default();
        };
        let mut advance = GameClockAdvance::default();
        self.pe_idle_scheduler.continuation = Some(PeIdleContinuation::AdmitDisaster);
        if dispatch == PeIdleDispatch::Calendar {
            let start = self.game_clock;
            self.game_clock = start.wrapping_add_days(1);
            advance.days = 1;
            advance.months = u32::from(start.month() != self.game_clock.month());
            if self.game_clock.day() == 0 {
                self.begin_live_pe_day_zero();
            } else {
                self.apply_game_clock_advance(start, advance);
            }
        }
        // Mode 1 checks queued requests after calendar; mode 2 runs this path
        // without a date step. A newly admitted request gets its first scan in
        // this same wrapper invocation, as in PE 0x45cf10.
        if !self.pe_dialog_owns_input() {
            self.resume_pe_idle_invocation();
        }
        advance
    }

    pub(super) fn reduce_fastest_speed_on_pe_disaster_entry(&mut self) {
        if !self.engine_fidelity.uses_win95()
            || self.mode != AppMode::CityView
            || self.traffic.speed() != GameSpeed::AfricanSwallow
        {
            return;
        }
        // App SetMode(2), PE 0x406a50: reduce 5 to 4 on disaster entry.
        let speed = GameSpeed::Cheetah;
        self.traffic.set_speed(speed);
        self.menu_checks.speed = speed;
        mac_menu::set_game_speed(speed);
        if let Some(value) = speed_misc_value_for_city(&self.city, self.engine_fidelity, speed) {
            persist_speed_misc_value_to_city(&mut self.city, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pe_fiscal_notice_native_gate_and_timer_states() {
        let mut cases = 0;
        let mut admitted = 0;
        for line in include_str!("fixtures/pe_fiscal_notice_native_20261023.csv")
            .lines()
            .skip(1)
        {
            let row: Vec<i64> = line
                .split(',')
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(row.len(), 27);
            let notice = pe_fiscal_crisis_after_settlement(row[0] as i32, row[1] != 0);
            assert_eq!(i64::from(notice), row[16], "notice gate: {line}");
            assert_eq!(if notice { 0 } else { row[1] }, row[9], "flag: {line}");
            let mut scheduler = PeIdleScheduler {
                cycle: row[4] as u32,
                disaster_parity: row[5] as u16,
                pending: row[6] as u32,
                ..Default::default()
            };
            if notice {
                // Native MessageBox keeps entry speed for its delivered timers.
                // No PeModalPause entry, saved speed or paused predicate applies.
                for _ in 0..row[7] {
                    scheduler.timer_pulse(row[2] as u16, row[3] as u16);
                }
            }
            assert_eq!(scheduler.cycle, row[11] as u32, "cycle: {line}");
            assert_eq!(scheduler.disaster_parity, row[12] as u16, "parity: {line}");
            assert_eq!(scheduler.pending, row[13] as u32, "pending: {line}");
            assert_eq!(scheduler.palette_314, row[14] as u32);
            assert_eq!(scheduler.palette_318, row[15] as u32);
            assert!(!scheduler.modal.active());
            assert_eq!(row[22], if notice { row[2] } else { -1 });
            assert_eq!(
                row[10],
                if notice && row[8] >= 0 {
                    row[8]
                } else {
                    row[2]
                },
                "the native message body does not restore a speed snapshot"
            );
            assert_eq!((row[23], row[24]), (0, 0));
            cases += 1;
            admitted += usize::from(notice);
        }
        assert_eq!((cases, admitted), (6918, 2306));
    }

    #[derive(Default)]
    struct ModalEvents {
        callbacks: u32,
        paused: u32,
        flagged: u32,
        enters: u32,
        exits: u32,
        refreshes: u32,
    }

    fn fixture_modal_callback(
        scheduler: &mut PeIdleScheduler,
        speed: &mut u16,
        mode: u16,
        depth: u32,
        levels: u32,
        pulses: u32,
        mutation: i32,
        events: &mut ModalEvents,
    ) {
        let outer = !scheduler.modal.active();
        scheduler.modal.enter(speed);
        if outer {
            events.enters += 1;
            events.refreshes += 1;
        }
        events.callbacks += 1;
        events.paused += u32::from(*speed == 1);
        events.flagged += u32::from(scheduler.modal.flag == 1);
        for _ in 0..pulses {
            scheduler.timer_pulse(*speed, mode);
        }
        if depth < levels {
            fixture_modal_callback(
                scheduler,
                speed,
                mode,
                depth + 1,
                levels,
                pulses,
                mutation,
                events,
            );
        }
        if mutation >= 0 {
            *speed = mutation as u16;
        }
        scheduler.modal.leave(speed);
        if outer {
            events.exits += 1;
            events.refreshes += 1;
        }
    }

    #[test]
    fn pe_modal_pause_and_pulses_match_all_original_x86_cases() {
        let mut count = 0;
        for line in include_str!("fixtures/pe_modal_native_20261023.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let number = |i: usize| fields[i].parse::<u32>().unwrap();
            let mut speed = number(0) as u16;
            let mut scheduler = PeIdleScheduler {
                cycle: number(2),
                disaster_parity: number(3) as u16,
                pending: number(4),
                ..Default::default()
            };
            let mut events = ModalEvents::default();
            // The fixture's immediate-rejection rows never enter the admitted
            // modal state. The selector-1 native message wait is excluded.
            let rejected = number(8) != 0 && number(9) != 1;
            if !rejected {
                fixture_modal_callback(
                    &mut scheduler,
                    &mut speed,
                    number(1) as u16,
                    1,
                    number(5),
                    number(6),
                    fields[7].parse::<i32>().unwrap(),
                    &mut events,
                );
            }
            assert_eq!(
                (
                    scheduler.cycle,
                    u32::from(scheduler.disaster_parity),
                    scheduler.pending,
                    u32::from(speed),
                    scheduler.modal.depth,
                    scheduler.modal.flag
                ),
                (
                    number(10),
                    number(11),
                    number(12),
                    number(13),
                    number(14),
                    number(15)
                ),
                "native modal state: {line}"
            );
            assert_eq!(
                (
                    events.callbacks,
                    events.paused,
                    events.flagged,
                    events.enters,
                    events.exits,
                    events.refreshes
                ),
                (
                    number(16),
                    number(17),
                    number(18),
                    number(20),
                    number(21),
                    number(22)
                ),
                "native callback/outer-return sequence: {line}"
            );
            assert_eq!(number(19), if rejected { 0 } else { 0x1234 });
            assert_eq!(scheduler.modal.saved_speed, None);
            count += 1;
        }
        assert_eq!(count, 2320);
    }

    #[test]
    fn pe_idle_pulse_matches_all_original_x86_cases() {
        let mut scheduler = PeIdleScheduler::default();
        let mut count = 0;
        for line in include_str!("fixtures/pe_pulse_native_20261023.csv")
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split(',').collect();
            let number = |i: usize| fields[i].parse::<u32>().unwrap();
            if number(6) == 1 {
                scheduler = PeIdleScheduler {
                    cycle: number(2),
                    disaster_parity: number(3) as u16,
                    pending: number(4),
                    ..Default::default()
                };
            }
            assert_eq!(
                (
                    scheduler.cycle,
                    u32::from(scheduler.disaster_parity),
                    scheduler.pending
                ),
                (number(7), number(8), number(9)),
                "before: {line}"
            );
            scheduler.timer_pulse(number(0) as u16, number(1) as u16);
            assert_eq!(
                (
                    scheduler.cycle,
                    u32::from(scheduler.disaster_parity),
                    scheduler.pending,
                    scheduler.palette_314,
                    scheduler.palette_318
                ),
                (number(10), number(11), number(12), number(13), number(14)),
                "after: {line}"
            );
            if fields[5] == "true" {
                scheduler.pending = 0;
            }
            scheduler.palette_314 = 0;
            scheduler.palette_318 = 0;
            count += 1;
        }
        assert_eq!(count, 8960);
    }

    #[test]
    fn pe_idle_pending_latches_until_consumer_and_rechecks_exit_speed() {
        let mut scheduler = PeIdleScheduler::default();
        scheduler.advance_timers(PE_TIMER_PERIOD, GameSpeed::AfricanSwallow, 1);
        for _ in 0..3 {
            assert_eq!(
                scheduler.idle_dispatch(GameSpeed::AfricanSwallow, 1, false),
                Some(PeIdleDispatch::Calendar)
            );
            scheduler.finish_idle(GameSpeed::AfricanSwallow);
        }
        // A calendar callback started a disaster and changed speed to 4.
        scheduler.finish_idle(GameSpeed::Cheetah);
        assert_eq!(scheduler.idle_dispatch(GameSpeed::Cheetah, 2, false), None);
    }

    #[test]
    fn pe_things_opportunity_is_a_separate_one_shot_from_calendar_pending() {
        let mut scheduler = PeIdleScheduler::default();
        scheduler.advance_timers(Duration::from_millis(400), GameSpeed::Turtle, 1);
        assert_eq!(scheduler.pending, 0);
        assert!(scheduler.take_things_opportunity(GameSpeed::Turtle, 1, false));
        assert!(!scheduler.take_things_opportunity(GameSpeed::Turtle, 1, false));

        scheduler.advance_timers(PE_TIMER_PERIOD, GameSpeed::Turtle, 1);
        assert!(!scheduler.take_things_opportunity(GameSpeed::Turtle, 1, true));
        assert!(!scheduler.take_things_opportunity(GameSpeed::Turtle, 1, false));

        scheduler.advance_timers(PE_TIMER_PERIOD, GameSpeed::Paused, 1);
        assert!(!scheduler.take_things_opportunity(GameSpeed::Paused, 1, false));
        assert!(!scheduler.take_things_opportunity(GameSpeed::Turtle, 1, false));
    }

    #[test]
    fn pe_idle_delayed_frames_coalesce_calendar_work_and_preserve_timer_phase() {
        let mut scheduler = PeIdleScheduler::default();
        scheduler.advance_timers(Duration::from_millis(2050), GameSpeed::Llama, 1);
        assert_eq!(
            scheduler.idle_dispatch(GameSpeed::Llama, 1, false),
            Some(PeIdleDispatch::Calendar)
        );
        scheduler.finish_idle(GameSpeed::Llama);
        assert_eq!(scheduler.idle_dispatch(GameSpeed::Llama, 1, false), None);
        scheduler.advance_timers(Duration::from_millis(150), GameSpeed::Llama, 1);
        assert_eq!(scheduler.cycle, 3); // 11th pulse, after the modulo-eight reset.
        assert_eq!(scheduler.idle_dispatch(GameSpeed::Llama, 1, false), None);
        scheduler.advance_timers(PE_TIMER_PERIOD, GameSpeed::Llama, 1);
        assert_eq!(
            scheduler.idle_dispatch(GameSpeed::Llama, 1, false),
            Some(PeIdleDispatch::Calendar)
        );
    }

    #[test]
    fn pe_idle_busy_gate_follows_palette_consumption_and_retains_pending() {
        let mut scheduler = PeIdleScheduler::default();
        scheduler.advance_timers(Duration::from_millis(1600), GameSpeed::Cheetah, 1);
        assert_eq!(scheduler.palette_314, 1);
        assert_eq!(scheduler.palette_318, 1);
        assert_eq!(scheduler.idle_dispatch(GameSpeed::Cheetah, 1, true), None);
        assert_eq!(scheduler.palette_314, 0);
        assert_eq!(scheduler.palette_318, 0);
        assert_eq!(scheduler.pending, 1);
        assert_eq!(
            scheduler.idle_dispatch(GameSpeed::Cheetah, 1, false),
            Some(PeIdleDispatch::Calendar)
        );
    }

    #[test]
    fn pe_mode2_timer_parity_can_clear_pending_while_input_is_held() {
        let mut scheduler = PeIdleScheduler::default();

        // Placement input holds the idle consumer, but delivered WM_TIMER
        // messages still run the native pulse leaf. In mode 2, two admitted
        // pulses increment parity twice and the second clears the one-bit
        // pending latch before idle_dispatch observes the busy gate.
        scheduler.advance_timers(Duration::from_millis(400), GameSpeed::Cheetah, 2);

        assert_eq!(scheduler.cycle, 2);
        assert_eq!(scheduler.disaster_parity, 2);
        assert_eq!(scheduler.pending, 0);
        assert_eq!(scheduler.idle_dispatch(GameSpeed::Cheetah, 2, true), None);
        assert_eq!(scheduler.pending, 0);
    }
}
