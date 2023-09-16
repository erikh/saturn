-   HEAD:
    -   `sui` now supports `edit <id>` syntax; use `edit recur <id>` to edit
        recurring tasks. Launches `$EDITOR` and works the same way as `saturn edit`.
    -   `saturn edit` will now edit the properties for a calendar item. Launches
        `$EDITOR` and commits changes back to the local or remote DB. Use `-r`
        for recurring tasks.
    -   `saturn dump` will now dump the properties of a calendar item. Use `-r`
        for recurring tasks.
    -   Support date endings (`th`, `rd`, `st`, etc) in entry syntax. See docs
        for more.
    -   Some style/color changes to `sui`
    -   Changed the strategy that the home directory was found; now using the
        `dirs` crate.
-   `v0.2.0`:
    -   Introduction of `sui`, a graphical TUI built with the same principles
        as `saturn`. Uses the same data and databases and configuration file,
        and works great with Google Calendar.
    -   Improvement of recurring task management with Google Calendar. Not all
        bugs are snipped here, but the big ones are.
    -   `midnight` and `noon` are now valid times in the entry language.
    -   `saturn delete` can now take multiple IDs to delete at once.
    -   `saturn notify` can now accept notification icons with the `--icon` flag.
