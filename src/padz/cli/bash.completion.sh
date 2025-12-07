
_pa_complete() {
    local cur prev cmd
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local global_opts="--global -g --verbose -v --help -h --version -V"
    local commands="create list view edit open delete pin unpin search path config init completions"
    local aliases="n ls v e o rm p u"

    for word in "${COMP_WORDS[@]:1}"; do
        case "$word" in
            -g|--global|--verbose|-v|-h|--help|-V|--version) ;;
            create|list|view|edit|open|delete|pin|unpin|search|path|config|init|completions|n|ls|v|e|o|rm|p|u)
                cmd="$word"
                break
                ;;
        esac
    done

    if [[ -z "$cmd" ]]; then
        COMPREPLY=( $(compgen -W "$global_opts $commands $aliases" -- "$cur") )
        return 0
    fi

    if [[ "$cur" == --* ]]; then
        case "$cmd" in
            create|n)
                COMPREPLY=( $(compgen -W "--no-editor" -- "$cur") )
                return 0
                ;;
            list|ls)
                COMPREPLY=( $(compgen -W "--deleted --search" -- "$cur") )
                return 0
                ;;
        esac
    fi

    case "$cmd" in
        completions)
            COMPREPLY=( $(compgen -W "bash zsh" -- "$cur") )
            return 0
            ;;
        view|v|edit|e|open|o|delete|rm|pin|p|unpin|u|path)
            if __pa_pad_index_completion "$cmd" "$cur"; then
                return 0
            fi
            ;;
    esac
}

__pa_pad_index_completion() {
    local cmd="$1"
    local cur="$2"
    local include_deleted="no"
    case "$cmd" in
        view|v|open|o|path)
            include_deleted="yes"
            ;;
    esac

    local -a scope_flags=()
    for word in "${COMP_WORDS[@]:1}"; do
        case "$word" in
            -g|--global)
                scope_flags+=(--global)
                ;;
        esac
    done

    local cmdline=(pa "${scope_flags[@]}" __complete-pads)
    if [[ "$include_deleted" == "yes" ]]; then
        cmdline+=(--deleted)
    fi

    local pad_output
    pad_output="$( ${cmdline[@]} 2>/dev/null )" || return 1
    if [[ -z "$pad_output" ]]; then
        return 1
    fi

    local IFS=$'
'
    local values=()
    local shown=()
    while IFS=$'	' read -r index title; do
        [[ -z "$index" ]] && continue
        values+=("$index")
        shown+=("$index # $title")
    done <<< "$pad_output"

    COMPREPLY=( $(compgen -W "${values[*]}" -- "$cur") )
    if [[ ${#COMPREPLY[@]} -gt 1 ]]; then
        printf '\n'
        printf '%s\n' "${shown[@]}"
    fi
    return 0
}

complete -F _pa_complete pa
