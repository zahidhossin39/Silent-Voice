# Starts the full dev app (cargo is not on the system PATH on this machine).
$env:PATH += ";$env:USERPROFILE\.cargo\bin"
npm run tauri:dev
