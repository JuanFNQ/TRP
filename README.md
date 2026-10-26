# TRP (Tushonka Runtime Project)

A Rust rewrite, on Bevy, of the first-person character controller of the game Single Player Tushonka (SPT) runs on. It reads everything it shows from your own SPT install each time it starts: no files of any game are in this repository.

**The code, what works, and what was learned and how: [`TRP/README.md`](TRP/README.md). What changed in each release: [`PATCH_NOTES.md`](PATCH_NOTES.md).**

## Requirements

- **Windows 10 / 11**
- **Escape From Tarkov** *(required)*: you must own your own copy of the game
- **The Forge (Single Player Tushonka)** *(required)*: [Download installer](https://sp-mod.com/installer)
- **S.T.A.L.K.E.R. Anomaly / GAMMA** *(optional)*

## Install and play

1. Download TRP: on GitHub, **Code > Download ZIP** (or the latest release's source code), and unzip it. Or `git clone https://github.com/JuanFNQ/TRP.git`.
2. Double-click **`Install.bat`**, once. It checks for the C++ build tools and Rust and offers to install what is missing, builds TRP (several minutes the first time; about 10 GB on disk in all), and asks where each of your games' folders is (SPT, then GAMMA and Anomaly if you have them).
3. Double-click **`Play.bat`** to play. After an update it rebuilds by itself first.

On the range, the crate straight ahead holds a rifle, a vest and a backpack; the tables along the low wall hold every weapon of the install. `F1` shows the keys.

The maintainer's working brief, the day-by-day log and the recording plugin stay in the private repository; the public one holds what building and running the app needs, its documentation, and a short brief for a coding agent (`AGENTS.md`; the words the documents use: `TRP/README.md`, "Reading this repository").

An unofficial fan project, not affiliated with or endorsed by Battlestate Games, GSC Game World, or the teams of Single Player Tushonka, Anomaly or GAMMA.
