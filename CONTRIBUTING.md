# Contributing to Sonix Studio

Contributions are welcome — code, bug reports, translations, plugin adapters and presets. Sonix
is one person's project that grew, and it gets better when more people touch it.

## The short version

1. Open an issue first for anything larger than a bug fix. A short description of what you want
   to change saves a week of work being thrown away.
2. Fork, branch, and make one change per branch.
3. `cargo test --locked --bin sonix` must be green, and `cargo build --release --locked` must
   produce **zero warnings** — the build gate fails on a single one.
4. Sign your commit: `git commit -s`. The `Signed-off-by` line is the Developer Certificate of
   Origin (https://developercertificate.org), and it is the only paperwork asked for.
5. Send the pull request with what it changes and how you measured it.

## Licence of your contribution

Sonix Studio is licensed **GPL-3.0-only** (see [LICENSE](LICENSE)). By signing off a commit you
confirm you have the right to submit the code, and that it may be distributed under that licence.
No copyright assignment, no contributor licence agreement — your contribution stays yours, under
the same licence as the rest of the program.

The practical consequence of GPL-3.0, in one line: **anyone may build, use, change and share
Sonix, and anyone who distributes a changed Sonix must publish the source on the same terms.**
Nobody can take it, close it and sell the closed result.

## Plugins and extensions

The plugin host loads CLAP, VST3 and VST2 modules as **separate binaries at runtime**, over
interfaces that ship with the plugin formats — not by linking them into Sonix. A plugin you write
is therefore its own work: you may license it however you like, including commercially and
closed-source. The program's licence does not reach your plugin. (This is the same reading that
Ardour and LMMS rely on; for a commercial plugin deal, confirm with your own lawyer rather than
taking my word for it.)

What is welcome in this repository:

- Host-side improvements: the plugin scanner, the CLAP/VST3 wrapper, MIDI/MPE routing.
- New internal effects and instruments — see `src/audio/` and `PLUGINS.md`.
- Presets and sample-kit definitions, if they are yours to license.

## Code conventions

- **Code in English**: identifiers, comments, log lines, error messages, test names.
- Comments explain *why*, one line, where the code cannot show it.
- A new visible string needs an i18n key in all six languages (`src/i18n.rs`); the key *is* the
  Swedish text.
- New non-trivial logic (a branch, a loop, a parser) gets one small test. The suite is
  `cargo test --locked --bin sonix`.
- Never write over a channel/region that already has notes without saying so, and never touch the
  master volume from an automated path.

## Reporting bugs

Include: what you did, what happened, what you expected, and the output of
`sonix --selftest` if the problem is audio. A screenshot of the wrong state is worth ten
sentences.

## Branding

The code is GPL, the *name* is not. A fork is welcome — it must carry its own name and icon, so
users can tell the two apart, and it must not present itself as "Sonix Studio" or as the official
build. That is a trademark matter, separate from the licence, and it is not a registration claim.
