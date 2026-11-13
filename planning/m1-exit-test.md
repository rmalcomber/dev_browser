# M1 exit test (on your machine, with two real accounts)

The M1 exit test from the milestones file: *log in to the same site as two identities in two workspaces, restart, both still signed in,
nothing shared.* The automated checks (`tools\m12-workspaces.ps1`, `tools\m15-navigation.ps1` for S10, `tools\m17-restore.ps1`) prove the
mechanism with a stand-in site. This is the same thing by hand, on a real site, which is the only way to learn whether a real login
survives. Takes about 10 minutes. Do not use an account you cannot afford to be logged out of.

1. `tools\build.ps1 run` (use `-- --data-dir=C:\temp\devb-exit` for a throw-away profile, or no flag for your real one).
2. In the first workspace open a site you have two accounts on (a mail provider, a code host) and sign in as account A. Tick "stay signed in".
3. `Alt+N`, name a second workspace, and open the same site there. Expected: **signed out**, not account A. Sign in as account B.
4. Open a few more tabs in each workspace; group some (`Ctrl+Shift+G`). Resize the window.
5. Close the window with the X (once). Start the app again.
   Expected: both workspaces, the same tabs and groups, the same selected tab, the same window size, **A in the first workspace and B in the
   second, no prompt to log in**.
6. Open the site in the first workspace's sidebar button "to Second" (opens in the other workspace's own session). Expected: B, not A.
7. Kill the app from Task Manager (End task on every `dev_browser.exe`), wait a minute, start it again. Expected: same as 5. A login made in
   the last half minute before a kill may be lost; that is Chromium's cookie flush interval, not a bug.
8. Delete the second workspace. Expected: its tabs go, the first is still signed in as A, and the site, opened in a new workspace,
   shows nobody signed in.

Record the result (pass, or what failed) under "M1.7" in `docs/ai/decisions.md`.
