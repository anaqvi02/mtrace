#!/bin/bash
./target/release/mtrace -t dummy /Library/Apple/Steam.app/Contents/MacOS/steam_osx &
MTRACE_PID=$!
echo "Spawned mtrace with PID $MTRACE_PID"
sleep 10

echo "--- BEFORE KILL ---"
ps -ef | grep -v grep | grep "steam\|mtrace"

echo "Sending SIGINT to mtrace ($MTRACE_PID)..."
kill -SIGINT $MTRACE_PID
sleep 2

echo "--- AFTER KILL ---"
ps -ef | grep -v grep | grep "steam\|mtrace"
