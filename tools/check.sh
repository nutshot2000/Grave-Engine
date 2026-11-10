#!/usr/bin/env bash
# Check everything in one go: build, run every test, then simulate every
# scene headless and save a screenshot of each into shots/check/.
# Look at the screenshots afterwards: they show whether things look right.
#
#   tools/check.sh            (from the project folder)
#   tools/check.sh 600        (simulate each scene for 600 frames; default 300)
#
# If the editor is open on Windows, close it first: it locks the program file.
set -u
cd "$(dirname "$0")/.."
FRAMES="${1:-300}"
EXE=target/release/grave-engine
[ -f "$EXE.exe" ] && EXE="$EXE.exe"

echo "== Building"
cargo build --release || { echo "BUILD FAILED (is the editor open? close it and try again)"; exit 1; }
[ -f target/release/grave-engine.exe ] && EXE=target/release/grave-engine.exe

echo "== Tests"
if cargo test --release 2>&1 | tee shots_test.log | grep -E "^test result|panicked|FAILED"; then :; fi
if grep -qE "test result: FAILED|panicked at" shots_test.log; then TESTS=failed; else TESTS=ok; fi
rm -f shots_test.log

echo "== Scenes ($FRAMES frames each)"
mkdir -p shots/check
PROBLEMS=0
for scene in scenes/*.txt; do
  name=$(basename "$scene" .txt)
  out=$("$EXE" --run "$scene" --frames "$FRAMES" --scale 2 --shot "shots/check/$name.png" 2>&1)
  code=$?
  line=$(echo "$out" | grep -E "ms/frame" | sed 's/.*in /in /')
  player=$(echo "$out" | grep -E "^Player" | head -1)
  if [ $code -ne 0 ]; then
    PROBLEMS=$((PROBLEMS + 1))
    echo "  $name: PROBLEMS (exit $code)"
    echo "$out" | grep -iE "problem|error" | sed 's/^/      /'
  else
    echo "  $name: ok $line ${player:+| $player}"
  fi
done

echo
echo "Tests: $TESTS. Scenes with problems: $PROBLEMS. Screenshots: shots/check/"
[ "$TESTS" = ok ] && [ $PROBLEMS -eq 0 ]
