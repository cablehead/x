
## Target Usage

Tentative

```
$ x stream <sock> broadcast
$ x stream <sock> split      // TBD
$ x stream <sock> merge      // currently tcp / merge
$ x stream <sock> http       // TBD

<sock> is tcp:<[host:]port> or unix:<path> // TBD

$ x log ./path write
$ x log ./path read

$ x exec -- <command> <args>...
```

## Todo

### x log - read

- assert cursor is at a message boundary
- assert cursor isn't passed end of stream
- add utilities to help track cursor:
    - at least once, convenience to save the cursor while consuming stdout
    - at most once, convenience to run a command, and only advance if the command is successful

### x stream - http

- [ ] log server startup

## To test

### x stream

- [ ] human friendly message when unable to bind to desired port
- [ ] `x --port 2000 merge | x --port 2001 spread` works OK

### x stream - http

- [ ] response isn't valid JSON
- [ ] `request_id` isn't pending
- [ ] success
- [ ] should see log response for each of the above
