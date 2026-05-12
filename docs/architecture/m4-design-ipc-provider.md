# M4 详细设计：IPC 桥接与 ProviderConfig

[返回架构索引](../architecture.md)

权威范围：Electron 主进程与 Rust daemon 之间的 IPC 协议选型、传输层实现、以及多 LLM Provider 配置的数据模型与持久化设计。

## 1. IPC 协议选型

### 1.1 候选方案对比

| 方案 | 优势 | 劣势 | M4 适用性 |
|------|------|------|----------|
| **Electron IPC** (ipcMain/ipcRenderer) | 零网络开销、天然安全（contextIsolation）、Electron 原生支持 | 仅限 Electron，无法扩展到 Web | **M4 选择** |
| WebSocket | 跨平台（Web/Electron/RN 通用）、双向流式天然支持 | 需要本地 WS server、端口冲突风险、额外网络层 | M5 预留 |
| HTTP (REST) | 简单、调试方便、工具生态丰富 | 无原生流式支持（需 SSE）、请求头开销大 | M5 预留 |
| Unix Domain Socket | 极低延迟、无端口占用 | Electron renderer 不支持直接 UDS、需额外桥接 | 不采用 |

**决策**：M4 采用 **Electron IPC** 作为最小可行方案。理由：
1. MVP 当前仅 Electron 一个前端平台（`docs/architecture/frontend.md` 明确"首版只冻结 TypeScript + Electron SDK"）。
2. IPC 的往返延迟 < 1ms（本地），满足所有交互场景。
3. `contextIsolation: true` 已配置，preload 脚本作为可信边界天然安全。
4. WebSocket 和 HTTP 在 M5 作为多端移植时引入，届时可在 `@seaki/transport` 中增加对应适配器。

### 1.2 IPC 协议映射

```
Electron Main Process                    Rust Daemon Process
├─ app.whenReady()                       ├─ seaki-daemon binary
│  ├─ spawn daemon (stdio/pidfile)       │  ├─ tokio runtime
│  ├─ health check (ping/pong)           │  ├─ TcpListener / IPC socket
│  ├─ ipcMain.handle("daemon.*")  ←────→ │  ├─ API Gateway
│  └─ forward to daemon                  │  └─ CoreLedger
│                                          │
preload.ts (contextBridge)                 │
├─ exposeInMainWorld("electronAPI")       │
│  ├─ sendMessage(method, input)  ──────→│
│  ├─ onEvent(callback)         ←────────│  SSE push
│  └─ invoke(method, input)     ←───────→│  request/response
│                                          │
Renderer (React)                           │
├─ @seaki/transport                        │
│  ├─ createIpcTransportClient()           │
│  │  ├─ request(method, input)            │
│  │  └─ replay(fromSeq, handler)          │
│  └─ @seaki/domain                        │
│     ├─ workspace.init()                  │
│     ├─ search.query()                    │
│     └─ message.send()                    │
```

### 1.3 IPC 消息协议

所有消息统一使用 JSON 序列化，结构如下：

**Request：**
```json
{
  "id": "req_xxx",
  "method": "message.send",
  "input": { ... },
  "timestamp": "2026-05-12T13:00:00Z"
}
```

**Response（成功）：**
```json
{
  "id": "req_xxx",
  "ok": true,
  "output": { ... }
}
```

**Response（失败）：**
```json
{
  "id": "req_xxx",
  "ok": false,
  "error": {
    "type": "DaemonUnavailableError | ValidationError | RateLimited ...",
    "message": "...",
    "recoverable": true
  }
}
```

**Event（Server-Sent）：**
```json
{
  "type": "event",
  "seq": 42,
  "event": {
    "event_id": "evt_xxx",
    "type": "llm.stream.chunk",
    "payload": { "token": "hello", "index": 3 }
  }
}
```

### 1.4 Daemon 进程生命周期

