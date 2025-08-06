# wcc - a source code statistics tool
project('wcc', 'rust', version: '0.1.0',
        meson_version: '>=0.60.0')

# Get the source root directory
source_root = meson.project_source_root()
build_root = meson.project_build_root()

# Use Cargo to build (always run this)
cargo_build = custom_target('cargo-build',
  command: ['sh', '-c', 'cd ' + source_root + ' && cargo build --release && cp target/release/wcc ' + build_root + '/@OUTPUT@'],
  output: 'wcc',
  depend_files: files('Cargo.toml', 'src/main.rs', 'src/lib.rs'),
  install: true,
  install_dir: get_option('bindir'),
  install_tag: 'runtime',
  build_by_default: true
)

# Install the built executable directly from target directory
install_data(source_root + '/target/release/wcc',
             install_dir: get_option('bindir'),
             rename: 'wcc')

# Generate man page from README.md
readme_md = files('README.md')
wcc_man = custom_target('wcc-man',
                        command: ['sh', '-c', 'cd ' + source_root + ' && pandoc --base-header-level=2 -s -t man README.md -o ' + build_root + '/wcc.1'],
                        input: readme_md,
                        output: 'wcc.1',
                        install: true,
                        install_dir: get_option('man_dir') == '' ? join_paths(get_option('datadir'), 'man/man1') : get_option('man_dir'),
                        depend_files: files('README.md'),
                        install_tag: 'man',
                        build_by_default: true)

# Install bash completion script
install_data('wcc-completion.bash',
             install_dir: get_option('bash_completion_dir') == '' ? join_paths(get_option('datadir'), 'bash-completion/completions') : get_option('bash_completion_dir'),
             install_tag: 'data',
             rename: 'wcc')
