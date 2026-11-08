//! Port of the formatting/arithmetic halves of `appTimestamp`, `appGetTime`,
//! `appSystemTime`, all of `appSleep`/`appProcessSeconds`, the monotonic
//! `appSeconds`/`appSecondsSlow`/`appCycles`/`GSecondsPerCycle` quartet, and
//! `GetFileAgeDays` (`Core/Src/UnUnix.cpp`). The `time()`/`localtime_r()`/
//! `gettimeofday()` calls themselves stay native: they are real OS/
//! timezone-database queries, not policy this port owns. Weekday/month
//! names are the fixed "C" locale abbreviations this process always runs
//! under (no `setlocale` call exists anywhere in this codebase).

use crate::ansi::Tchar;
use crate::unix_text::{self, Hp2UnixTextServices};

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `appTimestamp`'s `wcsftime(Result,1024,TEXT("%a %b %e %H:%M:%S %Y"),&SysTime)`
/// step. `weekday`/`month` are `tm_wday`/`tm_mon` (0-based); `year_from_1900`
/// is raw `tm_year`. Returns `false` (matching `wcsftime` returning 0, the
/// original's "clear the buffer" case) if the formatted text plus NUL does
/// not fit in `capacity` TCHARs.
///
/// # Safety
/// `buffer` must be writable for `capacity` TCHARs. `weekday` must be in
/// `0..7` and `month` in `0..12` (guaranteed by a real `localtime_r` result).
pub unsafe fn format_timestamp(
    weekday: i32,
    month: i32,
    day_of_month: i32,
    hour: i32,
    minute: i32,
    second: i32,
    year_from_1900: i32,
    buffer: *mut Tchar,
    capacity: usize,
) -> bool {
    let text = format!(
        "{} {} {:2} {:02}:{:02}:{:02} {:04}",
        WEEKDAYS[weekday as usize],
        MONTHS[month as usize],
        day_of_month,
        hour,
        minute,
        second,
        year_from_1900 + 1900
    );
    let bytes = text.as_bytes();
    if bytes.len() + 1 > capacity {
        return false;
    }
    // SAFETY: `buffer` is writable for `capacity` TCHARs by contract, and
    // `bytes.len() + 1 <= capacity` was just checked.
    unsafe {
        for (i, &b) in bytes.iter().enumerate() {
            *buffer.add(i) = Tchar::from(b);
        }
        *buffer.add(bytes.len()) = 0;
    }
    true
}

/// `appGetTime`'s UTF-8 encode, `stat`, and range-check/cast steps. Returns
/// `(dword_result, exceeds_range)`; `exceeds_range` tells the caller whether
/// to `debugf` the 32-bit-overflow diagnostic (using its own already-decoded
/// `Filename`, not re-decoded here).
///
/// # Safety
/// `filename` must be a readable NUL-terminated TCHAR string. `services`
/// must satisfy its documented leaf contract.
pub unsafe fn get_time(
    services: &Hp2UnixTextServices,
    filename: *const Tchar,
) -> Result<(u32, bool), i32> {
    // SAFETY: `filename`/`services` are valid by contract.
    let filename_utf8 = unsafe { unix_text::to_utf8(services, filename) }?;
    // SAFETY: `filename_utf8` is a fresh NUL-terminated native allocation.
    unsafe { unix_text::normalize_path(filename_utf8) };
    let mut file_info: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `filename_utf8` is NUL-terminated; `file_info` is a valid
    // out-parameter.
    let stat_result = unsafe { libc::stat(filename_utf8.cast(), &mut file_info) };
    // SAFETY: `filename_utf8` came from the native system allocator.
    unsafe { libc::free(filename_utf8.cast()) };
    if stat_result != 0 || file_info.st_mtime < 0 {
        return Ok((0, false));
    }
    if file_info.st_mtime as u64 > u32::MAX as u64 {
        return Ok((u32::MAX, true));
    }
    Ok((file_info.st_mtime as u32, false))
}

/// `appSystemTime`'s field decomposition: `tm_year+1900`, `tm_mon+1`,
/// `tv_usec/1000`, and otherwise a straight field copy.
pub fn system_time(
    tm_year: i32,
    tm_mon: i32,
    tm_wday: i32,
    tm_mday: i32,
    tm_hour: i32,
    tm_min: i32,
    tm_sec: i32,
    tv_usec: i64,
) -> (i32, i32, i32, i32, i32, i32, i32, i32) {
    (
        tm_year + 1900,
        tm_mon + 1,
        tm_wday,
        tm_mday,
        tm_hour,
        tm_min,
        tm_sec,
        (tv_usec / 1000) as i32,
    )
}

/// `appSleep`: `usleep(appRound(Seconds * 1000000))`, where `appRound` on
/// this platform is a plain `static_cast<INT>` truncation toward zero (see
/// `UnGnuG.h`'s non-`__PSX2_EE__` `appRound`), not mathematical rounding.
///
/// # Safety
/// None beyond the `usleep` contract.
pub unsafe fn sleep(seconds: f32) {
    let sleep_time = (seconds * 1_000_000.0f32) as i32;
    // SAFETY: `usleep` has no pointer precondition; a negative `sleep_time`
    // reinterprets as a large `useconds_t`, matching the original's implicit
    // `INT` -> `useconds_t` conversion at its call site.
    unsafe { libc::usleep(sleep_time as libc::useconds_t) };
}

