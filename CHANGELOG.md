-   HEAD:
    - Fix an issue where all day tasks on sundays would not be shown
    -   Several style changes to `sui`:
        - Replaced underlining today's tasks with coloring them in bright
          white, using dark gray for all others.
        - Highlighting in light green tasks that occur within the next hour.
    -   Fix an issue where a bug would prevent recurring tasks from being entered into Google Calendar
-   `v0.3.6`:
    -   Fix `<command> -V` version output as well as adjust some help text for the CLI processor.
    -   Colorize outputs in `saturn list` commands.
-   `v0.3.5`:
    -   Add a configuration feature to never use the 12 hour adjustment.
    -   Only massage times in 12 hour format for today's date. Other dates time will be treated in 24h.
    -   Fixed more issues with iCal IDs. This will need to be changed more fundamentally in 0.4.0.
-   `v0.3.4`:
    -   Fix an issue where 24-hour time may not be represented properly after
        noon. Thanks to [@raphaelahrens](https://github.com/raphaelahrens) for the report.
-   `v0.3.3`:
    -   Fixed an issue where editing a task would result in a crash
-   `v0.3.2`:
    -   Fixed another issue with iCal information not be appropriately managed
        between database wipes.
-   `v0.3.1`:
    -   Fixed issue with items registered at midnight on the current Sunday not
        showing up in the list of events.
    -   Fixed issue with calendars not recording new information properly in
        Google after the local DB had been wiped.
-   `v0.3.0`:
    -   Implemented `show <id>` / `show recur <id>` for `sui` which displays task properties.
    -   Fixed alignment issues with state notifications in `sui`.
    -   Increased column widths for `sui`'s ID column in the events tab.
    -   Fixed a number of outstanding issues with recurring tasks.
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
