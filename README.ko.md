[English](README.md) | **한국어** | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

# bmo-research

플랫폼의 리서치 유닛입니다. 조사할 대상을 카테고리별 대기열로 관리하는 데몬으로, 소유자가 등록한 출처(Hugging Face
Hub, arXiv, GitHub 릴리스, 피드)에서 새 대상을 찾아내고, 각 대상을 전용 리서치 러너에게 맡겨 그 카테고리의 형식대로
조사하게 한 뒤, 실행이 보관한 문서의 공유 링크를 카테고리의 수신자에게 알립니다. 러너(`docs/runner.md`)는 채팅
에이전트가 `research/run`으로 요청하는 모든 조사와 그림 서비스의 `references` 실행에도, agency가 제공하는 슬롯과
역할 세션 위에서 응답합니다. `it`는 매뉴얼이나 조사를 요청하며, 대상이 랩의 카드 중 하나에 맞는다는 매뉴얼이 나오면
소유자의 결정을 묻습니다. `security`는 플랫폼의 스택에 비추어 판단한 보안 권고를 요청하고, 스택에 영향을 주는
권고라면 패치할지 묻습니다. `economy`는 하루치 항목을 하나의 브리핑으로 모으고 그 수치를 보관합니다. 데몬은 조사하고
알리기까지만 합니다. 보고서가 요구하는 작업은 소유자의 결정에 따라 채팅 에이전트가 올립니다.

설계는 `docs/design.md`, 경로와 도구와 이 유닛이 하는 호출은 `docs/api.md`, 설정은 `docs/settings.md`, 테이블은
`docs/store.md`, 출처 종류는 `docs/sources.md`에 있습니다.

## 구성

| 폴더 | 내용 |
|---|---|
| `crates/` | Rust 워크스페이스: `client`(경계의 타입과 HTTP 클라이언트)와 `server`(데몬, 경로, 저장소, 스캔과 딥 다이브, 그리고 `bmo-research` 바이너리) |
| `tools/` | 도구 호스트가 `research` 네임스페이스 아래에 두는 도구 `research`: 매니페스트, 스키마, 실행 파일의 크레이트 |
| `prompts/` | 형식마다 하나씩 있는 한국어 질문 템플릿, 알림 문구, 보관된 문서를 읽을 때 쓰는 문구 |
| `docs/` | 설계, API와 그 OpenAPI 문서, 설정, 저장소, 출처 |
| `src/` | 코드를 담지 않는 저장소 자체의 패키지 |
| `tests/` | 모든 테스트: 저장소와 대기열 규칙, 출처 리더, 대역 서버를 상대로 한 스캔과 딥 다이브, 경로, 저장소 검사 |
| `deploy/` | 배포가 재시작하고 감시하는 사용자 유닛 |

루트에는 Cargo 워크스페이스, `rustfmt.toml`, 언어 대장 `korean-ledger.json`, 테스트베드의 `testbed.toml`, `install.toml`,
`CLAUDE.md`, 그리고 MIT `LICENSE`가 있습니다.
