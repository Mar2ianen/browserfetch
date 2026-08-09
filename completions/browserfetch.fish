function __browserfetch_browsers
    browserfetch --complete 2>/dev/null
end

complete -c browserfetch -f -n '__fish_is_first_arg' -a '(__browserfetch_browsers)' -d 'browser'
complete -c browserfetch -l list -f -d 'list installed browsers'
complete -c browserfetch -l help -s h -f -d 'show help'
