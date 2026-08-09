_browserfetch() {
    local current="${COMP_WORDS[COMP_CWORD]}"
    local candidate
    local -a candidates=()

    if (( COMP_CWORD > 1 )); then
        return
    fi

    while IFS= read -r candidate; do
        [[ -n "$candidate" ]] && candidates+=("$candidate")
    done < <(browserfetch --complete 2>/dev/null)

    COMPREPLY=()
    for candidate in "${candidates[@]}"; do
        [[ "$candidate" == "$current"* ]] && COMPREPLY+=("$candidate")
    done
}

complete -F _browserfetch browserfetch
