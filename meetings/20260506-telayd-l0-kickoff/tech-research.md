# telayd-l0-kickoff Technical Research

> Source: `~/AI--SPILL-OVER/telayd-planning/research/competitive-landscape.md` (5-category 병렬 multi-agent research, 2026-05-06)

---

## Research 1: Competitive Landscape — AI agent mobile clients (Category C)

**Question**: Telayd MVP가 1st-party (Anthropic / OpenAI / Google) 모바일 클라이언트와 직접 경쟁하는가?

### Analysis Results

**🔴 Existential threat — Anthropic Claude Code Remote Control**:
- 출시: 2026-02-25 (Max → Pro 확장)
- 사양: `claude remote-control` 명령으로 local Mac에서 Claude Code 실행 → outbound polling → mobile app `Code` tab → push notification → 응답 inject
- **Telayd MVP의 거의 정확한 사양**
- 분배: Claude.ai mobile **12.48M MAU** (Feb 2026), App Store US #1 free 차트 (March 2026), ~50M 누적 다운로드, 4.8★
- 가격: Pro $20/mo / Max $100~200/mo 번들 무료 (research preview)

**Wedge 잔존 — Anthropic 구현 broken**:
- GitHub issues #29214 / #29438 / #34581 / #35637 — interactive permission prompt mobile rendering broken, push notification approval 안 fire, permission mode 변경 불가, `--dangerously-skip-permissions` 강제 비활성
- **이게 Telayd의 진짜 wedge** — 단 3-6개월 expiration (Anthropic 패치하면 닫힘)

