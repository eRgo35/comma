# comma: bash hook. Load with: eval "$(comma init bash)"
# A line starting with "," is sent to `comma ask` instead of being run.

__comma_process() {
  __COMMA_HANDLED=0
  (( ${__COMMA_PRIMARY:-1} )) || return 0  # continuation line (heredoc, open quote)
  [[ $READLINE_LINE == ,* ]] || return 0
  __COMMA_HANDLED=1
  local req="${READLINE_LINE#,}" out rc
  [[ -n ${req//[[:space:]]/} ]] || return 0
  history -s -- "$READLINE_LINE"
  out=$(COMMA_API_KEY="${COMMA_API_KEY-}" \
        COMMA_PREV_REQUEST="$__COMMA_PREV_REQUEST" COMMA_PREV_COMMAND="$__COMMA_PREV_COMMAND" \
        command comma ask "$req"); rc=$?
  case $rc in
    0) READLINE_LINE=$out; READLINE_POINT=${#out}
       __COMMA_PREV_REQUEST=$req; __COMMA_PREV_COMMAND=$out ;;
    2) READLINE_LINE=; READLINE_POINT=0
       __COMMA_PREV_REQUEST=$req; __COMMA_PREV_COMMAND= ;;
  esac  # any other code: error already printed, line left as typed
}

# Enter runs this hook, then whatever \C-x\C-a is bound to: accept-line for
# normal lines, a no-op redraw for "," lines so the result waits for a second Enter.
# Bound in every keymap so vi mode works too. The "$_" argument keeps $_ intact.
__COMMA_KEYMAPS=(emacs vi-insert vi-command)
__comma_hook() {
  __comma_process
  local next=accept-line m
  if (( __COMMA_HANDLED )); then next=redraw-current-line; else __COMMA_PRIMARY=0; fi
  for m in "${__COMMA_KEYMAPS[@]}"; do bind -m "$m" "\"\C-x\C-a\": $next"; done
}

if [[ $- == *i* ]] && command -v comma >/dev/null; then
  # Only lines typed at the main prompt are candidates; PROMPT_COMMAND marks it.
  PROMPT_COMMAND+=('__COMMA_PRIMARY=1')
  # Keep the key in this shell but out of other programs' environment
  # (npm scripts, build tools, ...); __comma_process passes it to comma only.
  export -n COMMA_API_KEY
  for __comma_m in "${__COMMA_KEYMAPS[@]}"; do
    bind -m "$__comma_m" -x '"\C-x\C-c": __comma_hook "$_"'
    bind -m "$__comma_m" '"\C-x\C-a": accept-line'
    bind -m "$__comma_m" '"\C-m": "\C-x\C-c\C-x\C-a"'
  done
  unset __comma_m
fi
