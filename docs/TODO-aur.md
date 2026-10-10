# TODO: publish to the AUR (blocked)

AUR new-account registration has been closed since about 2026-10-08 (bot wave). Finish this when it reopens; watch the Arch news feed.

Already done: `.github/workflows/aur.yml` (it skips until the secrets below exist), and an SSH key at `%USERPROFILE%\.ssh\aur_silent_voice` (keep it; it doesn't expire).

1. Register at aur.archlinux.org, then paste `aur_silent_voice.pub` into **My Account > SSH Public Key**.
2. Add these secrets under GitHub **Settings > Secrets and variables > Actions**: `AUR_USERNAME`, `AUR_EMAIL`, `AUR_SSH_PRIVATE_KEY`. To copy the private key:
   `Get-Content "$env:USERPROFILE\.ssh\aur_silent_voice" -Raw | Set-Clipboard`
3. In GitHub, open **Actions > Publish to AUR > Run workflow**, then check that https://aur.archlinux.org/packages/silent-voice-bin exists.
