# comma: bash hook. Load with: eval "$(comma init bash)"
# A line starting with "," is sent to `comma ask` instead of being run.

__comma_process() {
  __COMMA_HANDLED=0
  [[ $READLINE_LINE == ,* ]] || return 0
  __COMMA_HANDLED=1
  local req="${READLINE_LINE#,}" out rc
  [[ -n ${req//[[:space:]]/} ]] || return 0
  history -s -- "$READLINE_LINE"
  out=$(COMMA_PREV_REQUEST="$__COMMA_PREV_REQUEST" COMMA_PREV_COMMAND="$__COMMA_PREV_COMMAND" \
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
__comma_hook() {
  __comma_process
  if (( __COMMA_HANDLED )); then bind '"\C-x\C-a": redraw-current-line'
  else bind '"\C-x\C-a": accept-line'; fi
}

if [[ $- == *i* ]] && command -v comma >/dev/null; then
  bind -x '"\C-x\C-c": __comma_hook'
  bind '"\C-x\C-a": accept-line'
  bind '"\C-m": "\C-x\C-c\C-x\C-a"'
fi
