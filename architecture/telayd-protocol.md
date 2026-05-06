# Telayd Protocol — Spec v0.2.2 (MVP, Hybrid + Pluggable Transport/Push)

> 작성일: 2026-05-04 (v0.2.1 — b-3 transport 결정 반영) / 2026-05-06 (v0.2.2 — b-4 사업화 NO 결정 + audit fix 반영, §9 changelog 참조)
> 상태: **MVP lock — 데몬 prototype 본격 시작 가능**
> 검증 근거: `prototypes/01-tmux-roundtrip/` ~ `prototypes/06.4-gemini-hooks/` (총 8 spike)
> 라이선스: FSL-1.1-MIT (소스 공개, 2년 후 MIT 자동 전환 — `architecture/licensing-audit.md` 참조)

---

## 1. Goals & Non-Goals

### Goals
- LLM CLI 세션의 **인터랙티브 prompt를 외부 채널(모바일)로 라우팅**하는 메커니즘 정의
- LLM CLI의 **native 인터랙션 도구**를 깨끗하게 가로채기 (LLM은 Telayd 존재 모름)
- **장시간 작업의 진행 상황**을 외부에 알리는 메커니즘 (이건 LLM이 직접 emit)
- Claude Code / Gemini CLI / Codex CLI 어느 환경에서도 동일 패턴 적용 (LLM-agnostic 목표)

### Non-Goals
- **Transport** (데몬 ↔ 모바일 통신 프로토콜): 별도 spec, 추후
- **UI 렌더링**: 모바일/데스크탑 앱 자유 (어떤 컴포넌트로 그릴지)
- **인증/페어링**: 별도 spec
- **Skill 정의 포맷 표준** (LLM이 작성하는 Skill): MVP 범위 밖

---

## 2. Architecture Overview (v0.2 핵심)

Telayd v0.2는 **하이브리드** — 두 레이어로 분리:

| 레이어 | 대상 인터랙션 | 메커니즘 | LLM이 spec 알아야 함? |
|---|---|---|---|
| **Inquiry** | 질문/입력/확인 (choice / text / confirm) | Native 인터랙션 도구 (예: Claude의 `AskUserQuestion`) + PreToolUse hook 가로채기 + tmux send-keys 응답 inject | ❌ 모름 |
| **Status** | 진행 상황 알림 (응답 없음) | LLM이 stdout에 sentinel emit + 데몬이 pipe-pane으로 감지 | ⭕ 알아야 함 (간단 SKILL.md 1개) |

### Inquiry layer — 데이터 흐름

```
[LLM] AskUserQuestion 호출 (choice/text/confirm 의도)
       ↓
[PreToolUse hook] ← Telayd 데몬이 사전 install
       ↓ (exit 0, 즉시 통과)
       │
       ├── [native UI] 데스크탑 터미널에 dialog 그려짐
       │         ↓ 데스크탑 사용자가 키보드로 응답 가능
       │
       └── [Telayd 데몬] hook이 IPC로 question 전달
                   ↓
               [모바일 push]
                   ↓ (사용자 모바일 응답)
                   ↓ (initial timeout 분~시간 OK, hook은 이미 return)
                   ↓
           [tmux send-keys] 데몬이 native dialog에 응답 inject
                   ↓
           [native UI close] LLM이 native tool result로 답 수신
                   ↓
           [LLM] 작업 계속 — 어떤 채널로 답이 왔는지 모름
```

**Race 처리:** 데스크탑 사용자가 native UI에서 직접 답하면 dialog가 close됨 — 그 후 모바일 응답이 오면 데몬이 PostToolUse hook으로 close 감지하고 무시.

### Status layer — 데이터 흐름

```
[LLM] long task 진행 중
       ↓
[stdout에 sentinel emit] <<TELAYD|status|msg=Reviewing|pct=30>>
       ↓
[tmux pipe-pane] 데몬이 감지 (ANSI strip + grep)
       ↓
[모바일에 push notification] (응답 없음, fire-and-forget)
       ↓
[LLM] 계속 작업 (응답 안 기다림)
```

---

## 3. Inquiry Layer Spec

### 3.1 PreToolUse hook contract

