//! Rust port of `HarryPotter2/Unreal/SDLLaunch/Src/HP2Launch*.cpp`'s
//! pure logic (policy, store, model) -- excludes `HP2Launcher.cpp`'s
//! own SDL/ImGui UI loop, which stays C++ per the plan.
//!
//! **Known dead code in the real C++, not ported:**
//! `HP2LaunchPolicy.h` declares `ParseLauncherAutomationRequest` and
//! `BuildLauncherAutomationResult` (and the `LauncherAutomationRequest`/
//! `LauncherAutomationParseResult` types supporting them), but neither
//! function has a definition anywhere in this repository, and neither
//! the functions nor the types they use are referenced from any other
//! `.cpp`/`.mm` file (confirmed via `grep -rln` across the whole repo).
//! This is vestigial -- declared, never implemented, never called --
//! not a gap in this port.

pub mod model;
pub mod policy;
