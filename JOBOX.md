# JoBox Support

Branded RustDesk client for JoBox remote support (Windows 10/11 x64).

- Base: RustDesk 1.5.3 (stable).
- Built-in settings: `JOBOX_CLIENT_CONFIG` in `src/common.rs`
  (app name `JoBoxSupport`, JoBox server and key, connection only after the client clicks Accept).
- Build: `.github/workflows/jobox-build.yml` runs on every push to the `jobox` branch;
  the result is the `jobox-support-windows-x64` artifact (folder with `JoBoxSupport.exe`).
- The installer is built separately with Inno Setup from that folder.
