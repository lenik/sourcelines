# wcc - a source code statistics tool
project(
    'wcc',
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

# Use Cargo to build into the build directory (always run this)
cargo_target_dir = join_paths(build_root, 'cargo-target')
cargo_build = custom_target(
    'cargo-build',
    command: [
        source_root + '/cargo-build.sh',
        source_root,
        cargo_target_dir,
        '@OUTPUT@',
    ],
    output: 'wcc',
    depend_files: files('src/lib.rs', 'src/main.rs', 'Cargo.toml'),
    install: true,
    install_dir: get_option('bindir'),
    install_tag: 'runtime',
    build_by_default: true,
)

# No direct install from the source tree target directory; installation is handled by the custom_target above.

# Generate man page from README.md if pandoc is available
readme_md = files('README.md')
pandoc = find_program('pandoc', required: false)
if pandoc.found()
    wcc_man = custom_target(
        'wcc-man',
        command: [
            pandoc,
            '--shift-heading-level-by=1',
            '-s',
            '-t', 'man',
            '@INPUT@',
            '-o', '@OUTPUT@',
        ],
        input: readme_md,
        output: 'wcc.1',
        install: true,
        install_dir: get_option('man_dir') == '' ? join_paths(get_option('datadir'), 'man/man1') : get_option('man_dir'),
        depend_files: files('README.md'),
        install_tag: 'man',
        build_by_default: true,
    )
endif

# Install bash completion script
install_data(
    'wcc-completion.bash',
    install_dir: get_option('bash_completion_dir') == '' ? join_paths(get_option('datadir'), 'bash-completion/completions') : get_option('bash_completion_dir'),
    install_tag: 'data',
    rename: 'wcc',
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