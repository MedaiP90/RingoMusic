# General behavior

- All the code must be written using English for comments, variable names, functions, logs.
- Write meaningful comments on methods and classes but do not be too verbose (use `/simplified-technical-english`).
- Ask me whenever the situation involves a key implementation decision.
- DO NOT commit code by yourself in any case (unless explicitly authorized).
- Always update the `<description>` of the newest `<release>` in `data/io.github.MedaiP90.RingoMusic.metainfo.xml` so it holds the right release notes for the changes. The release workflow publishes this text (`scripts/release-notes.py`). Use only `<p>` and `<ul><li>` elements.
- Run `po/update-pot.sh` whenever you add or change a translatable string: `gettext()` in Rust code, or text in the metainfo or desktop file (release notes too). Commit the updated `po/ringomusic.pot` with the change. Add new Rust files with translatable strings to `po/POTFILES.in`.

# Plugins

Always use this plugins for efficiency:

- `/context-mode`
- `/caveman ultra`
- `/ponytail`

# Skills

When writing code use always this two skills:

- `/clean-code`
- `/oop-principles`
- `/simplified-technical-english` (comments and docs)

And load this ones when necessary:

- `/node-best-practices`: when writing NodeJS code
- `/javascript-best-practices`: when writing pure JavaScript code
- `/rust-best-practices` ( @./plan/skills/rust-best-practices.md ): when writing Rust code
- `/gtk-apps` ( @./plan/skills/gtk-apps.md ): when writing GTK4 code
