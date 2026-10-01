//! COM 진입점 보호.
//!
//! - 패닉: `catch_unwind`로 잡아 대신 값을 돌려준다. extern "system" 밖으로 풀리면 Rust가 프로세스를 abort한다
//!   (탐색기에 올라간 입력기가 죽으면 바탕화면이 검게 된다 — chewing #781, azooKey 메모). 한 번 패닉이 나면
//!   그 프로세스에서는 키를 먹지 않는다([`poisoned`]).
//! - FP 환경: 들어올 때 MXCSR을 표준값(0x1F80, 예외 모두 가림)으로 맞추고 나갈 때 되돌린다. Delphi 12 이전 앱은
//!   예외를 켜 둬서 0.0/0.0 같은 연산이 트랩이 된다(chewing #412). 바꾼 환경에서 도는 일은 `#[inline(never)]` 함수
//!   안에서 한다(바꾸기 전후로 FP 연산이 옮겨지지 않게).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

static POISONED: AtomicBool = AtomicBool::new(false);
static HOOK: Once = Once::new();

/// 이 프로세스에서 패닉이 난 적이 있는지. 그러면 입력기는 키를 모두 앱에 넘긴다.
pub fn poisoned() -> bool {
    POISONED.load(Ordering::Relaxed)
}

/// `f`를 표준 FP 환경에서 돌린다. 패닉이 나면 `fallback()`을 돌려준다.
pub fn guarded<T>(fallback: impl FnOnce() -> T, f: impl FnOnce() -> T) -> T {
    HOOK.call_once(|| {
        // 이 DLL의 std에만 걸린다(호스트가 Rust 앱이어도 그쪽 훅과 따로다).
        std::panic::set_hook(Box::new(|info| crate::debug_log(&format!("panic: {info}"))));
    });
    let _fp = FpGuard::enter();
    match catch_unwind(AssertUnwindSafe(|| call(f))) {
        Ok(v) => v,
        Err(_) => {
            POISONED.store(true, Ordering::Relaxed);
            fallback()
        }
    }
}

#[inline(never)]
fn call<T>(f: impl FnOnce() -> T) -> T {
    f()
}

/// 들어올 때 MXCSR을 저장하고 표준값으로, 나갈 때(Drop) 되돌린다.
struct FpGuard {
    #[cfg(target_arch = "x86_64")]
    saved: u32,
}

#[cfg(target_arch = "x86_64")]
const STANDARD_MXCSR: u32 = 0x1F80;

impl FpGuard {
    #[inline(always)]
    fn enter() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let mut saved = 0u32;
            // SAFETY: stmxcsr·ldmxcsr는 MXCSR 레지스터와 넘긴 4바이트만 읽고 쓴다.
            unsafe {
                std::arch::asm!("stmxcsr [{}]", in(reg) &mut saved, options(nostack, preserves_flags));
                std::arch::asm!("ldmxcsr [{}]", in(reg) &STANDARD_MXCSR, options(nostack, readonly, preserves_flags));
            }
            Self { saved }
        }
        #[cfg(not(target_arch = "x86_64"))]
        Self {}
    }
}

impl Drop for FpGuard {
    #[inline(always)]
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: enter()에서 읽어 둔 값을 되돌린다.
        unsafe {
            std::arch::asm!("ldmxcsr [{}]", in(reg) &self.saved, options(nostack, readonly, preserves_flags));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_arch = "x86_64")]
    fn mxcsr() -> u32 {
        let mut v = 0u32;
        unsafe { std::arch::asm!("stmxcsr [{}]", in(reg) &mut v, options(nostack, preserves_flags)) };
        v
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn restores_the_callers_fp_environment() {
        let before = mxcsr();
        // Delphi처럼 0으로 나누기·무효 연산 예외를 켠 상태(가림 비트 끔)를 흉내 낸다.
        let delphi = before & !0x0005;
        unsafe {
            std::arch::asm!("ldmxcsr [{}]", in(reg) &delphi, options(nostack, readonly, preserves_flags))
        };
        let inside = guarded(|| 0, mxcsr);
        let after = mxcsr();
        unsafe {
            std::arch::asm!("ldmxcsr [{}]", in(reg) &before, options(nostack, readonly, preserves_flags))
        };
        assert_eq!(inside, STANDARD_MXCSR);
        assert_eq!(after, delphi);
    }

    #[test]
    fn panics_become_the_fallback() {
        let v = guarded(|| -1, || -> i32 { panic!("시험") });
        assert_eq!(v, -1);
        assert!(poisoned());
        POISONED.store(false, Ordering::Relaxed);
    }
}
