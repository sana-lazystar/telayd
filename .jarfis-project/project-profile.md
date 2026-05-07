# Project Profile: Telayd

> Generated manually on 2026-05-06 (greenfield — no codebase yet, `/jarfis:project-init` deferred until L0 빌드 시작)
> Depth: medium (planned)
> Type: Fullstack (Daemon = Rust backend / PWA = React frontend / shared protocol types via typeshare L1+)
> org: /Users/sanhalee/AI-OVERALL/telayd-org
> Last-Commit: (initial)
> 빌드 위치: `~/AI-OVERALL/telayd-org/telayd/`

---

## Tech Stack

- **Language (Daemon)**: Rust 1.80+ (stable channel)
- **Language (Frontend)**: TypeScript 5.x
- **Runtime (Frontend)**: Node.js 22 LTS
- **Build (Daemon)**: Cargo (workspace)
- **Build (Frontend)**: Vite 6.x
- **Package Manager (JS)**: pnpm 9.x (workspace mode)
- **Desktop App framework**: Tauri 2.x (L1+, not L0)
- **Mobile**: PWA (Vite + React) → React Native (L2 if needed)

### Key Dependencies (planned, L0)

**Daemon (Rust)**:
- `tokio` — async runtime
- `axum` 또는 `tungstenite` — WebSocket server
- `serde` + `serde_json` — JSON serialization (hook payload)
- `rand` — OsRng for pairing token (32-byte URL-safe random)
- `directories` — `~/.config/telayd/` resolution (XDG)
- `tokio-tungstenite` — async WebSocket client (cloudflared 통신)
- `clap` — CLI parsing (`telayd init` / `telayd start` / `telayd stop` / `telayd logs`)

**Frontend (PWA)**:
- `react` + `react-dom`
- `vite` + `@vitejs/plugin-react`
- WebSocket native API (no library)
- `@types/react`

**External binary** (자동 install):
- `cloudflared` (Apache 2.0) — `~/.local/bin/cloudflared`로 자동 download + SHA256 검증

## Active Skills

> Sub-agent에 자동 주입할 skill. `~/.claude/commands/jarfis/skills/`의 파일 참조.

- `rust` — Rust 작성/리뷰
- `cargo-clippy` — Rust lint
- `tauri-backend` — Tauri ipc + command pattern
- `tauri-webview` — Tauri webview integration (L1+ desktop 진입 시)
- `react` — React + JSX patterns
- `nodejs` — Node.js / TypeScript backend (PWA build)
- `biome-lint` — Biome lint/format (TS code)

## Directory Structure (planned, L0)

```
telayd/
├── packages/
│   ├── daemon/                # ✅ L0 — Rust workspace member
│   │   ├── src/
│   │   │   ├── main.rs        # CLI entry (clap)
│   │   │   ├── hook_installer.rs    # ~/.claude/settings.json 등록/제거
│   │   │   ├── hook_script.rs       # bash script generation + permission
│   │   │   ├── tmux_controller.rs   # send-keys + dialog ready detection
│   │   │   ├── sentinel_parser.rs   # stub (L0 noop)
│   │   │   ├── cf_tunnel.rs         # cloudflared spawn + URL capture
│   │   │   ├── ws_bridge.rs         # localhost:7777 ws server
│   │   │   ├── ipc.rs               # Unix socket (~/.config/telayd/daemon.sock) listener
│   │   │   ├── pairing.rs           # 32-byte token gen + config.toml
│   │   │   └── lib.rs
│   │   ├── Cargo.toml
│   │   └── README.md
│   └── pwa/                   # ✅ L0 — Vite + React workspace member
│       ├── src/
│       │   ├── App.tsx              # Root component
│       │   ├── views/
│       │   │   ├── Pairing.tsx      # token + tunnel URL 입력
│       │   │   └── Session.tsx      # choice UI (4-button)
│       │   ├── lib/
│       │   │   ├── ws_client.ts     # WebSocket client + reconnect
│       │   │   └── types.ts         # Hook payload types (L1+ shared via typeshare)
│       │   └── main.tsx
│       ├── index.html
│       ├── package.json
│       ├── vite.config.ts
│       └── README.md
├── .jarfis-project/
│   ├── project-profile.md     # 이 파일
│   ├── project-rule.md
│   └── project-context.md     # 빌드 후 jarfis:project-update 자동 생성 (현재는 없음)
├── pnpm-workspace.yaml        # packages/* glob
├── package.json               # root, private: true
├── Cargo.toml                 # workspace root
├── README.md                  # placeholder
├── LICENSE                    # FSL-1.1-MIT
├── NOTICE                     # cloudflared Apache 2.0 attribution
├── CLAUDE.md                  # JARFIS 작업 규칙
└── .gitignore                 # ~/.config/telayd/ 제외, target/, node_modules/, dist/
```

