# wcc bash completion
_wcc() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    
    opts="-r --recursive -s --sum -v --verbose -k --actual-klocs -l --actual-loc -K --raw-klocs -L --raw-locs -w --words -c --chars -b --bytes -h --help -V --version --exclude --include"

    if [[ ${cur} == -* ]] ; then
        COMPREPLY=( $(compgen -W "${opts}" -- ${cur}) )
        return 0
    fi
}

complete -F _wcc wcc