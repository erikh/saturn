# saturn: a calendar for CLI nerds

This is all still very new and should be treated as such. It is strongly
suggested that if you like where this is heading, that you come to the issues
list and voice your ideas and concerns.

Saturn provides you with a CLI interface to calendaring much in the way
[taskwarrior](https://github.com/GothenburgBitFactory/taskwarrior) does with
tasks. It also provides you with several methods to query and notify yourself
of important appointments. It can act standalone or integrate fully with Google Calendar.

[Here](https://asciinema.org/a/XkRCXcgucQCRYassutGLMlWqq) is a demo of it in action.

## Installation

There is currently no crate, please use the `--git` method of installing crates
with cargo:

```
cargo install --git https://github.com/erikh/saturn
```

## Entry language

Entry language is basically:

```
ENTRY = [ "recur" <duration> ] <date> <AT | SCHEDULED | ALL DAY> ["notify" <duration>] <detail>
AT = at <time>
SCHEDULED = from <time> to <time>
ALL DAY = all day
```

You trigger it by using `saturn entry`:

```
saturn entry tomorrow at 8pm Take a Shower
```

This will schedule a shower tomorrow at 8pm with a notification at the time of
the appointment. You can also use `saturn e`.

### Formats

There are numerous formats that can be used for different times, dates, and
durations. Localization is desired but I haven't found a good set of tools for
doing it yet.

#### Dates

Dates can be represented a number of ways:

-   `today`, `tomorrow`, and `yesterday` are case-insensitive and have their
    traditional relative meanings.
-   A day (integer) by itself will assume the current month and year.
-   `month/day` (e.g. 8/7) will assume the current year.
-   `year/month/day` (e.g. 2023/8/7) will represent a full date.
-   The following characters can be used as date separators: `/`, `-`, and `.`.

#### Times

-   `hour:minute:second` represents a full time. You may also use `.` for the separators.
-   `hour:minute` when less than 13 represents the time in relationship to the
    current 12-hour clock. 13 and above are 24-hour time.
-   `hour:minute[pm|am]` represents the current 12 hour time with appropriate time of day designation.
-   `hour[pm|am]` represents the top of the hour in 12 hour time with the appropriate time of day designation.
-   `hour` represents the top of the hour in 12 hour time with the current time of day designation.

#### Durations

All duration rules take from the [fancy-duration](https://github.com/erikh/fancy-duration) crate.

Durations are combined in order of precedence with single character
designations for each unit. Example: `2h15m12s`, is "2 hours, 15 minutes, and
12 seconds".

-   `s`: seconds
-   `m`: minutes
-   `h`: hours
-   `d`: days
-   `w`: weeks
-   `m` (leading position only): months
-   `y`: years

## Querying

```
saturn list [--all]
```

Will list the database for today, or if `--all` is passed, will list the entire
db. Note that `saturn today` and `saturn t`, and `saturn l` are synonyms for
`saturn list`.

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

Will display a notification to the screen for every item that must be addressed
immediately. `--well` is similar to `now`'s functionality, and `--timeout`
configures how long to keep the notification up on the screen.

This is what a notification looks like in `dunst`, which the notification
system for `i3`. GNOME, KDE, MacOS, etc will look different, but have the
same text.

<img style="width:50%" src="notification.png" />

```
saturn delete <id>
```

Will delete a calendar record by ID, which is listed with the listing tools.

```
saturn complete <id>
```

Will mark a task as "completed". Completed tasks get a visual notification and
are automatically excluded from listing without the `--all` flag.

## Database & Configuration File

Saturn keeps a CBOR database in `~/.saturn.db`. Locking is flock(2), and quite
primitive. Suggestions and patches welcome.

The configuration file is only required in limited scenarios (such as remote
calendar support) and exists in `~/.saturn.conf`. It is a plain YAML file, but
is typically manipulated by `saturn config` commands, which may replace any
comments or other manipulations you previously did to the file by hand.

## Leveraging the well features with a periodic scheduler

The `--well` options take a duration. This duration is intended to roughly
match the frequency at which you run the program, so that there is little to no
overlap between event firings. This flag is provided for `saturn now` and
`saturn notify`.

Notifications (specified by a `notify` entry stanza) are only fired once in any
event. Events, on the other hand, are shown every time they fall into the
window, which is the current time, +/- the `--well` duration.

I hope this clears things up; I was trying to figure out a good way to run this
in `cron` etc without spamming myself with notifications for a long period of
time.

Here's an example: we run a loop of `saturn notify` with a well of two minutes,
and then we sleep for a minute. This allows notify to catch the alert only
once, passing it up by the next time it runs.

```bash
while true
do
    saturn notify --well 2m
    sleep 60
done
```

## Recurring tasks

Recurring tasks start their entry with the "recur" keyword and a duration.
Every time the program is run and touches the database, it will look to add
recurring tasks. Recurring tasks are based off the last task that was saved,
and every recurrence up to the current point will be added in the absence of
them. Until they are added, they will not have IDs nor can they be manipulated.
Commands like `now` and `notify` which only perform read operations also adjust
this data, so they can fire notifications properly for new tasks.

## Google Calendar Support

Google Calendar support is working, with OAuth credentials being
setup properly and limited control of the calendar is possible within the realm
of what saturn currently supports. More is anticipated to be built atop this
framework. Do not be surprised if functionality is confusing or missing. Please
put in issues with your concerns, thanks!

To use `saturn` with Google Calendar, you must create a Google Cloud account
and assign an OAuth application to it. One is not provided automatically by
using `saturn` to eliminate concerns of data provenance.

To do this, follow [these
steps](https://developers.google.com/calendar/api/quickstart/go), which go into
how to set up an application for development. Be sure to setup any accounts you
want to use as "Test Users", and ensure that
`https://www.googleapis.com/auth/calendar` is in your list of allowed scopes.
Once you have the "Client ID" and "Client Secret", run this command:

```bash
saturn config set-client <client id> <client secret>
saturn config get-token
saturn config db-type google
```

The `get-token` command will have you access a URL in your browser and make you
login to the google account you wish to use, which must be listed in your
"testing users" in the OAuth setup above. As a final step, it will call back
into a web service the application starts, which will feed it the token.

Your token will expire if you do not use the tool regularly. Stuffing `saturn
notify` in cron will alleviate this a bit.

Setting the db-type will change the source of data. If you were using a local
database and want to go back to it, `saturn config db-type unixfile`.

Notifications setup in Google Calendar are not honored yet. This will be
resolved soon!

Other things we want to do that aren't here yet:

-   Fields (URLs, Locations, etc)
-   Attendees

## Target Platform

For the unixfile DB type, Due to flock(2) use, which to the best of my
knowledge is the only reason, Windows probably does not work properly. Patches
welcome if there are windows users who'd like to use it.

## Author

Erik Hollensbe <erik+github@hollensbe.org>
