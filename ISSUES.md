# List of saturn's issues

## CBOR database

The CBOR database is really effective for maintaining small numbers of records, but for large volumes of recurring tasks, the cost to write the db (which has to be done entirely for every write) is very expensive. Reading it also gets expensive for large databases, sometimes to the point where my timing loops expect it to be done and start operating on the data, which is also an issue.

## Database Backends

I primarily use it with google calendar. There are tests for both database models, but I am worried the memory database implementation is poorly tested.

Google Calendar is also very slow. This is because of a number of issues:

### Too many requests for a single operation

There are far too many requests for a single operation, this is largely impacted by the fact that we refresh the DB after every write.

### Many requests are needlessly made

This is different than the above; we make lots of requests at the time of execution instead of just consulting the database, which makes the database somewhat pointless, but also ends up with large pockets of response latency against user queries. A mechanism that used TTLs to better manage when a read request in particular needs to be made would be better.

## Code Smells

These are a few big issues that are impacting the whole codebase.

### macro use stinks

The macros are used to unify `sui` and `saturn` operations in a lot of situations, as well as get around some typing issues. There are better solutions for this, and this is just a case of it creeping up and growing on me before I've had an opportunity to resolve it. The downsides are that it tends to make the compiler and rust-analyzer a little confused, and it's also probably generating much larger code. A traits system, maybe with enums in the right places, would be better here.

### tui + async

This is a big problem that I don't know how to fix yet. TUI toolkits in rust are all expecting synchronous/threaded code. I have a function called `sit` which is a very bad behaving function; starting a whole tokio reactor just to run a function and clean it all up, just for the purposes of running async code in a sync environment. Not only is this probably hideously expensive and unnecessary, it's a huge wart and it's used everywhere. A good solution would either be a async TUI toolkit or a solution that separates the concerns of the async and sync code.

## Design

-   Saturn needs coloring on the current time's tasks in lists
-   sui needs a new layout. I hate it and a lot of the screen is wasted.
