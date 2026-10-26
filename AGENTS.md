# TRP (Tushonka Runtime Project): the brief for an agent working in the public copy

Escape from Tarkov's first-person character controller, rebuilt in Rust on Bevy as a reusable
runtime; STALKER GAMMA is its first intended host. The workspace is `TRP\`, its four crates under
`TRP\crates\`: `trp-data` reads the install, `trp-sim` is the controller with no engine types,
`trp-physics` the game's own PhysX 4.1, `trp-app` the Bevy app (`trp.exe`). It reads its data at
runtime from the person's own Single Player Tushonka (SPT) install; the repository holds code,
documents and the figures of the app's own weapons and outfits (`TRP\custom\`), nothing of a game.
"Exact" means measured against the real game, not judged by feel.

**This is the public copy.** The maintainer's private repository also holds their working brief
and agent skills (`.claude\`), the recorder that made the recordings of the game (`hosts\`), the
recordings (`local\traces`), the log (`history.md`), the scratch tools (`workbench.md`) and the
decisions (`decisions.md`): a pointer to any of these names that repository. So a claim of
"compared with the game" cannot be run again here; what can be run is the app, its tests and its
tools, against the person's own install. The words the documents use, who "the user" is, and what
can be verified here: `TRP\README.md`, "Reading this repository".

## Rules that hold for any change

1. **Research before code.** Prefer, in order: documented format, published source, decompilation.
   Before touching a part, read its spec in `TRP\docs\spec\` (the section, never the whole file).
2. **The installs are inputs.** Never write into SPT's, Anomaly's or GAMMA's folders.
3. **No game content in the repository.** No assets, extracted files, decompiler output or pasted
   game code. Scratch goes in `TRP\local\` (ignored by git; make the folder).
4. **Spec, then implement.** Behaviour read from the game goes into `docs\spec\` in our own words,
   then original code is written from that. The README's "What was learned, and how" must stay true.
5. **Every constant has a source:** the install, or the spec. A guess is marked `ESTIMATE`, an
   unconfirmed rule `PROVISIONAL`; both are listed in `docs\status.md`, "Placeholders, marked in
   the code".
6. **Claims are measured.** "Matches Tarkov" needs a recording of the game beside the simulation.
   Without one, say what was read from code, what was run, and what was not.
7. **Ask the person before** launching a game or installing a toolchain. Running the app, its tests
   and its tools is fine.
8. **Do not depart from Tarkov.** Where the game's code or a recording says what the game does, do
   that, even where it looks worse. The only liberty is how results are drawn between and across
   frames, and that must leave the simulation's results unchanged. A departure on purpose (the
   features' EXPERIMENTAL list) is the maintainer's call: ask first.
9. **Every system has a switch** among the options' features ("restart required"): set to off, the
   system does nothing at all. A new one follows the existing ones in `trp-app`'s
   `options_view.rs` (`Features`); the rules: `docs\spec\settings.md`, "Features: what is switched".

## Working here

Run from `TRP\`. The install's path comes from `EFT_DIR` or `local\eft_dir.txt`; the app asks for
it when run from a console without either (`Install.bat` runs that question).

```
cargo build --workspace                                cargo test --workspace
cargo run -p trp-app                                   (keys and options: docs\manual.md)
cargo run -p trp-data --bin probe -- states 61a4c8884f95bc3b2c5dc96f reload
```

- **The documents are long** (`findings.md` 460 KB, `status.md` 220 KB, a spec up to 215 KB; each
  line a paragraph, so a few lines are many tokens): never read one whole. List its headings first
  (the lines that begin with `#`), then read the one section, by its line numbers. A source file
  over 2,000 lines may have `// ==== ... ====` lines for its sections: list them first.
  `findings.md`'s dated sections are newest last.
- **Before taking a figure or a rule from the game**, look in the install with the probe (`cargo
  run -p trp-data --bin probe`; `manual.md`, "Inspecting the install") rather than guess.
- **The tests**: the unit tests need nothing; those in the `tests\` folders of `trp-data`,
  `trp-sim` and `trp-physics` run against the configured install and pass with a note when there
  is none; a few need recordings that are not published, and pass without them.
- **After a change to `trp-sim` or `trp-physics`**, the maintainer's regression over the kept
  recordings cannot be run here: say so in what you report, and say what you did check.
- **Keep each file's line ends** as you find them. With PowerShell 5.1: no `2>&1` on cargo
  (redirect `*>` to a file); no `Set-Content` for a source file (it adds a byte-order mark). Bevy
  is pinned to 0.18, `physx-sys` to 0.8.2 (PhysX 4.1).
- **The version** is one number for the whole of TRP, `version` under `[workspace.metadata.trp]`
  in `TRP\Cargo.toml`, below 1.0, raised with every change the maintainer lands (`manual.md`,
  "Versions"); a contribution leaves the number alone and says what it changes for a player in
  `PATCH_NOTES.md`, short and plain.
- **Licence**: Apache-2.0 (`LICENSE`, at the top) for the code and the documents alike: copy either
  with the notice. It grants nothing in the games this reads.

## Where the rest is (under `TRP\`)

| For | Read |
|---|---|
| What is built, how to run it, the layout, how the knowledge came; the project's words | `README.md` |
| What each step was compared with ("The account"); every placeholder | `docs\status.md` |
| What is missing, by feature | `docs\backlog.md` |
| Every key, option and tool; looking into the install; recording and comparing | `docs\manual.md` |
| How the game behaves, part by part, and what each part was compared with | `docs\spec\*.md` |
| What the install, the code and the recordings have shown; traps | `docs\findings.md` |
| What changed in each release, for a player | `..\PATCH_NOTES.md` |