```rust
// crates/seaki-daemon/src/lifecycle.rs

pub struct DaemonProcess {
    child: std::process::Child,
    pid_file: PathBuf,
    health_check_interval: Duration,
}

impl DaemonProcess {
    /// 启动 daemon 进程。
    /// 1. 查找 daemon 二进制（`target/release/seaki-daemon` 或 `resources/seaki-daemon`）
    /// 2. 写入 PID 文件（`$SEAKI_DATA_DIR/daemon.pid`）
    /// 3. 启动健康检查循环（ping/pong）
    /// 4. 崩溃时自动重启（最多 3 次，指数退避）
    pub fn spawn(data_dir: &Path) -> Result<Self, DaemonSpawnError>;

    /// 发送 SIGTERM，等待优雅退出（最多 5s）。
    pub fn shutdown(self) -> Result<(), DaemonShutdownError>;

    /// 检查 daemon 是否存活（通过 IPC ping）。
    pub fn is_alive(&self) -> bool;
}
```

**Electron main.ts 集成：**

```typescript
// apps/electron/src/electron/main.ts
import { spawn } from "node:child_process";
import path from "node:path";
import { app, ipcMain } from "electron";

let daemonProcess: ReturnType<typeof spawn> | null = null;

function startDaemon() {
  const daemonPath = app.isPackaged
    ? path.join(process.resourcesPath, "seaki-daemon")
    : path.join(__dirname, "../../target/release/seaki-daemon");

  daemonProcess = spawn(daemonPath, [], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, SEAKI_DATA_DIR: getDataDir() },
  });

  daemonProcess.on("exit", (code) => {
    console.warn(`daemon exited with code ${code}, restarting...`);
    // 指数退避重试（最多 3 次）
  });
}

app.whenReady().then(() => {
  startDaemon();
  createWindow();
});

app.on("before-quit", () => {
  if (daemonProcess) {
    daemonProcess.kill("SIGTERM");
  }
});
```

### 1.5 Preload 脚本封装

```typescript
// apps/electron/src/electron/preload.ts
import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("electronAPI", {
  // 原有
  platform: process.platform,

  // M4 新增：IPC 通信
  sendMessage: (method: string, input: unknown) =>
    ipcRenderer.invoke("daemon.request", { method, input }),

  onEvent: (callback: (event: unknown) => void) =>
    ipcRenderer.on("daemon.event", (_event, data) => callback(data)),

  offEvent: (callback: (event: unknown) => void) =>
    ipcRenderer.removeListener("daemon.event", callback),

  // 配置 API
  getConfig: (key: string) => ipcRenderer.invoke("config.get", key),
  setConfig: (key: string, value: unknown) =>
    ipcRenderer.invoke("config.set", { key, value }),
});
```

### 1.6 `@seaki/transport` 真实 IPC 实现

```typescript
// packages/transport/src/ipcTransport.ts
import type { TransportClient, FrontendEventHandler } from "./index";

declare global {
  interface Window {
    electronAPI?: {
      sendMessage: (method: string, input: unknown) => Promise<unknown>;
      onEvent: (callback: (event: unknown) => void) => void;
      offEvent: (callback: (event: unknown) => void) => void;
    };
  }
}

export function createIpcTransportClient(): TransportClient {
  const api = window.electronAPI;
  if (!api) {
    throw new Error("electronAPI not available — is this running in Electron?");
  }

  return {
    async request<TOutput, TInput>(method: string, input: TInput): Promise<TOutput> {
      const response = (await api.sendMessage(method, input)) as {
        ok: boolean;
        output?: TOutput;
        error?: { type: string; message: string; recoverable: boolean };
      };
      if (!response.ok) {
        throw new TransportError(response.error!);
      }
      return response.output as TOutput;
    },

    async replay(fromSeq: number, onEvent: FrontendEventHandler): Promise<number> {
      // IPC 模式下 replay 通过一次性拉取实现
      const events = (await api.sendMessage("daemon.replay", { fromSeq })) as unknown[];
      let lastSeq = fromSeq;
      for (const event of events) {
        onEvent(event as FrontendTransportEvent);
        lastSeq = (event as { seq: number }).seq;
      }
      return lastSeq;
    },
  };
}

class TransportError extends Error {
  constructor(
    readonly error: { type: string; message: string; recoverable: boolean }
  ) {
    super(error.message);
    this.name = error.type;
  }
}
```

### 1.7 连接状态机