L0에서 **만들지 않는 패키지** (L1+ 진입 시): `desktop/` (Tauri), `ui-shared/`, `protocol-types/`, `apps/mobile-rn/`.

## 별도 위치 SSOT (telayd repo 안에 두지 않음)

JARFIS sub-agent가 컨텍스트 inject 받을 때 다음 경로 reference (모두 absolute):

| 자료 | 위치 | 용도 |
|---|---|---|
| Planning SSOT root | `~/AI-OVERALL/telayd-org/planning/` | 모든 기획 산출물 + INDEX/CLAUDE |
| Architecture spec | `~/AI-OVERALL/telayd-org/planning/architecture/{telayd-protocol,licensing-audit}.md` | spec v0.2.x + 라이선스 audit |
| Scope spec (L0/L1/L2) | `~/AI-OVERALL/telayd-org/planning/mvp-scope/scope.md` | v0.2.3, 21 acceptance criteria + module dep graph |
| Decisions (ADR) | `~/AI-OVERALL/telayd-org/planning/decisions/{README, 001-008}.md` | 8 ADR sequential, 결정 history |
| Brainstorms | `~/AI-OVERALL/telayd-org/planning/brainstorms/b-{1..N}.md` | append-only 사고 흐름 |
| Research | `~/AI-OVERALL/telayd-org/planning/research/competitive-landscape.md` | 5 카테고리 시장조사 |
| Narrative (글감) | `~/AI-OVERALL/telayd-org/planning/narrative/{README, 01-06}.md` | LinkedIn / landing copy 자료 |
| **JARFIS meetings** | `~/repos/jarfis/.personal/orgs/Telayd/meetings/{YYYYMMDD}-<name>/` | work-meeting.md spec align ($JARFIS_ORG_DIR/meetings/) |
| Spike code | `~/AI-OVERALL/telayd-org/planning/prototypes/{NN-name}/` | throwaway 검증 코드 archive |

## Scripts & Commands

| Task | Command |
|------|---------|
| Build daemon (release) | `cargo build --release -p telayd-daemon` |
| Build daemon (dev) | `cargo build -p telayd-daemon` |
| Run daemon | `./target/release/telayd start` |
| PWA dev server | `pnpm --filter pwa dev` |
| PWA build | `pnpm --filter pwa build` |
| PWA preview | `pnpm --filter pwa preview` |
| Lint Rust | `cargo clippy --workspace -- -D warnings` |
| Lint TS | `pnpm --filter pwa lint` (Biome) |
| Test Rust | `cargo test --workspace` |
| Format | `cargo fmt --all && pnpm --filter pwa format` |
| Init (사용자) | `telayd init` (cloudflared install + token gen + config) |

## Config Summary

- **Cargo workspace**: `packages/daemon/` 단일 member L0. Cargo.toml workspace root에 resolver = "2".
- **pnpm workspace**: `packages/*` glob. L0 active = `pwa/`만.
- **Biome**: Vite default + `recommended: true`. pnpm 통합.
- **TypeScript**: `strict: true`, `target: ES2022`, `module: ESNext`.
- **Rust edition**: 2021. clippy = `cargo clippy --all-targets --all-features -- -D warnings` enforce.

## Coding Conventions (medium analysis — 빌드 후 보강)

> L0 빌드 시작 후 `/jarfis:project-update`로 자동 보강. 현재는 minimum.

- **File Naming**:
  - Rust: `snake_case.rs`
  - TS: `PascalCase.tsx` (component) / `camelCase.ts` (lib/util)
- **Component Structure**: feature-based co-location (`views/Pairing.tsx` 한 파일에 component + sub-types)
- **Import Rules**: Rust = explicit `use crate::module::Item;`. TS = relative + barrel index.ts 회피 (Vite tree-shake)
- **Type Definitions**: Rust = `struct` + `serde::{Serialize, Deserialize}` + `derive` macro. TS = `type` (shape) / `interface` (extension 가능 시).
- **Error Handling**: Rust = `anyhow::Result<T>` (daemon-level), `thiserror` (library 경계). TS = explicit error type or `Result<T, E>` discriminated union.

## Notes & Caveats

- **Greenfield**: 이 profile은 코드 0인 시점에 manually 작성. JARFIS sub-agent들은 spec/scope 위주로 reasoning.
- **Project context**: `.jarfis-project/project-context.md`는 **빌드 후** (`/jarfis:project-update` 또는 manual)에 codebase navigation 보조용으로 생성.
- **Phase 1a 답안 (lock)**: `design.mode = text`, `responsive = pc-mobile`, `api.mode = null`, `devops = false`, `ux-direction = false`, `legal-review = false`.
- **L0 acceptance criteria**: scope.md §3.4 (21개) 참조. Anthropic GitHub issues #29214/#29438/#34581/#35637 시나리오 검증이 thesis 핵심.
