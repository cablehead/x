
## Target Usage

Tentative

```
$ x stream <sock> broadcast  // currently tcp / spread
$ x stream <sock> split      // TBD
$ x stream <sock> merge      // currently tcp / merge
$ x stream <sock> http       // TBD

<sock> is tcp:<[host:]port> or unix:<path> // TBD

$ x log ./path write
$ x log ./path read

$ x exec -- <command> <args>...
```

## To test

- [ ] human friendly message when unable to bind to desired port
- [ ] `x --port 2000 merge | x --port 2001 spread` works OK
