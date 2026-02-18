# Changelog

All notable changes to this project will be documented in this file.

## [unreleased]

## [0.11.1](https://github.com/ratatui/bevy_ratatui/compare/v0.11.0...v0.11.1) - 2026-02-18

### Other

- remove default crossterm dependency

## [0.11.0](https://github.com/ratatui/bevy_ratatui/compare/v0.10.0...v0.11.0) - 2025-11-12

### Other

- migration to bevy 0.18

## [0.10.0](https://github.com/ratatui/bevy_ratatui/compare/v0.9.3...v0.10.0) - 2025-11-12

### Other

- migration to bevy 0.17

## [0.9.3](https://github.com/ratatui/bevy_ratatui/compare/v0.9.2...v0.9.3) - 2025-10-06

### Other

- Bump tracing-subscriber from 0.3.19 to 0.3.20 ([#66](https://github.com/ratatui/bevy_ratatui/pull/66))
- bump soft_ratatui ([#68](https://github.com/ratatui/bevy_ratatui/pull/68))

## [0.9.2](https://github.com/ratatui/bevy_ratatui/compare/v0.9.1...v0.9.2) - 2025-05-27

### Other

- remove direct dependency on crossterm

## [0.9.1](https://github.com/ratatui/bevy_ratatui/compare/v0.9.0...v0.9.1) - 2025-05-26

### Other

- new feature for toggling crossterm context

## [0.9.0](https://github.com/ratatui/bevy_ratatui/compare/v0.8.3...v0.9.0) - 2025-05-13

### Fixed

- fixed workflows

### Other

- miscellaneous docs updates
- public facing module structure
- miscellaneous fixes for examples
- refactored for encapsulation of context type
- miscellaneous improvements and addressed comments
- rename window feature from 'soft' to 'windowed'
- integrate soft_ratatui for a windowed output mode

### Removed

- removed some cfg feature gates

## [0.8.3](https://github.com/ratatui/bevy_ratatui/compare/v0.8.2...v0.8.3) - 2025-04-27

### Other

- make control_c_interrupt public

## [0.8.2](https://github.com/ratatui/bevy_ratatui/compare/v0.8.1...v0.8.2) - 2025-04-27

### Other

- actually remove ctrl-c behavior from passthrough

## [0.8.1](https://github.com/ratatui/bevy_ratatui/compare/v0.8.0...v0.8.1) - 2025-04-27

### Fixed

- fixed issue when bevy/bevy_winit feature enabled

## [0.8.0](https://github.com/ratatui/bevy_ratatui/compare/v0.7.1...v0.8.0) - 2025-04-26

### Added

- Add passthrough `serde` feature

### Other

- make ctrl+c handling optional
- cargo fmt
- cargo fix --edition
- migration to bevy 0.16

## [0.7.1](https://github.com/ratatui/bevy_ratatui/compare/v0.7.0...v0.7.1) - 2025-03-21

### Other

- cleaned up README, examples
- transer ownership to cxreiff ([#39](https://github.com/ratatui/bevy_ratatui/pull/39))
- Bump the cargo-dependencies group across 1 directory with 4 updates ([#34](https://github.com/ratatui/bevy_ratatui/pull/34))

## [0.6.4](https://github.com/ratatui/bevy_ratatui/compare/v0.6.3...v0.6.4) - 2024-10-22

### Other

- Bump the cargo-dependencies group across 1 directory with 3 updates ([#24](https://github.com/ratatui/bevy_ratatui/pull/24))

## [0.6.3](https://github.com/ratatui/bevy_ratatui/compare/v0.6.2...v0.6.3) - 2024-08-27

### Other
- Bump ratatui from 0.28.0 to 0.28.1 in the cargo-dependencies group ([#21](https://github.com/ratatui/bevy_ratatui/pull/21))

## [0.6.2](https://github.com/ratatui/bevy_ratatui/compare/v0.6.1...v0.6.2) - 2024-08-13

### Other
- Bump ratatui from 0.27.0 to 0.28.0 in the cargo-dependencies group ([#20](https://github.com/ratatui/bevy_ratatui/pull/20))
- Bump the cargo-dependencies group with 2 updates ([#18](https://github.com/ratatui/bevy_ratatui/pull/18))

## [0.6.1](https://github.com/ratatui/bevy_ratatui/compare/v0.6.0...v0.6.1) - 2024-07-23

### Added
- Emit bevy_input keyboard events ([#13](https://github.com/ratatui/bevy_ratatui/pull/13))

### Other
- Removed dynamic_linking feature ([#17](https://github.com/ratatui/bevy_ratatui/pull/17))
- Pass crossterm key event to bevy key handling.

## [0.5.3](https://github.com/ratatui/bevy_ratatui/compare/v0.5.2...v0.5.3) - 2024-07-05

### Other
- Migrate to Bevy 0.14 ([#10](https://github.com/ratatui/bevy_ratatui/pull/10))

## [0.5.2](https://github.com/ratatui/bevy_ratatui/compare/v0.5.1...v0.5.2) - 2024-06-25

### Other
- Bump ratatui from 0.26.3 to 0.27.0 in the cargo-dependencies group ([#8](https://github.com/ratatui/bevy_ratatui/pull/8))

## [0.5.1](https://github.com/ratatui/bevy_ratatui/compare/v0.5.0...v0.5.1) - 2024-06-07

### Other
- Ensure kitty setup runs after terminal setup ([#6](https://github.com/ratatui/bevy_ratatui/pull/6))

## [0.5.0](https://github.com/ratatui/bevy_ratatui/compare/v0.4.0...v0.5.0) - 2024-06-05

### Added
- Add rest of crossterm events.

### Other
- Move mouse to separate plugin.
- Make kitty protocol configurable.
- Added kitty protocol support. ([#5](https://github.com/ratatui/bevy_ratatui/pull/5))
- Add mouse event example.
- Add more docs.

## [0.4.0](https://github.com/ratatui/bevy_ratatui/compare/v0.3.0...v0.4.0) - 2024-06-01

### Other
- Add docs and hello world example ([#2](https://github.com/ratatui/bevy_ratatui/pull/2))

- [3e7b6818](https://github.com/joshka/ratatui_bevy/commit/3e7b68186b896b8ddf5ab1a533e511ef8010a791): Initial implementation
- [f224c58a](https://github.com/joshka/ratatui_bevy/commit/f224c58a6d90807c51153a86ed03e60919d68f8f): Rearrange to modules
- [2e5a68fa](https://github.com/joshka/ratatui_bevy/commit/2e5a68fa45a09c46c1974f15d4c3ba0caaa7be2e): Add plugin per module
- [31bd387b](https://github.com/joshka/ratatui_bevy/commit/31bd387b75b224a0621a5d402acd5b5ff2fd5e29): Formatting
- [97004166](https://github.com/joshka/ratatui_bevy/commit/970041663ab9b09669fd57a3bc8c8276d1b48c04): Demonstrate how bevy makes changing state elsewhere in the app easy

  Some context: https://forum.ratatui.rs/t/how-do-i-represent-application-state-ergonomically/54/13
- [e35958a5](https://github.com/joshka/ratatui_bevy/commit/e35958a5ce47624b9e14255e62ba4c55ea2aaf44): Add readme
- [0c789e27](https://github.com/joshka/ratatui_bevy/commit/0c789e27084440329a23513bc3e778f8901faa94): Disable bevy default features for speeding up compile times
- [97bf55f6](https://github.com/joshka/ratatui_bevy/commit/97bf55f6000fa13efaea23da8826469ddca2a8c6): Create LICENSE
- [84d7155c](https://github.com/joshka/ratatui_bevy/commit/84d7155c7498fdd6ef785edc8487fc45b603ea8b): Add keys / instructions to readme
- [85d60840](https://github.com/joshka/ratatui_bevy/commit/85d608402ce58654e1f48fcbd8cb09def3358334): Animate the background color when changing from positive to negative
- [f3bc4c7c](https://github.com/joshka/ratatui_bevy/commit/f3bc4c7c4d9a21a83207b57545920ac3be78f24f): Add demo and tweak colors / timing
- [83603edf](https://github.com/joshka/ratatui_bevy/commit/83603edf13c5fe3e648835c9745d2bfd527b593f): Add readme info<!-- generated by git-cliff -->
