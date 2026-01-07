
#compdef pa

_pa_complete() {
    local -a scope_flags
    local cmd

    for word in ${words[@]:2}; do
        case $word in
            -g|--global)
                scope_flags+=(--global)
                ;;
        esac
    done

    for word in ${words[@]:2}; do
        case $word in
            -g|--global|--verbose|-v|-h|--help|-V|--version) ;;
            create|list|view|edit|open|delete|pin|unpin|search|path|config|init|completions|n|ls|v|e|o|rm|p|u)
                cmd=$word
                break
                ;;
        esac
    done

    if [[ -z $cmd ]]; then
        compadd create list view edit open delete pin unpin search path config init completions n ls v e o rm p u -- --global --verbose --help --version
        return
    fi

    case $cmd in
        create|n)
            compadd -- --no-editor
            return
            ;;
        list|ls)
            compadd -- --deleted --search
            return
            ;;
        completions)
            compadd bash zsh
            return
            ;;
        view|v|edit|e|open|o|delete|rm|pin|p|unpin|u|path)
            __pa_zsh_pad_indexes "$cmd" scope_flags
            return
            ;;
    esac
}

__pa_zsh_pad_indexes() {
    local cmd="$1"
    local -n scope_flags_ref=$2
    local include_deleted="no"
    case $cmd in
        view|v|open|o|path)
            include_deleted="yes"
            ;;
    esac

    local -a pad_entries
    local -a cmdline=(pa ${scope_flags_ref[@]} __complete-pads)
    if [[ $include_deleted == "yes" ]]; then
        cmdline+=(--deleted)
    fi

    local index title
    while IFS=$'	' read -r index title; do
        [[ -z $index ]] && continue
        pad_entries+="$index:$index # $title"
    done < <(${cmdline[@]} 2>/dev/null)

    if (( ${#pad_entries[@]} )); then
        _describe 'pads' pad_entries
    fi
}

compdef _pa_complete pa
