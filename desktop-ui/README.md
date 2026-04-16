# Desktop UI

`desktop-ui` is a Vite web dashboard for the Rust backend. It is not a working Electron app, and the Tauri wiring is incomplete at the moment.

## Prerequisites

- Node.js `24.13.1` or compatible
- npm `11+`
- Rust backend from the workspace root

## Install

```powershell
cd desktop-ui
npm install
```

## Development

Run the backend from the repository root:

```powershell
cargo run -p fork_hunter_bin --bin fork_hunter_bin
```

Run the UI in a second shell:

```powershell
cd desktop-ui
npm run dev
```

Expected local ports:

- UI: `http://localhost:1420`
- Backend HTTP/API: `http://localhost:8080`
- Backend WebSocket: `ws://localhost:8080/ws/v1/surebets`

## Build

```powershell
cd desktop-ui
npm run build
```

## Tauri status

`npm run tauri:dev` and `npm run tauri:build` are declared in `package.json`, but this repo does not currently contain a complete `src-tauri` application. Treat the UI as a web frontend until Tauri configuration is added.