데몬이 `~/.claude/settings.json` 또는 프로젝트 `.claude/settings.json`에 다음 hook을 자동 등록:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "AskUserQuestion",
        "hooks": [
          {
            "type": "command",
            "command": "telayd-hook-emit",
            "timeout": 5000
          }
        ]
      }
    ]
  }
}
```

**Hook script 동작 (5초 안에 끝나야 함):**

1. stdin에서 JSON 읽기
2. `tool_use_id`, `tool_input.questions[*]` 추출
3. Telayd 데몬 IPC(unix socket: `~/.config/telayd/daemon.sock`)에 emit
4. `exit 0` (allow — native UI가 정상적으로 그려지도록)

**Hook이 받는 JSON (Claude Code v2.x 기준 — spike 06 실측):**

```json
{
  "session_id": "...",
  "transcript_path": "...",
  "cwd": "...",
  "permission_mode": "default",
  "hook_event_name": "PreToolUse",
  "tool_name": "AskUserQuestion",
  "tool_input": {
    "questions": [
      {
        "question": "Which database should we use?",
        "header": "Database",
        "options": [
          {"label": "PostgreSQL", "description": "..."},
          {"label": "MySQL", "description": "..."},
          {"label": "SQLite", "description": "..."}
        ],
        "multiSelect": false
      }
    ]
  },
  "tool_use_id": "toolu_011T4ULmcv4RjEx3Y24FCsoW"
}
```

### 3.2 Telayd internal "Inquiry" record

데몬은 hook으로부터 받은 정보를 다음 형태로 보유 + 모바일에 push:

```json
{
  "kind": "inquiry",
  "tool_use_id": "toolu_011T4ULmcv4RjEx3Y24FCsoW",
  "session_id": "9551a90d-...",
  "tmux_session": "telayd-poc-...",
  "header": "Database",
  "questions": [
    {
      "question": "Which database should we use?",
      "options": [
        {"index": 1, "label": "PostgreSQL", "description": "..."},
        {"index": 2, "label": "MySQL", "description": "..."},
        {"index": 3, "label": "SQLite", "description": "..."}
      ],
      "multiSelect": false
    }
  ],
  "created_at": "<ISO-8601>"
}
```

→ Native AskUserQuestion 형식을 거의 그대로 채택. 변환 부담 최소.

### 3.3 응답 inject — Send-keys mapping

모바일에서 응답 수신 시 데몬이 native dialog에 키 입력 inject:

| 옵션 인덱스 | 옵션 수 | Inject keystroke (tmux send-keys) |
|---|---|---|
| 1 (default) | 임의 | `Enter` (default 선택) |
| 2~9 | ≤ 9 | `<digit>` `Enter` (예: `2` `Enter`) |
| 10+ | > 9 | `Down` × (idx-1) `Enter` (navigation fallback) |

**검증된 방식 (spike 06.1):**
- T1: `Down` `Enter` → 2번 옵션 선택 ✓
- T2: `3` `Enter` → 3번 옵션 선택 ✓
- T3: `Enter` only → 1번 default 선택 ✓

**send-keys 시점 (안전성):**
- Hook 발화 직후가 아니라 dialog가 fully rendered된 후
- 데몬은 pipe-pane stream을 감지해 `Enter to select` 또는 `↑/↓ to navigate` 마커 출현 후 inject
- 추가 안전 마진 0.5-1초

### 3.4 `text` (free input) 처리

Native AskUserQuestion이 옵션 N개 + "Type something" meta-option을 자동 제공. 사용자가 자유 입력 원하면:

1. 데몬이 send-keys로 `Down` × (option수) `Enter` → "Type something" 선택
2. Dialog가 텍스트 input mode로 전환
3. 데몬이 모바일에서 받은 텍스트를 `tmux send-keys '<text>' Enter`로 inject

→ `text` type을 별도로 spec할 필요 없음. AskUserQuestion이 통합 처리.

### 3.5 `confirm` (yes/no) 처리

Native AskUserQuestion에 옵션이 2개(예: `["Yes", "No"]`)면 자연스럽게 confirm. LLM이 호출 시 옵션 2개로 만들어주면 됨.

→ `confirm`도 `choice`와 동일 메커니즘. 별도 type 분리 없음.

### 3.6 `multiSelect: true` 처리

(spike S6.3에서 검증 예정.) 잠정 가설:
- Native dialog가 체크박스 형태로 그려짐
- `Space`로 toggle, `Enter`로 confirm
- 데몬이 인덱스 리스트 받아서 `Down`/`Space`/`Enter` 시퀀스 inject

상세는 spec v0.3에서 보강.

### 3.7 Async pattern (timeout 우회)

Hook 자체는 `timeout: 5000` (5초) 안에 return해야 함. 모바일 응답 대기는 분~시간 단위 가능. 해결:

- **Hook은 emit만 하고 즉시 exit 0** — blocking 안 함
- **Native UI가 그대로 떠있음** — 사용자가 데스크탑/모바일 어느 쪽으로든 응답 가능
- **데몬이 백그라운드에서 모바일 응답 대기 + send-keys inject** — hook 컨텍스트 밖
- **Race 처리**: 데스크탑 사용자가 먼저 응답하면 dialog close → 데몬이 PostToolUse hook으로 감지 → 모바일 응답 ignore

이 패턴이 spike 06.1에서 mechanically 검증됨.

### 3.8 Cancel / 응답 없음 처리

사용자가 데스크탑/모바일 어디서도 응답 안 함:
- Native dialog는 무한 대기 (Claude Code 내부 timeout 있을 수 있음, 미확인)
- MVP는 **그대로 둠** — 사용자가 돌아와서 응답할 때까지
- v1+: 데몬이 일정 시간 후 모바일에 reminder push

사용자가 명시적 cancel:
- Native UI에서 `Esc` → tool denied → LLM이 다른 길 모색
- 모바일에서 cancel 버튼 → 데몬이 `Esc` 키 inject

---

## 4. Status Layer Spec

### 4.1 Sentinel format

```
<<TELAYD|status|msg=<text>|pct=<0-100>>>
```

| 필드 | 필수 | 의미 |
|---|---|---|
| `msg` | ✓ | 진행 메시지 (공백 → `+`) |
| `pct` | ✗ | 진행률 정수 0-100 |

예:
```
<<TELAYD|status|msg=Reviewing+files|pct=30>>
<<TELAYD|status|msg=Building+release+artifacts>>
<<TELAYD|status|msg=완료+직전|pct=95>>  (Korean OK, multi-byte pass-through)
```

### 4.2 추출/파싱 규칙 (v0.1과 동일)

데몬은 다음 순서로 처리:

1. **ANSI strip**: byte stream에서 ANSI escape sequences 제거
   - reference: `prototypes/02-ansi-handling/spike.sh::strip_ansi()`
2. **Whitespace strip** (sentinel 내부 한정): TUI cursor positioning으로 공백 소실 가능 (spike 03 finding)
3. **경계 매칭**: 정규식 `<<TELAYD\|status\|[^>]*>>` 으로 추출
4. **필드 분리**: `|` split → first=`status` → key=value pairs
5. **값 디코딩**: `+` → space (`%XX` 인코딩은 status에서 사실상 불필요)

### 4.3 Buffering (Streaming 안전)

LLM이 token-by-token으로 출력 → 한 시점 stream에 partial sentinel만 있을 수 있음.

- 데몬은 byte stream 누적 buffer 유지
- `<<TELAYD|status|` 발견 시 `>>` 종결까지 다음 청크 대기
- 종결 도착 시 추출 후 buffer에서 제거
- **버퍼 상한: 8KB per session** (status sentinel은 훨씬 작음; 초과 시 mangled로 간주, 버림)

### 4.4 사용자 echo 구분

사용자가 paste한 프롬프트에 sentinel이 있으면 echo로 잡혀 false trigger 가능 (spike 03 finding #2).

- **Bracketed paste markers** (`\x1b[200~ ... \x1b[201~`) 사이에 있는 sentinel은 무시
- v0.2 MVP에서는 이것만으로 충분 (대부분 케이스 커버)

### 4.5 LLM-side packaging (SKILL.md)

데몬이 `~/.claude/skills/telayd-protocol/SKILL.md` (또는 프로젝트 `.claude/skills/`)에 다음을 install:

```markdown
---
name: telayd-protocol
description: Use to report progress during long tasks (>5 seconds) so the user can see you are still working, even when away from desktop.
---

# Telayd — Status Reporting

When doing a long task, periodically emit a status sentinel so the user (possibly on mobile) sees progress:

<<TELAYD|status|msg=Reviewing+files|pct=30>>

## Format
- One line, on its own line
- `msg`: required, spaces encoded as `+`
- `pct`: optional integer 0-100
- Multi-byte (Korean, Japanese, Chinese, emoji): pass through as-is

## When to use
- Tasks taking > 5 seconds
- Every 10-30 seconds during the task
- Final completion: `<<TELAYD|status|msg=Done|pct=100>>`

## Important
For asking questions, getting input, or confirming actions, just use the AskUserQuestion tool naturally. Telayd routes it automatically — you do NOT emit sentinels for those.
```

→ v0.1의 4 message types 패키징 대비 **압도적으로 단순**.

Other CLI packaging:
- Gemini CLI: `--system-prompt-file telayd-status.md` 또는 `gemini skills` 메커니즘 (S6.4에서 검증)
- Codex CLI: 동등 메커니즘 (별도 spike)

---

## 5. Identity & Routing

### 5.1 Inquiry routing

- **ID**: Native `tool_use_id` (예: `toolu_011T4ULmcv4RjEx3Y24FCsoW`) 그대로 사용
- 데몬이 `tool_use_id` → tmux session 매핑 유지
- 모바일 응답 시 데몬이 해당 session에 send-keys
- LLM은 native tool 결과로 받기 때문에 ID 매칭 불필요

### 5.2 Status routing

- ID 없음 (응답 없는 fire-and-forget)
- 데몬이 status를 어느 session에서 발생했는지만 추적해 모바일에 표시 (예: "Session 'work-1': Reviewing files (30%)")

---

## 6. Daemon Implementation (high-level modules)

확정된 5개 모듈:

### 6.1 Hook Installer
- 데몬 시작 시 `~/.claude/settings.json` 또는 프로젝트 `.claude/settings.json`에 PreToolUse hook 자동 등록
- 종료 시 cleanup (또는 install된 채로 두고 데몬 부재 시 hook이 no-op하게 — 더 robust)
- Conflict 처리: 기존 hook과 공존 (multiple matchers 지원)

### 6.2 Hook Script
- `telayd-hook-emit` (Telayd 데몬과 함께 배포되는 작은 실행 파일)
- stdin JSON → unix socket emit → exit 0
- 매우 작음 (~50 LOC bash 또는 ~100 LOC Rust)

### 6.3 Routing Daemon (메인 데몬)
- Unix socket listener (hook으로부터 inquiry 수신)
- pipe-pane log watcher (status sentinel 추출)
- 모바일 클라이언트 management (transport spec 후속)
- Inquiry → 모바일 push, 응답 대기, send-keys inject
- Status → 모바일 push notification

### 6.4 Tmux Controller
- `tmux list-sessions` / `tmux pipe-pane` / `tmux send-keys` 추상화
- Dialog ready detection (pipe-pane log에서 `Enter to select` 마커 polling)
- Send-keys mapping (option index → keystroke 변환)

### 6.5 Skill File Manager
- 데몬 시작 시 `telayd-protocol` SKILL.md를 적절한 위치에 install
- 종료 시 cleanup (또는 leave for next run)
- v0.2의 SKILL은 status용 1개뿐

---

## 7. End-to-End Examples

### 7.1 Inquiry (사용자 모바일 응답 시나리오)

타임라인:

```
T+0s    [LLM]   AskUserQuestion 호출 ("DB 골라줘", options=[postgres, mysql, sqlite])
T+0s    [Hook]  PreToolUse 발화, JSON을 telayd 데몬 IPC로 전달
T+0s    [Hook]  exit 0 (allow)
T+0.5s  [Native UI] 데스크탑 터미널에 dialog 렌더링
T+0.5s  [Daemon]    inquiry record 생성, 모바일 클라이언트에 push
T+30s   [User]      모바일에서 "mysql" 탭
T+30.1s [Daemon]    응답 수신, send-keys "2" Enter 결정 (mysql=옵션 2)
T+30.1s [Daemon]    tmux send-keys -t <session> "2" Enter
T+30.2s [Native UI] dialog가 "2" Enter 받고 close
T+30.2s [LLM]       tool result로 "MySQL" 받음, 작업 계속
```

### 7.2 Status (장시간 작업 시나리오)

```
T+0s    [LLM]    "이 PR을 review해줘" (10 파일)
T+0s    [LLM]    sentinel emit: <<TELAYD|status|msg=리뷰+시작|pct=0>>
T+0.1s  [Daemon] pipe-pane에서 감지, 모바일에 push
T+15s   [LLM]    sentinel emit: <<TELAYD|status|msg=3/10+파일|pct=30>>
T+15.1s [Daemon] 모바일 push
T+30s   [LLM]    sentinel emit: <<TELAYD|status|msg=6/10+파일|pct=60>>
... (반복)
T+90s   [LLM]    sentinel emit: <<TELAYD|status|msg=완료|pct=100>>
T+90s   [LLM]    AskUserQuestion ("이 변경사항 적용할까?", options=[Yes, No])
        → 7.1 시나리오로 진행
```

---

## 8. Open Questions / 후속 spike

| ID | 항목 | 우선도 |
|---|---|---|
| S6.3 | Multi-question / `multiSelect: true` 처리 검증 | M |
| S6.4 | Gemini CLI hook + 동등 도구 가로채기 (LLM-agnostic 입증) | H |
| S6.5 | Send-keys vs 사용자 키보드 race 실측 | M |
| S6.6 | PostToolUse hook으로 dialog close 감지 + race 처리 | M |
| S6.7 | Native dialog가 무응답 시 timeout 동작 (Claude Code 내부) | L |
| S6.8 | 데스크탑 사용자 응답 → 모바일에서 "이미 응답됨" UX 동기화 | M (mobile app 구현 시) |
| S6.9 | Status sentinel 이외 LLM이 sentinel을 emit해야 하는 case 발견 시 spec 보강 | L |

---

## 11. Pluggable Transport / PushProvider (사업화 확장성)

b-3에서 결정된 사업화 단계 진화(Phase 0 → 1+)를 위해 데몬에 두 추상화 미리 둠. 데몬 비즈니스 로직은 인터페이스만 알고, 구현 swap 자유.

### 11.1 Transport abstraction

```rust
trait Transport {
    async fn send(&self, session_id: &str, msg: Message) -> Result<()>;
    async fn receive(&self, callback: impl Fn(Message)) -> Result<()>;
    async fn pair(&self) -> Result<PairingInfo>;  // QR 페어링 등
}

// === L0 (quick tunnel, 0원) ===
struct CloudflareTunnelTransport {
    tunnel_url: String,           // L0: xyz.trycloudflare.com (ephemeral) / L1: tunnel.telayd.dev (named)
    cloudflared_proc: Child,       // 데몬이 cloudflared 자동 spawn
}

// === L2 (사업화 NO 결정 후 — 셀프호스팅 alt only) ===
// b-4에서 사업화 NO 결정. paid tier 매출용 X.
// 사용자가 본인 VPS / Fly.io / Cloudflare Workers에 셀프호스팅 가능.
struct HostedRelayTransport {
    server_url: String,           // 사용자 본인이 호스팅한 URL
    auth_token: String,           // 사용자 본인이 발급한 토큰
}

// === 옵션 (선택형 alternative) ===
struct TailscaleTransport { ... }      // Tailscale 사용 사용자
struct LocalLanTransport { ... }       // 같은 LAN 내 사용 (offline)
```

**Transport 선택 로직 (데몬 설정):**
- 기본: `CloudflareTunnelTransport` (L0 quick tunnel / L1 named tunnel)
- 사용자가 본인 셀프호스팅 relay 가졌으면 → `HostedRelayTransport` (L2)
- 사용자 명시 설정으로 alternative 선택 가능
- **사업화 NO 결정 (b-4)**: paid tier 매출 안 함, 셀프호스팅으로만 paid 회피

### 11.2 PushProvider abstraction

```rust
trait PushProvider {
    async fn notify(&self, device_id: &str, payload: NotificationPayload) -> Result<()>;
}

// === L0 ===
struct WebSocketOnlyProvider {
    active_connections: HashMap<DeviceId, WebSocketStream>,
}
// 앱 열려있을 때만 알림. 닫혀있으면 silent fail.

// === L1+ (Android 사용자 즉시 활성, 운영자 비용 0원) ===
struct FcmProvider {
    server_key: String,            // FCM free tier
}

// === L2 — APNs는 b-4 사업화 NO 결정으로 미진입 ===
// (PWA push API로 iOS push 대체 검토 — `mvp-scope/scope.md` §6.7 / §5.2 epic 6 / §9 L2 참조)
// struct ApnsProvider { ... }  // deferred / not implemented

// === 조합 (cascade) ===
struct FallbackProvider {
    providers: Vec<Box<dyn PushProvider>>,
}
// FCM 실패 → WebSocket 시도 (APNs는 미진입)
```

### 11.3 L0/L1/L2 Phase별 Tech 진화 (b-4 사업화 NO 결정 후 정정 — `mvp-scope/scope.md` §2 align)

| Phase | Transport 구현 | Push 구현 | 운영자 비용 |
|---|---|---|---|
| L0 (dogfooding) | CloudflareTunnel quick | WebSocketOnly | $0 |
| L1 (지인 N명) | CloudflareTunnel named (`tunnel.telayd.dev`) | + FCM (선택, Android only — 운영자 부담 0원) | $20/년 (도메인) |
| L2 (Vision) | + HostedRelay 셀프호스팅 spec | (push는 PWA push API로 통합) | 사용자 본인 부담 |

→ **L0 코드는 L1/L2에서 거의 안 바뀜.** 새 Transport/PushProvider 구현체만 추가.

**~~b-3 사업화 phase 모델 폐기~~** (b-4 사업화 NO 결정 후 stale).

### 11.4 페어링 / 인증 (별도 spec, v0.3 후보)

- 데몬과 모바일 사이 페어링 = QR 코드 + JWT 토큰 교환 (L1)
- E2E 암호화 layer = **L2 = public OSS release prerequisite** (`mvp-scope/scope.md` §6.11 + `licensing-audit.md` §5 참조 — 세 문서 align)
- L0/L1는 CF Tunnel HTTPS만 의존, L2 진입 시 X25519 ECDH + AES-GCM 추가
- 자세한 spec은 b-4 brainstorm + spec v0.3에서

---

## 12. Tech Stack (b-3 §1 결정 — 데몬 prototype 시작 시 채택)

**Desktop App:** Tauri (Rust + React + TypeScript) — Homebrew tap 배포
**Mobile App:** PWA (Phase 0) → React Native (v2)
**Daemon language:** Rust (Tauri backend로 통합 + 독립 실행 가능)
**Frontend hosting:** Cloudflare Pages 또는 GitHub Pages (둘 다 무료)
**Repo 구조:** monorepo (npm/pnpm workspaces)
```
telayd/
├── packages/
│   ├── daemon/       (Rust)       — Tauri backend + 독립 데몬
│   ├── ui-shared/    (React + TS) — 공통 컴포넌트/타입
│   ├── desktop/      (Tauri)      — 데스크탑 전용 view
│   └── pwa/          (Vite)       — 모바일 view
└── apps/                          — v2: mobile-rn/
```

---

## 9. Versioning + Changelog

- **v0.1** (2026-05-04 초안) — 4 message types(choice/text/confirm/status) 모두 sentinel emit. Spike 04 PASS but Spike 05에서 자동 로드 FAIL → 큰 reframe 필요.
- **v0.2** (2026-05-04) — Hybrid: 인터랙티브는 native AskUserQuestion + hook 가로채기. Status만 sentinel. Spike 06 + 06.1로 확정.
- **v0.2.1** (2026-05-04) — b-3 결정 반영: §11 Pluggable Transport/PushProvider 추가 (Cloudflare Tunnel 채택, 사업화 phase별 진화 path), §12 Tech Stack 명시 (Tauri+RN+Rust+monorepo). Spike 06.4 (Gemini hooks PARTIAL) 반영. 라이선스 = FSL-1.1-MIT.
- **v0.2.2** (2026-05-06 현재) — b-4 사업화 NO 결정 후 §11.1 (HostedRelayTransport `api.telayd.com` 예시 → 셀프호스팅 alt로 정정) + §11.3 (사업화 phase 모델 → L0/L1/L2 phase 모델로 정정) + §11.4 (E2E timing → L2 = public OSS release prerequisite, scope.md align). Spec body는 변경 없음.
  - **Breaking changes from v0.1:**
    - choice/text/confirm sentinel format 폐기 (LLM이 emit 안 함)
    - `id=` 필드 폐기 (native `tool_use_id` 사용)
    - SKILL.md가 status 1개 type만 다룸
  - **Preserved from v0.1:**
    - Status sentinel 문법 (`<<TELAYD|status|...>>`)
    - ANSI strip + whitespace strip + buffering 처리 규칙
    - Bracketed paste marker로 사용자 echo 무시

호환성: v0.x 변경은 breaking 가능. v1.0 이후 backward compat 보장.

---

## 10. 참고

- Spike: `../prototypes/01-tmux-roundtrip/`, `02-ansi-handling/`, `03-claude-interactive/`, `04-skill-emit-test/`, `05-skill-autoload/`, `06-hooks-intercept/`, `06.1-hook-roundtrip/`
- 결정 근거: `../brainstorms/b-2.md` (MVP scope), spike README들 (technical findings)
- 다음 단계: 데몬 prototype 모듈 (6장의 5개) 구현 시작 / S6.4 Gemini 호환성 검증
