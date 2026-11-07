#!/usr/bin/env bash
# End-to-end smoke test against a running Boulevard API (local by default).
# Walks the product: sign-in, profile, deck, a match with a stand-in, chat,
# Plus, a second live member matching the first, block, report, admin, and
# the chat WebSocket. Needs curl, jq, and node >= 22 (global WebSocket).
#
#   BASE=http://localhost:8787 bash scripts/smoke.sh
#
# The dev server must run with DEV_RETURN_OTP=true (see .dev.vars.example)
# and ADMIN_EMAILS containing peruna@smu.edu.
set -euo pipefail
BASE="${BASE:-http://localhost:8787}"
pass=0; fail=0
ok()   { pass=$((pass+1)); printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad()  { fail=$((fail+1)); printf '  \033[31m✗\033[0m %s\n' "$1"; }
check(){ if eval "$2"; then ok "$1"; else bad "$1 (got: $(echo "$3" | head -c 300))"; fi; }
req()  { # method path [token] [json]
  local m=$1 p=$2 t=${3:-} b=${4:-}
  if [ -n "$t" ] && [ -n "$b" ]; then curl -s -X "$m" "$BASE$p" -H "authorization: Bearer $t" -H 'content-type: application/json' --data "$b";
  elif [ -n "$t" ]; then curl -s -X "$m" "$BASE$p" -H "authorization: Bearer $t";
  elif [ -n "$b" ]; then curl -s -X "$m" "$BASE$p" -H 'content-type: application/json' --data "$b";
  else curl -s -X "$m" "$BASE$p"; fi
}
signin() { # email -> token
  local email=$1 start code verify
  start=$(req POST /v1/auth/start "" "{\"email\":\"$email\"}")
  code=$(echo "$start" | jq -r '.dev_code // empty')
  [ -n "$code" ] || { echo "no dev_code in: $start" >&2; return 1; }
  verify=$(req POST /v1/auth/verify "" "{\"email\":\"$email\",\"code\":\"$code\"}")
  echo "$verify" | jq -r '.token // empty'
}

echo "Boulevard smoke test against $BASE"
H=$(req GET /v1/health);           check "health" '[ "$(echo "$H" | jq -r .ok)" = true ]' "$H"
R=$(req GET /v1/rules);            check "rules published" '[ "$(echo "$R" | jq -r ".rules | length")" -ge 6 ]' "$R"
X=$(req POST /v1/auth/start "" '{"email":"someone@gmail.com"}'); check "non-SMU address rejected" '[ "$(echo "$X" | jq -r .error.code)" = bad_request ]' "$X"
X=$(req POST /v1/auth/start "" '{"email":"peruna+alt@smu.edu"}'); check "plus-addressing rejected" '[ "$(echo "$X" | jq -r .error.code)" = bad_request ]' "$X"

T1=$(signin peruna@smu.edu);       check "sign in (peruna@smu.edu)" '[ -n "$T1" ]' "$T1"
X=$(req POST /v1/auth/verify "" '{"email":"peruna@smu.edu","code":"000000"}'); check "wrong code rejected" '[ "$(echo "$X" | jq -r .error.code)" = bad_request ]' "$X"
ME=$(req GET /v1/me "$T1");        check "me before profile" '[ "$(echo "$ME" | jq -r .profile)" = null ]' "$ME"
X=$(req GET /v1/deck "$T1");       check "deck requires profile" '[ "$(echo "$X" | jq -r .error.code)" = need_profile ]' "$X"
X=$(req PUT /v1/me/profile "$T1" '{"display_name":"","gender":"man","year":"junior","major":"Nope","greek":"Independent","intent":"dating","looking_for":"women","bio":"hi"}')
check "profile validation lists every error" '[ "$(echo "$X" | jq -r ".error.fields | length")" = 2 ]' "$X"
P=$(req PUT /v1/me/profile "$T1" '{"display_name":"Peruna","gender":"man","year":"junior","major":"Finance","greek":"Independent","intent":"dating","looking_for":"women","bio":"Hilltop coffee beats a hidden ranking."}')
check "profile saved" '[ "$(echo "$P" | jq -r .ok)" = true ]' "$P"

D=$(req GET /v1/deck "$T1")
check "deck shows the six women stand-ins" '[ "$(echo "$D" | jq -r ".cards | length")" = 6 ]' "$D"
check "Ava ranks first with 80 (intent 40 + major 25 + year 15)" '[ "$(echo "$D" | jq -r ".cards[0].id")" = standin:ava ] && [ "$(echo "$D" | jq -r ".cards[0].score.total")" = 80 ]' "$D"
check "free member does not see likes_you" '[ "$(echo "$D" | jq -r ".cards[0] | has(\"likes_you\")")" = false ]' "$D"
L=$(req GET /v1/likes "$T1");      check "likes hidden on free (3 women like Peruna)" '[ "$(echo "$L" | jq -r .hidden)" = true ] && [ "$(echo "$L" | jq -r .count)" = 3 ]' "$L"

S=$(req POST /v1/swipes "$T1" '{"target_id":"standin:ava","direction":"like","note":"Saw the Cox line. Coffee this week?"}')
check "like Ava -> match" '[ "$(echo "$S" | jq -r .matched)" = true ]' "$S"
MID=$(echo "$S" | jq -r .match_id)
S2=$(req POST /v1/swipes "$T1" '{"target_id":"standin:ava","direction":"like"}')
check "repeat swipe is idempotent" '[ "$(echo "$S2" | jq -r .already)" = true ] && [ "$(echo "$S2" | jq -r .likes_today)" = 1 ]' "$S2"
S3=$(req POST /v1/swipes "$T1" '{"target_id":"standin:maya","direction":"pass"}')
check "pass Maya (no match)" '[ "$(echo "$S3" | jq -r .matched)" = false ]' "$S3"
M=$(req GET /v1/matches "$T1")
check "matches lists Ava with her note as last message" '[ "$(echo "$M" | jq -r ".matches | length")" = 1 ] && [ "$(echo "$M" | jq -r ".matches[0].other.standin")" = true ]' "$M"
HIS=$(req GET "/v1/matches/$MID/messages" "$T1")
check "chat seeded with both notes (hers first)" '[ "$(echo "$HIS" | jq -r ".messages | length")" = 2 ] && [ "$(echo "$HIS" | jq -r ".messages[0].sender_id")" = standin:ava ] && [ "$(echo "$HIS" | jq -r ".messages[0].kind")" = note ]' "$HIS"
SEND=$(req POST "/v1/matches/$MID/messages" "$T1" '{"body":"Fondren Library, Thursday 3pm?"}')
check "send message" '[ "$(echo "$SEND" | jq -r .ok)" = true ] && [ "$(echo "$SEND" | jq -r .message.id)" = 3 ]' "$SEND"
X=$(req POST "/v1/matches/$MID/messages" "$T1" '{"body":"   "}'); check "empty message rejected" '[ "$(echo "$X" | jq -r .error.code)" = invalid_fields ]' "$X"
X=$(req POST /v1/matches/mch_nope/messages "$T1" '{"body":"hi"}'); check "unknown match 404" '[ "$(echo "$X" | jq -r .error.code)" = not_found ]' "$X"

PL=$(req POST /v1/me/plus "$T1" '{"active":true}'); check "Plus preview unlock" '[ "$(echo "$PL" | jq -r .plus)" = true ]' "$PL"
D=$(req GET /v1/deck "$T1")
check "Plus sees likes_you and +15 on Elena (casual, Marketing, sophomore: adjacent year 15 + liked 15)" '[ "$(echo "$D" | jq -r "[.cards[] | select(.id==\"standin:elena\")][0].score.total")" = 30 ] && [ "$(echo "$D" | jq -r "[.cards[] | select(.id==\"standin:elena\")][0].likes_you")" = true ]' "$D"
L=$(req GET /v1/likes "$T1");      check "Plus sees who liked you with notes" '[ "$(echo "$L" | jq -r .hidden)" = false ] && [ "$(echo "$L" | jq -r ".people[0].note | length")" -gt 0 ]' "$L"
F=$(req PUT /v1/me/filters "$T1" '{"major":"Finance"}'); check "Plus major filter accepted" '[ "$(echo "$F" | jq -r .ok)" = true ]' "$F"
D=$(req GET /v1/deck "$T1");       check "major filter leaves no Finance women unswiped (Ava matched)" '[ "$(echo "$D" | jq -r ".cards | length")" = 0 ]' "$D"
F=$(req PUT /v1/me/filters "$T1" '{}'); check "filters cleared" '[ "$(echo "$F" | jq -r .ok)" = true ]' "$F"
PL=$(req POST /v1/me/plus "$T1" '{"active":false}'); check "Plus off again" '[ "$(echo "$PL" | jq -r .plus)" = false ]' "$PL"
X=$(req PUT /v1/me/filters "$T1" '{"greek":"Sigma Chi"}'); check "free member cannot set house filter" '[ "$(echo "$X" | jq -r .error.code)" = forbidden ]' "$X"

# A second live member who wants men: Peruna appears on her deck, mutual like matches.
T2=$(signin mustang@smu.edu);      check "sign in (mustang@smu.edu)" '[ -n "$T2" ]' "$T2"
P=$(req PUT /v1/me/profile "$T2" '{"display_name":"Mustang","gender":"woman","year":"senior","major":"Finance","greek":"Pi Beta Phi","intent":"dating","looking_for":"men","bio":"Senior, Cox, direct."}')
check "second profile saved" '[ "$(echo "$P" | jq -r .ok)" = true ]' "$P"
D=$(req GET /v1/deck "$T2")
check "Mustang's deck has Peruna (live) first: 40 + 25 + 15 = 80, then Liam and Ethan" '[ "$(echo "$D" | jq -r ".cards[0].id" | cut -c1-4)" = usr_ ] && [ "$(echo "$D" | jq -r ".cards[0].score.total")" = 80 ] && [ "$(echo "$D" | jq -r ".cards | length")" = 3 ]' "$D"
PID=$(echo "$D" | jq -r '.cards[0].id')
S=$(req POST /v1/swipes "$T2" "{\"target_id\":\"$PID\",\"direction\":\"like\",\"note\":\"Finance and dating, finally.\"}")
check "Mustang likes Peruna: no match yet" '[ "$(echo "$S" | jq -r .matched)" = false ]' "$S"
D=$(req GET /v1/deck "$T1")
check "Peruna's deck now includes Mustang" '[ "$(echo "$D" | jq -r "[.cards[] | select(.display_name==\"Mustang\")] | length")" = 1 ]' "$D"
MUID=$(echo "$D" | jq -r '[.cards[] | select(.display_name=="Mustang")][0].id')
S=$(req POST /v1/swipes "$T1" "{\"target_id\":\"$MUID\",\"direction\":\"like\"}")
check "Peruna likes back: match" '[ "$(echo "$S" | jq -r .matched)" = true ]' "$S"
MID2=$(echo "$S" | jq -r .match_id)
HIS=$(req GET "/v1/matches/$MID2/messages" "$T2")
check "live chat seeded with Mustang's note" '[ "$(echo "$HIS" | jq -r ".messages | length")" = 1 ] && [ "$(echo "$HIS" | jq -r ".messages[0].body")" = "Finance and dating, finally." ]' "$HIS"

# WebSocket: Mustang connects, Peruna sends over REST, Mustang receives it live.
WS=$(node --input-type=module -e "
const base = process.argv[1].replace('http', 'ws'); const mid = process.argv[2]; const t2 = process.argv[3]; const t1 = process.argv[4];
const ws = new WebSocket(base + '/v1/matches/' + mid + '/ws?token=' + t2);
const got = [];
const done = new Promise((res) => setTimeout(res, 2500));
ws.onmessage = (e) => got.push(JSON.parse(e.data));
ws.onopen = async () => {
  ws.send(JSON.stringify({ type: 'typing' }));
  await fetch(process.argv[1] + '/v1/matches/' + mid + '/messages', { method: 'POST', headers: { authorization: 'Bearer ' + t1, 'content-type': 'application/json' }, body: JSON.stringify({ body: 'Fondren, Thursday?' }) });
  ws.send(JSON.stringify({ type: 'message', body: 'Thursday works.' }));
};
await done; ws.close();
console.log(JSON.stringify({ types: got.map(g => g.type), bodies: got.filter(g => g.type === 'message').map(g => g.message.body) }));
" "$BASE" "$MID2" "$T2" "$T1" 2>/dev/null || echo '{"types":[]}')
check "WebSocket: hello, then both messages arrive live" '[ "$(echo "$WS" | jq -r ".types[0]")" = hello ] && [ "$(echo "$WS" | jq -r ".bodies | length")" = 2 ]' "$WS"
M=$(req GET /v1/matches "$T1")
check "last message mirrored to the match list" '[ "$(echo "$M" | jq -r "[.matches[] | select(.id==\"$MID2\")][0].last_message.preview")" = "Thursday works." ]' "$M"

RP=$(req POST /v1/reports "$T2" "{\"target_id\":\"$PID\",\"reason\":\"other\",\"details\":\"smoke test report\",\"match_id\":\"$MID2\"}")
check "report filed" '[ "$(echo "$RP" | jq -r .ok)" = true ]' "$RP"
RID=$(echo "$RP" | jq -r .report_id)
X=$(req GET /v1/admin/reports "$T2"); check "non-admin blocked from moderation" '[ "$(echo "$X" | jq -r .error.code)" = forbidden ]' "$X"
A=$(req GET "/v1/admin/reports?status=open" "$T1"); check "admin sees the open report" '[ "$(echo "$A" | jq -r ".reports[0].id")" = "$RID" ]' "$A"
TR=$(req GET "/v1/admin/matches/$MID2/transcript" "$T1"); check "admin transcript has 3 messages" '[ "$(echo "$TR" | jq -r ".messages | length")" = 3 ]' "$TR"
RS=$(req POST "/v1/admin/reports/$RID/resolve" "$T1" '{"action":"dismiss","note":"smoke"}'); check "report dismissed" '[ "$(echo "$RS" | jq -r .ok)" = true ]' "$RS"
B=$(req POST /v1/blocks "$T2" "{\"user_id\":\"$PID\"}"); check "block closes the match" '[ "$(echo "$B" | jq -r .closed_match)" = "$MID2" ]' "$B"
X=$(req POST "/v1/matches/$MID2/messages" "$T1" '{"body":"hello?"}'); check "closed match refuses messages (410)" '[ "$(echo "$X" | jq -r .error.code)" = match_closed ]' "$X"
D=$(req GET /v1/deck "$T1");       check "blocked member gone from the deck" '[ "$(echo "$D" | jq -r "[.cards[] | select(.display_name==\"Mustang\")] | length")" = 0 ]' "$D"
U=$(req GET "/v1/admin/users/$MUID" "$T1"); check "admin user view has events" '[ "$(echo "$U" | jq -r ".events | length")" -gt 3 ]' "$U"

PH=$(curl -s -X POST "$BASE/v1/me/photos" -H "authorization: Bearer $T1" --data-binary 'notanimage'); check "photos report disabled without R2" '[ "$(echo "$PH" | jq -r .error.code)" = photos_disabled ]' "$PH"
DEL=$(req DELETE /v1/me "$T2");    check "second account deleted" '[ "$(echo "$DEL" | jq -r .ok)" = true ]' "$DEL"
X=$(req GET /v1/me "$T2");         check "deleted account's session is dead" '[ "$(echo "$X" | jq -r .error.code)" = unauthorized ]' "$X"
LO=$(req POST /v1/auth/logout "$T1"); check "logout" '[ "$(echo "$LO" | jq -r .ok)" = true ]' "$LO"
X=$(req GET /v1/me "$T1");         check "token revoked after logout" '[ "$(echo "$X" | jq -r .error.code)" = unauthorized ]' "$X"

echo; echo "passed: $pass  failed: $fail"
[ "$fail" -eq 0 ]
