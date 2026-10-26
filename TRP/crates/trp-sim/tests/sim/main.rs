//! The tests of `trp-sim` that need the install or the recordings, one binary: each file here
//! was a test binary of its own until 0.100.3, twenty links at every `cargo test`. A test is
//! named by its file: `cargo test -p trp-sim --test sim install::` runs the old `install` set,
//! `--test sim vault` every test with that word. Each passes with a note when there is no install.

mod aim_sway;
mod ambient;
mod armor;
mod attributes;
mod blind_fire;
mod left_stance;
mod malfunctions;
mod mounting;
mod builds;
mod clouds;
mod every_weapon;
mod fixture;
mod free_look;
mod grenade_knife;
mod hands;
mod hbao;
mod health;
mod health_replay;
mod install;
mod kinds;
mod panels;
mod look;
mod head;
mod limits;
mod puppet;
mod room;
mod slopes;
mod sights;
mod standing;
mod steps;
mod third_person;
mod time_of_day;
mod zeroing;
mod build_swap;
mod magazine_clips;
mod no_chamber;
mod zoom_ring;
mod stimulators;
