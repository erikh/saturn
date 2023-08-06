# saturn: a calendar for CLI nerds

This is all still very new and should be treated as such.

saturn provides you with a CLI interface to calendaring much in the way [timewarrior](https://github.com/GothenburgBitFactory/timewarrior) does with tasks. It also provides you with several methods to query and notify yourself of important appointments.

[Here](https://asciinema.org/a/XkRCXcgucQCRYassutGLMlWqq) is a demo of it in action.

## Entry language

Entry language is basically:

```
ENTRY = <date> <AT | SCHEDULED> ["notify" <duration>] <detail>
AT = at <time>
SCHEDULED = from <time> to <time>
```

You trigger it by using `saturn entry`:

```
saturn entry tomorrow at 8pm Take a Shower
```

This will schedule a shower tomorrow at 8pm with a notification at the time of
the appointment.

## Querying

```
saturn list [--all]
```

Will list the database for today, or if `--all` is passed, will list the entire
db. Note that `saturn today` and `saturn t` are synonyms for `saturn list`.

```
saturn now [--well=<duration>]
```

Will list the items that need to be addressed immediately. To configure how
much of a time to wrap around what "now" means, use the `--well` option.
Durations are specified in
[fancy-duration](https://github.com/erikh/fancy-duration) format.

`saturn n` is an alias for `saturn now`.

Likewise,

```
saturn notify [--well=<duration>] [--timeout=<duration>]
```

This is what a notification looks like in `dunst`, which the notification
system for `i3`. GNOME, KDE, MacOS, Windows will look different, but have the
same text.

<center><img src="notification.png" /></center>

Will display a notification to the screen for every item that must be addressed
immediately. `--well` is similar to `now`'s functionality, and `--timeout`
configures how long to keep the notification up on the screen.

```
saturn delete <id>
```

Will delete a calendar record by ID, which is listed with the listing tools.

## Database

Saturn keeps a CBOR database in ~/.saturn.db. Locking is not in use and you've
been warned. We'll get there.

## Future Plans

-   google calendar support (maybe proton too)
-   tasks maybe

## Author

Erik Hollensbe <erik+github@hollensbe.org>