**🟡 Latent threat — ChatGPT mobile + Codex CLI**:
- ChatGPT mobile ~592M MAU Feb 2026 — distribution 거대
- Codex CLI ↔ ChatGPT mobile sync 아직 출시 안 됨 (May 2026)
- OpenAI가 명백히 만들고 있음 (Connections/SSH stubs in Settings, GitHub issue #20757 + discussion #9200 active)
- 3-9개월 window

**🟡 Cursor agents (web/PWA)**:
- 2025-06-30 cursor.com/agents launch — assign tasks, monitor progress, prompts from phone
- 단 Cursor cloud agent 한정 (local CLI attach 안 함)

**🟢 Google / Warp / GitHub Mobile**:
- Gemini AI Studio "Build Anything" — local CLI session attach 안 함
- Warp — 모바일 앱 없음
- GitHub Mobile + Copilot — cloud agent only

### Recommendations

1. **Cross-CLI 통합이 진짜 durable wedge** — Claude + Gemini + Codex 한 mobile UI. **1st-party는 절대 안 만듦** (자기 경쟁자 promote 안 함). 18-36+ months defensible.
2. **Permission UX 정확도 wedge** — Anthropic 4 GitHub issues 해결 — 단 3-6개월 expiry라 L0 acceptance에 explicit 검증 항목 박음.
3. **L0은 Claude Code only** — 1st-party 따라잡기 X. cross-CLI는 L1 진입 시.

---

## Research 2: OSS 동일 architecture 경쟁자 — happy.engineering / claudecodeui

**Question**: 동일 dogfooding pattern + bring-your-own-CLI 무료 OSS 경쟁자가 존재하는가?

### Analysis Results

**happy.engineering**:
- bring-your-own-CLI native iOS/Android 앱
- "continue same session from iOS/Android/Web" 명시
- 무료 OSS

**claudecodeui (siteboon)**:
- Claude Code + Cursor CLI + Codex 지원 webui
- 수만 GitHub stars

**claude-remote-approver (yuuichieguchi)**:
- ntfy 기반 approval pusher
- Anthropic Remote Control의 permission approval gap을 정확히 채움

### Recommendations

1. **무료 OSS와 유료 차별화 어려움** — Tailscale 모델 ("paid = 편의성") 작동한 사례 부족.
2. **사업화 NO 결정과 align** — happy.engineering과는 협력/coexistence 가능, 차별 = Telayd Protocol (sentinel + hook hybrid)의 의미적 우월성 + JARFIS dogfooding으로서의 가치.
3. **FSL → 2년 후 MIT 자동 전환** — fork+MIT 우회 시나리오 막을 방법 없으니 FSL은 portfolio 효과로만 의미.

---

## Research 3: LLM Provider Routing — green-field 영역 확인 (Category E)

**Question**: Telayd v1+ vision의 multi-provider redundancy가 이미 구현된 영역인가?

### Analysis Results

**API-level redundancy — 이미 구현됨**:
- **OpenRouter** $50M ARR (early 2026), $1.3B valuation 라운드 진행. 290+ models, multi-trillion weekly tokens. Auto failover, `:nitro`/`:floor` routing.
- **Vercel AI Gateway** — zero-markup BYOK, 무료 distribution
- **Cloudflare AI Gateway** — 무료 core, fallback retries
- **Portkey** — enterprise (PANW 인수 pending). conditional routing/fallback configs
- **LiteLLM** OSS — 40K+ stars, 표준

**Skill/agent harness level redundancy — 비어있음**:
- 모든 incumbent가 OpenAI-compatible API call 단위로만 routing
- Skill (Claude Code의 `/refactor` 같은 multi-step + tool-use loop) 단위 redundancy 없음
- output validation (다른 agent의 결과가 의미적으로 동등한가) 없음

### Recommendations

1. **Telayd v1+ vision (Skill-level redundancy + output validation) = genuinely uncontested green-field** (L2)
2. **단 incumbent (Portkey post-PANW / Vercel + AI SDK / LiteLLM)이 12-24 months 안에 메울 가능성 medium-high**
3. **L0/L1에선 redundancy X** — L2에서 first impl. API-level redundancy (OpenRouter 등)를 underlying layer로 사용 가능.

---

## Research 4: CLI/터미널 모바일 원격 (Category A)

**Question**: 일반 SSH/터미널 모바일 클라이언트가 Telayd와 경쟁하는가?

### Analysis Results

- **Termius** — $4M ARR, 24k paying customers, 3M Play installs. SSH/SFTP polished UX. **Raw terminal — LLM CLI semantic 처리 X**.
- **Blink Shell** — iOS, $19.99/year. SSH/Mosh, polished. Raw terminal.
- **sshx** (ekzhang) — relay-based, modern stack, 7.5k★. 가장 가까운 spiritual match (relay no-infra) but pair-debugging 용도.
- **tmate** (2019) — relay-based, GitHub Actions debug. 미관리.
- **Tabby** (71k★) — desktop terminal only.

### Recommendations

1. **Raw terminal vs Telayd Protocol layer** — 모든 incumbent가 raw terminal exposure. Telayd의 unique value = **LLM CLI semantic 처리 + native mobile UI** (sentinel + hook hybrid).
2. **Termius/Blink과 경쟁 X** — 사용 case 다름 (general SSH vs LLM CLI prompt routing).

---

## Research 5: 모바일 코딩/IDE 원격 (Category B)

**Question**: 클라우드 dev environment + 모바일 access 제품이 경쟁자인가?

### Analysis Results

- **GitHub Codespaces / Replit / StackBlitz / Coder / Gitpod** — 모두 cloud VM model. 사용자 local 환경 X.
- **Replit Mobile** — 50M+ creators, native iOS/Android, agent runs in Replit cloud. interaction pattern 가장 가까움 but Replit sandbox 한정.
- **Cursor agents** — 2025-06-30 PWA, Cursor cloud agent 한정.
- **Warp** — 700K-1M devs, 모바일 없음.

### Recommendations

1. **"Your laptop, your data, your provider" angle은 Telayd 유니크** — local CLI attach + 모바일 routing은 누구도 안 함 (1st-party는 cloud agent 모델).
2. **Pricing anchor $20/mo Pro tier** ubiquitous — Telayd 사업화 시 정당화 어려움 (사업화 NO 이미 결정).

---

## Research 6: LLM Workflow Orchestration (Category D)

**Question**: n8n/Pipedream/Dify 같은 workflow tool이 Telayd MVP과 겹치는가?

### Analysis Results

- **n8n** 230k users / $2.5B valuation — visual DAG + HITL via Slack/Telegram approval. 가장 가까운 "human approves AI step from phone" but workflow approval, not tmux-attached interactive CLI.
- **Pipedream / Make.com / Dify / Flowise / Langflow / Zapier / CrewAI** — 모두 cloud-only or builder. local CLI agent 통합 X.

### Recommendations

1. **Direct overlap zero** for L0/L1.
2. **L2 Skill DSL이 workflow 영역과 collision** — 단 Telayd는 runtime + remote prompt-handling, n8n은 builder. 본질 다름.

---

## 종합 함의 (b-4 thesis)

1. **L0 wedge** = Anthropic 4 GitHub issues fix — **3-6개월 expiry**, scope §3.4 acceptance에 explicit 박음
2. **L1+ wedge** = cross-CLI 통합 — **18-36+ months defensible** (1st-party 절대 안 만듦)
3. **L2 wedge** = Skill-level redundancy + output validation — **green-field** but 12-24 months incumbent 추격 가능
4. **사업화는 모든 wedge가 시간 압박 + customer dev 입구 막힘** — b-4 사업화 NO 결정 정당화
5. **OSS dogfooding으로서의 가치 = 영구** — 외부 보상 의존 X

---

전체 5 카테고리 raw 결과는 `~/AI--SPILL-OVER/telayd-planning/research/competitive-landscape.md` 참조 (10000+ words).
