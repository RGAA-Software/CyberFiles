---
name: cargo-run-verify
description: >-
  After implementing or answering on Rust/GPUI app work, run the app with cargo run
  and monitor the Cursor terminals folder for panics, build failures, and runtime errors.
  Use on every turn when CyberFiles or this repo was changed or discussed, when the user
  asks to verify/run the app, or when finishing any coding task in this workspace.
---

# cargo run 并监听 terminal

## 硬性规则

每次回答（尤其是改代码、修 bug、加功能之后）结束前必须：

1. 在 workspace 根目录执行 **`cargo run`**（本仓库默认：`cargo run -p cyberfiles`）。
2. **监听 terminal**：读取 `terminals/` 下对应会话文件（或 `Await` 轮询后台 shell），确认：
   - 编译是否成功；
   - 进程是否启动；
   - 是否有 **panic**、**error**、异常退出。
3. 在回复中简要说明运行结果（成功 / 编译失败 / 运行中 / 发现的问题）。

不要忘记。即使用户未再次提醒，也按此流程执行。

## 命令

```powershell
Set-Location <workspace-root>
cargo run -p cyberfiles
```

若刚改过依赖或需确认编译，可先 `cargo check`，但**仍须** `cargo run` 验证 GUI 启动。

## 监听方式

- 长时间运行的 GUI：用 `block_until_ms: 0` 放后台，再用 `Await` 或读取 `terminals/*.txt`。
- 关注输出中的：`Finished`、`error:`、`panic`、`thread 'main' panicked`。
- 若已有同项目的 `cargo run` 在跑，先读该 terminal 状态；需要重启时再启新进程。

## 失败时

- 编译失败：修到 `cargo check` 通过，再 `cargo run`。
- 运行 panic：根据 stack trace 修代码，重复 run + 监听，直到启动无 panic 或向用户说明阻塞原因。

## 回复模板（可简短）

```markdown
### 运行验证
- `cargo run -p cyberfiles`: <成功 | 编译失败 | 运行中>
- Terminal: <无异常 | 错误摘要>
```
