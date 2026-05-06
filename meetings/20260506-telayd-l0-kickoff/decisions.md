# telayd-l0-kickoff Decisions

| # | Decision | Rationale | Alternatives Considered | Decided By | Status |
|---|----------|-----------|------------------------|------------|--------|
| 1 | **사업화 NO** — dogfooding + portfolio + JARFIS testbed only | 회사 만들 의지 약함 + 5명 인터뷰 못 잡음 + GTM 견디는 편. Anthropic Remote Control이 Claude Code 단독 wedge 이미 commodity화. | (a) full 사업화 (취소 — 1+5+4 조합으로 OUT) / (b) 순수 portfolio glas만 (취소 — 약속 이행 안 됨) / (c) side-build OSS + portfolio narrative (선택) / (d) 미니멀 종결 (1편 글) | Owner | Confirmed |
| 2 | **L0 dogfooding lock** — 본인 mac 1대에서 매일 쓰는 working spike | 외부 보상 의존 X / 1st-party 따라잡기 X / 사용자 매주 사용 = sustainability metric | L1 (지인 N명 OK) / L2 (vision 도달) — 둘 다 L0 done 후 별도 brainstorm | Owner | Confirmed |
| 3 | **Time budget 무제한 + 완성도 최우선** | dogfooding 동기. 단 weekly self-check + acceptance criteria가 scope drift 가드레일 | 주 N시간 × 8주 cap (취소 — 사용자 의지에 반함) | Owner | Confirmed |
| 4 | **Brand = telayd** (e 한 개, 텔레이드, "tele+relay+d") | b-1 brand audit (2026-05-04): 도메인 11 TLD + GitHub + npm + PyPI + Homebrew 모두 사용 가능, 검색 결과 0건 | Estafette (브랜드) + telayd (CLI) — 취소 | b-1 lock | Confirmed |
| 5 | **GitHub repo `sana-lazystar/telayd`** | 오타 `telayed` → rename 완료 | (별도 GitHub org `telayd` — L1 진입 시 검토) | b-4 | Confirmed |
| 6 | **Monorepo (pnpm workspace) — 단일 git repo** | TypeScript types/utils 공유 (Rust↔TS protocol type drift 방지) + atomic commit + 솔로 cognitive overhead 회피 | Polyrepo (취소 — drift risk + multi-repo 동기화 cost) | b-3 §1 + b-4 confirm | Confirmed |
| 7 | **L0 빌드 패키지 = `packages/daemon/` + `packages/pwa/`만** | trigger 발생 시 생성 정신. 다른 패키지 (desktop / ui-shared / protocol-types / mobile-rn / apps)는 L1+ 진입 시 | 모든 패키지 빈 scaffold 미리 생성 (취소 — anti-pattern) | b-4 | Confirmed |
| 8 | **Tech Stack** = Tauri + Rust + React + TypeScript + Vite + pnpm workspace | b-3 §1 결정. Desktop App = Tauri (L1+), Daemon = Rust, Mobile App = PWA → RN(L2 if needed) | Electron (취소 — bundle size) / 네이티브 Swift (취소 — cross-platform) / Native iOS (취소 — RN으로 대체 가능 시) | b-3 | Confirmed |
| 9 | **Transport = Cloudflare Tunnel** (L0 quick / L1 named / L2 hosted relay 셀프호스팅) | 사용자 마찰 zero, 운영자 비용 zero. cloudflared Apache 2.0. b-3 lock. | Tailscale (취소 — 사용자 가입 마찰 + Free tier 1 user 제한 시 충돌) / 자체 relay (취소 — 운영자 비용) / WebRTC (취소 — overkill) | b-3 §2 | Confirmed |
| 10 | **Push = WebSocketOnly L0**, abstraction trait 미리. L1+ FCM (Android), APNs는 b-4 사업화 NO로 미진입 (PWA push API로 iOS 대체 검토 L2) | iOS push 비용 = $99/년 Apple Dev. 사업화 NO이므로 미진입. PWA push API (서비스워커 + VAPID)는 iOS 16.4+ 부분 지원 | + APNs $99/년 운영자 부담 (취소 — b-4) / + 자체 push 서버 (취소 — 운영자 비용) | b-3 §3 + b-4 정정 | Confirmed |
| 11 | **라이선스 = FSL-1.1-MIT** (소스 100% 공개, 첫 2년 직접 경쟁 SaaS 차단, 2년 후 자동 MIT 전환) | b-3 §라이선스 + licensing-audit.md. portfolio 효과 + 사업화 가능성 보존 (실제 사업화 NO이지만 옵션 보존) | MIT 즉시 (취소 — fork+MIT로 우회 위험) / BSL (취소 — 채택률 ↓) / proprietary (취소 — OSS 정신과 충돌) | b-3 + b-4 | Confirmed |
| 12 | **IPC 채널 분리** — mobile-facing localhost:7777 (plain ws, cloudflared가 wss로 termination) + hook IPC Unix socket `~/.config/telayd/daemon.sock` | port collision 회피 + filesystem permission으로 security. spec v0.2.2 §3.1 align. | 단일 port + path-based routing (취소 — 복잡도) / TCP localhost 두 port (취소 — port 점유 두 배) | b-4 audit fix | Confirmed |
| 13 | **Hook Installer L0 = global `~/.claude/settings.json`** (project-local 미사용) | 사용자가 multi-project 다닐 때 매번 install 부담 회피. spec v0.2.2 §3.1 둘 다 허용하나 L0 lock | project-local `.claude/settings.json` (취소 L0 / L1 검토 open question) | b-4 audit fix | Confirmed |
| 14 | **Pairing token L0** = 32-byte (256-bit) URL-safe random, RFC 4648 base64url no-padding (43 chars), `telayd init` 생성, `~/.config/telayd/config.toml` (chmod 600) | OsRng / `/dev/urandom` 기반. 단일 device 전제. L1에서 QR + JWT 5분 expiry 추가. | 더 짧은 token (취소 — entropy 부족) / longer + complex (취소 — manual 입력 마찰) | b-4 audit fix | Confirmed |
| 15 | **L0 Acceptance Criteria 21개** = Functional 9 + Quality 3 + Performance 1 + Security 4 + Build/Distribution 4. **Anthropic 4 wedge issues 검증** 핵심 (#29214/#29438/#34581/#35637). | thesis 검증 = "permission UX 정확도가 Anthropic Remote Control보다 더 정확한가". 측정 가능성 확보. | "본인 7일 사용" 단일 acceptance (취소 — wedge 검증 불가) | b-4 audit | Confirmed |
| 16 | **JARFIS v4 workflow에 빌드 위임** — `/jarfis:work L0 daemon prototype --meeting telayd-l0-kickoff` 호출 | dogfooding으로서의 핵심 가치 = JARFIS 자체 검증/진화 + multi-agent 병렬 빌드 검증. | 직접 빌드 (취소 — JARFIS dogfooding 가치 sacrifice) | b-4 | Confirmed |

## Open Items (다음 단계 결정 필요)

- [ ] L1 진입 시점 결정 — L0 acceptance 모두 충족 + 7일 사용 후 부족함 N개 누적 시 별도 brainstorm
- [ ] Brand asset lock (telayd.dev 도메인 등록 / Homebrew tap 빈 repo) — L1 진입 prerequisite, 30분 작업
- [ ] Skill DSL 형식 (YAML / TS DSL / Markdown front-matter) — L2 진입 시 결정
- [ ] LinkedIn JARFIS series 후속작 — L0 done 시 1편 / OSS release 시 1편 (timing TBD)
- [ ] Hook async 5초 timeout L0 옵션 (a)/(b)/(c) 중 (a) 잠정 — dogfooding으로 검증
