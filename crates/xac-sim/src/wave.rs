use xac_core::EnemyKind;

pub(crate) const WAVE_PERIOD_TICKS: u64 = 80;
pub(crate) const FIRST_WAVE_TICK: u64 = 20;

pub(crate) fn current_wave(tick: u64) -> u32 {
    (tick / WAVE_PERIOD_TICKS) as u32 + 1
}

pub(crate) fn next_wave_in(tick: u64) -> u32 {
    let phase = tick % WAVE_PERIOD_TICKS;
    if phase < FIRST_WAVE_TICK {
        (FIRST_WAVE_TICK - phase) as u32
    } else {
        (WAVE_PERIOD_TICKS + FIRST_WAVE_TICK - phase) as u32
    }
}

pub(crate) fn should_spawn_wave(tick: u64) -> bool {
    tick >= FIRST_WAVE_TICK && tick % WAVE_PERIOD_TICKS == FIRST_WAVE_TICK
}

pub(crate) fn wave_enemies(wave: u32) -> Vec<EnemyKind> {
    let mut enemies = Vec::new();
    let grunt_count = 1 + wave.saturating_sub(1) / 2;
    enemies.extend(std::iter::repeat_n(EnemyKind::Grunt, grunt_count as usize));

    if wave >= 2 {
        enemies.push(EnemyKind::Runner);
    }
    if wave >= 3 {
        enemies.push(EnemyKind::Armored);
    }
    if wave >= 4 {
        enemies.push(EnemyKind::WireCutter);
    }

    enemies
}
