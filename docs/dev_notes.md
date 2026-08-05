# 开发笔记 (dev_notes)

## svelte-check 4.7.4 × Svelte 5 runes 类型推断问题

### 现象

`npm run check` 报告大量错误:

```
Type 'PortInfo[]' is not assignable to type 'never'. (ts)
Block-scoped variable '$props' used before its declaration. (ts)
```

### 根因 (已通过源码确认)

`node_modules/svelte-check/dist/src/index.js:94939` 的 `handle$propsRune` 仅在 **处理 export 声明时**被调用:

```js
} else if (ts.isSourceFile(parent)) {
    this.handleExportedVariableDeclarationList(...);
    for (const declaration of declarationList.declarations) {
        if (... && declaration.initializer.expression.getText() === '$props') {
            this.handle$propsRune(declaration);  // ← 只在 export 时触发
            break;
        }
    }
}
```

Svelte 5 runes 模式下 props 通过 `let { ... } = $props()` 声明,不需要 `export`。但 svelte-check 4.7.4 仅识别 `export let` 形式,导致 runes 模式下 props 类型被推断为 `never`。

### 影响

- 仅影响 `npm run check` 的类型检查输出
- **不影响实际运行**: Svelte 编译器 (`vite build`) 正确处理 runes
- **不影响 IDE 大部分功能**: 代码补全、跳转、查找引用仍正常工作

### 临时方案

1. 在组件中用 `as any` cast 绕过类型检查:
   ```ts
   const props = $props() as any;
   ```
2. 全局响应式状态集中在 `*.svelte.ts` 模块文件中,避免在 `.svelte` 中用 `$state`

### 长期方案

- 等待 svelte-check 修复 (issue 跟踪)
- 或升级到 svelte-check 5.x (npm 上目前 4.7.4 最新)
- 或迁移到 SvelteKit (其使用不同的 svelte-check 配置)

### 验证清单

即使 `npm run check` 报错,以下必须通过:
- `npm run build` (Svelte 编译器处理)
- `npm test` (Vitest 测试)
- `npm run tauri:dev` (实际运行)
- `npm run tauri:build` (打包)

## 串口 (serialport-rs) API 适配

serialport 4.9 重构了 API,从 `SerialPortSettings` + `open_with_settings` 改为 builder pattern:

```rust
// 旧 (4.5 及以下)
let settings = SerialPortSettings { baud_rate: 9600, ... };
let port = serialport::open_with_settings("/dev/ttyUSB0", &settings)?;

// 新 (4.6+)
let port = serialport::new("/dev/ttyUSB0", 9600)
    .data_bits(DataBits::Eight)
    .stop_bits(StopBits::One)
    .parity(Parity::None)
    .flow_control(FlowControl::None)
    .timeout(Duration::from_millis(100))
    .open()?;
```

`ErrorKind` 变体也变了,4.9 只有 `NoDevice, InvalidInput, Unknown, Io(io::ErrorKind)`。ENOENT 映射到 `Io(NotFound)`,EBUSY 映射到 `NoDevice`。

## Tauri 2 API 注意

- `app.emit("event", payload)` 需要 `use tauri::Emitter;` 导入
- `tauri::Manager` trait 在 2.x 仅保留 `setup` 闭包内的方法,如不需要无需显式 use
- `tauri::State<'_, Arc<T>>` 是 commands 接收 state 的标准方式

## PTY 回环测试

`tests/integration.rs` 用 `nix::pty::openpty` 创建 PTY 对,把 slave 当串口打开,master 用于读回环。

- nix 0.29 的 `openpty` 返回 `OpenptyResult` 结构体,不是 tuple
- `nix` crate 的 `pty` 模块在 `term` feature 下 (不能直接加 `pty` feature)
- PTY 默认是 cooked 模式,serialport 打开后会自动设置 raw + 关闭 echo
