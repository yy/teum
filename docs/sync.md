# Sync

teum stores everything as text files in a directory. How you sync that directory is up to you.

## Option 1: iCloud (recommended for Apple devices)

Point the data directory to iCloud Drive. Files sync automatically across Mac, iPhone, and iPad.

```toml
# ~/.config/teum/config.toml
data_dir = "~/Library/Mobile Documents/com~apple~CloudDocs/teum"
```

Then run `teum init` to create the directory.

On iOS, open the Files app and navigate to iCloud Drive > teum. The `.txt` files are editable directly.

Pros:
- Automatic sync
- Editable on iOS without extra apps
- Works offline (syncs when connected)

Cons:
- No version history
- Conflict resolution is file-level (last write wins)
- Apple ecosystem only

## Option 2: Git

Keep the data directory as a git repository. This gives you version history and works with any git hosting.

```toml
# ~/.config/teum/config.toml
sync = "git"
auto_commit = false
```

Setup:

```bash
teum init                        # creates data dir + git init
cd ~/.local/share/teum
git remote add origin <url>      # add your remote
teum sync                        # add, commit, pull --rebase, push
```

The `teum sync` command runs:

1. `git add -A`
2. `git commit -m "teum: sync YYYY-MM-DD HH:MM"`
3. `git pull --rebase`
4. `git push`

If `auto_commit = true`, `teum stop` automatically commits (but does not push). Run `teum sync` when you want to push.

### Merge conflicts

Conflicts are rare because:
- Each week has its own file, so two devices rarely touch the same file
- Within a file, new entries are appended (different lines)
- Timestamps are unique, so two entries never collide

If a conflict does occur, both sides are human-readable lines. Keep both, sort by timestamp.

### Mobile with Working Copy

[Working Copy](https://workingcopy.app) is a full git client for iOS. You can:
- Clone the data repo
- Edit `.txt` files directly
- Commit and push

This gives you git-based sync with mobile editing.

## Option 3: Local only

The default. Data stays in `~/.local/share/teum/`. Back up the directory yourself.

## Option 4: Other sync tools

Any file sync tool works: Dropbox, Syncthing, rsync, Unison. Point `data_dir` at a synced directory.

## Mobile workflows

### iOS Shortcuts

Create a "Start Timer" shortcut:

1. **Get Current Date** > format as `yyyy-MM-dd HH:mm`
2. **Choose from Menu**: Focus, Support, Side, Personal
3. **Ask for Input**: activity description
4. Construct the line: `{date} -       | @{project} #activity | {description}`
5. **Append to File**: current week's `.txt` in iCloud Drive

A "Stop Timer" shortcut:

1. Read the last line of the current week file
2. Get current time, format as `HH:mm`
3. Replace the blank end time with the current time
4. Write the file back

### Claude Code remote triggers

If you use Claude Code, you can set up scheduled triggers:

```bash
# Stop tracking at end of day
teum stop --at 17:00

# Check what's running
teum status
```
