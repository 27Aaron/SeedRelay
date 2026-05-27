# SeedRelay

[![Rust](https://img.shields.io/badge/Rust-1.85+-000000.svg?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-WTFPL-FF4136.svg)](http://www.wtfpl.net/)

**OpenAI 兼容的 Seed-ASR 实时转写桥接服务**

中文 | [English](README.md)

SeedRelay 是一个轻量级本地服务器，对外暴露 OpenAI Realtime API 端点，内部转接到 Seed-ASR 后端。任何支持 OpenAI Realtime 协议的工具都能直接对接——它负责设备注册、音频重采样、Opus 编码和流式转写。

## 功能特性

- **OpenAI 兼容** — 实现 `/v1/realtime` WebSocket 端点，支持会话管理、音频流和转写事件
- **流式转写** — 实时中间结果和最终转写文本
- **音频处理** — 自动采样率转换和 Opus 编码
- **内置 Web UI** — 带音频可视化的实时测试界面（`--web`）
- **API Key 认证** — 可选的 `api_key` 参数校验
- **零配置凭证** — 自动设备注册和令牌管理
- **调试模式** — 详细日志输出（`--debug`）

## 架构

```
客户端 (OpenAI Realtime API)
  │  WebSocket /v1/realtime
  ▼
SeedRelay
  ├── JSON 事件解析 (session.update, audio.append, commit)
  ├── PCM16 重采样 → Opus 编码
  ├── Protobuf 消息构建
  ▼
Seed-ASR 后端
  │  WebSocket
  ▼
流式转写事件回传至客户端
```

## 快速开始

### 前置条件

- Rust 工具链（1.85+）
- Opus 库（用于音频编码）

### 构建

```bash
git clone https://github.com/27Aaron/SeedRelay.git
cd SeedRelay
cp .env.example .env
cargo build --release
```

### 运行

```bash
# 启动并开启 Web UI
cargo run --release -- --web

# 开启调试日志
cargo run --release -- --web --debug

# 自定义监听地址
cargo run --release -- --bind 0.0.0.0:8080 --web

# 设置 API Key
cargo run --release -- --api-key your-secret-key --web
```

首次运行会自动注册设备并获取凭证，保存到 `.env` 文件供后续使用。

## 命令行参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--bind <ADDR>` | `127.0.0.1:8000` | 服务器监听地址 |
| `--env-path <PATH>` | `.env` | 环境变量文件路径 |
| `--model <MODEL>` | `seed-asr` | ASR 模型标识 |
| `--api-key <KEY>` | *(未启用)* | 客户端需提供的 API Key |
| `--web` | 关闭 | 启用内置 Web 测试界面 |
| `--debug` | 关闭 | 启用详细调试日志 |
| `--reset-credentials` | 关闭 | 强制重新注册设备 |

## 配置

编辑 `.env` 进行持久化配置，命令行参数优先级更高。

```bash
host=127.0.0.1    # 服务器地址
port=8000         # 服务器端口
model=seed-asr    # 模型名称
api_key=          # 可选 API Key（留空则不启用认证）
```

以下凭证由程序自动管理，请勿手动编辑：

```bash
device_id=        # 注册后自动填充
install_id=       # 注册后自动填充
cdid=             # 注册后自动填充
openudid=         # 注册后自动填充
clientudid=       # 注册后自动填充
token=            # 注册后自动填充
```

## API

### WebSocket 端点

```
ws://127.0.0.1:8000/v1/realtime?model=seed-asr[&api_key=...]
```

### 客户端事件

| 事件 | 说明 |
|------|------|
| `session.update` | 配置会话（如 `input_audio_format`、`input_sample_rate`） |
| `input_audio_buffer.append` | 发送 base64 编码的 PCM16 音频片段 |
| `input_audio_buffer.commit` | 标记音频结束，开始转写 |
| `session.close` | 关闭会话 |

### 服务端事件

| 事件 | 说明 |
|------|------|
| `session.created` | 会话建立，返回模型信息 |
| `session.updated` | 会话配置已确认 |
| `input_audio_buffer.committed` | 音频缓冲区已提交处理 |
| `conversation.item.input_audio_transcript.delta` | 中间转写片段 |
| `conversation.item.input_audio_transcript.completed` | 最终转写结果 |
| `error` | 错误信息 |

## 技术栈

| 类别 | 技术 |
|------|------|
| 运行时 | Tokio（异步） |
| WebSocket | tokio-tungstenite |
| HTTP 客户端 | reqwest |
| 音频编解码 | opus |
| 协议缓冲区 | prost |
| 命令行 | clap |
| 序列化 | serde / serde_json |

## 项目结构

```
src/
├── main.rs            # 入口
├── cli.rs             # 命令行参数定义
├── config.rs          # 环境变量与配置加载
├── server.rs          # WebSocket 服务器与连接处理
├── client.rs          # 豆包后端 WebSocket 客户端
├── credentials.rs     # 设备注册与令牌管理
├── protocol.rs        # Protobuf 消息定义
├── realtime.rs        # OpenAI Realtime 协议事件
├── audio.rs           # PCM16 重采样与 Opus 编码
├── response.rs        # 豆包响应解析
└── web.rs             # 内嵌 HTTP 服务器（Web UI）
```

## 许可证

[WTFPL](http://www.wtfpl.net/) — Do What The Fuck You Want To Public License
