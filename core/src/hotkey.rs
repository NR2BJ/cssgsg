//! 수식키 단독 탭 판정(NRIME 최종 로직을 옮겼다).
//!
//! - 시간은 이벤트 타임스탬프로 잰다(벽시계 X). 입력 스레드가 밀려도 판정이 흔들리지 않는다.
//! - 좌우는 이벤트의 키로 구분한다(셸이 디바이스 비트로 좌우를 가려서 보낸다).
//! - 누를 때 다른 수식키가 눌려 있거나, 사이에 다른 키/수식키가 끼거나,
//!   뗄 때 다른 수식키가 아직 눌려 있으면 탭이 아니다.

use crate::key::{Key, KeyEvent};

#[derive(Debug, Default, Clone)]
pub struct TapTracker {
    candidate: Option<(Key, f64)>,
}

impl TapTracker {
    /// 이벤트를 보고, 탭이 완성됐으면 그 수식키를 돌려준다.
    pub fn observe(&mut self, ev: &KeyEvent, threshold_secs: f64) -> Option<Key> {
        if !ev.key.is_modifier() {
            if ev.down {
                self.candidate = None;
            }
            return None;
        }
        if ev.down {
            if ev.repeat {
                return None;
            }
            let others = ev.mods.any_held_except(ev.key.modifier_bit());
            self.candidate = if others || self.candidate.is_some() { None } else { Some((ev.key, ev.time)) };
            return None;
        }
        let (key, t0) = self.candidate.take()?;
        let solo = key == ev.key && !ev.mods.any_held_except(0);
        (solo && ev.time - t0 <= threshold_secs).then_some(key)
    }

    /// 마우스 클릭 등으로 진행 중인 탭을 무효로 한다.
    pub fn cancel(&mut self) {
        self.candidate = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::Mods;

    const T: f64 = 0.2;

    fn down(k: Key, mods: u32, t: f64) -> KeyEvent {
        KeyEvent::down(k, Mods(mods), t)
    }
    fn up(k: Key, mods: u32, t: f64) -> KeyEvent {
        KeyEvent::up(k, Mods(mods), t)
    }

    #[test]
    fn quick_solo_tap() {
        let mut tr = TapTracker::default();
        assert_eq!(tr.observe(&down(Key::SHIFT_RIGHT, Mods::SHIFT_R, 1.0), T), None);
        assert_eq!(tr.observe(&up(Key::SHIFT_RIGHT, 0, 1.08), T), Some(Key::SHIFT_RIGHT));
    }

    #[test]
    fn slow_hold_is_not_a_tap() {
        let mut tr = TapTracker::default();
        tr.observe(&down(Key::SHIFT_RIGHT, Mods::SHIFT_R, 1.0), T);
        assert_eq!(tr.observe(&up(Key::SHIFT_RIGHT, 0, 1.5), T), None);
    }

    #[test]
    fn combo_cancels_tap() {
        let mut tr = TapTracker::default();
        tr.observe(&down(Key::SHIFT_LEFT, Mods::SHIFT_L, 1.0), T);
        tr.observe(&down(Key::A, Mods::SHIFT_L, 1.02), T);
        assert_eq!(tr.observe(&up(Key::SHIFT_LEFT, 0, 1.05), T), None);
    }

    #[test]
    fn other_modifier_cancels_tap() {
        // ⌘ 누른 채 Shift 탭(⌘⇧ 단축키)은 전환이 아니다.
        let mut tr = TapTracker::default();
        tr.observe(&down(Key::SHIFT_RIGHT, Mods::SHIFT_R | Mods::META_L, 1.0), T);
        assert_eq!(tr.observe(&up(Key::SHIFT_RIGHT, Mods::META_L, 1.05), T), None);
        // 양쪽 Shift를 겹쳐 누른 것도 아니다.
        tr.observe(&down(Key::SHIFT_LEFT, Mods::SHIFT_L, 2.0), T);
        tr.observe(&down(Key::SHIFT_RIGHT, Mods::SHIFT_L | Mods::SHIFT_R, 2.01), T);
        assert_eq!(tr.observe(&up(Key::SHIFT_RIGHT, Mods::SHIFT_L, 2.03), T), None);
        assert_eq!(tr.observe(&up(Key::SHIFT_LEFT, 0, 2.04), T), None);
    }

    #[test]
    fn caps_lock_state_does_not_block_tap() {
        let mut tr = TapTracker::default();
        tr.observe(&down(Key::SHIFT_RIGHT, Mods::SHIFT_R | Mods::CAPS, 1.0), T);
        assert_eq!(tr.observe(&up(Key::SHIFT_RIGHT, Mods::CAPS, 1.05), T), Some(Key::SHIFT_RIGHT));
    }
}
