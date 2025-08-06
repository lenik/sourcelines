# wcc

`wcc` is a Rust CLI tool to count source code statistics: actual lines of code (excluding empty lines and pure comment lines), raw lines, words, characters, and bytes for each file or directory argument. It supports recursive directory traversal and flexible output options.

## Features
- Counts: actual lines of code, raw lines, words, characters, bytes
- Supports many languages (comment syntax auto-detected by extension, shebang, or content)
- Flexible output columns: select any combination of stats
- Recursive directory traversal (`-r`/`--recursive`)
- Summary line output (`-s`/`--sum`)
- Language detection (shown in output)

## Usage

```sh
wcc [OPTIONS] FILES...
```


### Options

- `-r`, `--recursive`         : Recursively process directories
- `-s`, `--sum`               : Output a summary line at the end
- `-v`, `--verbose`           : Verbose output: with -s, print all file stats; for directories, print per-language summary
- `--exclude WILDCARD`        : Exclude files/directories matching these wildcard patterns (can be used multiple times)
- `--include WILDCARD`        : Include files/directories matching these wildcard patterns (can be used multiple times)
- `-k`, `--actual-klocs`      : Show actual KLOCs (actual lines/1000)
- `-l`, `--actual-loc`        : Show actual LOC (default if no -k)
- `-K`, `--raw-klocs`         : Show raw KLOCs (raw lines/1000)
- `-L`, `--raw-locs`          : Show raw LOC (default if no -K)
- `-w`, `--words`             : Show word count
- `-c`, `--chars`             : Show character count
- `-b`, `--bytes`             : Show byte count
- `-h`, `--help`              : Show help message
- `-V`, `--version`           : Show version

By default, the following are excluded: `.git`, `.svn`, `node_modules`, `target`, `build`, `builddir`, `~*`, `$*`, `*.tmp`, `*.lock`. Use `--include` to re-include any of these, or `--exclude` to add more patterns. Patterns use shell-style wildcards (globs).

If neither `-k` nor `-l` is given, only one is shown (default: LOC). Same for `-K`/`-L`.

### Output Format

Each output line:

    [actual-klocs|actual-loc] [raw-klocs|raw-loc] [words] [chars] [bytes] <language> FILE

For summary line (with `-s`):

    ... <*> (sum)

## Example

```sh
wcc -r -l -L -w src/
wcc -k -K -c -b file.rs
```

## Build

### With Cargo

```sh
cargo build --release
```

### With Meson

```sh
meson setup builddir
meson compile -C builddir
```

To install:

```sh
meson install -C builddir
```

This will install:
- The `wcc` binary
- The man page to `DATADIR/man/man1/wcc.1`
- Bash completion to `DATADIR/bash-completion/completions/wcc`

## Manpage

The markdown doc (`README.md`) is compiled to a man page (`wcc.1`) using `pandoc` in the build script.

## License

GPL