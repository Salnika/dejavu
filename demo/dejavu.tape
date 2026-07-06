# VHS tape for the README demo GIF.
#
# Install VHS (https://github.com/charmbracelet/vhs), then run from the repo root:
#   brew install vhs
#   cargo build --release
#   vhs demo/dejavu.tape
#
# Output: demo/dejavu.gif

Output demo/dejavu.gif

Set Shell "bash"
Set FontSize 16
Set Width 1300
Set Height 860
Set Padding 24
Set Theme "Dracula"
Set TypingSpeed 45ms
Set PlaybackSpeed 1.0

# The demo script builds a throwaway project + fake `pnpm`, then runs the same
# command three times so Dejavu can show: bounded first output, "unchanged",
# and a small delta. It cleans up after itself.
Type "demo/run-demo.sh"
Sleep 600ms
Enter
Sleep 11s