```typescript
// packages/state/src/connection.ts

type ConnectionState =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "disconnected";

interface ConnectionConfig {
  maxRetries: number;
  baseDelayMs: number;
  maxDelayMs: number;
}

export function createConnectionManager(config: ConnectionConfig) {
  let state: ConnectionState = "idle";
  let retryCount = 0;
  let abortController: AbortController | null = null;

  async function connect(): Promise<void> {
    state = "connecting";
    abortController = new AbortController();

    try {
      // 尝试建立 IPC 连接
      await attemptConnect(abortController.signal);
      state = "connected";
      retryCount = 0;
    } catch (e) {
      if (retryCount < config.maxRetries) {
        state = "reconnecting";
        retryCount++;
        const delay = Math.min(
          config.baseDelayMs * 2 ** retryCount,
          config.maxDelayMs
        );
        await sleep(delay);
        return connect();
      }
      state = "disconnected";
      throw e;
    }
  }

  return { connect, getState: () => state, disconnect: () => abortController?.abort() };
}
```

## 2. 多 LLM Provider 配置设计

### 2.1 当前状态

```rust
// crates/seaki-agent/src/llm.rs (当前)
pub struct OpenAiClientConfig {
    pub api_base: String,
    pub api_key: String,
    pub default_model: String,
    pub timeout_secs: u64,
}

impl OpenAiClientConfig {
    pub fn from_env() -> Option<Self> {
        // 仅读取 SEAKI_LLM_API_BASE / KEY / MODEL
    }
}
```

当前限制：
- 仅支持一个 provider（通过环境变量）
- 无持久化，重启后需重新配置
- 无运行时切换能力
- Provider 类型单一（仅 OpenAI-compatible）

### 2.2 ProviderConfig 数据模型

```rust
// crates/seaki-core/src/config_store.rs

/// 统一的 Provider 配置枚举，覆盖所有支持的 LLM provider 类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider_type", rename_all = "snake_case")]
pub enum ProviderConfig {
    OpenAi(OpenAiProviderConfig),
    Azure(AzureProviderConfig),
    Ollama(OllamaProviderConfig),
    Anthropic(AnthropicProviderConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiProviderConfig {
    pub provider_id: String,
    pub display_name: String,
    pub api_base: String,
    pub api_key: String,
    pub default_model: String,
    pub timeout_secs: u64,
    #[serde(default)]
    pub models: Vec<String>, // 该 provider 支持的模型列表
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AzureProviderConfig {
    pub provider_id: String,
    pub display_name: String,
    pub api_base: String,
    pub api_key: String,
    pub deployment_id: String,
    pub api_version: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaProviderConfig {
    pub provider_id: String,
    pub display_name: String,
    pub api_base: String, // 默认 http://localhost:11434/v1
    pub default_model: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnthropicProviderConfig {
    pub provider_id: String,
    pub display_name: String,
    pub api_base: String, // 默认 https://api.anthropic.com/v1
    pub api_key: String,
    pub default_model: String, // claude-3-sonnet, claude-3-opus
    pub timeout_secs: u64,
}
```

### 2.3 Provider 注册表

```rust
// crates/seaki-agent/src/provider_registry.rs

pub struct LlmProviderRegistry {
    providers: HashMap<String, Box<dyn LlmClient>>,
    configs: HashMap<String, ProviderConfig>,
    active_provider_id: String,
    fallback_provider_id: Option<String>,
}

impl LlmProviderRegistry {
    /// 从配置存储加载所有 provider。
    pub fn from_config_store(store: &ConfigStore) -> Result<Self, RegistryError>;

    /// 设置当前活跃 provider。
    pub fn set_active_provider(&mut self, provider_id: &str) -> Result<(), RegistryError>;

    /// 获取当前活跃 provider 的 client。
    pub fn active_client(&self) -> Result<&dyn LlmClient, RegistryError>;

    /// 调用 complete，主 provider 失败时自动 fallback。
    pub fn complete_with_fallback(
        &self,
        request: LlmRequest,
    ) -> Result<LlmResponse, LlmError> {
        match self.active_client()?.complete(request.clone()) {
            Ok(resp) => Ok(resp),
            Err(LlmError::RateLimited { .. } | LlmError::ModelUnavailable(_))
                if self.fallback_provider_id.is_some() =>
            {
                let fallback = self.fallback_client()?;
                fallback.complete(request)
            }
            Err(e) => Err(e),
        }
    }

    /// 列出所有已配置的 provider（供前端展示）。
    pub fn list_providers(&self) -> Vec<ProviderSummary>;
}

pub struct ProviderSummary {
    pub provider_id: String,
    pub display_name: String,
    pub provider_type: String,
    pub is_active: bool,
    pub is_fallback: bool,
}
```

