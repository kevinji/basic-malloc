# basic-malloc
A basic malloc implementation, based on [Dan Luu's blog post](https://danluu.com/malloc-tutorial/).

## Running on macOS
Assuming a program exists at `c/test.c`:

```bash
cc c/test.c -o c/test -Wl,-flat_namespace
DYLD_INSERT_LIBRARIES=./target/release/libbasic_malloc.dylib ./c/test
```
