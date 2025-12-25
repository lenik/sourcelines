# sourcelines bash completion
_sourcelines() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    
    opts="-r --recursive -L --follow-symlinks -i --ignorelist -I --no-ignorelist -s --sum -v --verbose -k --actual-klocs -l --actual-loc -K --raw-klocs -R --raw-locs -w --words -c --chars -b --bytes --text --html --latex --pdf --markdown -h --help -V --version --exclude --include"

    if [[ ${cur} == -* ]] ; then
        COMPREPLY=( $(compgen -W "${opts}" -- ${cur}) )
        return 0
    fi
}

complete -F _sourcelines sourcelines