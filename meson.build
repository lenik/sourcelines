# wcc - a source code statistics tool
# wcc - a source code statistics tool
# Ensure clean out-of-source builds by avoiding references to the source tree's target directory.
project('wcc', 'rust', version: '0.1.0',
        meson_version: '>=0.60.0')

# Get the source root directory
source_root = meson.project_source_root()
build_root = meson.project_build_root()

# Use Cargo to build into the build directory (always run this)
sh_prog = find_program('sh')
cargo_target_dir = join_paths(build_root, 'cargo-target')
cargo_build = custom_target('cargo-build',
  command: [
    sh_prog, '-c',
    'set -eu; '\
    + 'cd ' + meson.project_source_root() + ' && ' \
    + 'cargo build --release --target-dir ' + cargo_target_dir + ' && ' \
    + 'cp ' + join_paths(cargo_target_dir, 'release', 'wcc') + ' ' + '@OUTPUT@'
  ],
  output: 'wcc',
  depend_files: files('Cargo.toml', 'src/main.rs', 'src/lib.rs'),
  install: true,
  install_dir: get_option('bindir'),
  install_tag: 'runtime',
  build_by_default: true
)

# No direct install from the source tree target directory; installation is handled by the custom_target above.

# Generate man page from README.md if pandoc is available
readme_md = files('README.md')
pandoc = find_program('pandoc', required: false)
if pandoc.found()
  wcc_man = custom_target('wcc-man',
                          command: [pandoc, '--shift-heading-level-by=1', '-s', '-t', 'man', '@INPUT@', '-o', '@OUTPUT@'],
                          input: readme_md,
                          output: 'wcc.1',
                          install: true,
                          install_dir: get_option('man_dir') == '' ? join_paths(get_option('datadir'), 'man/man1') : get_option('man_dir'),
                          depend_files: files('README.md'),
                          install_tag: 'man',
                          build_by_default: true)
endif

# Install bash completion script
install_data('wcc-completion.bash',
             install_dir: get_option('bash_completion_dir') == '' ? join_paths(get_option('datadir'), 'bash-completion/completions') : get_option('bash_completion_dir'),
             install_tag: 'data',
             rename: 'wcc')
