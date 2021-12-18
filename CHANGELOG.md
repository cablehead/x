
# 2021-12-18: v0.0.4

## exec -> v0.0.4

- improve handling when the child process terminates for our stdin closes
- avoid spawning a final child process when there is no more stdin
- make `-l` a short cut for `--max-lines`
