//! Mac CODE 8 cohort arithmetic. The complete monthly transition is not yet
//! scheduled; helpers here retain the native ordering and random draw gates.

/// Inputs already computed by the original export 238 before its age walk.
#[allow(dead_code)]
pub(super) struct MacCohortAgeInputs {
    pub school: u32,
    pub college: u32,
    pub life_penalty: u32,
    pub ordinance_bit: bool,
}

/// Raw native inputs used by the cohort setup immediately before mortality.
#[allow(dead_code)]
pub(super) struct MacCohortSetup {
    pub school_tiles: u16,
    pub college_tiles: u16,
    pub hospital_tiles: u16,
    pub school_funding: u32,
    pub college_funding: u32,
    pub hospital_funding: u32,
    pub health_budget: u32,
    pub flags_a: u8,
    pub flags_b: u8,
    pub pollution: u32,
    pub scalar: u32,
    pub class_two: u32,
}

#[allow(dead_code)]
pub(super) struct MacCohortPreparedInputs {
    pub age: MacCohortAgeInputs,
    pub health: u32,
    pub life_expectancy: u32,
}

#[allow(dead_code)]
pub(super) struct MacCohortSummary {
    pub auxiliary_average: u32,
    pub life_average: u32,
    pub workforce_ratio: u32,
}

/// Inputs already computed by CODE 8 before its fertile-birth pass.
#[allow(dead_code)]
pub(super) struct MacCohortBirthInputs {
    pub pollution_penalty: u32,
    pub health_cap: u32,
    pub life_weight: u32,
    pub auxiliary_rate: u32,
}

/// Reproduce CODE 8 setup at `0x39e1a..0x39efc`, and its pollution
/// adjustment at `0x39fde..0x3a010`. A zero denominator faults in the
/// original; return None so callers can handle that exceptional state.
#[allow(dead_code)]
pub(super) fn mac_cohort_prepare_inputs(raw: &MacCohortSetup) -> Option<MacCohortPreparedInputs> {
    let scaled = |base: u32, funding: u32| (base.wrapping_mul(funding) as i32 / 100) as u32;
    let school = scaled(u32::from(raw.school_tiles / 9) * 15, raw.school_funding);
    let college = scaled(u32::from(raw.college_tiles >> 4) * 50, raw.college_funding);
    let mut health = scaled(u32::from(raw.hospital_tiles / 9) * 25, raw.hospital_funding);
    if raw.flags_b & 0x40 != 0 {
        health = health.wrapping_add((raw.health_budget as i32 / 400) as u32);
    }
    let life_expectancy = 85
        + u32::from(raw.flags_a & 2 != 0) * 5
        + u32::from(raw.flags_a & 4 != 0) * 5
        + u32::from(raw.flags_b & 0x20 != 0) * 5;
    let denominator = raw
        .scalar
        .wrapping_add(raw.class_two.wrapping_mul(10))
        .wrapping_add(1);
    let quotient = raw.pollution.checked_div(denominator)?;
    let life_penalty = if (quotient as i32) > 3 { 3 } else { quotient };
    Some(MacCohortPreparedInputs {
        age: MacCohortAgeInputs {
            school,
            college,
            life_penalty,
            ordinance_bit: raw.flags_a & 1 != 0,
        },
        health,
        life_expectancy,
    })
}

/// CODE 8 `0x3a010..0x3a104`: visit source indices 18 down to 0.
#[allow(dead_code)]
pub(super) fn mac_cohort_age_month(
    counts: &mut [u32; 20],
    auxiliary: &mut [u32; 20],
    life: &mut [u32; 20],
    inputs: &MacCohortAgeInputs,
    mut random: impl FnMut() -> u16,
) {
    for dest in (1..20).rev() {
        let source = dest - 1;
        let count = counts[source];
        if count == 0 {
            continue;
        }
        let mut moved = count / 60;
        if u32::from(random()) % 60 < count % 60 {
            moved = moved.wrapping_add(1);
        }
        moved = moved.min(count);
        if moved == 0 {
            continue;
        }
        let aux_share = auxiliary[source].wrapping_mul(moved) / count;
        auxiliary[source] = auxiliary[source].wrapping_sub(aux_share);
        let mut aux_added = aux_share;
        if dest < 3 {
            aux_added = aux_added.wrapping_add(35_u32.wrapping_mul(moved.min(inputs.school)));
        } else if dest == 3 {
            let college_share = aux_share.wrapping_mul(moved.min(inputs.college)) / moved;
            aux_added = aux_added.wrapping_add(college_share >> 1);
        }
        if !inputs.ordinance_bit {
            aux_added = aux_added.wrapping_sub(moved);
        }
        auxiliary[dest] = auxiliary[dest].wrapping_add(aux_added);
        let life_share = life[source].wrapping_mul(moved) / count;
        life[source] = life[source].wrapping_sub(life_share);
        life[dest] = life[dest].wrapping_add(life_share.wrapping_sub(inputs.life_penalty));
        counts[source] = counts[source].wrapping_sub(moved);
        counts[dest] = counts[dest].wrapping_add(moved);
    }
}

/// Apply the original CODE 8 fertile-cohort pass at `0x3a104..0x3a1b2`.
/// This component is not scheduled in the live monthly transition yet.
#[allow(dead_code)]
pub(super) fn mac_cohort_births(
    counts: &mut [u32; 20],
    auxiliary: &mut [u32; 20],
    life: &mut [u32; 20],
    positive_balance: &mut u32,
    inputs: &MacCohortBirthInputs,
    mut random: impl FnMut() -> u16,
) -> u32 {
    let fertile_sum = counts[4..=8]
        .iter()
        .fold(0_u32, |sum, count| sum.wrapping_add(*count));
    let mut births = ((fertile_sum as i32) / 300) as u32;
    let draw = u32::from(random()) % 300;
    let fractional_threshold = fertile_sum.wrapping_sub(inputs.pollution_penalty.wrapping_mul(300));
    if (fractional_threshold as i32) > draw as i32 {
        births = births.wrapping_add(1);
    }

    if births != 0 {
        let capped = births.min(inputs.health_cap);
        life[0] = life[0]
            .wrapping_add(inputs.life_weight.wrapping_mul(capped))
            .wrapping_add(35_u32.wrapping_mul(births.wrapping_sub(capped)));
        auxiliary[0] = auxiliary[0].wrapping_add(births.wrapping_mul(inputs.auxiliary_rate) / 5);
        counts[0] = counts[0].wrapping_add(births);
        *positive_balance = positive_balance.wrapping_add(births);
    }
    births
}

