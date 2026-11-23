[English](README.md) | [한국어](README.ko.md) | [日本語](README.ja.md) | **简体中文**

# bmo-research

平台的调研单元:一个按类别维护待调研队列的守护进程。它在所有者登记的来源(Hugging Face Hub、arXiv、GitHub 发布、
订阅源)上发现新条目,把每个条目交给专属的调研运行器按所属类别的格式进行调研,再把该次运行归档的文档的分享链接
通知给该类别的接收者。运行器(`docs/runner.md`)还在 agency 提供的槽位和角色会话上,承接聊天代理通过
`research/run` 发起的每一项调研,以及绘图服务的 `references` 运行。`it` 请求一份手册或一项调查,若手册表明该条目
适合实验室的某张显卡,则征求所有者的意见;`security` 请求一份对照平台技术栈评估过的安全公告,若公告涉及该技术栈,
则询问是否打补丁;`economy` 把一天的条目汇总成一份简报并保存其中的数据。守护进程只负责调研和通知:报告所要求的
工作,由聊天代理根据所有者的决定去提交。

设计见 `docs/design.md`;路由、工具以及本单元发出的调用见 `docs/api.md`;设置见 `docs/settings.md`;数据表见
`docs/store.md`;来源类型见 `docs/sources.md`。

## 结构

| 目录 | 内容 |
|---|---|
| `crates/` | Rust 工作区:`client`(边界的类型和 HTTP 客户端)与 `server`(守护进程、路由、存储、扫描与深度调研,以及 `bmo-research` 二进制文件) |
| `tools/` | 工具宿主放在 `research` 命名空间下的工具 `research`:清单、模式定义和可执行文件的 crate |
| `prompts/` | 每种格式一份的韩语提问模板、通知用语,以及阅读已归档文档时使用的用语 |
| `docs/` | 设计、API 及其 OpenAPI 文档、设置、存储和来源 |
| `src/` | 仓库自身的包,不含代码 |
| `tests/` | 全部测试:存储与队列规则、来源读取器、针对替身服务器的扫描与深度调研、路由,以及仓库自身的检查 |
| `deploy/` | 部署时重启并监视的用户单元 |

根目录下是 Cargo 工作区、`rustfmt.toml`、语言台账 `korean-ledger.json`、测试平台的 `testbed.toml`、`install.toml`、`CLAUDE.md`
以及 MIT `LICENSE`。
