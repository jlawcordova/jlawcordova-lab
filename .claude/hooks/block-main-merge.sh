#!/usr/bin/env bash
# PreToolUse hook: agents may merge pull requests into feature branches
# (feat/**), never into main, and never push to main. main is what the owner
# installs Pixel Lab from, so the owner merges into it by hand. Exit 2 blocks
# the tool call and shows the message to Claude. Copied from the site repo's
# .claude/hooks/block-main-merge.sh.
input=$(cat)
tool=$(jq -r '.tool_name' <<<"$input")

# Whether a push's destination is main: a refspec's part after the last ":"
# (or the whole refspec), without a leading "+" or "refs/heads/".
targets_main() {
  local dst=${1//[\"\']/}
  dst=${dst#+}
  dst=${dst##*:}
  dst=${dst#refs/heads/}
  [[ "$dst" == main ]]
}

# Checks one simple command (no ;, &&, || or |) for a push to main. Only a
# command whose first word is git counts, so "git push … main" inside an echo,
# a quoted PR body or a heredoc line doesn't. $dir is where a bare push runs.
check_push() {
  local -a words
  read -ra words <<<"$1"
  local i=0 n=${#words[@]} where=$dir
  while (( i < n )) && [[ "${words[i]}" == *=* ]]; do ((i++)); done
  if [[ "${words[i]}" == cd ]] && (( i + 1 < n )); then
    dir=${words[i+1]}
    return 0
  fi
  [[ "${words[i]}" == git ]] || return 0
  ((i++))
  # git's own options before the subcommand: -C <dir>, -c <name=value>, --flags.
  while (( i < n )) && [[ "${words[i]}" == -* ]]; do
    case "${words[i]}" in
      -C) where=${words[i+1]}; ((i += 2)) ;;
      -c) ((i += 2)) ;;
      *) ((i++)) ;;
    esac
  done
  [[ "${words[i]}" == push ]] || return 0
  ((i++))
  local -a args=()
  while (( i < n )); do
    local w=${words[i]}
    case "$w" in
      *'>'* | *'<'*) break ;;
      --all | --mirror) echo "Blocked: git push $w can update main. Push one named branch instead." >&2; exit 2 ;;
      -o | --push-option | --repo | --receive-pack | --exec) ((i += 2)); continue ;;
      -*) ;;
      *) args+=("$w") ;;
    esac
    ((i++))
  done
  # args[0] is the remote; the rest are refspecs.
  if (( ${#args[@]} > 1 )); then
    local spec
    for spec in "${args[@]:1}"; do
      targets_main "$spec" && block_push
    done
    return 0
  fi
  # No refspec: git pushes the current branch to its push destination, or,
  # when push.default can't name one, its upstream. Either one being main blocks.
  local ref dest
  for ref in '@{push}' '@{upstream}'; do
    dest=$(git -C "${where:-.}" rev-parse --abbrev-ref --symbolic-full-name "$ref" 2>/dev/null) || continue
    targets_main "${dest#*/}" && block_push
  done
  return 0
}

block_push() {
  echo "Blocked: only the owner updates main. Push to a feature or topic branch instead." >&2
  exit 2
}

if [[ "$tool" == "Bash" ]]; then
  cmd=$(jq -r '.tool_input.command // ""' <<<"$input")
  dir=$(jq -r '.cwd // empty' <<<"$input")
  # A direct push to main counts too. Check each simple command on its own,
  # so a later word "main" elsewhere in the line doesn't count.
  merge=
  while IFS= read -r segment; do
    check_push "$segment"
    # A merge is a command that starts with gh pr merge, not those words in a body.
    [[ "$segment" =~ ^[[:space:]]*gh[[:space:]]+pr[[:space:]]+merge([[:space:]]|$) ]] && merge=$segment
  done < <(sed -E 's/(&&|\|\||;|\||&)/\n/g' <<<"$cmd")
  [[ -n "$merge" ]] || exit 0
  # A pull request number after "merge", or the current branch's PR when none.
  pr=$(grep -oE 'gh[[:space:]]+pr[[:space:]]+merge[[:space:]]+#?[0-9]+' <<<"$merge" | grep -oE '[0-9]+$')
  base=$(gh pr view ${pr:+"$pr"} --json baseRefName -q .baseRefName 2>/dev/null)
elif [[ "$tool" == *merge_pull_request ]]; then
  owner=$(jq -r '.tool_input.owner' <<<"$input")
  repo=$(jq -r '.tool_input.repo' <<<"$input")
  pr=$(jq -r '.tool_input.pullNumber' <<<"$input")
  base=$(gh api "repos/$owner/$repo/pulls/$pr" -q .base.ref 2>/dev/null)
else
  exit 0
fi

if [[ "$base" != feat/* ]]; then
  echo "Blocked: agents never merge pull requests into main or any branch outside feat/** (base: ${base:-unknown})." >&2
  exit 2
fi
exit 0
