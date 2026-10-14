# TRP (Tushonka Runtime Project)

A Rust rewrite, on Bevy, of the first-person character controller of the game Single Player Tushonka (SPT) runs on. It reads everything it shows from your own SPT install each time it starts: no files of any game are in this repository.

**The code, how to build and run it, what works, and what was learned and how: [`TRP/README.md`](TRP/README.md).**

## Requirements

- **Windows 10 / 11**
- **Escape From Tarkov** *(required)*: you must own your own copy of the game
- **The Forge (Single Player Tushonka)** *(required)*: [Download installer](https://sp-mod.com/installer)
- **S.T.A.L.K.E.R. Anomaly / GAMMA** *(optional)*

Quick start:

```
git clone https://github.com/JuanFNQ/TRP.git
cd TRP\TRP
cargo run -p trp-app
```

The first start asks in the console where your SPT folder is, and, if you have them, S.T.A.L.K.E.R. Anomaly's and GAMMA's.

`AGENTS.md` is the working brief of the AI sessions this was written with: its rules, the user's decisions and the state of the work.

An unofficial fan project, not affiliated with or endorsed by Battlestate Games, GSC Game World, or the teams of Single Player Tushonka, Anomaly or GAMMA.
