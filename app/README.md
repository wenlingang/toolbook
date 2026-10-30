# Toolbook app

The Mac app: `App.tsx` is the shell (sidebar, menus, views), and this directory is also a Rust crate whose native modules (`toolbook/native`) hold the note editor, the Markdown view, the preview and the library. See the [main README](../README.md) for how to build and test it.

```sh
nsh run .                                       # run in development
TOOLBOOK_HOME=$(mktemp -d)/lib nsh test .       # all tests; they refuse to run on ~/Toolbook
nsh package . --target macos                    # .nsh/out/Toolbook.app (unsigned)
```