/// Refill a negative phase-21 population balance with the native age weights.
/// This covers only CODE 8's deficit branch; the complete worker is unscheduled.
#[allow(dead_code)]
pub(super) fn mac_cohort_replenish_deficit(
    counts: &mut [u32; 20],
    auxiliary: &mut [u32; 20],
    life: &mut [u32; 20],
    negative_balance: &mut u32,
    positive_balance: u32,
) {
    if *negative_balance <= positive_balance {
        return;
    }
    *negative_balance = negative_balance.wrapping_sub(positive_balance);
    let per_cohort = (*negative_balance >> 4).wrapping_add(1);
    while *negative_balance != 0 {
        for index in 4..8 {
            let amount = per_cohort.min(*negative_balance);
            life[index] = life[index].wrapping_add(amount.wrapping_mul(65 - index as u32));
            auxiliary[index] =
                auxiliary[index].wrapping_add(amount.wrapping_mul(90 - index as u32));
            counts[index] = counts[index].wrapping_add(amount);
            *negative_balance = negative_balance.wrapping_sub(amount);
        }
        for index in 0..12 {
            if *negative_balance == 0 {
                break;
            }
            let amount = per_cohort.min(*negative_balance);
            let auxiliary_weight = if index < 3 {
                17_u32.wrapping_add((index as u32).wrapping_mul(35))
            } else {
                90 - index as u32
            };
            life[index] = life[index].wrapping_add(amount.wrapping_mul(65 - index as u32));
            auxiliary[index] = auxiliary[index].wrapping_add(amount.wrapping_mul(auxiliary_weight));
            counts[index] = counts[index].wrapping_add(amount);
            *negative_balance = negative_balance.wrapping_sub(amount);
        }
    }
}

/// Publish CODE 8's post-transition cohort averages and workforce ratio.
/// The native loop sums buckets 4 through 10 with 32-bit unsigned arithmetic.
#[allow(dead_code)]
pub(super) fn mac_cohort_summary(
    counts: &[u32; 20],
    auxiliary: &[u32; 20],
    life: &[u32; 20],
    scalar: u32,
) -> Option<MacCohortSummary> {
    let (count_sum, auxiliary_sum, life_sum) = (4..=10).fold((0_u32, 0_u32, 0_u32), |sums, i| {
        (
            sums.0.wrapping_add(counts[i]),
            sums.1.wrapping_add(auxiliary[i]),
            sums.2.wrapping_add(life[i]),
        )
    });
    if count_sum == 0 {
        return Some(MacCohortSummary {
            auxiliary_average: 0,
            life_average: 0,
            workforce_ratio: 0,
        });
    }
    let denominator = scalar.wrapping_add(1);
    Some(MacCohortSummary {
        auxiliary_average: auxiliary_sum / count_sum,
        life_average: life_sum / count_sum,
        workforce_ratio: count_sum.wrapping_mul(100).checked_div(denominator)?,
    })
}

/// Apply CODE 8's positive-balance cohort emigration using the recovered
/// integer result of its SANE Comp/extended proportional calculation. The
/// caller supplies `positive_balance` after first subtracting the negative
/// balance at CODE 8 `0x3a2ba..0x3a2c6`; that native branch leaves the negative
/// balance stored. This component is not scheduled in the live monthly
/// transition.
#[allow(dead_code)]
pub(super) fn mac_cohort_emigrate_surplus(
    counts: &mut [u32; 20],
    auxiliary: &mut [u32; 20],
    life: &mut [u32; 20],
    scalar: u32,
    positive_balance: &mut u32,
    mut random: impl FnMut() -> u16,
) -> Option<()> {
    while *positive_balance != 0 {
        // CODE 8 snapshots the pass amount in D5, but reads fffff022 again
        // inside each slot's denominator. Earlier removals therefore change
        // the denominator without changing this pass's numerator.
        let pass_surplus = *positive_balance;
        for index in 0..20 {
            if *positive_balance == 0 {
                break;
            }
            let count = counts[index];
            if count == 0 {
                continue;
            }
            let denominator = positive_balance.wrapping_add(scalar);
            if denominator == 0 {
                return None;
            }
            let mut take =
                ((u64::from(count) * u64::from(pass_surplus)) / u64::from(denominator)) as u32;
            if take == 0 && random() & 3 == 0 {
                take = 1;
            }
            take = take.min(count).min(*positive_balance);
            if take == 0 {
                continue;
            }
            let aux_lost =
                ((u64::from(auxiliary[index]) * u64::from(take)) / u64::from(count)) as u32;
            let life_lost = ((u64::from(life[index]) * u64::from(take)) / u64::from(count)) as u32;
            auxiliary[index] = auxiliary[index].wrapping_sub(aux_lost);
            life[index] = life[index].wrapping_sub(life_lost);
            counts[index] = counts[index].wrapping_sub(take);
            *positive_balance = positive_balance.wrapping_sub(take);
        }
        if counts.iter().all(|count| *count == 0) && *positive_balance != 0 {
            // The original scan relies on a valid city having enough population
            // to consume the positive balance; report malformed/inconsistent input.
            return None;
        }
    }
    Some(())
}