### 2.4 配置持久化

```rust
// crates/seaki-core/src/config_store.rs

/// 用户级配置持久化存储。
/// 默认路径：~/.seaki/config.toml
pub struct ConfigStore {
    path: PathBuf,
    data: ConfigData,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ConfigData {
    /// 版本号，用于未来迁移。
    version: u32,

    /// LLM Provider 配置列表。
    #[serde(default)]
    llm_providers: Vec<ProviderConfig>,

    /// 当前活跃 provider ID。
    active_llm_provider: Option<String>,

    /// 备用 provider ID（fallback）。
    fallback_llm_provider: Option<String>,

    /// 前端运行模式。
    #[serde(default = "default_mode")]
    mode: String, // "mock" | "real"

    /// 其他未来扩展配置...
}

impl ConfigStore {
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::default_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let data: ConfigData = toml::from_str(&content)?;
            Ok(Self { path, data })
        } else {
            // 首次启动：从环境变量初始化默认配置
            let data = Self::init_from_env()?;
            let store = Self { path, data };
            store.save()?;
            Ok(store)
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(&self.data)?;
        fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn set_active_provider(&mut self,
        provider_id: &str,
    ) -> Result<(), ConfigError> {
        self.data.active_llm_provider = Some(provider_id.to_string());
        self.save()
    }

    pub fn get_mode(&self) -> &str {
        &self.data.mode
    }

    pub fn set_mode(&mut self, mode: &str) -> Result<(), ConfigError> {
        self.data.mode = mode.to_string();
        self.save()
    }

    fn default_path() -> Result<PathBuf, ConfigError> {
        let home = dirs::home_dir().ok_or(ConfigError::NoHomeDir)?;
        Ok(home.join(".seaki").join("config.toml"))
    }

    fn init_from_env() -> Result<ConfigData, ConfigError> {
        let mut data = ConfigData::default();
        data.version = 1;

        // 从环境变量初始化默认 OpenAI provider
        if let Some(config) = OpenAiClientConfig::from_env() {
            data.llm_providers.push(ProviderConfig::OpenAi(
                OpenAiProviderConfig {
                    provider_id: "openai-default".to_string(),
                    display_name: "OpenAI".to_string(),
                    api_base: config.api_base,
                    api_key: config.api_key,
                    default_model: config.default_model,
                    timeout_secs: config.timeout_secs,
                    models: vec!["gpt-4o".to_string(), "gpt-4o-mini".to_string()],
                }
            ));
            data.active_llm_provider = Some("openai-default".to_string());
        }

        data.mode = std::env::var("SEAKI_MODE").unwrap_or_else(|_| "mock".to_string());
        Ok(data)
    }
}
```

### 2.5 配置 TOML 示例

```toml
# ~/.seaki/config.toml
version = 1
active_llm_provider = "openai-default"
fallback_llm_provider = "ollama-local"
mode = "real"

[[llm_providers]]
provider_type = "openai"
provider_id = "openai-default"
display_name = "OpenAI"
api_base = "https://api.openai.com/v1"
api_key = "sk-..."
default_model = "gpt-4o-mini"
timeout_secs = 120
models = ["gpt-4o", "gpt-4o-mini"]

[[llm_providers]]
provider_type = "ollama"
provider_id = "ollama-local"
display_name = "Ollama (Local)"
api_base = "http://localhost:11434/v1"
default_model = "llama3"
timeout_secs = 300

[[llm_providers]]
provider_type = "azure"
provider_id = "azure-eastus"
display_name = "Azure OpenAI"
api_base = "https://my-resource.openai.azure.com/openai/deployments/my-deployment"
api_key = "..."
deployment_id = "my-deployment"
api_version = "2024-02-01"
timeout_secs = 120
```

### 2.6 热重载设计

