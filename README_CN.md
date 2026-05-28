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
- **内置 Web UI** — 带音频可视化的实时测试界面（`--webui`）
- **API Key 认证** — 可选的 Bearer token、`api_key` 查询参数或 WebSocket 子协议校验
- **零配置凭证** — 自动设备注册和令牌管理

## 架构

```
客户端 (OpenAI Realtime API)
  │  WebSocket /v1/realtime
  ▼
SeedRelay
  ├── JSON 事件解析 (session.update, input_audio_buffer.append, input_audio_buffer.commit)
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
cargo build --release
```

### 运行

```bash
# 默认启动
./target/release/seedrelay

# 启动并开启 Web UI
./target/release/seedrelay --webui

# 自定义地址和端口
./target/release/seedrelay --host 0.0.0.0 --port 8080

# 设置 API Key
./target/release/seedrelay --api-key your-secret-key --webui
```

首次运行会自动注册设备并获取凭证，保存到当前工作目录下的 `.seedrelay/credentials.json`。

## 命令行参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--host <ADDR>` | `0.0.0.0` | 服务器监听地址 |
| `--port <PORT>` | `8000` | 服务器监听端口 |
| `--model <MODEL>` | `seed-asr` | ASR 模型标识 |
| `--api-key <KEY>` | *(未启用)* | 客户端需提供的 API Key |
| `--webui` | 关闭 | 启用内置 Web 测试界面 |
| `--reset` | 关闭 | 重置设备凭证并重新注册 |

## Docker

```bash
docker compose up -d
```

通过 `compose.yaml` 的 `command:` 字段自定义参数。

## API

### WebSocket 端点

```
ws://127.0.0.1:8000/v1/realtime?model=seed-asr[&api_key=...]
```

### OpenAI 兼容范围

SeedRelay 实现的是 OpenAI Realtime transcription 这一块，用来接入实时语音转文字客户端：

- `GET /v1/models`
- `GET /v1/models/{model}`
- `WS /v1/realtime?model=seed-asr`
- `Authorization: Bearer <api-key>`、`api_key` 查询参数、`openai-insecure-api-key.<api-key>` WebSocket 子协议鉴权
- `session.update`
- `input_audio_buffer.append`
- `input_audio_buffer.commit`
- `input_audio_buffer.clear`
- `conversation.item.input_audio_transcription.delta`
- `conversation.item.input_audio_transcription.completed`

SeedRelay 不实现 chat completions、文本生成、Responses API、文件上传转写、embeddings、assistants、batches、fine-tuning 等接口。

### 客户端事件

| 事件 | 说明 |
|------|------|
| `session.update` | 配置会话（如 `session.audio.input.format.type`、`session.audio.input.format.rate`） |
| `input_audio_buffer.append` | 发送 base64 编码的 PCM16 音频片段 |
| `input_audio_buffer.commit` | 标记音频结束，开始转写 |
| `input_audio_buffer.clear` | 清空音频缓冲区 |
| `session.close` | 关闭会话 |

### 服务端事件

| 服务端事件 | 说明 |
| --- | --- |
| `session.created` | 会话建立，返回模型信息 |
| `session.updated` | `session.update` 后返回会话配置确认 |
| `input_audio_buffer.committed` | 音频缓冲区已提交转写 |
| `input_audio_buffer.cleared` | 音频缓冲区已清空 |
| `conversation.item.input_audio_transcription.delta` | 中间转写片段 |
| `conversation.item.input_audio_transcription.completed` | 最终转写结果 |
| `error` | OpenAI 风格协议错误或后端错误 |

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
├── config.rs          # 配置解析
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
