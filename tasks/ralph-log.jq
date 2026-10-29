# Human-readable view of Claude Code JSONL (stream-json output or session transcripts).
# Usage: tail -n +1 -F log.jsonl | jq -R -r --unbuffered -f tasks/ralph-log.jq

def c(code): "\u001b[" + code + "m" + . + "\u001b[0m";
def oneline: gsub("\\s+"; " ") | ltrimstr(" ");
def trunc(n): if length > n then .[:n] + "…" else . end;
def short: sub("^.*/iron-designs/"; "");
def ts: (.timestamp // "" | .[11:19]) as $t | if $t == "" then "" else ($t | c("2")) + " " end;
def resulttext: if type == "array" then map(.text? // "") | join(" ") else tostring end;

def tool:
  .input as $i
  | if .name == "Bash" and ($i.command | test("git commit")) then "✔ commit  " + ($i.command | capture("-m\\s+[\"'](?<m>[^\"'\\n]*)").m? // "" ) | c("1;32")
    elif .name == "Bash" then "$ " + (($i.description // $i.command) | oneline | trunc(110)) | c("36")
    elif .name == "Edit" or .name == "MultiEdit" then "✎ edit    " + ($i.file_path | short) | c("33")
    elif .name == "Write" then "✚ write   " + ($i.file_path | short) | c("33")
    elif .name == "Read" then "  read    " + ($i.file_path | short) | c("2")
    elif .name == "Grep" or .name == "Glob" then "  search  " + ($i.pattern | trunc(80)) | c("2")
    elif .name == "TodoWrite" then "☐ todo    " + ([$i.todos[]? | select(.status == "in_progress") | .content] | join(", ")) | c("35")
    else "  " + .name + " " + ($i | tostring | trunc(80)) | c("2")
    end;

(fromjson? // empty)
| if .type == "system" and .subtype == "init" then
    "▶ session " + (.session_id // "" | .[:8]) + " · " + (.model // "") | c("1")
  elif .type == "assistant" then
    ts as $ts
    | .message.content[]?
    | if .type == "text" then $ts + ("💬 " + (.text | oneline | trunc(220)))
      elif .type == "tool_use" then $ts + tool
      else empty end
  elif .type == "user" then
    .message.content | arrays | .[]
    | select(.type? == "tool_result" and .is_error == true)
    | "  ✗ " + (.content | resulttext | oneline | trunc(160)) | c("31")
  elif .type == "result" then
    ("■ " + (.subtype // "done") + " · " + ((.duration_ms // 0) / 1000 | floor | tostring) + "s · "
      + ((.num_turns // 0) | tostring) + " turns · $" + ((.total_cost_usd // 0) * 100 | round / 100 | tostring)) | c("1")
  else empty end
