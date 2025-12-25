# sourcelines - a source code statistics tool
project(
    'sourcelines',
    'rust',
    version: run_command(
        [
            'head', '-1', meson.project_source_root() + '/release-version.txt',
        ],
        check: true,
    ).stdout().strip(),
    meson_version: '>=0.60.0',
)

# Get the source root directory
source_root = meson.project_source_root()
build_root = meson.project_build_root()

cargoEx = find_program('cargo-ex', required: true)

cargo_build = custom_target(
    'cargo-build',
    command: [
        cargoEx,
        '-Z', 'unstable-options', '-C', source_root,
        'build',
        '-o', 'sourcelines',
        '--copy-to', build_root,
        '--release',
        '--target-dir', build_root,
    ],
    output: 'sourcelines',
    depend_files: files('src/lib.rs', 'src/main.rs', 'Cargo.toml'),
    install: true,
    install_dir: get_option('bindir'),
    install_tag: 'runtime',
    build_by_default: true,
)

# Generate sourcelines.txt report after successful build
# This runs the built sourcelines executable with --text to generate a report
# Run from source directory to scan source files, not build files
sourcelines_txt = custom_target(
    'sourcelines-txt',
    command: [
        'sh', '-c',
        'EXE="@0@"; OUT="@1@"; SRCDIR="@2@"; if [ -x "$EXE" ]; then cd "$SRCDIR" && "$EXE" --text . > "$OUT" 2>"$OUT.err" || (echo "Error: sourcelines --text failed. Check $OUT.err" >&2; touch "$OUT"); fi'.format(
            build_root / 'sourcelines',
            source_root / 'sourcelines.txt',
            source_root
        ),
    ],
    output: 'sourcelines.txt',
    depends: cargo_build,
    build_by_default: true,
    build_always_stale: true,
)

# No direct install from the source tree target directory; installation is handled by the custom_target above.

# Generate man page from README.md (enabled by default)
readme_md = files('README.md')
build_manpage = get_option('build_manpage')
pandoc = find_program('pandoc', required: build_manpage)
if build_manpage and pandoc.found()
    sourcelines_man = custom_target(
        'sourcelines-man',
        command: [
            pandoc,
            '--shift-heading-level-by=2',
            '-s',
            '-t', 'man',
            '@INPUT@',
            '-o', '@OUTPUT@',
        ],
        input: readme_md,
        output: 'sourcelines.1',
        install: true,
        install_dir: get_option('man_dir') == '' ? join_paths(get_option('datadir'), 'man/man1') : get_option('man_dir'),
        depend_files: files('README.md'),
        install_tag: 'man',
        build_by_default: true,
    )
endif

# Install bash completion script
install_data(
    'sourcelines-completion.bash',
    install_dir: get_option('bash_completion_dir') == '' ? join_paths(get_option('datadir'), 'bash-completion/completions') : get_option('bash_completion_dir'),
    install_tag: 'data',
    rename: 'sourcelines',
)

tarball_name = meson.project_name() + '-' + meson.project_version() + '.tar.gz'
archive_prefix = meson.project_name() + '-' + meson.project_version() + '/'

src_tar = custom_target(
    'source-tarball',
    command: [
        'sh', '-c', 'git -C "$1" archive --format=tar --prefix="$2" HEAD | gzip -9 > "$3"',
        'ignored',
        source_root,
        archive_prefix,
        '@OUTPUT@',
    ],
    output: tarball_name,
    build_by_default: false,
)

alias_target('distribute', src_tar)
alias_target('tarball', src_tar)