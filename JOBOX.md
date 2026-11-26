# JoBox Support

Branded RustDesk client for JoBox remote support (Windows 10/11 x64).

- Base: RustDesk 1.5.3 (stable).
- Built-in settings: `JOBOX_CLIENT_CONFIG` in `src/common.rs`
  (app name `JoBoxSupport`, JoBox server and key locked; by default a session starts
  only after the client clicks Accept, password access can be enabled in
  Settings -> Security, which needs administrator rights to unlock).
  `JoBoxSupport.exe --password <pwd>` (admin, used by the JoBox programs' error report
  when the client asks for it) also switches approve-mode to password + click.
- Updates: `src/jobox_update.rs` reads the `jbsupport` section of
  `https://ячейка-пвз.рф/update_jobox.json` (the same file the other JoBox programs use)
  and compares `latest_version` with the version the installer wrote to the registry.
  The home page then shows an "Обновить" card; the installer is downloaded from the
  JoBox site only and started with UAC. The version number lives only in the installer script.
- Build: `.github/workflows/jobox-build.yml` runs on every push to the `jobox` branch;
  the result is the `jobox-support-windows-x64` artifact (folder with `JoBoxSupport.exe`).
- The installer is built separately with Inno Setup from that folder.