```rust
// crates/seaki-daemon/src/gateway.rs

impl ApiGateway {
    /// config.get — 读取当前配置项
    async fn handle_config_get(
        &self,
        key: String,
    ) -> Result<ConfigValue, GatewayError> {
        let store = self.config_store.read().await;
        match key.as_str() {
            "mode" => Ok(ConfigValue::String(store.get_mode().to_string())),
            "active_provider" => Ok(ConfigValue::String(
                store.data.active_llm_provider.clone().unwrap_or_default()
            )),
            "providers" => Ok(ConfigValue::Array(
                store.data.llm_providers.iter().map(|p| ...).collect()
            )),
            _ => Err(GatewayError::UnknownConfigKey(key)),
        }
    }

    /// config.set — 修改配置并持久化
    async fn handle_config_set(
        &self,
        key: String,
        value: ConfigValue,
    ) -> Result<(), GatewayError> {
        let mut store = self.config_store.write().await;
        match key.as_str() {
            "mode" => {
                store.set_mode(value.as_str()?)?;
                // 广播配置变更事件到所有前端连接
                self.broadcast_event(ConfigChangedEvent { key, value });
            }
            "active_provider" => {
                store.set_active_provider(value.as_str()?)?;
                self.provider_registry.write().await.set_active_provider(value.as_str()?)?;
                self.broadcast_event(ConfigChangedEvent { key, value });
            }
            _ => return Err(GatewayError::UnknownConfigKey(key)),
        }
        Ok(())
    }

    /// config.reload — 从磁盘重新加载配置
    async fn handle_config_reload(&self) -> Result<(), GatewayError> {
        let new_store = ConfigStore::load()?;
        let mut store = self.config_store.write().await;
        *store = new_store;
        // 重新初始化 provider registry
        let mut registry = self.provider_registry.write().await;
        *registry = LlmProviderRegistry::from_config_store(&store)?;
        self.broadcast_event(ConfigReloadedEvent {});
        Ok(())
    }
}
```

## 3. 前端 Provider 选择器 UI

```typescript
// ChatPanel.tsx header 新增
interface ProviderSelectorProps {
  providers: ProviderSummary[];
  activeProviderId: string;
  onSwitch: (providerId: string) => void;
}

function ProviderSelector({ providers, activeProviderId, onSwitch }: ProviderSelectorProps) {
  return (
    <select
      value={activeProviderId}
      onChange={(e) => onSwitch(e.target.value)}
      className="text-xs bg-transparent border-none outline-none"
    >
      {providers.map((p) => (
        <option key={p.provider_id} value={p.provider_id}>
          {p.display_name} {p.is_fallback ? "(fallback)" : ""}
        </option>
      ))}
    </select>
  );
}
```

## 4. 迁移路径

### 4.1 Mock → Real IPC 渐进迁移

```typescript
// appModel.ts
import { createMockTransportClient } from "@seaki/transport";
import { createIpcTransportClient } from "@seaki/transport/ipc";

export async function createElectronAppModel() {
  // 根据配置选择 transport
  const mode = await getRuntimeMode(); // "mock" | "real"
  const transport = mode === "real"
    ? createIpcTransportClient()
    : createMockTransportClient({ ... });

  const runtime = createDomainRuntime(transport);
  // ...
}
```

### 4.2 向后兼容

- `createMockTransportClient()` 保留，所有现有测试继续使用 mock 模式。
- 新增 `createIpcTransportClient()` 仅在 `mode === "real"` 时调用。
- 环境变量 `SEAKI_LLM_ENABLED` 仍然有效，作为 `mode` 的初始值来源。

## 5. 设计约束与不变式

1. **IPC 安全**：preload 脚本是唯一可信边界，renderer 不直接访问 Node.js API。
2. **配置版本化**：`config.toml` 包含 `version` 字段，未来迁移时读取旧版本并自动升级。
3. **Provider 隔离**：每个 provider 的 API key 不暴露给前端，仅 daemon 侧持有。
4. **Fallback 透明**：前端不感知 fallback 发生，仅在响应延迟上略有差异。
5. **事件有序**：SSE 流式事件按 `seq` 严格递增，乱序事件触发重连。

## 6. 相关文件

- `apps/electron/src/electron/main.ts` — Electron 主进程（daemon 生命周期）
- `apps/electron/src/electron/preload.ts` — IPC 协议封装
- `packages/transport/src/ipcTransport.ts` — 真实 IPC transport 实现
- `packages/state/src/connection.ts` — 连接状态机
- `crates/seaki-daemon/src/gateway.rs` — API Gateway
- `crates/seaki-daemon/src/lifecycle.rs` — Daemon 进程生命周期
- `crates/seaki-agent/src/provider_registry.rs` — Provider 注册表
- `crates/seaki-core/src/config_store.rs` — 配置持久化