/// `appProcessSeconds`: CPU time consumed by this process, excluding time
/// spent asleep. `clock_gettime(CLOCK_PROCESS_CPUTIME_ID, ...)` is a plain
/// POSIX syscall (unlike `appSeconds`/`appCycles`, which stay native because
/// they read `std::chrono::steady_clock`, a C++ standard-library clock whose
/// specific underlying syscall is implementation-defined and not safe to
/// silently swap for a Rust equivalent). Returns `0.0` if the clock is
/// unavailable, matching the original.
pub fn process_seconds() -> f64 {
    let mut process_time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `process_time` is a valid writable `timespec`.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut process_time) };
    if status == 0 {
        process_time.tv_sec as f64 + process_time.tv_nsec as f64 / 1_000_000_000.0
    } else {
        0.0
    }
}

/// `std::chrono::steady_clock::now()`'s exact underlying call, confirmed by
/// disassembling this toolchain's system `libc++.1.dylib`:
/// `clock_gettime(CLOCK_MONOTONIC_RAW, &ts)` (Darwin's raw hardware
/// monotonic clock, immune to NTP slewing), converted to nanoseconds as
/// `ts.tv_sec * 1_000_000_000 + ts.tv_nsec`. libstdc++ on Linux uses the
/// ordinary `CLOCK_MONOTONIC` for the same clock. A nonzero return here
/// matches libc++'s own "this must never happen on a supported platform"
/// assumption -- it throws `std::system_error`, uncaught at this call site
/// (an effective abort), so this aborts too instead of returning a bogus
/// time.
fn steady_now_ns() -> i64 {
    #[cfg(target_os = "macos")]
    const CLOCK_ID: libc::clockid_t = libc::CLOCK_MONOTONIC_RAW;
    #[cfg(not(target_os = "macos"))]
    const CLOCK_ID: libc::clockid_t = libc::CLOCK_MONOTONIC;

    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid writable `timespec`.
    let status = unsafe { libc::clock_gettime(CLOCK_ID, &mut ts) };
    if status != 0 {
        std::process::abort();
    }
    ts.tv_sec as i64 * 1_000_000_000 + ts.tv_nsec as i64
}

/// The first-call timestamp libc++'s function-local static start captured,
/// shared by `appSeconds`/`appSecondsSlow`/`appCycles` exactly as the
/// original's single anonymous-namespace `GetSteadyStart()` was (one shared
/// start across all three, lazily initialized on first use).
static STEADY_START_NS: std::sync::LazyLock<i64> = std::sync::LazyLock::new(steady_now_ns);

fn steady_elapsed_ns() -> i64 {
    steady_now_ns() - *STEADY_START_NS
}

/// `appSecondsSlow`/`appSeconds`: `GetSteadySeconds()`'s conversion from
/// elapsed nanoseconds to a `double` of seconds. The built object's
/// disassembly shows a real `fdiv`-by-1e9, not a multiply by a reciprocal;
/// replicated here as a literal division to stay bit-exact. Returns the raw
/// seconds value; the caller wraps it in an `FTime` the same way
/// `appProcessSeconds` already does.
pub fn seconds_slow() -> f64 {
    (steady_elapsed_ns() as f64) / 1_000_000_000.0
}

/// Native `appSeconds()` after its exact eight-byte FTime construction.
#[inline]
pub fn seconds() -> i64 {
    crate::time::from_double(seconds_slow())
}

/// Native `appProcessSeconds()` after its exact FTime construction.
#[inline]
pub fn process_time() -> i64 {
    crate::time::from_double(process_seconds())
}

/// `appCycles`: elapsed nanoseconds since the shared start, floored at 0 for
/// a (unreachable in practice) negative delta, then truncated to the low 32
/// bits -- the original's documented 32-bit wraparound.
pub fn cycles() -> u32 {
    let elapsed = steady_elapsed_ns();
    if elapsed < 0 { 0 } else { elapsed as u32 }
}

/// `appPlatformPreInit`'s `GSecondsPerCycle = (FLOAT)(period::num/period::den)`.
/// `steady_clock::period` is `std::nano` (`ratio<1,1'000'000'000>`) on both
/// libc++ and libstdc++, so this constant needs no platform `cfg`.
pub fn seconds_per_cycle() -> f32 {
    (1.0_f64 / 1_000_000_000.0_f64) as f32
}

/// `GetFileAgeDays`'s encode/normalize/`stat`/`difftime`/`appRound` steps
/// (`Core/Src/UnUnix.cpp`, used by the file-cache cleaning pass). Returns 0
/// if `stat` fails, matching the original.
///
/// # Safety
/// `filename` must be a readable NUL-terminated TCHAR string. `services`
/// must satisfy its documented leaf contract.
pub unsafe fn file_age_days(
    services: &Hp2UnixTextServices,
    filename: *const Tchar,
) -> Result<i32, i32> {
    // SAFETY: `filename`/`services` are valid by contract.
    let filename_utf8 = unsafe { unix_text::to_utf8(services, filename) }?;
    // SAFETY: `filename_utf8` is a fresh NUL-terminated native allocation.
    unsafe { unix_text::normalize_path(filename_utf8) };
    let mut file_info: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `filename_utf8` is NUL-terminated; `file_info` is a valid out-param.
    let stat_result = unsafe { libc::stat(filename_utf8.cast(), &mut file_info) };
    // SAFETY: `filename_utf8` came from the native system allocator.
    unsafe { libc::free(filename_utf8.cast()) };
    if stat_result != 0 {
        return Ok(0);
    }
    let mut current_time: libc::time_t = 0;
    // SAFETY: a valid writable `time_t` out-param.
    unsafe { libc::time(&mut current_time) };
    // SAFETY: a plain `difftime` call on two already-valid `time_t` values.
    let diff_seconds = unsafe { libc::difftime(current_time, file_info.st_mtime) } as f32;
    // `appRound` truncates toward zero (see `sleep`'s comment above).
    Ok((diff_seconds / 60.0f32 / 60.0f32 / 24.0f32) as i32)
}
