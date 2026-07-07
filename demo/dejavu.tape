# VHS tape for the README demo GIF.
#
#   brew install vhs
#   cargo build --release
#   vhs demo/dejavu.tape       # writes demo/dejavu.gif
#
# Story: an agent re-runs the same failing test suite; Dejavu shows the full
# failure once, dedups the re-run, celebrates the fix, and tallies the savings.

Output demo/dejavu.gif

Set Shell "bash"
Set FontSize 15
Set Width 1240
Set Height 780
Set Padding 22
Set Theme "Dracula"
Set TypingSpeed 40ms

# Boot the prepared project + dejavu session off-screen, then start clean.
Hide
Type "demo/session.sh"
Enter
Sleep 4s
Type "clear"
Enter
Sleep 500ms
Show

Type "# Your agent re-runs the same commands all day. Dejavu dedups what they read."
Enter
Sleep 3s

Type "pnpm test"
Enter
Sleep 6s

Type "pnpm test   # agent re-checks -> same failure, compact 'unchanged'"
Enter
Sleep 6s

Type "sed -i.bak 's/subtotal + discount/subtotal - discount/' src/billing.ts   # fix the bug"
Enter
Sleep 2500ms

Type "pnpm test"
Enter
Sleep 6s

Type "dejavu stats"
Enter
Sleep 8s