/// Apply one mortality slot from `export_238 @ 0x39f02..0x39fd4`.
/// The incoming and outgoing balance is Mac `fffff01e`.
#[allow(dead_code)]
pub(super) fn mac_cohort_mortality_slot(
    index: usize,
    count: &mut u32,
    auxiliary: &mut u32,
    life: &mut u32,
    balance: &mut u32,
    mut random: impl FnMut() -> u16,
) {
    assert!((1..20).contains(&index));
    let n = *count;
    if n == 0 {
        return;
    }
    let mean = *life / n;
    let score = (mean.wrapping_mul(100) as i32) / (5 * index as i32);
    if score >= 100 {
        return;
    }
    let estimate = n.wrapping_mul(100_u32.wrapping_sub(score as u32)) / 24;
    let mut deaths = ((estimate as i32) / 100) as u32;
    let roll = u32::from(random()) % 100;
    if (estimate.wrapping_sub(deaths.wrapping_mul(100)) as i32) > roll as i32 {
        deaths = deaths.wrapping_add(1);
    }
    deaths = deaths.min(n);
    if deaths == 0 {
        return;
    }
    let aux_lost = auxiliary.wrapping_mul(deaths) / n;
    *auxiliary = auxiliary.wrapping_sub(aux_lost.min(*auxiliary));
    *life = life.wrapping_sub(life.wrapping_mul(deaths) / n);
    *count = count.wrapping_sub(deaths);
    *balance = balance.wrapping_add(deaths);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cohort_setup_matches_original_68020() {
        let data = include_str!("../../tests/fixtures/simulation/mac/mac_cohort_inputs.csv");
        for line in data.lines().skip(1) {
            let v: Vec<u32> = line.split(',').map(|x| x.parse().unwrap()).collect();
            let raw = MacCohortSetup {
                school_tiles: v[0] as u16,
                college_tiles: v[1] as u16,
                hospital_tiles: v[2] as u16,
                school_funding: v[3],
                college_funding: v[4],
                hospital_funding: v[5],
                health_budget: v[6],
                flags_a: v[7] as u8,
                flags_b: v[8] as u8,
                pollution: v[9],
                scalar: v[10],
                class_two: v[11],
            };
            let result = mac_cohort_prepare_inputs(&raw).unwrap();
            assert_eq!(
                [
                    result.age.school,
                    result.age.college,
                    result.health,
                    result.life_expectancy,
                    result.age.life_penalty
                ],
                [v[12], v[13], v[14], v[15], v[16]],
                "{line}"
            );
            assert_eq!(result.age.ordinance_bit, raw.flags_a & 1 != 0);
        }
    }

    #[test]
    fn age_walk_does_not_reage_a_transfer_in_the_same_month() {
        let (mut counts, mut aux, mut life) = ([0; 20], [0; 20], [0; 20]);
        counts[0] = 60;
        counts[1] = 59;
        let inputs = MacCohortAgeInputs {
            school: 0,
            college: 0,
            life_penalty: 0,
            ordinance_bit: true,
        };
        let mut calls = 0;
        mac_cohort_age_month(&mut counts, &mut aux, &mut life, &inputs, || {
            calls += 1;
            59
        });
        assert_eq!(&counts[..3], &[59, 60, 0]);
        assert_eq!(calls, 2);
    }

    #[test]
    fn age_walk_matches_original_68020() {
        let data = include_str!("../../tests/fixtures/simulation/mac/mac_cohort_age.csv");
        for line in data.lines().skip(1) {
            let v: Vec<u32> = line.split(',').map(|x| x.parse().unwrap()).collect();
            let dest = v[0] as usize;
            let (mut counts, mut aux, mut life) = ([0; 20], [0; 20], [0; 20]);
            counts[dest - 1] = v[1];
            aux[dest - 1] = v[2];
            life[dest - 1] = v[3];
            let inputs = MacCohortAgeInputs {
                school: v[6],
                college: v[7],
                life_penalty: v[8],
                ordinance_bit: v[5] != 0,
            };
            let mut draws = 0;
            mac_cohort_age_month(&mut counts, &mut aux, &mut life, &inputs, || {
                draws += 1;
                v[4] as u16
            });
            assert_eq!(
                [
                    counts[dest - 1],
                    aux[dest - 1],
                    life[dest - 1],
                    counts[dest],
                    aux[dest],
                    life[dest],
                    draws
                ],
                [v[9], v[10], v[11], v[12], v[13], v[14], v[15]],
                "{line}"
            );
        }
    }

    #[test]
    fn births_match_original_68020_and_draw_gate() {
        let data = include_str!("../../tests/fixtures/simulation/mac/mac_cohort_birth.csv");
        for line in data.lines().skip(1) {
            let v: Vec<u32> = line.split(',').map(|x| x.parse().unwrap()).collect();
            let mut counts = [0; 20];
            counts[4..=8].copy_from_slice(&v[0..5]);
            let mut auxiliary = [0; 20];
            let mut life = [0; 20];
            auxiliary[0] = v[10];
            life[0] = v[11];
            let mut balance = v[12];
            let inputs = MacCohortBirthInputs {
                pollution_penalty: v[5],
                health_cap: v[7],
                life_weight: v[8],
                auxiliary_rate: v[9],
            };
            let mut draws = 0;
            let births = mac_cohort_births(
                &mut counts,
                &mut auxiliary,
                &mut life,
                &mut balance,
                &inputs,
                || {
                    draws += 1;
                    v[6] as u16
                },
            );
            assert_eq!(
                [births, auxiliary[0], life[0], balance, draws],
                [v[13], v[14], v[15], v[16], v[17]],
                "{line}"
            );
            assert_eq!(counts[0], v[13]);
        }
    }

    #[test]
    fn deficit_refill_matches_original_68020() {
        let data = include_str!("../../tests/fixtures/simulation/mac/mac_cohort_deficit.csv");
        for line in data.lines().skip(1) {
            let v: Vec<&str> = line.split(',').collect();
            let parse_lane = |field: &str| {
                let parsed: Vec<u32> = field
                    .split(':')
                    .map(|value| u32::from_str_radix(value, 16).unwrap())
                    .collect();
                parsed.try_into().unwrap()
            };
            let mut counts = parse_lane(v[2]);
            let mut auxiliary = parse_lane(v[3]);
            let mut life = parse_lane(v[4]);
            let mut negative = v[0].parse().unwrap();
            let positive = v[1].parse().unwrap();
            mac_cohort_replenish_deficit(
                &mut counts,
                &mut auxiliary,
                &mut life,
                &mut negative,
                positive,
            );
            let expected_counts = parse_lane(v[7]);
            let expected_auxiliary = parse_lane(v[8]);
            let expected_life = parse_lane(v[9]);
            assert_eq!(
                [negative, positive],
                [v[5].parse::<u32>().unwrap(), v[6].parse::<u32>().unwrap()],
                "{line}"
            );
            assert_eq!(counts, expected_counts, "{line}");
            assert_eq!(auxiliary, expected_auxiliary, "{line}");
            assert_eq!(life, expected_life, "{line}");
        }
    }

    #[test]
    fn joined_empty_city_phase21_cohort_path_matches_original_68020() {
        let data =
            include_str!("../../tests/fixtures/simulation/mac/mac_phase21_cohort_refill_join.csv");
        let row = data.lines().nth(1).expect("native joined fixture row");
        let v: Vec<&str> = row.split(',').collect();
        let parse_lane = |field: &str| -> [u32; 20] {
            field
                .split(':')
                .map(|value| u32::from_str_radix(value, 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };

        let mut counts = [0; 20];
        let mut auxiliary = [0; 20];
        let mut life = [0; 20];
        let raw = MacCohortSetup {
            school_tiles: v[6].parse().unwrap(),
            college_tiles: v[7].parse().unwrap(),
            hospital_tiles: 0,
            school_funding: 0,
            college_funding: 0,
            hospital_funding: 0,
            health_budget: 0,
            flags_a: 0,
            flags_b: 0,
            pollution: 0,
            scalar: 0,
            class_two: 0,
        };
        let prepared = mac_cohort_prepare_inputs(&raw).unwrap();
        assert_eq!(prepared.life_expectancy, v[9].parse::<u32>().unwrap());

        mac_cohort_age_month(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &prepared.age,
            || panic!("empty cohort age walk must not draw"),
        );
        let mut positive_balance = 0;
        let mut birth_draws = 0;
        let births = mac_cohort_births(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &mut positive_balance,
            &MacCohortBirthInputs {
                pollution_penalty: prepared.age.life_penalty,
                health_cap: prepared.health,
                life_weight: prepared.life_expectancy,
                auxiliary_rate: 0,
            },
            || {
                birth_draws += 1;
                0
            },
        );
        assert_eq!(births, 0);
        assert_eq!(birth_draws, 1, "native makes its unconditional birth draw");

        let mut negative_balance: u32 = v[4].parse().unwrap();
        mac_cohort_replenish_deficit(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &mut negative_balance,
            positive_balance,
        );

        let actual_totals = [
            counts.iter().sum::<u32>(),
            auxiliary.iter().sum::<u32>(),
            life.iter().sum::<u32>(),
            negative_balance,
            positive_balance,
        ];
        let expected_totals = [
            v[12].parse::<u32>().unwrap(),
            v[13].parse::<u32>().unwrap(),
            v[14].parse::<u32>().unwrap(),
            v[15].parse::<u32>().unwrap(),
            v[16].parse::<u32>().unwrap(),
        ];
        assert_eq!(actual_totals, expected_totals);
        let expected_counts = parse_lane(v[17]);
        let expected_auxiliary = parse_lane(v[18]);
        let expected_life = parse_lane(v[19]);
        assert_eq!(counts, expected_counts);
        assert_eq!(auxiliary, expected_auxiliary);
        assert_eq!(life, expected_life);
    }

    #[test]
    fn joined_flint_phase21_nonzero_cohort_transition_matches_original_68020() {
        let data = include_str!(
            "../../tests/fixtures/simulation/mac/mac_phase21_flint_cohort_transition.csv"
        );
        let checkpoint = |name: &str| -> Vec<&str> {
            data.lines()
                .skip(1)
                .map(|line| line.split(',').collect::<Vec<_>>())
                .find(|row| row[0] == name)
                .unwrap_or_else(|| panic!("missing native checkpoint {name}"))
        };
        let parse_lane = |field: &str| -> [u32; 20] {
            field
                .split(':')
                .map(|value| u32::from_str_radix(value, 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let assert_checkpoint = |row: &[&str],
                                 counts: &[u32; 20],
                                 auxiliary: &[u32; 20],
                                 life: &[u32; 20],
                                 deficit: u32,
                                 surplus: u32| {
            assert_eq!(row[1].parse::<u32>().unwrap(), 98_040);
            assert_eq!(row[2].parse::<u32>().unwrap(), deficit, "{}", row[0]);
            assert_eq!(row[3].parse::<u32>().unwrap(), surplus, "{}", row[0]);
            assert_eq!(*counts, parse_lane(row[4]), "{} counts", row[0]);
            assert_eq!(*auxiliary, parse_lane(row[5]), "{} auxiliary", row[0]);
            assert_eq!(*life, parse_lane(row[6]), "{} life", row[0]);
        };

        // CODE 8 @ 0x39e1a reads these XBLD counts and active-document
        // funding fields from the saved Flint state: school D6=63,
        // college D9=16, hospital D1=27, and all three funding values 100.
        let prepared = mac_cohort_prepare_inputs(&MacCohortSetup {
            school_tiles: 63,
            college_tiles: 16,
            hospital_tiles: 27,
            school_funding: 100,
            college_funding: 100,
            hospital_funding: 100,
            health_budget: 0,
            flags_a: 0x80,
            flags_b: 0x0b,
            pollution: 0,
            scalar: 98_040,
            class_two: 0,
        })
        .unwrap();
        assert_eq!(
            [
                prepared.age.school,
                prepared.age.college,
                prepared.health,
                prepared.life_expectancy,
                prepared.age.life_penalty,
            ],
            [105, 50, 75, 85, 0]
        );
        assert!(!prepared.age.ordinance_bit);

        let input = checkpoint("input");
        let mut counts = parse_lane(input[4]);
        let mut auxiliary = parse_lane(input[5]);
        let mut life = parse_lane(input[6]);
        let mut deficit = input[2].parse::<u32>().unwrap();
        let mut surplus = input[3].parse::<u32>().unwrap();
        assert_checkpoint(&input, &counts, &auxiliary, &life, deficit, surplus);

        // Native QuickDraw values after the 21 industry-prefix draws in the
        // controlled phase-21 replay; signed words are passed as raw WORDs.
        let mut mortality_draws =
            [-26702_i16, 21793, -1423, 6763, 26431, -26853, -25065].into_iter();
        for index in 1..20 {
            mac_cohort_mortality_slot(
                index,
                &mut counts[index],
                &mut auxiliary[index],
                &mut life[index],
                &mut deficit,
                || mortality_draws.next().expect("native mortality draw") as u16,
            );
        }
        assert!(mortality_draws.next().is_none());
        let after_mortality = checkpoint("after_mortality");
        assert_checkpoint(
            &after_mortality,
            &counts,
            &auxiliary,
            &life,
            deficit,
            surplus,
        );

        let mut age_draws = [
            8833_i16, 19562, -11425, 12096, 7146, -13691, 5607, 10548, 16553, 12632, -25888, 4078,
            1484, -25925, -23104, 7487, 10464, -23811, -23925,
        ]
        .into_iter();
        mac_cohort_age_month(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &prepared.age,
            || age_draws.next().expect("native age-transfer draw") as u16,
        );
        assert!(age_draws.next().is_none());
        let after_age = checkpoint("after_age");
        assert_checkpoint(&after_age, &counts, &auxiliary, &life, deficit, surplus);

        let mut birth_draws = [21669_i16].into_iter();
        let births = mac_cohort_births(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &mut surplus,
            &MacCohortBirthInputs {
                pollution_penalty: prepared.age.life_penalty,
                health_cap: prepared.health,
                life_weight: prepared.life_expectancy,
                auxiliary_rate: 86,
            },
            || birth_draws.next().expect("unconditional native birth draw") as u16,
        );
        assert!(birth_draws.next().is_none());
        assert_eq!(births, 48);
        let after_births = checkpoint("after_births");
        assert_checkpoint(&after_births, &counts, &auxiliary, &life, deficit, surplus);

        mac_cohort_replenish_deficit(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &mut deficit,
            surplus,
        );
        let after_refill = checkpoint("after_refill");
        assert_checkpoint(&after_refill, &counts, &auxiliary, &life, deficit, surplus);

        // CODE 8 continues from the refill boundary into its final summary
        // arm at 0x3a46e, which publishes MISC +0x48/+0x4c and the ratio.
        let summary = mac_cohort_summary(&counts, &auxiliary, &life, 98_040).unwrap();
        let after_summary = checkpoint("after_summary");
        assert_checkpoint(&after_summary, &counts, &auxiliary, &life, deficit, surplus);
        assert_eq!(
            [
                summary.auxiliary_average,
                summary.life_average,
                summary.workforce_ratio,
            ],
            [
                after_summary[7].parse::<u32>().unwrap(),
                after_summary[8].parse::<u32>().unwrap(),
                after_summary[9].parse::<u32>().unwrap(),
            ]
        );
    }

    #[test]
    fn scheduler_flint_phase21_cohort_transition_matches_original_68020() {
        let checkpoints = include_str!(
            "../../tests/fixtures/simulation/mac/mac_phase21_scheduler_cohort_transition.csv"
        );
        let checkpoint = |name: &str| -> Vec<&str> {
            checkpoints
                .lines()
                .skip(1)
                .map(|line| line.split(',').collect::<Vec<_>>())
                .find(|row| row[0] == name)
                .unwrap_or_else(|| panic!("missing native checkpoint {name}"))
        };
        let parse_lane = |field: &str| -> [u32; 20] {
            field
                .split(':')
                .map(|value| u32::from_str_radix(value, 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let assert_checkpoint = |row: &[&str],
                                 scalar: u32,
                                 counts: &[u32; 20],
                                 auxiliary: &[u32; 20],
                                 life: &[u32; 20],
                                 deficit: u32,
                                 surplus: u32| {
            assert_eq!(row[4].parse::<u32>().unwrap(), scalar, "{} scalar", row[0]);
            assert_eq!(
                row[5].parse::<u32>().unwrap(),
                deficit,
                "{} deficit",
                row[0]
            );
            assert_eq!(
                row[6].parse::<u32>().unwrap(),
                surplus,
                "{} surplus",
                row[0]
            );
            assert_eq!(*counts, parse_lane(row[10]), "{} counts", row[0]);
            assert_eq!(*auxiliary, parse_lane(row[11]), "{} auxiliary", row[0]);
            assert_eq!(*life, parse_lane(row[12]), "{} life", row[0]);
        };

        // Saved Flint census and funding inputs at CODE 8's setup entry. The
        // current scan publishes scalar 32,050 with a pre-worker deficit 640;
        // this specifically joins the scheduler capture to the same helpers
        // previously compared on an isolated post-scan snapshot.
        let prepared = mac_cohort_prepare_inputs(&MacCohortSetup {
            school_tiles: 63,
            college_tiles: 16,
            hospital_tiles: 27,
            school_funding: 100,
            college_funding: 100,
            hospital_funding: 100,
            health_budget: 0,
            flags_a: 0x80,
            flags_b: 0x0b,
            pollution: 0,
            scalar: 32_050,
            class_two: 0,
        })
        .unwrap();
        assert_eq!(
            [
                prepared.age.school,
                prepared.age.college,
                prepared.health,
                prepared.life_expectancy,
                prepared.age.life_penalty,
            ],
            [105, 50, 75, 85, 0]
        );

        let quickdraw = include_str!(
            "../../tests/fixtures/simulation/mac/mac_code2_phase21_quickdraw_order.csv"
        );
        let draws_at = |site: &str| -> Vec<u16> {
            quickdraw
                .lines()
                .skip(1)
                .map(|line| line.split(',').collect::<Vec<_>>())
                .filter(|row| row[3] == site)
                .map(|row| row[5].parse::<u16>().unwrap())
                .collect()
        };

        let input = checkpoint("input");
        assert_eq!(input[1], "0x39e1a");
        let mut counts = parse_lane(input[10]);
        let mut auxiliary = parse_lane(input[11]);
        let mut life = parse_lane(input[12]);
        let mut deficit = input[5].parse::<u32>().unwrap();
        let mut surplus = input[6].parse::<u32>().unwrap();
        assert_checkpoint(&input, 32_050, &counts, &auxiliary, &life, deficit, surplus);

        let mut mortality_draws = draws_at("0x39f5a").into_iter();
        assert_eq!(mortality_draws.len(), 7);
        for index in 1..20 {
            mac_cohort_mortality_slot(
                index,
                &mut counts[index],
                &mut auxiliary[index],
                &mut life[index],
                &mut deficit,
                || mortality_draws.next().expect("native mortality draw"),
            );
        }
        assert!(mortality_draws.next().is_none());
        let after_mortality = checkpoint("after_mortality");
        assert_checkpoint(
            &after_mortality,
            32_050,
            &counts,
            &auxiliary,
            &life,
            deficit,
            surplus,
        );

        let mut age_draws = draws_at("0x3a02e").into_iter();
        assert_eq!(age_draws.len(), 19);
        mac_cohort_age_month(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &prepared.age,
            || age_draws.next().expect("native age-transfer draw"),
        );
        assert!(age_draws.next().is_none());
        let after_age = checkpoint("after_age");
        assert_checkpoint(
            &after_age, 32_050, &counts, &auxiliary, &life, deficit, surplus,
        );

        let mut birth_draws = draws_at("0x3a142").into_iter();
        assert_eq!(birth_draws.len(), 1);
        let births = mac_cohort_births(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &mut surplus,
            &MacCohortBirthInputs {
                pollution_penalty: prepared.age.life_penalty,
                health_cap: prepared.health,
                life_weight: prepared.life_expectancy,
                auxiliary_rate: input[8].parse::<u32>().unwrap(),
            },
            || birth_draws.next().expect("native birth draw"),
        );
        assert!(birth_draws.next().is_none());
        assert_eq!(births, 48);
        let after_births = checkpoint("after_births");
        assert_checkpoint(
            &after_births,
            32_050,
            &counts,
            &auxiliary,
            &life,
            deficit,
            surplus,
        );

        mac_cohort_replenish_deficit(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &mut deficit,
            surplus,
        );
        let after_refill = checkpoint("after_refill");
        assert_checkpoint(
            &after_refill,
            32_050,
            &counts,
            &auxiliary,
            &life,
            deficit,
            surplus,
        );

        let summary = mac_cohort_summary(&counts, &auxiliary, &life, 32_050).unwrap();
        let after_summary = checkpoint("after_summary");
        assert_checkpoint(
            &after_summary,
            32_050,
            &counts,
            &auxiliary,
            &life,
            deficit,
            surplus,
        );
        assert_eq!(
            [
                summary.workforce_ratio,
                summary.auxiliary_average,
                summary.life_average,
            ],
            [
                after_summary[7].parse::<u32>().unwrap(),
                after_summary[8].parse::<u32>().unwrap(),
                after_summary[9].parse::<u32>().unwrap(),
            ]
        );
    }

    #[test]
    fn scheduler_dullsville_phase21_surplus_transition_matches_original_68020() {
        let checkpoints = include_str!(
            "../../tests/fixtures/simulation/mac/mac_phase21_dullsville_cohort_transition.csv"
        );
        let setup_csv = include_str!(
            "../../tests/fixtures/simulation/mac/mac_phase21_dullsville_cohort_setup.csv"
        );
        let draws_csv = include_str!(
            "../../tests/fixtures/simulation/mac/mac_phase21_dullsville_cohort_draws.csv"
        );
        let checkpoint = |name: &str| -> Vec<&str> {
            checkpoints
                .lines()
                .skip(1)
                .map(|line| line.split(',').collect::<Vec<_>>())
                .find(|row| row[0] == name)
                .unwrap_or_else(|| panic!("missing native checkpoint {name}"))
        };
        let parse_lane = |field: &str| -> [u32; 20] {
            field
                .split(':')
                .map(|value| u32::from_str_radix(value, 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let assert_checkpoint = |row: &[&str],
                                 counts: &[u32; 20],
                                 auxiliary: &[u32; 20],
                                 life: &[u32; 20],
                                 deficit: u32,
                                 surplus: u32| {
            assert_eq!(row[1].parse::<u32>().unwrap(), 1_530, "{} scalar", row[0]);
            assert_eq!(
                row[2].parse::<u32>().unwrap(),
                deficit,
                "{} deficit",
                row[0]
            );
            assert_eq!(
                row[3].parse::<u32>().unwrap(),
                surplus,
                "{} surplus",
                row[0]
            );
            assert_eq!(*counts, parse_lane(row[4]), "{} counts", row[0]);
            assert_eq!(*auxiliary, parse_lane(row[5]), "{} auxiliary", row[0]);
            assert_eq!(*life, parse_lane(row[6]), "{} life", row[0]);
        };

        let setup_row = setup_csv.lines().nth(1).expect("native setup row");
        let v: Vec<u32> = setup_row
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect();
        let prepared = mac_cohort_prepare_inputs(&MacCohortSetup {
            school_tiles: v[0] as u16,
            school_funding: v[1],
            college_tiles: v[2] as u16,
            college_funding: v[3],
            hospital_tiles: v[4] as u16,
            hospital_funding: v[5],
            health_budget: v[6],
            flags_a: v[7] as u8,
            flags_b: v[8] as u8,
            pollution: v[9],
            scalar: v[10],
            class_two: v[11],
        })
        .unwrap();
        assert_eq!(
            [
                prepared.age.school,
                prepared.age.college,
                prepared.health,
                prepared.life_expectancy,
                prepared.age.life_penalty,
            ],
            [0, 0, 0, 85, 0]
        );

        let draws_at = |site: &str| -> Vec<u16> {
            draws_csv
                .lines()
                .skip(1)
                .map(|line| line.split(',').collect::<Vec<_>>())
                .filter(|row| row[0] == "cohort" && row[2] == site)
                .map(|row| row[3].parse::<i16>().unwrap() as u16)
                .collect()
        };
        let input = checkpoint("input");
        assert_checkpoint(
            &input,
            &parse_lane(input[4]),
            &parse_lane(input[5]),
            &parse_lane(input[6]),
            0,
            150,
        );
        let mut counts = parse_lane(input[4]);
        let mut auxiliary = parse_lane(input[5]);
        let mut life = parse_lane(input[6]);
        let mut deficit = input[2].parse::<u32>().unwrap();
        let mut surplus = input[3].parse::<u32>().unwrap();

        let mut mortality_draws = draws_at("0x39f5a").into_iter();
        assert_eq!(mortality_draws.len(), 3);
        for index in 1..20 {
            mac_cohort_mortality_slot(
                index,
                &mut counts[index],
                &mut auxiliary[index],
                &mut life[index],
                &mut deficit,
                || mortality_draws.next().expect("native mortality draw"),
            );
        }
        assert!(mortality_draws.next().is_none());
        let after_mortality = checkpoint("after_mortality");
        assert_checkpoint(
            &after_mortality,
            &counts,
            &auxiliary,
            &life,
            deficit,
            surplus,
        );

        let mut age_draws = draws_at("0x3a02e").into_iter();
        assert_eq!(age_draws.len(), 14);
        mac_cohort_age_month(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &prepared.age,
            || age_draws.next().expect("native age-transfer draw"),
        );
        assert!(age_draws.next().is_none());
        let after_age = checkpoint("after_age");
        assert_checkpoint(&after_age, &counts, &auxiliary, &life, deficit, surplus);

        let mut birth_draws = draws_at("0x3a142").into_iter();
        assert_eq!(birth_draws.len(), 1);
        let births = mac_cohort_births(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &mut surplus,
            &MacCohortBirthInputs {
                pollution_penalty: prepared.age.life_penalty,
                health_cap: prepared.health,
                life_weight: prepared.life_expectancy,
                auxiliary_rate: input[7].parse().unwrap(),
            },
            || birth_draws.next().expect("native birth draw"),
        );
        assert!(birth_draws.next().is_none());
        assert_eq!(births, 3);
        let after_births = checkpoint("after_births");
        assert_checkpoint(&after_births, &counts, &auxiliary, &life, deficit, surplus);

        let mut emigration_draws = draws_at("0x3a348").into_iter();
        assert_eq!(emigration_draws.len(), 2);
        // Native CODE 8 first subtracts the one-person deficit from fffff022
        // at 0x3a2ba..0x3a2c6, then emigration consumes the resulting 152.
        // It leaves fffff01e unchanged on this branch.
        surplus = surplus.wrapping_sub(deficit);
        mac_cohort_emigrate_surplus(
            &mut counts,
            &mut auxiliary,
            &mut life,
            1_530,
            &mut surplus,
            || emigration_draws.next().expect("native emigration draw"),
        )
        .unwrap();
        assert!(emigration_draws.next().is_none());
        let after_refill = checkpoint("after_refill");
        assert_checkpoint(&after_refill, &counts, &auxiliary, &life, deficit, surplus);

        let summary = mac_cohort_summary(&counts, &auxiliary, &life, 1_530).unwrap();
        let after_summary = checkpoint("after_summary");
        assert_checkpoint(&after_summary, &counts, &auxiliary, &life, deficit, surplus);
        assert_eq!(
            [
                summary.auxiliary_average,
                summary.life_average,
                summary.workforce_ratio,
            ],
            [
                after_summary[7].parse::<u32>().unwrap(),
                after_summary[8].parse::<u32>().unwrap(),
                after_summary[9].parse::<u32>().unwrap(),
            ]
        );
    }

    #[test]
    fn scheduler_seven_spires_phase21_cohort_transition_matches_original_68020() {
        let checkpoints = include_str!(
            "../../tests/fixtures/simulation/mac/mac_phase21_sevenspires_cohort_transition.csv"
        );
        let setup_csv = include_str!(
            "../../tests/fixtures/simulation/mac/mac_phase21_sevenspires_cohort_setup.csv"
        );
        let draws_csv = include_str!(
            "../../tests/fixtures/simulation/mac/mac_phase21_sevenspires_cohort_draws.csv"
        );
        let checkpoint = |name: &str| -> Vec<&str> {
            checkpoints
                .lines()
                .skip(1)
                .map(|line| line.split(',').collect::<Vec<_>>())
                .find(|row| row[0] == name)
                .unwrap_or_else(|| panic!("missing native checkpoint {name}"))
        };
        let parse_lane = |field: &str| -> [u32; 20] {
            field
                .split(':')
                .map(|value| u32::from_str_radix(value, 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let assert_checkpoint = |row: &[&str],
                                 counts: &[u32; 20],
                                 auxiliary: &[u32; 20],
                                 life: &[u32; 20],
                                 deficit: u32,
                                 surplus: u32| {
            assert_eq!(row[1].parse::<u32>().unwrap(), 8_880, "{} scalar", row[0]);
            assert_eq!(
                row[2].parse::<u32>().unwrap(),
                deficit,
                "{} deficit",
                row[0]
            );
            assert_eq!(
                row[3].parse::<u32>().unwrap(),
                surplus,
                "{} surplus",
                row[0]
            );
            assert_eq!(*counts, parse_lane(row[4]), "{} counts", row[0]);
            assert_eq!(*auxiliary, parse_lane(row[5]), "{} auxiliary", row[0]);
            assert_eq!(*life, parse_lane(row[6]), "{} life", row[0]);
        };

        let setup_row = setup_csv.lines().nth(1).expect("native setup row");
        let v: Vec<u32> = setup_row
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect();
        let prepared = mac_cohort_prepare_inputs(&MacCohortSetup {
            school_tiles: v[0] as u16,
            school_funding: v[1],
            college_tiles: v[2] as u16,
            college_funding: v[3],
            hospital_tiles: v[4] as u16,
            hospital_funding: v[5],
            health_budget: v[6],
            flags_a: v[7] as u8,
            flags_b: v[8] as u8,
            pollution: v[9],
            scalar: v[10],
            class_two: v[11],
        })
        .unwrap();
        assert_eq!(
            [
                prepared.age.school,
                prepared.age.college,
                prepared.health,
                prepared.life_expectancy,
                prepared.age.life_penalty,
            ],
            [0, 0, 27, 100, 0]
        );

        let draws_at = |site: &str| -> Vec<u16> {
            draws_csv
                .lines()
                .skip(1)
                .map(|line| line.split(',').collect::<Vec<_>>())
                .filter(|row| row[0] == "cohort" && row[2] == site)
                .map(|row| row[3].parse::<i16>().unwrap() as u16)
                .collect()
        };
        let input = checkpoint("input");
        let mut counts = parse_lane(input[4]);
        let mut auxiliary = parse_lane(input[5]);
        let mut life = parse_lane(input[6]);
        let mut deficit = input[2].parse::<u32>().unwrap();
        let mut surplus = input[3].parse::<u32>().unwrap();
        assert_checkpoint(&input, &counts, &auxiliary, &life, deficit, surplus);

        let mut mortality_draws = draws_at("0x39f5a").into_iter();
        assert_eq!(mortality_draws.len(), 6);
        for index in 1..20 {
            mac_cohort_mortality_slot(
                index,
                &mut counts[index],
                &mut auxiliary[index],
                &mut life[index],
                &mut deficit,
                || mortality_draws.next().expect("native mortality draw"),
            );
        }
        assert!(mortality_draws.next().is_none());
        let after_mortality = checkpoint("after_mortality");
        assert_checkpoint(
            &after_mortality,
            &counts,
            &auxiliary,
            &life,
            deficit,
            surplus,
        );

        let mut age_draws = draws_at("0x3a02e").into_iter();
        assert_eq!(age_draws.len(), 19);
        mac_cohort_age_month(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &prepared.age,
            || age_draws.next().expect("native age-transfer draw"),
        );
        assert!(age_draws.next().is_none());
        let after_age = checkpoint("after_age");
        assert_checkpoint(&after_age, &counts, &auxiliary, &life, deficit, surplus);

        let mut birth_draws = draws_at("0x3a142").into_iter();
        assert_eq!(birth_draws.len(), 1);
        let births = mac_cohort_births(
            &mut counts,
            &mut auxiliary,
            &mut life,
            &mut surplus,
            &MacCohortBirthInputs {
                pollution_penalty: prepared.age.life_penalty,
                health_cap: prepared.health,
                life_weight: prepared.life_expectancy,
                auxiliary_rate: input[7].parse().unwrap(),
            },
            || birth_draws.next().expect("native birth draw"),
        );
        assert!(birth_draws.next().is_none());
        assert_eq!(births, 12);
        let after_births = checkpoint("after_births");
        assert_checkpoint(&after_births, &counts, &auxiliary, &life, deficit, surplus);

        // As in the Dullsville branch, CODE 8 cancels the negative balance
        // against fffff022 before the proportional emigration loop. The native
        // checkpoint shows 4 retained in fffff01e and all 18 remaining people
        // consumed from fffff022.
        surplus = surplus.wrapping_sub(deficit);
        let mut emigration_draws = draws_at("0x3a348").into_iter();
        assert_eq!(emigration_draws.len(), 19);
        mac_cohort_emigrate_surplus(
            &mut counts,
            &mut auxiliary,
            &mut life,
            8_880,
            &mut surplus,
            || emigration_draws.next().expect("native emigration draw"),
        )
        .unwrap();
        assert!(emigration_draws.next().is_none());
        let after_refill = checkpoint("after_refill");
        assert_checkpoint(&after_refill, &counts, &auxiliary, &life, deficit, surplus);

        let summary = mac_cohort_summary(&counts, &auxiliary, &life, 8_880).unwrap();
        let after_summary = checkpoint("after_summary");
        assert_checkpoint(&after_summary, &counts, &auxiliary, &life, deficit, surplus);
        assert_eq!(
            [
                summary.auxiliary_average,
                summary.life_average,
                summary.workforce_ratio,
            ],
            [
                after_summary[7].parse::<u32>().unwrap(),
                after_summary[8].parse::<u32>().unwrap(),
                after_summary[9].parse::<u32>().unwrap(),
            ]
        );
    }

    #[test]
    fn summary_clears_outputs_when_workforce_cohorts_are_empty() {
        let empty = [0_u32; 20];
        let summary = mac_cohort_summary(&empty, &empty, &empty, 0).unwrap();
        assert_eq!(
            [
                summary.auxiliary_average,
                summary.life_average,
                summary.workforce_ratio,
            ],
            [0, 0, 0]
        );
    }

    #[test]
    fn surplus_emigration_matches_modeled_sane_original_68020() {
        let data = include_str!("../../tests/fixtures/simulation/mac/mac_cohort_surplus.csv");
        for line in data.lines().skip(1) {
            let v: Vec<&str> = line.split(',').collect();
            let parse_lane = |field: &str| {
                let parsed: Vec<u32> = field
                    .split(':')
                    .map(|value| u32::from_str_radix(value, 16).unwrap())
                    .collect();
                parsed.try_into().unwrap()
            };
            let mut counts = parse_lane(v[2]);
            let mut auxiliary = parse_lane(v[3]);
            let mut life = parse_lane(v[4]);
            let mut surplus = v[0].parse::<u32>().unwrap();
            let scalar = v[1].parse::<u32>().unwrap();
            let random_word = v[5].parse::<u16>().unwrap();
            let mut draws = 0;
            mac_cohort_emigrate_surplus(
                &mut counts,
                &mut auxiliary,
                &mut life,
                scalar,
                &mut surplus,
                || {
                    let value = if draws == 0 { random_word } else { 0 };
                    draws += 1;
                    value
                },
            )
            .expect("fixture keeps the original denominator and population valid");
            assert_eq!(surplus, v[6].parse::<u32>().unwrap(), "{line}");
            assert_eq!(counts, parse_lane(v[7]), "{line}");
            assert_eq!(auxiliary, parse_lane(v[8]), "{line}");
            assert_eq!(life, parse_lane(v[9]), "{line}");
            assert_eq!(draws, v[10].parse::<usize>().unwrap(), "{line}");
        }
    }

    #[test]
    fn surplus_emigration_rejects_unconsumable_balance_without_spinning() {
        let (mut counts, mut auxiliary, mut life) = ([0; 20], [0; 20], [0; 20]);
        let mut surplus = 1;
        let mut random_calls = 0;

        assert_eq!(
            mac_cohort_emigrate_surplus(
                &mut counts,
                &mut auxiliary,
                &mut life,
                1,
                &mut surplus,
                || {
                    random_calls += 1;
                    0
                },
            ),
            None
        );
        assert_eq!(surplus, 1);
        assert_eq!(random_calls, 0);
        assert_eq!(counts, [0; 20]);
        assert_eq!(auxiliary, [0; 20]);
        assert_eq!(life, [0; 20]);
    }

    #[test]
    fn mortality_matches_original_68020_and_draw_gates() {
        let data = include_str!("../../tests/fixtures/simulation/mac/mac_cohort_mortality.csv");
        for line in data.lines().skip(1) {
            let v: Vec<u32> = line.split(',').map(|x| x.parse().unwrap()).collect();
            let (mut count, mut aux, mut life, mut balance) = (v[1], v[2], v[3], v[5]);
            let mut draws = 0;
            mac_cohort_mortality_slot(
                v[0] as usize,
                &mut count,
                &mut aux,
                &mut life,
                &mut balance,
                || {
                    draws += 1;
                    v[4] as u16
                },
            );
            assert_eq!(
                [count, aux, life, balance, draws],
                [v[6], v[7], v[8], v[9], v[10]],
                "{line}"
            );
        }
    }
}
