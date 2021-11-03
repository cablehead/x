
## Target Usage

- Tentative

```
$ x stream <sock> broadcast  # currently tcp / spread
$ x stream <sock> split      # TBD
$ x stream <sock> merge      # currently tcp / merge

$ x wal ./path

$ x exec -- <command> <args>...
```

## To test

- [ ] human friendly message when unable to bind to desired port
- [ ] `x --port 2000 merge | x --port 2001 spread` works OK
